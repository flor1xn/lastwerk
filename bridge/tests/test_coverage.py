"""coverage: Status je Registereintrag, Daten, Dateien, fehlende Originale (SPEC 12, M4b)."""

from __future__ import annotations

import json
from datetime import date
from pathlib import Path

from lastwerk_bridge.archive import Archive
from lastwerk_bridge.cli import EXIT_OK, EXIT_USAGE
from lastwerk_bridge.registry import REGISTRY

from .fakes import FakeClock
from .synthetic import Account, activity, cli, make_zip


def full_archive(root: Path, token_dir: Path, clock: FakeClock):
    acc = Account(
        [
            activity(103, date(2026, 10, 12)),
            activity(101, date(2026, 10, 8)),
            activity(102, date(2026, 10, 9)),
        ],
        missing_hrv=(date(2026, 10, 11),),
        empty_sleep=(date(2026, 10, 10),),
    )
    acc.missing_original.add(101)
    acc.zips[102] = make_zip([("readme.txt", b"ohne fit")])
    client = acc.client()
    cli(["backfill"], client, clock, root=root, token_dir=token_dir)
    return acc, client


def rows_by_name(report: dict) -> dict[str, dict]:
    return {row["name"]: row for row in report["entries"]}


def test_coverage_lists_every_registry_entry_with_status_dates_and_files(
    root: Path, token_dir: Path, clock: FakeClock
) -> None:
    _acc, client = full_archive(root, token_dir, clock)
    calls = len(client.calls)

    code, out, _err, factory = cli(["coverage"], client, clock, root=root, token_dir=token_dir)
    assert code == EXIT_OK
    assert factory.calls == [] and len(client.calls) == calls, "coverage sendet nichts"

    report = json.loads((root / "coverage.json").read_text(encoding="utf-8"))
    assert [row["name"] for row in report["entries"]] == [e.name for e in REGISTRY]
    for entry in REGISTRY:
        assert entry.name in out, f"{entry.name} fehlt in der Textausgabe"
    rows = rows_by_name(report)

    sleep = rows["sleep"]
    assert sleep["status"] == "available" and sleep["status_label"] == "verfügbar"
    assert (sleep["oldest"], sleep["newest"]) == ("2026-10-05", "2026-10-14")
    # Zehn Tage, einer davon leer (keine Datei): neun Dateien.
    assert sleep["files"] == 9
    assert sleep["keys"] == {"ok": 9, "empty": 1, "not_available": 0, "error": 0}

    assert rows["hrv"]["keys"]["not_available"] == 1 and rows["hrv"]["status"] == "available"
    assert rows["profile"]["status"] == "available" and rows["profile"]["oldest"] is None
    # Leer: Antwort ohne Inhalt. Nicht verfügbar: 404 bei jeder Aktivität.
    assert rows["exercise_sets"]["status"] == "empty"
    assert rows["exercise_sets"]["status_label"] == "leer" and rows["exercise_sets"]["files"] == 0
    assert rows["weather"]["status"] == "not_available"
    assert rows["weather"]["status_label"] == "nicht verfügbar"
    assert rows["weather"]["oldest"] is None
    # Aktivitätsdaten tragen das Datum der Aktivität.
    assert (rows["summary"]["oldest"], rows["summary"]["newest"]) == ("2026-10-08", "2026-10-12")
    assert rows["summary"]["files"] == 3
    assert (rows["activities_list"]["oldest"], rows["activities_list"]["newest"]) == (
        "2026-10-08",
        "2026-10-12",
    )
    assert rows["sleep_daily"]["status"] == "available" and rows["sleep_daily"]["files"] >= 1
    assert rows["hydration"]["class"] == "S3"

    for label in ("verfügbar", "leer", "nicht verfügbar"):
        assert label in out
    assert "Endpunkt" in out and "Ältestes" in out and "Jüngstes" in out
    # Das abgeleitete original_fit steht zusätzlich in Bericht und Ausgabe.
    assert report["derived"][0]["name"] == "original_fit" and report["derived"][0]["files"] == 1
    assert "original_fit" in out


