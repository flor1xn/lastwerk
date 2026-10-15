"""normalize: exchange/out aus dem Archiv, Sperre von sensitive/, Wiederholbarkeit (SPEC 12 M4d).

Das Archiv entsteht zur Laufzeit unter ``tmp_path``: ``backfill`` gegen die Attrappe des
Kontos (``synthetic.py``) mit erfundenen Tageswerten. Es liegen keine echten Daten und keine
``*.fit``-Dateien im Index. ``normalize`` selbst sendet nichts: Der Client wird nie erzeugt
und der Socket ist gesperrt (``conftest.py``).
"""

from __future__ import annotations

import builtins
import hashlib
import io
import json
import os
import shutil
import subprocess
import sys
from datetime import UTC, date, datetime
from pathlib import Path
from typing import Any

import pytest

from lastwerk_bridge import __version__
from lastwerk_bridge import normalize as nm
from lastwerk_bridge.archive import DERIVED_FILES, Archive
from lastwerk_bridge.cli import EXIT_ERROR, EXIT_OK, EXIT_USAGE
from lastwerk_bridge.manifest import ManifestEntry, load_schema, validate_value
from lastwerk_bridge.registry import REGISTRY

from .fakes import FakeClock, FakeGarmin
from .synthetic import Account, Vitals, activity, cli

BRIDGE_DIR = Path(__file__).resolve().parents[1]
REPO_DIR = BRIDGE_DIR.parent
SCHEMA = load_schema(REPO_DIR / "schema" / "recovery-day.schema.json")
MARKER = "SPERRMARKER-4711"
FIRST, LAST = date(2026, 10, 6), date(2026, 10, 14)
NO_SLEEP = date(2026, 10, 10)  # Schlafantwort leer
NO_HRV = date(2026, 10, 11)  # HRV 404


def vitals_for(day: date) -> Vitals:
    i = (day - FIRST).days
    return Vitals(hrv=60.0 + i, rhr=48.0 + i % 3, sleep_s=25200 + 60 * i, sleep_score=80 + i)


def build(
    root: Path,
    token_dir: Path,
    clock: FakeClock,
    *,
    vitals: dict[date, Vitals] | None = None,
) -> tuple[FakeGarmin, Account]:
    """Archiv per ``backfill`` gegen das erfundene Konto. Sensible Antworten tragen Marker."""
    days = [date.fromordinal(n) for n in range(FIRST.toordinal(), LAST.toordinal() + 1)]
    acc = Account(
        [
            activity(103, date(2026, 10, 12)),
            activity(101, date(2026, 10, 8)),
            activity(102, date(2026, 10, 9)),
        ],
        empty_sleep=(NO_SLEEP,),
        missing_hrv=(NO_HRV,),
        vitals=vitals if vitals is not None else {d: vitals_for(d) for d in days},
    )
    client = acc.client()
    for entry in REGISTRY:
        if entry.sensitivity == "S3":
            client.set(entry.method, {"marker": f"{MARKER}-{entry.name}"})
    code, _out, _err, _factory = cli(["backfill"], client, clock, root=root, token_dir=token_dir)
    assert code == EXIT_OK
    return client, acc


def run(
    root: Path,
    out: Path,
    client: FakeGarmin,
    clock: FakeClock,
    token_dir: Path,
    *extra: str,
) -> tuple[int, str, str]:
    before = len(client.calls)
    code, text, err, factory = cli(
        ["normalize", "--out", str(out), *extra],
        client,
        clock,
        root=root,
        token_dir=token_dir,
    )
    assert factory.calls == [], "normalize erzeugt keinen Client"
    assert len(client.calls) == before, "normalize ruft die Bibliothek nicht auf"
    return code, text, err


def tree(path: Path) -> dict[str, tuple[str, int]]:
    """Relative Pfade mit SHA-256 und Änderungszeit."""
    return {
        p.relative_to(path).as_posix(): (
            hashlib.sha256(p.read_bytes()).hexdigest(),
            p.stat().st_mtime_ns,
        )
        for p in sorted(path.rglob("*"))
        if p.is_file()
    }


