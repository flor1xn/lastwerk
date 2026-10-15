"""Sonde (SPEC 12, M4a Teil 1; SPEC 5.5 Befehle; SPEC 14 O24, O26).

Je Registereintrag genau ein Aufruf für ein Datum der letzten Woche, danach die
Messung der größten Bereichsspanne je Bereichsendpunkt (binäre Suche nach der
größten Spanne, die ohne Fehler zurückkommt) und der Historientiefe (frühestes
Datum mit nicht-leerer Antwort, grobe Suche). Alle Aufrufe laufen über den
gedrosselten Aufrufer.

Ausgabe nur in den Ordner ``--out``: ``probe-report.json`` ohne Werte (nur
Feldnamen, Typen, Status und Zählungen) und die Rohantworten unter ``responses/``
für die Auswertung durch den Nutzer in L1. Beides bleibt unter ``local/``.
"""

from __future__ import annotations

from dataclasses import asdict, dataclass, field
from datetime import UTC, date, datetime, timedelta
from pathlib import Path
from typing import Any

from .archive import encode_payload, is_empty, write_bytes_atomic
from .fetch import call_args, profile_number
from .registry import REGISTRY, RegistryEntry
from .throttle import (
    AuthRequiredError,
    BlockedResponseError,
    LockoutError,
    ThrottledCaller,
    http_status_of,
)

REPORT_NAME = "probe-report.json"


@dataclass(frozen=True)
class ProbeConfig:
    """Grenzen der Messung (Faustregeln, halten die Aufrufzahl klein)."""

    # Abstand des Sondentages von heute, innerhalb der letzten Woche.
    probe_day_offset: int = 3
    # Höchstzahl Aufrufe je Bereichsendpunkt für die Spannenmessung.
    span_max_calls: int = 8
    # Obergrenze der getesteten Spanne in Tagen (10 Jahre).
    span_cap_days: int = 3660
    # Höchstzahl Aufrufe je Eintrag für die Historientiefe.
    history_max_calls: int = 10
    # Obergrenze der Historiensuche in Tagen (20 Jahre).
    history_cap_days: int = 20 * 365
    # Auflösung, bei der die Historiensuche endet.
    history_resolution_days: int = 30
    # Fensterbreite für Bereichsendpunkte in der Historiensuche.
    history_window_days: int = 7
    # Historientiefe nur für Einträge messen, die Lastwerk liest (consumer).
    history_consumers_only: bool = True


@dataclass
class EntryResult:
    name: str
    method: str
    granularity: str
    sensitivity: str
    status: str = "skipped"
    http_status: int | None = None
    error_class: str | None = None
    fields: Any = None
    bytes: int | None = None
    params_used: dict[str, Any] = field(default_factory=dict)
    max_span_days: int | None = None
    span_bounded: bool | None = None
    span_failure: str | None = None
    span_calls: int = 0
    history_earliest: str | None = None
    history_empty_before: str | None = None
    history_calls: int = 0
    skipped_reason: str | None = None


def shape(value: Any, depth: int = 2) -> Any:
    """Feldnamen und Typen ohne Werte. Große Objekte werden nur gezählt."""
    if isinstance(value, dict):
        if depth <= 0 or len(value) > 50:
            return f"object({len(value)} keys)"
        return {str(k): shape(v, depth - 1) for k, v in value.items()}
    if isinstance(value, list):
        if not value:
            return "array(0)"
        return [f"array({len(value)})", shape(value[0], depth - 1)]
    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, int):
        return "integer"
    if isinstance(value, float):
        return "number"
    if isinstance(value, str):
        return "string"
    if value is None:
        return "null"
    if isinstance(value, (bytes, bytearray)):
        return f"bytes({len(value)})"
    return type(value).__name__


