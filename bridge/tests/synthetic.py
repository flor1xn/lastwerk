"""Synthetisches Konto für die Tests von backfill, sync und coverage.

Alle Antworten sind erfunden und deterministisch. Das Konto liefert eine
Aktivitätsliste (neu nach alt, seitenweise), je Aktivität einen Detailsatz und ein
ZIP mit einer Schein-FIT-Datei, Tageswerte, leere Antworten und 404.
"""

from __future__ import annotations

import io
import zipfile
from dataclasses import dataclass
from datetime import date, timedelta
from pathlib import Path
from typing import Any

from garminconnect import GarminConnectNotFoundError, GarminConnectTooManyRequestsError

from lastwerk_bridge.allowlist import BLOCKLIST
from lastwerk_bridge.cli import main
from lastwerk_bridge.registry import REGISTRY

from .fakes import FakeClock, FakeGarmin

# Die Uhr der Fixture ``clock`` steht auf 2026-10-15 12:00 UTC.
TODAY = date(2026, 10, 15)
DAY = 86400.0


def fit_bytes(activity_id: Any) -> bytes:
    return b"\x0e\x10\x00\x00synthetic-fit-" + str(activity_id).encode()


def make_zip(members: list[tuple[str, bytes]]) -> bytes:
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, data in members:
            archive.writestr(name, data)
    return buffer.getvalue()


@dataclass(frozen=True)
class Vitals:
    """Erfundene Tageswerte für die Antworten von Schlaf, HRV und Ruhepuls.

    Die Form der Antworten folgt den Annahmen in ``normalize.FIELD_MAP`` (bis L1 nicht gegen
    das echte Konto geprüft). Ein Wert ``None`` fehlt in der Antwort.
    """

    hrv: float | None = None
    rhr: float | None = None
    sleep_s: int | None = None
    sleep_score: int | None = None


def activity(activity_id: int, day: date, hour: int = 7) -> dict[str, Any]:
    return {
        "activityId": activity_id,
        "activityName": "Synthetisch",
        "startTimeLocal": f"{day.isoformat()} {hour:02d}:15:00",
        "startTimeGMT": f"{day.isoformat()} {hour - 2:02d}:15:00",
    }


class Account:
    """Veränderliches Konto: Tests können Aktivitäten hinzufügen und Antworten ändern."""

    def __init__(
        self,
        activities: list[dict[str, Any]],
        *,
        profile_created: str | None = "2026-10-05T08:00:00.0",
        empty_sleep: tuple[date, ...] = (),
        missing_hrv: tuple[date, ...] = (),
        no_weather: bool = True,
        no_exercise_sets: bool = True,
        vitals: dict[date, Vitals] | None = None,
    ) -> None:
        self.activities = list(activities)
        self.profile_created = profile_created
        self.empty_sleep = empty_sleep
        self.missing_hrv = missing_hrv
        self.zips: dict[int, bytes] = {}
        self.missing_original: set[int] = set()
        self.fail_download_after: int | None = None
        self.no_weather = no_weather
        self.no_exercise_sets = no_exercise_sets
        self.vitals = dict(vitals or {})

    # Antworten

    def profile(self) -> dict[str, Any]:
        data: dict[str, Any] = {"id": 4711, "userProfileNumber": 4711, "displayName": "Test"}
        if self.profile_created:
            data["createDate"] = self.profile_created
        return data

    def page(self, start: int, limit: int) -> list[dict[str, Any]]:
        ordered = sorted(
            self.activities,
            key=lambda a: (a["startTimeLocal"], a["activityId"]),
            reverse=True,
        )
        return [dict(a) for a in ordered[start : start + limit]]

    def sleep(self, cdate: str) -> dict[str, Any]:
        day = date.fromisoformat(cdate)
        if day in self.empty_sleep:
            return {"dailySleepDTO": {}}
        vitals = self.vitals.get(day)
        if vitals is not None:
            dto: dict[str, Any] = {}
            if vitals.sleep_s is not None:
                dto["sleepTimeSeconds"] = vitals.sleep_s
            if vitals.sleep_score is not None:
                dto["sleepScores"] = {"overall": {"value": vitals.sleep_score}}
            return {"dailySleepDTO": dto or {"synthetic": True}}
        return {"dailySleepDTO": {"synthetic": True, "day": cdate}}

    def hrv(self, cdate: str) -> dict[str, Any]:
        day = date.fromisoformat(cdate)
        if day in self.missing_hrv:
            raise GarminConnectNotFoundError("client error (404)")
        vitals = self.vitals.get(day)
        if vitals is not None and vitals.hrv is not None:
            return {"hrvSummary": {"lastNightAvg": vitals.hrv}}
        return {"hrvSummary": {"synthetic": True, "day": cdate}}

    def rhr(self, cdate: str) -> dict[str, Any]:
        vitals = self.vitals.get(date.fromisoformat(cdate))
        if vitals is not None and vitals.rhr is not None:
            metric = {"value": vitals.rhr, "calendarDate": cdate}
            return {"allMetrics": {"metricsMap": {"WELLNESS_RESTING_HEART_RATE": [metric]}}}
        return {"synthetic": True, "day": cdate}

    def download(self, activity_id: int, **_kwargs: Any) -> bytes:
        if activity_id in self.missing_original:
            raise GarminConnectNotFoundError("client error (404)")
        if activity_id in self.zips:
            return self.zips[activity_id]
        return make_zip(
            [("README.txt", b"x"), (f"{activity_id}_ACTIVITY.fit", fit_bytes(activity_id))]
        )

    def client(self) -> FakeGarmin:
        responses: dict[str, Any] = {
            "get_user_profile": lambda: self.profile(),
            "get_activities": lambda start, limit: self.page(start, limit),
            "get_sleep_data": lambda cdate: self.sleep(cdate),
            "get_hrv_data": lambda cdate: self.hrv(cdate),
            "get_rhr_day": lambda cdate: self.rhr(cdate),
            "download_activity": self.download,
            "get_activity_weather": GarminConnectNotFoundError("client error (404)")
            if self.no_weather
            else {"synthetic": True},
            "get_activity_exercise_sets": {"exerciseSets": None}
            if self.no_exercise_sets
            else {"synthetic": True},
            "get_activity": lambda activity_id: {"activityId": activity_id, "synthetic": True},
        }
        return FakeGarmin(responses, default={"synthetic": True})