def digests(path: Path) -> dict[str, str]:
    return {k: v[0] for k, v in tree(path).items()}


def expected_fetched_at(root: Path, day: date) -> str:
    """Jüngster Abrufzeitpunkt der Quellen eines Tages laut Manifest, als UTC mit ``Z``."""
    stamps = [
        datetime.fromisoformat(e.fetched_at).astimezone(UTC)
        for e in Archive(root, lib_version="x").latest().values()
        if e.kind == "daily"
        and e.status == "ok"
        and e.endpoint in nm.field_map_endpoints()
        and (e.params or {}).get("date") == day.isoformat()
    ]
    return max(stamps).strftime("%Y-%m-%dT%H:%M:%SZ")


def read_day(out: Path, day: date) -> dict[str, Any]:
    return json.loads((out / "recovery" / f"{day.isoformat()}.json").read_text(encoding="utf-8"))


@pytest.fixture
def archive_env(tmp_path: Path, root: Path, token_dir: Path, clock: FakeClock):
    client, acc = build(root, token_dir, clock)
    return {
        "root": root,
        "out": tmp_path / "exchange" / "out",
        "client": client,
        "acc": acc,
        "clock": clock,
        "token_dir": token_dir,
    }


def normalize(env: dict[str, Any], *extra: str) -> tuple[int, str, str]:
    return run(env["root"], env["out"], env["client"], env["clock"], env["token_dir"], *extra)


# Inhalt der Ausgabe


def test_activities_are_copied_unchanged_and_recovery_days_follow_the_schema(archive_env) -> None:
    env = archive_env
    code, text, err = normalize(env)
    assert code == EXIT_OK, err
    out: Path = env["out"]

    # Aktivitäten: Bytes von original.fit, Name nach Kennung.
    assert sorted(p.name for p in (out / "activities").iterdir()) == [
        "101.fit",
        "102.fit",
        "103.fit",
    ]
    archive = Archive(env["root"], lib_version="x")
    for entry in archive.latest().values():
        if entry.endpoint == "original_fit":
            ident = entry.params["activity_id"]
            produced = (out / "activities" / f"{ident}.fit").read_bytes()
            assert produced == (env["root"] / entry.path).read_bytes()
            assert hashlib.sha256(produced).hexdigest() == entry.sha256
    assert "Aktivitäten: 3 mit original.fit, 3 kopiert, 0 unverändert" in text

    # Erholungstage: ein Tag, eine Datei, gültig nach Schema 7.3.
    names = sorted(p.name for p in (out / "recovery").iterdir())
    expected_days = [date.fromordinal(n) for n in range(FIRST.toordinal(), LAST.toordinal() + 1)]
    assert names == [f"{d.isoformat()}.json" for d in expected_days]
    for day in expected_days:
        doc = read_day(out, day)
        validate_value(doc, SCHEMA)
        assert doc["schema_version"] == 1
        assert doc["date"] == day.isoformat()
        assert doc["source"] == f"garmin-bridge {__version__}"
        # Zeitpunkt des Abrufs aus dem Manifest (die Uhr des Tests läuft beim Abruf weiter).
        assert doc["fetched_at"] == expected_fetched_at(env["root"], day)
        assert doc["fetched_at"].startswith("2026-10-15T12:")
        v = vitals_for(day)
        if day != NO_HRV:
            assert doc["hrv_overnight_ms"] == v.hrv
        if day != NO_SLEEP:
            assert doc["sleep_duration_s"] == v.sleep_s
            assert doc["sleep_score"] == v.sleep_score
        assert doc["resting_hr_bpm"] == v.rhr
    # Teilweise Tage: fehlende Werte fehlen in der Datei, nichts wird erfunden.
    assert "hrv_overnight_ms" not in read_day(out, NO_HRV)
    assert "sleep_duration_s" not in read_day(out, NO_SLEEP)
    assert "sleep_score" not in read_day(out, NO_SLEEP)
    assert "Erholungstage: 10 Tage mit Quelle, 9 geschrieben" in text

    # Bericht im Archiv: abgeleitet, ohne Manifest-Eintrag, ohne Messwerte.
    report_path = env["root"] / "normalize-report.json"
    assert "normalize-report.json" in DERIVED_FILES
    report = json.loads(report_path.read_text(encoding="utf-8"))
    assert report["activities"] == {"total": 3, "copied": 3, "unchanged": 0}
    assert report["recovery"]["written"] == 9
    assert report["sensitive"] == {"allowed": [], "read": 0}
    assert report["problems"] == []
    assert archive.verify() == []