class Probe:
    def __init__(
        self,
        caller: ThrottledCaller,
        out_dir: Path,
        *,
        today: date,
        lib_version: str,
        registry: tuple[RegistryEntry, ...] = REGISTRY,
        config: ProbeConfig | None = None,
    ) -> None:
        self.caller = caller
        self.out_dir = Path(out_dir)
        self.today = today
        self.lib_version = lib_version
        self.registry = registry
        self.config = config or ProbeConfig()
        self.results: dict[str, EntryResult] = {}
        self._profile_number: Any = None
        self._activity: dict[str, Any] | None = None

    # Hilfen

    def _call(self, entry: RegistryEntry, params: dict[str, Any]) -> tuple[str, Any, Any]:
        """Ein Aufruf: (status, value, exception)."""
        args, kwargs = call_args(entry, params, self.caller.client)
        try:
            value = self.caller.call(entry.method, *args, **kwargs)
        except (LockoutError, AuthRequiredError):
            raise
        except BlockedResponseError as exc:
            return "error", None, exc
        except Exception as exc:  # noqa: BLE001, Fehler werden berichtet, nicht geworfen
            if http_status_of(exc) == 404:
                return "not_available", None, exc
            return "error", None, exc
        return ("empty" if is_empty(value) else "ok"), value, None

    def _params_for(self, entry: RegistryEntry) -> tuple[dict[str, Any] | None, str | None]:
        cfg = self.config
        if entry.granularity == "once":
            if "userProfileNumber" in entry.params:
                if self._profile_number is None:
                    return None, "Profilnummer nicht aus profile ermittelbar"
                return {"userProfileNumber": self._profile_number}, None
            return {}, None
        if entry.granularity == "day":
            return {"date": (self.today - timedelta(days=cfg.probe_day_offset)).isoformat()}, None
        if entry.granularity == "range":
            return {
                "start": (self.today - timedelta(days=7)).isoformat(),
                "end": (self.today - timedelta(days=1)).isoformat(),
            }, None
        if entry.granularity == "page":
            return {"start": 0, "limit": 20}, None
        if self._activity is None:
            return None, "keine Aktivität in der letzten Woche"
        return {"activity_id": self._activity.get("activityId")}, None

    def _save_response(self, entry: RegistryEntry, value: Any) -> None:
        payload = encode_payload(value)
        target = self.out_dir / "responses" / f"{entry.name}.{entry.ext}"
        write_bytes_atomic(target, payload)

    # Schritte

    def probe_entry(self, entry: RegistryEntry) -> EntryResult:
        result = EntryResult(entry.name, entry.method, entry.granularity, entry.sensitivity)
        self.results[entry.name] = result
        params, reason = self._params_for(entry)
        if params is None:
            result.skipped_reason = reason
            return result
        result.params_used = params
        status, value, exc = self._call(entry, params)
        result.status = status
        if exc is not None:
            result.error_class = type(exc).__name__
            result.http_status = http_status_of(exc)
            return result
        result.fields = shape(value)
        if isinstance(value, (bytes, bytearray)):
            result.bytes = len(value)
        if value is not None:
            self._save_response(entry, value)
        if entry.name == "profile" and isinstance(value, dict):
            self._profile_number = profile_number(value)
        if entry.name == "activities_by_date" and isinstance(value, list) and value:
            first = value[0]
            if isinstance(first, dict) and "activityId" in first:
                self._activity = first
        return result

    def _range_ok(self, entry: RegistryEntry, span_days: int) -> tuple[bool, Exception | None]:
        # Fenster mit span_days Tagen einschließlich beider Enden, endet gestern.
        params = {
            "start": (self.today - timedelta(days=span_days)).isoformat(),
            "end": (self.today - timedelta(days=1)).isoformat(),
        }
        status, _value, exc = self._call(entry, params)
        return status in ("ok", "empty"), exc

    def measure_span(self, entry: RegistryEntry, result: EntryResult) -> None:
        """Größte Spanne ohne Fehler: erst verdoppeln, dann binär eingrenzen."""
        cfg = self.config
        ok_span = 7
        fail_span: int | None = None
        span = 14
        while result.span_calls < cfg.span_max_calls and span <= cfg.span_cap_days:
            ok, exc = self._range_ok(entry, span)
            result.span_calls += 1
            if ok:
                ok_span = span
                span *= 2
            else:
                fail_span = span
                result.span_failure = type(exc).__name__ if exc else None
                if isinstance(exc, BlockedResponseError):
                    break
                break
        while (
            fail_span is not None
            and fail_span - ok_span > 1
            and result.span_calls < cfg.span_max_calls
            and result.span_failure != "BlockedResponseError"
        ):
            mid = (ok_span + fail_span) // 2
            ok, _exc = self._range_ok(entry, mid)
            result.span_calls += 1
            if ok:
                ok_span = mid
            else:
                fail_span = mid
        result.max_span_days = ok_span
        result.span_bounded = fail_span is not None

    def _nonempty_at(self, entry: RegistryEntry, offset_days: int, window: int) -> bool:
        day = self.today - timedelta(days=offset_days)
        if entry.granularity == "day":
            params: dict[str, Any] = {"date": day.isoformat()}
        else:
            params = {"start": (day - timedelta(days=window)).isoformat(), "end": day.isoformat()}
        status, _value, _exc = self._call(entry, params)
        return status == "ok"

    def measure_history(self, entry: RegistryEntry, result: EntryResult) -> None:
        """Frühestes Datum mit Daten, grob: Jahresschritte verdoppeln, dann eingrenzen."""
        cfg = self.config
        window = cfg.history_window_days
        if result.max_span_days is not None:
            window = min(window, result.max_span_days)
        last_nonempty = 0
        first_empty: int | None = None
        offset = 365
        while result.history_calls < cfg.history_max_calls and offset <= cfg.history_cap_days:
            result.history_calls += 1
            if self._nonempty_at(entry, offset, window):
                last_nonempty = offset
                offset *= 2
            else:
                first_empty = offset
                break
        while (
            first_empty is not None
            and first_empty - last_nonempty > cfg.history_resolution_days
            and result.history_calls < cfg.history_max_calls
        ):
            mid = (last_nonempty + first_empty) // 2
            result.history_calls += 1
            if self._nonempty_at(entry, mid, window):
                last_nonempty = mid
            else:
                first_empty = mid
        if last_nonempty:
            result.history_earliest = (self.today - timedelta(days=last_nonempty)).isoformat()
        if first_empty is not None:
            result.history_empty_before = (self.today - timedelta(days=first_empty)).isoformat()

    def run(self) -> dict[str, Any]:
        # profile und activities_by_date zuerst, weil andere Einträge ihre Werte brauchen.
        first = [e for e in self.registry if e.name in ("profile", "activities_by_date")]
        rest = [e for e in self.registry if e.name not in ("profile", "activities_by_date")]
        for entry in first + rest:
            self.probe_entry(entry)
        for entry in self.registry:
            result = self.results[entry.name]
            if entry.granularity == "range" and result.status in ("ok", "empty"):
                self.measure_span(entry, result)
        for entry in self.registry:
            result = self.results[entry.name]
            if entry.granularity not in ("day", "range"):
                continue
            if self.config.history_consumers_only and not entry.consumer:
                continue
            if result.status in ("ok", "empty"):
                self.measure_history(entry, result)
        return self.write_report()

    def write_report(self) -> dict[str, Any]:
        report = {
            "generated_at": datetime.fromtimestamp(self.caller.clock.now(), tz=UTC).isoformat(
                timespec="seconds"
            ),
            "today": self.today.isoformat(),
            "lib_version": self.lib_version,
            "throttle": asdict(self.caller.config),
            "calls": self.caller.stats.as_dict(),
            "entries": [
                asdict(self.results[e.name]) for e in self.registry if e.name in self.results
            ],
        }
        payload = encode_payload(report)
        write_bytes_atomic(self.out_dir / REPORT_NAME, payload)
        return report


def run_probe(
    caller: ThrottledCaller,
    out_dir: Path,
    *,
    today: date,
    lib_version: str,
    registry: tuple[RegistryEntry, ...] = REGISTRY,
    config: ProbeConfig | None = None,
) -> dict[str, Any]:
    return Probe(
        caller, out_dir, today=today, lib_version=lib_version, registry=registry, config=config
    ).run()