class StrictGarmin(FakeGarmin):
    """Schlägt bei jeder Methode der Sperrliste sofort fehl, auch wenn jemand sie erreicht."""

    def __getattr__(self, name: str) -> Any:
        if name in BLOCKLIST or "menstrual" in name or "pregnancy" in name:
            raise AssertionError(f"Sperrlisten-Methode erreicht den Client: {name}")
        return super().__getattr__(name)


class LimitedGarmin(FakeGarmin):
    """Antwortet nach ``healthy_calls`` Aufrufen nur noch mit 429."""

    def __init__(self, base: FakeGarmin, healthy_calls: int) -> None:
        super().__init__(base.responses, base.default)
        self.healthy_calls = healthy_calls

    def __getattr__(self, name: str) -> Any:
        inner = super().__getattr__(name)

        def wrapped(*args: Any, **kwargs: Any) -> Any:
            if len(self.calls) >= self.healthy_calls:
                self.calls.append((name, args, kwargs))
                raise GarminConnectTooManyRequestsError("429 Too Many Requests")
            return inner(*args, **kwargs)

        return wrapped


def limited_client(account: Account, *, healthy_calls: int) -> LimitedGarmin:
    return LimitedGarmin(account.client(), healthy_calls)


def strict_client(account: Account) -> StrictGarmin:
    base = account.client()
    return StrictGarmin(base.responses, default=base.default)


class Factory:
    def __init__(self, client: FakeGarmin) -> None:
        self.client = client
        self.calls: list[Path] = []

    def __call__(self, token_dir: Path) -> FakeGarmin:
        self.calls.append(token_dir)
        return self.client


def cli(
    argv: list[str],
    client: FakeGarmin,
    clock: FakeClock,
    *,
    root: Path,
    token_dir: Path,
    fast: bool = True,
) -> tuple[int, str, str, Factory]:
    """Ruft die Kommandozeile mit Attrappen-Client. Gibt Exit-Code, Ausgabe und Fehler zurück."""
    factory = Factory(client)
    out, err = io.StringIO(), io.StringIO()
    options = ["--root", str(root), "--token-dir", str(token_dir)]
    if fast:
        options += ["--min-interval", "0.1"]
    code = main([*argv, *options], client_factory=factory, clock=clock, stdout=out, stderr=err)
    return code, out.getvalue(), err.getvalue(), factory


DAY_METHODS = {e.method for e in REGISTRY if e.granularity == "day"}
RANGE_METHODS = {e.method for e in REGISTRY if e.granularity == "range"}
PER_ACTIVITY_METHODS = {e.method for e in REGISTRY if e.granularity == "per_activity"}
DAY_ENTRY_COUNT = len([e for e in REGISTRY if e.granularity == "day"])


def days_back(count: int, *, end: date | None = None) -> list[date]:
    last = end or TODAY
    return [last - timedelta(days=i) for i in range(count)]