def test_plausible_ranges_match_the_schema_and_the_field_map_is_valid() -> None:
    props = SCHEMA["properties"]
    for name, (low, high) in nm.LIMITS.items():
        assert props[name]["minimum"] == low and props[name]["maximum"] == high, name
        assert props[name]["type"] == ("integer" if name in nm.INTEGER_FIELDS else "number")
    assert set(nm.FIELD_MAP) == set(nm.LIMITS)
    nm.check_field_map()
    assert len(nm.source_name()) <= SCHEMA["properties"]["source"]["maxLength"]


def test_values_outside_the_schema_ranges_are_left_out_and_counted(
    tmp_path: Path, root: Path, token_dir: Path, clock: FakeClock
) -> None:
    vitals = {
        date(2026, 10, 12): Vitals(hrv=500.0, rhr=10.0, sleep_s=90000, sleep_score=120),
        date(2026, 10, 13): Vitals(hrv=400.0, rhr=120.0, sleep_s=86400, sleep_score=100),
        date(2026, 10, 14): Vitals(hrv=61.5, rhr=19.9, sleep_s=25200, sleep_score=-1),
    }
    client, _acc = build(root, token_dir, clock, vitals=vitals)
    out = tmp_path / "out"
    code, text, _err = run(root, out, client, clock, token_dir)
    assert code == EXIT_OK
    # Der 12. hat nur ungültige Werte und damit keine Datei, die Grenzen selbst gelten.
    assert not (out / "recovery" / "2026-10-12.json").exists()
    edge = read_day(out, date(2026, 10, 13))
    assert (edge["hrv_overnight_ms"], edge["resting_hr_bpm"]) == (400.0, 120.0)
    assert (edge["sleep_duration_s"], edge["sleep_score"]) == (86400, 100)
    partial = read_day(out, date(2026, 10, 14))
    assert partial["hrv_overnight_ms"] == 61.5 and partial["sleep_duration_s"] == 25200
    assert "resting_hr_bpm" not in partial and "sleep_score" not in partial
    for doc_path in (out / "recovery").iterdir():
        validate_value(json.loads(doc_path.read_text(encoding="utf-8")), SCHEMA)
    # 4 + 2 Werte außerhalb: 12. (vier), 14. (Ruhepuls und Schlafwert).
    report = json.loads((root / "normalize-report.json").read_text(encoding="utf-8"))
    assert report["recovery"]["fields_out_of_range"] == 6
    assert report["recovery"]["without_values"] >= 1
    assert "Werte außerhalb des Bereichs weggelassen: 6" in text


def test_non_numeric_and_missing_fields_are_ignored() -> None:
    responses = {
        "hrv": {"hrvSummary": {"lastNightAvg": "viel"}},
        "sleep": {"dailySleepDTO": {"sleepTimeSeconds": True, "sleepScores": {"overall": None}}},
        "rhr": {"allMetrics": {"metricsMap": {"WELLNESS_RESTING_HEART_RATE": []}}},
        "heart_rates": {"restingHeartRate": 51},
    }
    values, skipped = nm.extract_fields(responses)
    # Der zweite Weg für den Ruhepuls greift, wenn der erste nichts liefert.
    assert values == {"resting_hr_bpm": 51.0}
    assert skipped == 0
    assert nm.extract_fields({}) == ({}, 0)
    assert (
        nm.extract_fields({"sleep": {"dailySleepDTO": {"sleepTimeSeconds": float("nan")}}})[0] == {}
    )
    # Ganzzahlfelder werden gerundet.
    rounded, _ = nm.extract_fields({"sleep": {"dailySleepDTO": {"sleepTimeSeconds": 25199.6}}})
    assert rounded == {"sleep_duration_s": 25200} and isinstance(rounded["sleep_duration_s"], int)


