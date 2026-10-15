"""Fortsetzung ohne Doppelabruf, negative Zwischenspeicherung (SPEC 5.5, SPEC 12 M4a)."""

from __future__ import annotations

import json
from datetime import date, timedelta
from pathlib import Path

from garminconnect import GarminConnectConnectionError, GarminConnectNotFoundError

from lastwerk_bridge.archive import Archive
from lastwerk_bridge.fetch import fetch_one
from lastwerk_bridge.manifest import validate_entry
from lastwerk_bridge.registry import by_name
from lastwerk_bridge.throttle import ThrottleConfig, ThrottledCaller

from .fakes import FakeClock, FakeGarmin

TODAY = date(2026, 10, 13)
EMPTY_DAY = TODAY - timedelta(days=9)
MISSING_DAY = TODAY - timedelta(days=12)


def synthetic_sleep(cdate: str):
    day = date.fromisoformat(cdate)
    if day == EMPTY_DAY:
        return {"dailySleepDTO": {}}
    if day == MISSING_DAY:
        raise GarminConnectNotFoundError("client error (404)")
    return {"dailySleepDTO": {"synthetic": True, "day": cdate}}


def make(root: Path, clock: FakeClock, client: FakeGarmin) -> tuple[Archive, ThrottledCaller]:
    archive = Archive(root, lib_version="0.3.17", clock=clock)
    config = ThrottleConfig(min_interval_s=0.1, jitter_s=0.0)
    caller = ThrottledCaller(client, state_dir=archive.state_dir, config=config, clock=clock)
    return archive, caller


def run_days(archive: Archive, caller: ThrottledCaller, days: int = 14) -> int:
    """Holt sleep für die letzten ``days`` Tage, wie backfill es täte. Liefert Aufrufe."""
    before = caller.stats.calls
    latest = archive.latest()
    for offset in range(1, days + 1):
        day = TODAY - timedelta(days=offset)
        outcome = fetch_one(
            archive, caller, by_name("sleep"), {"date": day}, latest=latest, today=TODAY
        )
        if outcome.entry is not None:
            latest[outcome.entry.key] = outcome.entry
    return caller.stats.calls - before


def test_second_run_fetches_only_refresh_window(root: Path, clock: FakeClock) -> None:
    client = FakeGarmin({"get_sleep_data": synthetic_sleep})
    archive, caller = make(root, clock, client)

    first = run_days(archive, caller)
    assert first == 14
    latest = archive.latest()
    assert (
        latest[
            f"daily|sleep|{json.dumps({'date': EMPTY_DAY.isoformat()}, separators=(',', ':'))}"
        ].status
        == "empty"
    )
    assert (
        latest[
            f"daily|sleep|{json.dumps({'date': MISSING_DAY.isoformat()}, separators=(',', ':'))}"
        ].status
        == "not_available"
    )
    files = sorted(p for p in root.rglob("sleep*.json"))
    assert len(files) == 12

    # Zweiter Lauf: nur die letzten 7 Tage werden neu geholt, nichts Altes doppelt,
    # die leere und die fehlende Antwort auch nicht.
    second = run_days(archive, caller)
    assert second == 7
    refetched = {args[0] for name, args, _ in client.calls[14:]}
    assert refetched == {(TODAY - timedelta(days=d)).isoformat() for d in range(1, 8)}
    assert EMPTY_DAY.isoformat() not in refetched
    assert MISSING_DAY.isoformat() not in refetched

    # Jede Datei hat genau einen Manifest-Eintrag mit passender SHA-256.
    assert archive.verify() == []
    entries = archive.entries()
    assert len(entries) == 14 + 7
    for entry in entries:
        validate_entry(entry.to_dict())
    with_path = [e for e in entries if e.path]
    assert len(with_path) == len({e.path for e in with_path})
    # Neu geholte Dateien haben Zeitstempel im Namen, die alten blieben unverändert.
    assert len(list(root.rglob("sleep*.json"))) == 12 + 7


def test_error_entries_are_retried_on_next_run(root: Path, clock: FakeClock) -> None:
    broken = FakeGarmin({"get_sleep_data": GarminConnectConnectionError("client error (400)")})
    archive, caller = make(root, clock, broken)
    day = TODAY - timedelta(days=20)
    outcome = fetch_one(archive, caller, by_name("sleep"), {"date": day}, today=TODAY)
    assert outcome.entry is not None and outcome.entry.status == "error"
    assert outcome.entry.error_class == "GarminConnectConnectionError"
    assert outcome.entry.http_status == 400

    healthy = FakeGarmin({"get_sleep_data": synthetic_sleep})
    archive, caller = make(root, clock, healthy)
    outcome = fetch_one(archive, caller, by_name("sleep"), {"date": day}, today=TODAY)
    assert outcome.entry is not None and outcome.entry.status == "ok"
    # Ein dritter Versuch holt nichts mehr.
    outcome = fetch_one(archive, caller, by_name("sleep"), {"date": day}, today=TODAY)
    assert outcome.skipped
    assert len(healthy.calls) == 1


def test_once_entries_are_fetched_once(root: Path, clock: FakeClock) -> None:
    client = FakeGarmin({"get_user_profile": {"userProfileNumber": 1, "synthetic": True}})
    archive, caller = make(root, clock, client)
    assert not fetch_one(archive, caller, by_name("profile"), {}, today=TODAY).skipped
    assert fetch_one(archive, caller, by_name("profile"), {}, today=TODAY).skipped
    assert len(client.calls) == 1
    assert (root / "account" / "profile.json").is_file()


def test_sensitive_entries_land_in_sensitive_folder(root: Path, clock: FakeClock) -> None:
    client = FakeGarmin({"get_hydration_data": {"synthetic": 1}})
    archive, caller = make(root, clock, client)
    outcome = fetch_one(
        archive, caller, by_name("hydration"), {"date": TODAY - timedelta(days=30)}, today=TODAY
    )
    assert outcome.entry is not None
    assert outcome.entry.path.startswith("sensitive/hydration/")
    assert archive.verify() == []


def test_binary_payload_is_stored_unchanged(root: Path, clock: FakeClock) -> None:
    blob = bytes(range(256)) * 4
    client = FakeGarmin({"download_activity": blob})
    archive, caller = make(root, clock, client)
    outcome = fetch_one(
        archive, caller, by_name("original"), {"activity_id": 4711, "year": "2026"}, today=TODAY
    )
    assert outcome.entry is not None
    assert outcome.entry.path == "activities/2026/4711/original.zip"
    assert (root / outcome.entry.path).read_bytes() == blob
    name, args, kwargs = client.calls[0]
    assert name == "download_activity"
    assert args == (4711,)
    assert kwargs["dl_fmt"] is FakeGarmin.ActivityDownloadFormat.ORIGINAL
    assert archive.verify() == []


def test_registry_copy_is_written(root: Path, clock: FakeClock) -> None:
    archive = Archive(root, lib_version="0.3.17", clock=clock)
    path = archive.copy_registry()
    assert path.name == "registry.yaml"
    assert "unchecked_until_L1" in path.read_text(encoding="utf-8")
    assert archive.verify() == []