def test_activities_without_original_fit_are_listed_with_reason(
    root: Path, token_dir: Path, clock: FakeClock
) -> None:
    full_archive(root, token_dir, clock)
    code, out, _err, _f = cli(["coverage"], FakeNone(), clock, root=root, token_dir=token_dir)
    assert code == EXIT_OK
    report = json.loads((root / "coverage.json").read_text(encoding="utf-8"))
    assert report["activities"] == {"known": 3, "without_original_fit": 2}
    missing = {item["activity_id"]: item for item in report["activities_without_original_fit"]}
    assert set(missing) == {101, 102}, "103 hat original.fit"
    assert (
        missing[101]["reason"] == "nicht verfügbar (404)" and missing[101]["date"] == "2026-10-08"
    )
    assert missing[102]["reason"] == "Entpacken fehlgeschlagen (NoFitInZip)"
    assert "ohne original.fit: 101 (2026-10-08), nicht verfügbar (404)" in out
    assert "3 bekannt, 2 ohne original.fit" in out


class FakeNone:
    """Client, der bei jeder Benutzung fehlschlägt: coverage darf ihn nie anfassen."""

    def __getattr__(self, name: str):
        raise AssertionError(f"coverage hat den Client benutzt: {name}")


def test_unfetched_entries_are_unchecked_and_originals_not_yet_fetched(
    root: Path, token_dir: Path, clock: FakeClock
) -> None:
    client = Account([activity(103, date(2026, 10, 12))]).client()
    cli(["backfill", "--max-calls", "20"], client, clock, root=root, token_dir=token_dir)
    code, out, _err, _f = cli(["coverage"], client, clock, root=root, token_dir=token_dir)
    assert code == EXIT_OK
    report = json.loads((root / "coverage.json").read_text(encoding="utf-8"))
    rows = rows_by_name(report)
    assert rows["profile"]["status"] == "available"
    assert rows["sleep"]["status"] == "unchecked" and rows["sleep"]["status_label"] == "ungeprüft"
    assert rows["sleep"]["files"] == 0 and rows["sleep"]["oldest"] is None
    assert "ungeprüft" in out
    item = report["activities_without_original_fit"][0]
    assert item["activity_id"] == 103 and item["reason"] == "nicht geholt"


def test_coverage_json_is_derived_and_not_a_manifest_problem(
    root: Path, token_dir: Path, clock: FakeClock
) -> None:
    full_archive(root, token_dir, clock)
    manifest = (root / "manifest.jsonl").read_bytes()
    cli(["coverage"], FakeNone(), clock, root=root, token_dir=token_dir)
    cli(["coverage"], FakeNone(), clock, root=root, token_dir=token_dir)
    assert (root / "manifest.jsonl").read_bytes() == manifest, "coverage ändert das Manifest nicht"
    assert Archive(root, lib_version="x", clock=clock).verify() == []


def test_coverage_needs_no_tokens_and_a_missing_archive_is_a_usage_error(
    root: Path, tmp_path: Path, clock: FakeClock
) -> None:
    code, _out, err, factory = cli(
        ["coverage"], FakeNone(), clock, root=root, token_dir=tmp_path / "none"
    )
    assert code == EXIT_USAGE and "nicht gefunden" in err
    assert factory.calls == [] and not root.exists()

    root.mkdir(parents=True)
    code, out, _err, _f = cli(
        ["coverage"], FakeNone(), clock, root=root, token_dir=tmp_path / "none"
    )
    assert code == EXIT_OK, "leeres Archiv: alle Einträge ungeprüft"
    report = json.loads((root / "coverage.json").read_text(encoding="utf-8"))
    assert {row["status"] for row in report["entries"]} == {"unchecked"}
    assert "0 Manifest-Einträge" in out