# Wiederholbarkeit


def test_a_second_run_writes_nothing_and_checksums_stay_equal(archive_env) -> None:
    env = archive_env
    assert normalize(env)[0] == EXIT_OK
    first = tree(env["out"])
    assert len(first) == 3 + 9
    env["clock"].advance(3600)
    code, text, _err = normalize(env)
    assert code == EXIT_OK
    assert tree(env["out"]) == first, "weder Inhalt noch Änderungszeit ändern sich"
    assert "0 kopiert, 3 unverändert" in text
    assert "0 geschrieben, 9 unverändert" in text
    assert not list(env["out"].rglob(".*.tmp")), "keine liegen gebliebenen temporären Dateien"

    # Eine gelöschte oder beschädigte Ausgabedatei entsteht mit denselben Bytes neu.
    victim = env["out"] / "recovery" / "2026-10-08.json"
    damaged = env["out"] / "activities" / "101.fit"
    victim.unlink()
    damaged.write_bytes(b"kaputt")
    normalize(env)
    assert digests(env["out"]) == {k: v[0] for k, v in first.items()}


def test_a_newer_source_changes_only_its_own_day(archive_env) -> None:
    env = archive_env
    normalize(env)
    before = digests(env["out"])
    # Garmin finalisiert nachträglich: neue Schlafdatei mit Zeitstempel im Namen.
    env["clock"].advance(86400)
    archive = Archive(env["root"], lib_version="x", clock=env["clock"])
    payload = json.dumps(
        {"dailySleepDTO": {"sleepTimeSeconds": 20000, "sleepScores": {"overall": {"value": 55}}}}
    ).encode()
    archive.store(
        "daily",
        "sleep",
        {"date": "2026-10-12"},
        payload,
        Path("daily/2026/2026-10-12/sleep.json"),
    )
    normalize(env)
    after = digests(env["out"])
    changed = {k for k in after if after[k] != before[k]}
    assert changed == {"recovery/2026-10-12.json"}
    doc = read_day(env["out"], date(2026, 10, 12))
    assert (doc["sleep_duration_s"], doc["sleep_score"]) == (20000, 55)
    # Der neueste Abruf der benutzten Quellen bestimmt fetched_at.
    assert doc["fetched_at"] == expected_fetched_at(env["root"], date(2026, 10, 12))
    assert doc["fetched_at"].startswith("2026-10-16T12:")
    assert doc["hrv_overnight_ms"] == vitals_for(date(2026, 10, 12)).hrv


# Sperre von sensitive/


class OpenSpy:
    """Zeichnet jeden Pfad auf, den Programm oder Bibliothek öffnet oder auflisten lässt."""

    def __init__(self, monkeypatch: pytest.MonkeyPatch) -> None:
        self.paths: list[str] = []
        real_open = builtins.open
        real_scandir, real_listdir = os.scandir, os.listdir

        def note(target: Any) -> None:
            if isinstance(target, (str, bytes, os.PathLike)):
                self.paths.append(os.fsdecode(os.fspath(target)))

        def spy_open(file: Any, *args: Any, **kwargs: Any) -> Any:
            note(file)
            return real_open(file, *args, **kwargs)

        def spy_scandir(path: Any = ".") -> Any:
            note(path)
            return real_scandir(path)

        def spy_listdir(path: Any = ".") -> Any:
            note(path)
            return real_listdir(path)

        monkeypatch.setattr(builtins, "open", spy_open)
        monkeypatch.setattr(io, "open", spy_open)
        monkeypatch.setattr(os, "scandir", spy_scandir)
        monkeypatch.setattr(os, "listdir", spy_listdir)

    def touched(self, part: str) -> list[str]:
        return [p for p in self.paths if part in p.replace("\\", "/")]


