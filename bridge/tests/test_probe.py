"""Sonde mit Attrappen-Client (SPEC 12 M4a Teil 1, SPEC 14 O24 und O26)."""

from __future__ import annotations

import json
from datetime import date, timedelta
from pathlib import Path

from garminconnect import GarminConnectConnectionError, GarminConnectNotFoundError

from lastwerk_bridge.probe import REPORT_NAME, ProbeConfig, run_probe, shape
from lastwerk_bridge.registry import REGISTRY, by_name
from lastwerk_bridge.throttle import ThrottleConfig, ThrottledCaller

from .fakes import FakeClock, FakeGarmin

TODAY = date(2026, 10, 13)
MARKER = "MARKER-VALUE-4711"
MAX_SPAN = 90
HISTORY_DAYS = 1000


def day_handler(cdate: str):
    if date.fromisoformat(cdate) < TODAY - timedelta(days=HISTORY_DAYS):
        return {}
    return {"calendarDate": MARKER, "value": 42, "nested": {"a": 1.5, "b": None}}


def span_of(start: str, end: str) -> int:
    """Tage im Fenster einschließlich beider Enden, so zählt auch die Sonde."""
    return (date.fromisoformat(end) - date.fromisoformat(start)).days + 1


def range_handler(start: str, end: str, *_args, **_kwargs):
    span = span_of(start, end)
    if span > MAX_SPAN:
        raise GarminConnectConnectionError(f"client error (400): span {span} too large")
    if date.fromisoformat(end) < TODAY - timedelta(days=HISTORY_DAYS):
        return []
    return [{"calendarDate": MARKER, "value": 1}]


def activities_handler(start: str, end: str, *_args, **_kwargs):
    span = span_of(start, end)
    if span > MAX_SPAN:
        raise GarminConnectConnectionError("client error (400)")
    if date.fromisoformat(end) < TODAY - timedelta(days=HISTORY_DAYS):
        return []
    return [{"activityId": 4711, "activityName": MARKER}]


def build_client() -> FakeGarmin:
    client = FakeGarmin(default={"field": MARKER})
    for entry in REGISTRY:
        if entry.granularity == "day":
            client.set(entry.method, day_handler)
        elif entry.granularity == "range":
            client.set(entry.method, range_handler)
    client.set("get_activities_by_date", activities_handler)
    client.set("get_user_profile", {"userProfileNumber": 99, "displayName": MARKER})
    client.set("download_activity", b"\x0e\x10FIT" + bytes(64))
    client.set("get_activity_weather", GarminConnectNotFoundError("client error (404)"))
    return client


def make_caller(client: FakeGarmin, tmp_path: Path, clock: FakeClock) -> ThrottledCaller:
    config = ThrottleConfig(min_interval_s=1.5, jitter_s=0.0)
    return ThrottledCaller(client, state_dir=tmp_path / "state", config=config, clock=clock)


def test_one_call_per_entry_without_measurement(tmp_path: Path, clock: FakeClock) -> None:
    client = build_client()
    caller = make_caller(client, tmp_path, clock)
    config = ProbeConfig(span_max_calls=0, history_max_calls=0)
    report = run_probe(caller, tmp_path / "out", today=TODAY, lib_version="0.3.17", config=config)
    entries = {e["name"]: e for e in report["entries"]}
    assert len(entries) == len(REGISTRY)
    probed = [e for e in entries.values() if e["skipped_reason"] is None]
    assert len(client.calls) == len(probed) == len(REGISTRY)
    # Jede Datenart genau einmal, jede Methode genau einmal.
    assert sorted(client.names_called()) == sorted(e.method for e in REGISTRY)
    assert entries["profile"]["status"] == "ok"
    assert entries["gear"]["params_used"] == {"userProfileNumber": 99}
    assert entries["summary"]["params_used"] == {"activity_id": 4711}
    assert entries["weather"]["status"] == "not_available"
    assert entries["weather"]["http_status"] == 404
    assert entries["original"]["bytes"] == 69
    # Tag der letzten Woche.
    assert entries["sleep"]["params_used"]["date"] == (TODAY - timedelta(days=3)).isoformat()
    assert entries["hrv_range"]["params_used"] == {
        "start": (TODAY - timedelta(days=7)).isoformat(),
        "end": (TODAY - timedelta(days=1)).isoformat(),
    }
    # Alle Aufrufe gedrosselt: Mindestabstand zwischen jedem Paar.
    assert len(clock.sleeps) == len(client.calls) - 1
    assert min(clock.sleeps) >= 1.5


