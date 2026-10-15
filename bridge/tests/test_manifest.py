"""Manifest, Schema und atomares Schreiben (SPEC 5.5 Manifest, SPEC 12 M4a)."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

import pytest

from lastwerk_bridge.archive import Archive, write_bytes_atomic
from lastwerk_bridge.manifest import (
    ManifestEntry,
    ManifestValidationError,
    default_schema_path,
    load_schema,
    validate_entry,
)
from lastwerk_bridge.registry import by_name

VALID = {
    "kind": "daily",
    "endpoint": "sleep",
    "params": {"date": "2026-10-06"},
    "status": "ok",
    "http_status": 200,
    "fetched_at": "2026-10-13T12:00:00+00:00",
    "lib_version": "0.3.17",
    "path": "daily/2026/2026-10-06/sleep.json",
    "bytes": 12,
    "sha256": "a" * 64,
}


def test_schema_file_exists_and_matches_spec() -> None:
    path = default_schema_path()
    assert path.is_file(), path
    schema = load_schema(path)
    assert schema["$id"] == "archive-manifest.schema.json"
    assert schema["additionalProperties"] is False
    assert set(schema["required"]) == {"kind", "endpoint", "status", "fetched_at", "lib_version"}
    assert schema["properties"]["kind"]["enum"] == [
        "account",
        "activity",
        "daily",
        "range",
        "device_usb",
    ]
    assert schema["properties"]["status"]["enum"] == ["ok", "empty", "not_available", "error"]


def test_valid_entry_passes() -> None:
    validate_entry(VALID)
    validate_entry(
        {
            "kind": "range",
            "endpoint": "hrv_range",
            "status": "empty",
            "fetched_at": "2026-10-13T12:00:00Z",
            "lib_version": "0.3.17",
        }
    )


@pytest.mark.parametrize("missing", ["kind", "endpoint", "status", "fetched_at", "lib_version"])
def test_each_required_field_is_enforced(missing: str) -> None:
    data = {k: v for k, v in VALID.items() if k != missing}
    with pytest.raises(ManifestValidationError):
        validate_entry(data)


@pytest.mark.parametrize(
    "field,value",
    [
        ("kind", "unknown"),
        ("status", "maybe"),
        ("sha256", "xyz"),
        ("sha256", "A" * 64),
        ("bytes", -1),
        ("bytes", 1.5),
        ("http_status", "200"),
        ("fetched_at", "gestern"),
        ("endpoint", "e" * 129),
        ("params", ["liste"]),
    ],
)
def test_invalid_values_are_rejected(field: str, value: object) -> None:
    data = dict(VALID)
    data[field] = value
    with pytest.raises(ManifestValidationError):
        validate_entry(data)


def test_additional_properties_are_rejected() -> None:
    data = dict(VALID)
    data["note"] = "x"
    with pytest.raises(ManifestValidationError):
        validate_entry(data)


def test_manifest_entry_roundtrip_drops_none() -> None:
    entry = ManifestEntry(
        kind="account",
        endpoint="profile",
        status="ok",
        fetched_at="2026-10-13T12:00:00+00:00",
        lib_version="0.3.17",
    )
    data = entry.to_dict()
    assert "path" not in data and "params" not in data
    assert ManifestEntry.from_dict(data) == entry


def test_store_writes_file_and_matching_manifest_line(root: Path, clock) -> None:
    archive = Archive(root, lib_version="0.3.17", clock=clock)
    payload = b'{"synthetic": true}\n'
    entry = archive.store(
        "daily", "sleep", {"date": "2026-10-06"}, payload, Path("daily/2026/2026-10-06/sleep.json")
    )
    written = root / "daily" / "2026" / "2026-10-06" / "sleep.json"
    assert written.read_bytes() == payload
    assert entry.sha256 == hashlib.sha256(payload).hexdigest()
    assert entry.bytes == len(payload)
    assert entry.path == "daily/2026/2026-10-06/sleep.json"
    lines = (root / "manifest.jsonl").read_text(encoding="utf-8").splitlines()
    assert len(lines) == 1
    validate_entry(json.loads(lines[0]))
    assert archive.verify() == []
    # Keine temporäre Datei bleibt zurück.
    assert not list(root.rglob("*.tmp"))


def test_refetch_never_overwrites_but_adds_timestamped_file(root: Path, clock) -> None:
    archive = Archive(root, lib_version="0.3.17", clock=clock)
    rel = Path("daily/2026/2026-10-06/sleep.json")
    first = archive.store("daily", "sleep", {"date": "2026-10-06"}, b"one\n", rel)
    clock.advance(60)
    second = archive.store("daily", "sleep", {"date": "2026-10-06"}, b"two\n", rel)
    assert first.path != second.path
    assert (root / first.path).read_bytes() == b"one\n"
    assert (root / second.path).read_bytes() == b"two\n"
    assert archive.latest()[first.key] == second
    assert archive.verify() == []


def test_verify_detects_tampered_file_and_orphan(root: Path, clock) -> None:
    archive = Archive(root, lib_version="0.3.17", clock=clock)
    entry = archive.store("account", "profile", None, b"abc", Path("account/profile.json"))
    (root / entry.path).write_bytes(b"abd")
    write_bytes_atomic(root / "account" / "orphan.json", b"{}")
    problems = archive.verify()
    assert any("SHA-256" in p for p in problems)
    assert any("orphan.json" in p for p in problems)


def test_record_rejects_invalid_entry(root: Path, clock) -> None:
    archive = Archive(root, lib_version="0.3.17", clock=clock)
    bad = ManifestEntry(
        kind="daily", endpoint="sleep", status="weird", fetched_at="now", lib_version="0.3.17"
    )
    with pytest.raises(ManifestValidationError):
        archive.record(bad)
    assert not (root / "manifest.jsonl").exists()


def test_layout_paths_follow_spec(root: Path, clock) -> None:
    archive = Archive(root, lib_version="0.3.17", clock=clock)
    assert archive.layout_path(by_name("profile"), {}) == Path("account/profile.json")
    assert archive.layout_path(by_name("activities_list"), {"start": 20, "limit": 20}) == Path(
        "activities/list/page-0020.json"
    )
    assert archive.layout_path(by_name("summary"), {"activity_id": 4711, "year": "2026"}) == Path(
        "activities/2026/4711/summary.json"
    )
    assert archive.layout_path(by_name("original"), {"activity_id": 4711, "year": "2026"}) == Path(
        "activities/2026/4711/original.zip"
    )
    assert archive.layout_path(by_name("sleep"), {"date": "2026-10-06"}) == Path(
        "daily/2026/2026-10-06/sleep.json"
    )
    assert archive.layout_path(
        by_name("hrv_range"), {"start": "2026-10-01", "end": "2026-10-07"}
    ) == Path("ranges/hrv_range/2026-10-01_2026-10-07.json")
    assert archive.layout_path(by_name("hydration"), {"date": "2026-10-06"}) == Path(
        "sensitive/hydration/hydration/2026-10-06.json"
    )
    assert archive.layout_path(
        by_name("weigh_ins"), {"start": "2026-10-01", "end": "2026-10-07"}
    ) == Path("sensitive/weight/weigh_ins/2026-10-01_2026-10-07.json")