def sensitive_files(root: Path) -> list[Path]:
    return [p for p in sorted((root / "sensitive").rglob("*")) if p.is_file()]


def assert_no_marker(path: Path) -> None:
    for p in sorted(path.rglob("*")):
        if p.is_file():
            assert MARKER.encode() not in p.read_bytes(), f"Marker in {p}"


def test_sensitive_folder_is_never_read_and_markers_never_reach_the_output(
    archive_env, monkeypatch: pytest.MonkeyPatch
) -> None:
    env = archive_env
    marked = sensitive_files(env["root"])
    assert marked, "das Archiv enthält Dateien unter sensitive/"
    assert all(MARKER in p.read_text(encoding="utf-8") for p in marked)
    categories = {p.relative_to(env["root"] / "sensitive").parts[0] for p in marked}
    assert categories == set(nm.sensitive_categories())

    spy = OpenSpy(monkeypatch)
    code, _text, err = normalize(env)
    assert code == EXIT_OK, err
    assert spy.paths, "der Spion sieht die Zugriffe"
    assert spy.touched("/sensitive") == [], "sensitive/ wurde berührt"
    assert_no_marker(env["out"])
    assert MARKER not in (env["root"] / "normalize-report.json").read_text(encoding="utf-8")
    assert MARKER not in err


def test_allowing_a_category_does_not_read_it_yet(
    archive_env, monkeypatch: pytest.MonkeyPatch
) -> None:
    """In M4d verarbeitet keine Kategorie etwas: Die Freigabe wird nur vermerkt."""
    env = archive_env
    normalize(env)
    plain = digests(env["out"])

    spy = OpenSpy(monkeypatch)
    code, text, _err = normalize(
        env, "--allow-sensitive", "weight", "--allow-sensitive", "hydration"
    )
    assert code == EXIT_OK
    assert spy.touched("/sensitive") == []
    assert digests(env["out"]) == plain
    assert_no_marker(env["out"])
    assert "Freigegebene Kategorien: hydration, weight" in text
    report = json.loads((env["root"] / "normalize-report.json").read_text(encoding="utf-8"))
    assert report["sensitive"] == {"allowed": ["hydration", "weight"], "read": 0}


def test_unknown_category_is_refused_with_exit_code_2(archive_env) -> None:
    env = archive_env
    code, _text, err = normalize(env, "--allow-sensitive", "menstruation")
    assert code == EXIT_USAGE and "Unbekannte Kategorie" in err
    assert not env["out"].exists()