def test_report_has_field_names_but_no_values(tmp_path: Path, clock: FakeClock) -> None:
    client = build_client()
    caller = make_caller(client, tmp_path, clock)
    config = ProbeConfig(span_max_calls=0, history_max_calls=0)
    run_probe(caller, tmp_path / "out", today=TODAY, lib_version="0.3.17", config=config)
    report_text = (tmp_path / "out" / REPORT_NAME).read_text(encoding="utf-8")
    assert MARKER not in report_text
    report = json.loads(report_text)
    sleep = next(e for e in report["entries"] if e["name"] == "sleep")
    assert sleep["fields"] == {
        "calendarDate": "string",
        "value": "integer",
        "nested": {"a": "number", "b": "null"},
    }
    # Die Rohantworten liegen daneben, nur dort stehen Werte.
    raw = (tmp_path / "out" / "responses" / "sleep.json").read_text(encoding="utf-8")
    assert MARKER in raw
    assert (tmp_path / "out" / "responses" / "original.zip").is_file()
    assert report["throttle"]["min_interval_s"] == 1.5
    assert report["calls"]["calls"] == len(client.calls)


def test_span_measurement_finds_largest_working_span(tmp_path: Path, clock: FakeClock) -> None:
    client = build_client()
    caller = make_caller(client, tmp_path, clock)
    config = ProbeConfig(span_max_calls=12, history_max_calls=0)
    report = run_probe(caller, tmp_path / "out", today=TODAY, lib_version="0.3.17", config=config)
    hrv = next(e for e in report["entries"] if e["name"] == "hrv_range")
    assert hrv["max_span_days"] == MAX_SPAN
    assert hrv["span_bounded"] is True
    assert hrv["span_failure"] == "GarminConnectConnectionError"
    assert hrv["span_calls"] <= 12
    # Tageswerte haben keine Spanne.
    sleep = next(e for e in report["entries"] if e["name"] == "sleep")
    assert sleep["max_span_days"] is None


def test_span_measurement_is_bounded_by_cap(tmp_path: Path, clock: FakeClock) -> None:
    client = build_client()
    client.set("get_hrv_data_range", lambda *_a, **_k: [{"x": 1}])
    caller = make_caller(client, tmp_path, clock)
    config = ProbeConfig(span_max_calls=5, history_max_calls=0)
    report = run_probe(caller, tmp_path / "out", today=TODAY, lib_version="0.3.17", config=config)
    hrv = next(e for e in report["entries"] if e["name"] == "hrv_range")
    assert hrv["span_calls"] == 5
    assert hrv["span_bounded"] is False
    assert hrv["max_span_days"] == 7 * 2**5


def test_history_depth_is_found_coarsely(tmp_path: Path, clock: FakeClock) -> None:
    client = build_client()
    caller = make_caller(client, tmp_path, clock)
    config = ProbeConfig(span_max_calls=0, history_max_calls=10)
    report = run_probe(caller, tmp_path / "out", today=TODAY, lib_version="0.3.17", config=config)
    entries = {e["name"]: e for e in report["entries"]}
    sleep = entries["sleep"]
    earliest = date.fromisoformat(sleep["history_earliest"])
    bound = date.fromisoformat(sleep["history_empty_before"])
    true_start = TODAY - timedelta(days=HISTORY_DAYS)
    assert bound < true_start <= earliest
    assert (earliest - bound).days <= config.history_resolution_days
    assert sleep["history_calls"] <= 10
    # Nur Einträge, die Lastwerk liest, werden vermessen.
    assert entries["floors"]["history_calls"] == 0
    assert entries["floors"]["history_earliest"] is None
    assert by_name("floors").consumer is False


def test_menstrual_and_pregnancy_never_called(tmp_path: Path, clock: FakeClock) -> None:
    client = build_client()
    caller = make_caller(client, tmp_path, clock)
    run_probe(caller, tmp_path / "out", today=TODAY, lib_version="0.3.17")
    for name in client.names_called():
        assert "menstrual" not in name and "pregnancy" not in name
        assert not name.startswith(("set_", "add_", "delete_", "upload", "update_", "create_"))


def test_shape_limits_depth_and_size() -> None:
    assert shape({"a": [1, 2, 3]}) == {"a": ["array(3)", "integer"]}
    assert shape([]) == "array(0)"
    assert shape({str(i): i for i in range(60)}) == "object(60 keys)"
    assert shape({"a": {"b": {"c": 1}}}) == {"a": {"b": "object(1 keys)"}}
    assert shape(True) == "boolean"
    assert shape(b"xy") == "bytes(2)"