def test_manifest_paths_into_sensitive_or_outside_the_archive_are_not_followed(
    archive_env, monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Auch ein manipuliertes Manifest führt nicht in sensitive/ oder aus dem Archiv heraus."""
    env = archive_env
    root: Path = env["root"]
    secret = sensitive_files(root)[0]
    outside = tmp_path / "ausserhalb.json"
    outside.write_text(json.dumps({"hrvSummary": {"lastNightAvg": 77.0}}), encoding="utf-8")
    link_dir = root / "daily" / "2026" / "2026-10-12"
    (link_dir / "hrv.link.json").symlink_to(secret)
    (link_dir / "sleep.link.json").symlink_to(outside)
    archive = Archive(root, lib_version="x", clock=env["clock"])
    for endpoint, day, path in [
        ("hrv", "2026-10-12", "sensitive/" + secret.relative_to(root / "sensitive").as_posix()),
        ("sleep", "2026-10-13", "../ausserhalb.json"),
        ("rhr", "2026-10-13", str(outside)),
        ("hrv", "2026-10-14", "daily/2026/../../sensitive/x.json"),
        ("sleep", "2026-10-14", "daily/2026/2026-10-12/hrv.link.json"),
        ("hrv", "2026-10-11", "daily/2026/2026-10-12/sleep.link.json"),
    ]:
        archive.record(
            ManifestEntry(
                kind="daily",
                endpoint=endpoint,
                params={"date": day},
                status="ok",
                fetched_at="2026-10-16T12:00:00+00:00",
                lib_version="x",
                path=path,
            )
        )
    spy = OpenSpy(monkeypatch)
    code, text, _err = normalize(env)
    assert code == EXIT_ERROR, "ein nicht lesbarer Pfad ist ein Problem im Bericht"
    assert spy.touched("/sensitive") == []
    assert spy.touched("ausserhalb") == []
    assert text.count("Problem source_missing") == 6
    assert_no_marker(env["out"])
    # Die Werte der Datei außerhalb des Archivs (77.0) sind nirgends gelandet.
    assert b"77.0" not in b"".join(p.read_bytes() for p in (env["out"] / "recovery").iterdir())


# Fehler im Archiv


def test_corrupt_or_missing_sources_are_reported_and_the_run_continues(archive_env) -> None:
    env = archive_env
    root: Path = env["root"]
    (root / "activities/2026/101/original.fit").write_bytes(b"verfaelscht")
    (root / "daily/2026/2026-10-07/hrv.json").unlink()
    (root / "daily/2026/2026-10-09/sleep.json").write_text("{ kein json", encoding="utf-8")
    code, text, _err = normalize(env)
    assert code == EXIT_ERROR
    assert "Problem source_checksum_mismatch: 101" in text
    assert "Problem source_missing: hrv 2026-10-07" in text
    assert "Problem source_checksum_mismatch: sleep 2026-10-09" in text
    out: Path = env["out"]
    assert not (out / "activities" / "101.fit").exists()
    assert (out / "activities" / "102.fit").is_file()
    # Der Tag ohne HRV-Datei hat die übrigen Werte, der mit beschädigtem Schlaf ebenfalls.
    assert "hrv_overnight_ms" not in read_day(out, date(2026, 10, 7))
    assert "sleep_duration_s" not in read_day(out, date(2026, 10, 9))
    assert read_day(out, date(2026, 10, 9))["hrv_overnight_ms"] == vitals_for(date(2026, 10, 9)).hrv
    report = json.loads((root / "normalize-report.json").read_text(encoding="utf-8"))
    assert len(report["problems"]) == 3


def test_invalid_activity_ids_are_skipped(archive_env) -> None:
    env = archive_env
    archive = Archive(env["root"], lib_version="x", clock=env["clock"])
    for ident in ("../../evil", "12 3", ""):
        archive.record(
            ManifestEntry(
                kind="activity",
                endpoint="original_fit",
                params={"activity_id": ident, "year": "2026"},
                status="ok",
                fetched_at="2026-10-16T12:00:00+00:00",
                lib_version="x",
                path="activities/2026/101/original.fit",
            )
        )
    code, text, _err = normalize(env)
    assert code == EXIT_ERROR
    assert text.count("Problem invalid_activity_id") == 3
    assert sorted(p.name for p in env["out"].rglob("*.fit")) == ["101.fit", "102.fit", "103.fit"]
    assert not (env["out"].parent / "evil.fit").exists()


# Aufruf


def test_missing_archive_and_overlapping_folders_end_with_exit_code_2(
    archive_env, tmp_path: Path
) -> None:
    env = archive_env
    code, _out, err = run(
        tmp_path / "gibt-es-nicht",
        tmp_path / "out",
        env["client"],
        env["clock"],
        env["token_dir"],
    )
    assert code == EXIT_USAGE and "Archivordner nicht gefunden" in err
    assert not (tmp_path / "out").exists()

    for inside in (env["root"] / "exchange", env["root"], env["root"].parent):
        code, _out, err = run(env["root"], inside, env["client"], env["clock"], env["token_dir"])
        assert code == EXIT_USAGE and "überlappen" in err
    assert not (env["root"] / "exchange").exists()
    assert Archive(env["root"], lib_version="x").verify() == []


def test_an_empty_archive_yields_an_empty_exchange_folder(
    tmp_path: Path, token_dir: Path, clock: FakeClock
) -> None:
    root = tmp_path / "leer"
    root.mkdir()
    code, text, _err = run(root, tmp_path / "out", FakeGarmin(), clock, token_dir)
    assert code == EXIT_OK
    assert "Aktivitäten: 0 mit original.fit" in text
    assert not (tmp_path / "out").exists() or not list((tmp_path / "out").rglob("*"))


def _bridge(*args: str, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
    env = {k: v for k, v in os.environ.items() if k != "PYTHONPATH"}
    return subprocess.run(
        [sys.executable, "-m", "lastwerk_bridge", *args],
        capture_output=True,
        text=True,
        timeout=120,
        check=False,
        env=env,
        cwd=cwd,
    )


def test_command_runs_as_subprocess_and_matches_the_in_process_result(archive_env) -> None:
    env = archive_env
    assert normalize(env)[0] == EXIT_OK
    expected = digests(env["out"])
    other = env["out"].parent / "out2"
    result = _bridge("normalize", "--root", str(env["root"]), "--out", str(other))
    assert result.returncode == EXIT_OK, result.stderr
    assert "Traceback" not in result.stderr
    assert digests(other) == expected

    bad = _bridge("normalize", "--root", str(env["root"] / "nein"), "--out", str(other))
    assert bad.returncode == EXIT_USAGE and "Traceback" not in bad.stderr


def test_writes_the_exchange_folder_for_the_rust_acceptance_command(archive_env) -> None:
    """Schreibt nach ``../target/exchange/out`` (vom Ordner bridge aus), damit der Befehl aus
    SPEC 12 M4d und docs/tasks/T14 wörtlich läuft: der Rust-Befehl ``import-recovery`` mit
    ``--dir target/exchange/out/recovery --db target/m4.db`` im Wurzelordner des Repositorys.

    Der Ordner liegt unter target/ (nicht versioniert) und wird hier neu aufgebaut.
    """
    env = archive_env
    exchange = REPO_DIR / "target" / "exchange" / "out"
    shutil.rmtree(exchange, ignore_errors=True)
    result = _bridge(
        "normalize",
        "--root",
        str(env["root"]),
        "--out",
        "../target/exchange/out",
        cwd=BRIDGE_DIR,
    )
    assert result.returncode == EXIT_OK, result.stderr
    produced = sorted((exchange / "recovery").iterdir())
    assert len(produced) == 9
    for path in produced:
        assert path.name == json.loads(path.read_text(encoding="utf-8"))["date"] + ".json"
        validate_value(json.loads(path.read_text(encoding="utf-8")), SCHEMA)
    assert len(list((exchange / "activities").iterdir())) == 3
    assert_no_marker(exchange)


def test_a_missing_schema_file_stops_before_anything_is_written(
    archive_env, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    env = archive_env
    gone = tmp_path / "ohne" / "recovery-day.schema.json"
    monkeypatch.setattr(nm, "recovery_schema_path", lambda: gone)
    code, _text, err = normalize(env)
    assert code == EXIT_USAGE
    assert "Schemadatei fehlt" in err and str(gone) in err
    assert "Nichts geschrieben" in err
    # Weder Erholungsdateien noch Aktivitäten noch Bericht: es lief nichts, auch nicht teilweise.
    assert not env["out"].exists()
    assert not (env["root"] / nm.NORMALIZE_REPORT_NAME).exists()


def test_an_unreadable_schema_file_stops_before_anything_is_written(
    archive_env, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    env = archive_env
    broken = tmp_path / "kaputt.schema.json"
    broken.write_text("{ kein json", encoding="utf-8")
    monkeypatch.setattr(nm, "recovery_schema_path", lambda: broken)
    code, _text, err = normalize(env)
    assert code == EXIT_USAGE
    assert "Schemadatei nicht lesbar" in err
    assert not env["out"].exists()
