"""reconcile: Lücken in beide Richtungen, Zuordnung, Bericht ohne Werte (SPEC 12, M4c).

Das Archiv der API entsteht aus erfundenen Listenseiten und Zusammenfassungen, die
Geräteordner zur Laufzeit (``device_fixture.py``).
"""

from __future__ import annotations

import io
import json
import os
import subprocess
import sys
from datetime import date
from pathlib import Path
from typing import Any

from lastwerk_bridge.archive import Archive
from lastwerk_bridge.cli import EXIT_OK, EXIT_USAGE, main
from lastwerk_bridge.reconcile import DEFAULT_TOLERANCE_S

from .device_fixture import make_device
from .fakes import FakeClock
from .synthetic import Account, activity, cli

# activity(id, day, hour=7) startet um 05:15:00 GMT (hour - 2, siehe synthetic.py).
A101 = activity(101, date(2026, 10, 8))
A102 = activity(102, date(2026, 10, 9))
A103 = activity(103, date(2026, 10, 12))
NAME_101 = "2026-10-08-05-15-00.fit"
NAME_102 = "2026-10-09-05-15-00.fit"
NAME_103 = "2026-10-12-05-15-00.fit"


def run(argv: list[str], clock: FakeClock) -> tuple[int, str, str]:
    out, err = io.StringIO(), io.StringIO()
    code = main(argv, clock=clock, stdout=out, stderr=err)
    return code, out.getvalue(), err.getvalue()


def make_api_archive(
    root: Path,
    clock: FakeClock,
    items: list[dict[str, Any]],
    *,
    summaries: dict[int, dict[str, Any]] | None = None,
) -> Archive:
    """Listenseite und optional ``summary.json`` je Aktivität, so wie ``backfill`` sie ablegt."""
    archive = Archive(root, lib_version="test", clock=clock)
    payload = (json.dumps(items) + "\n").encode()
    archive.store(
        "activity",
        "activities_list",
        {"start": 0, "limit": 100},
        payload,
        Path("activities/list/page-0000.json"),
    )
    for activity_id, summary in (summaries or {}).items():
        archive.store(
            "activity",
            "summary",
            {"activity_id": activity_id, "year": "2026"},
            (json.dumps(summary) + "\n").encode(),
            Path(f"activities/2026/{activity_id}/summary.json"),
        )
    return archive


def import_names(tmp_path: Path, root: Path, clock: FakeClock, *names: str, **kw: Any) -> None:
    device = make_device(tmp_path, activity=names, **kw)
    code, _out, err = run(["import-device", str(device), "--root", str(root)], clock)
    assert code == EXIT_OK, err


def reconcile(root: Path, clock: FakeClock, *extra: str) -> tuple[int, str, str]:
    return run(["reconcile", "--root", str(root), *extra], clock)


def report_of(root: Path) -> dict[str, Any]:
    return json.loads((root / "reconcile-report.json").read_text(encoding="utf-8"))


def test_gaps_are_reported_in_both_directions(tmp_path: Path, root: Path, clock: FakeClock) -> None:
    make_api_archive(root, clock, [A103, A101, A102])
    # 102 genau, 103 mit 90 s Abweichung (innerhalb der Toleranz), 101 fehlt auf dem Gerät,
    # die Datei vom 20.10. fehlt in der API.
    import_names(
        tmp_path, root, clock, NAME_102, "2026-10-12-05-16-30.fit", "2026-10-20-09-00-00.fit"
    )

    code, out, _err = reconcile(root, clock)
    assert code == EXIT_OK
    report = report_of(root)
    assert report["nur_api"] == ["101"]
    assert report["nur_geraet"] == ["DEVICE/2026-10-20-09-00-00.fit"]
    assert report["beide"] == 2
    assert report["counts"] == {
        "api": 3,
        "device": 3,
        "beide": 2,
        "nur_api": 1,
        "nur_geraet": 1,
        "nicht_zuordenbar": 0,
    }
    assert "beide: 2" in out and "nur API: 1" in out and "nur Gerät: 1" in out
    assert "nur API: 101" in out
    assert "nur Gerät: DEVICE/2026-10-20-09-00-00.fit" in out
    assert "Bericht:" in out


def test_everything_matches(tmp_path: Path, root: Path, clock: FakeClock) -> None:
    make_api_archive(root, clock, [A101, A102, A103])
    import_names(tmp_path, root, clock, NAME_101, NAME_102, NAME_103)
    code, _out, _err = reconcile(root, clock)
    assert code == EXIT_OK
    report = report_of(root)
    assert (report["nur_api"], report["nur_geraet"], report["beide"]) == ([], [], 3)


def test_tolerance_is_120_seconds(tmp_path: Path, root: Path, clock: FakeClock) -> None:
    assert DEFAULT_TOLERANCE_S == 120
    make_api_archive(root, clock, [A101, A102])
    # 101 + 120 s: noch gleich. 102 + 121 s: schon verschieden.
    import_names(tmp_path, root, clock, "2026-10-08-05-17-00.fit", "2026-10-09-05-17-01.fit")
    reconcile(root, clock)
    report = report_of(root)
    assert report["beide"] == 1
    assert report["nur_api"] == ["102"]
    assert report["nur_geraet"] == ["DEVICE/2026-10-09-05-17-01.fit"]

    reconcile(root, clock, "--tolerance", "121")
    assert report_of(root)["beide"] == 2
    reconcile(root, clock, "--tolerance", "0")
    assert report_of(root)["beide"] == 0


def test_each_activity_and_file_is_matched_at_most_once(
    tmp_path: Path, root: Path, clock: FakeClock
) -> None:
    make_api_archive(root, clock, [A101])
    import_names(tmp_path, root, clock, "2026-10-08-05-15-40.fit", "2026-10-08-05-15-10.fit")
    reconcile(root, clock)
    report = report_of(root)
    # Die nähere Datei gewinnt, die andere bleibt übrig.
    assert report["beide"] == 1 and report["nur_api"] == []
    assert report["nur_geraet"] == ["DEVICE/2026-10-08-05-15-40.fit"]


def test_activity_id_pattern_matches_long_ids(tmp_path: Path, root: Path, clock: FakeClock) -> None:
    long_a = {**A101, "activityId": 12345678901}
    long_b = {**A102, "activityId": 12345678902}
    make_api_archive(root, clock, [long_a, long_b])
    import_names(tmp_path, root, clock, "12345678901.fit", "12345678902_ACTIVITY.FIT", "999999.fit")
    reconcile(root, clock)
    report = report_of(root)
    assert report["beide"] == 2 and report["nur_api"] == []
    assert report["nur_geraet"] == ["DEVICE/999999.fit"]


def test_files_with_other_names_are_not_assignable_and_not_device_only(
    tmp_path: Path, root: Path, clock: FakeClock
) -> None:
    make_api_archive(root, clock, [A101])
    import_names(tmp_path, root, clock, NAME_101, "BACKUP.fit", "2026-13-45-99-00-00.fit")
    code, out, _err = reconcile(root, clock)
    assert code == EXIT_OK
    report = report_of(root)
    assert report["beide"] == 1 and report["nur_geraet"] == []
    assert report["nicht_zuordenbar"] == ["DEVICE/2026-13-45-99-00-00.fit", "DEVICE/BACKUP.fit"]
    assert "nicht zuordenbar" in out


def test_name_pattern_and_offset_are_configurable(
    tmp_path: Path, root: Path, clock: FakeClock
) -> None:
    make_api_archive(root, clock, [A101])
    # Gerätename in Ortszeit UTC+2 und anderes Muster: 07:15 Ortszeit = 05:15 UTC.
    import_names(tmp_path, root, clock, "run_20261008_071500.fit")
    pattern = (
        r"^run_(?P<year>\d{4})(?P<month>\d{2})(?P<day>\d{2})_"
        r"(?P<hour>\d{2})(?P<minute>\d{2})(?P<second>\d{2})\.fit$"
    )
    reconcile(root, clock, "--name-pattern", pattern)
    assert report_of(root)["beide"] == 0, "ohne Versatz gilt der Name als UTC"
    code, _out, _err = reconcile(
        root, clock, "--name-pattern", pattern, "--name-offset-minutes", "120"
    )
    assert code == EXIT_OK
    report = report_of(root)
    assert report["beide"] == 1 and report["nur_api"] == [] and report["nur_geraet"] == []
    assert report["name_pattern"] == pattern and report["name_offset_minutes"] == 120


def test_invalid_pattern_exits_2(tmp_path: Path, root: Path, clock: FakeClock) -> None:
    make_api_archive(root, clock, [A101])
    import_names(tmp_path, root, clock, NAME_101)
    code, _out, err = reconcile(root, clock, "--name-pattern", "(")
    assert code == EXIT_USAGE and "Namensmuster" in err


def test_start_time_comes_from_the_summary_when_present(
    tmp_path: Path, root: Path, clock: FakeClock
) -> None:
    # Die Listenzeile trägt keine Startzeit, die Zusammenfassung schon (oben oder in summaryDTO).
    bare = [{"activityId": 101}, {"activityId": 102}, {"activityId": 103}]
    make_api_archive(
        root,
        clock,
        bare,
        summaries={
            101: {"activityId": 101, "startTimeGMT": "2026-10-08 05:15:00"},
            102: {"activityId": 102, "summaryDTO": {"startTimeGMT": "2026-10-09T05:15:00.0"}},
        },
    )
    import_names(tmp_path, root, clock, NAME_101, NAME_102, NAME_103)
    reconcile(root, clock)
    report = report_of(root)
    assert report["beide"] == 2
    assert report["nur_api"] == ["103"], "ohne Startzeit keine Zuordnung über die Zeit"
    assert report["nur_geraet"] == ["DEVICE/" + NAME_103]


def test_no_archive_exits_2(tmp_path: Path, root: Path, clock: FakeClock) -> None:
    code, out, err = reconcile(root, clock)
    assert code == EXIT_USAGE and out == ""
    assert "Archivordner nicht gefunden" in err
    assert not root.exists()
    code, _out, err = run(["reconcile"], clock)
    assert code == EXIT_USAGE


def test_archive_without_device_copy_exits_2(root: Path, clock: FakeClock) -> None:
    make_api_archive(root, clock, [A101])
    code, out, err = reconcile(root, clock)
    assert code == EXIT_USAGE and out == ""
    assert "device_usb" in err and "import-device" in err
    assert not (root / "reconcile-report.json").exists()


def test_archive_with_only_monitor_files_exits_2(
    tmp_path: Path, root: Path, clock: FakeClock
) -> None:
    make_api_archive(root, clock, [A101])
    device = make_device(tmp_path, monitor=("1A2B3C4D.FIT",))
    run(["import-device", str(device), "--root", str(root)], clock)
    code, _out, err = reconcile(root, clock)
    assert code == EXIT_USAGE and "Activity" in err


def test_device_files_without_any_api_activity_are_device_only(
    tmp_path: Path, root: Path, clock: FakeClock
) -> None:
    import_names(tmp_path, root, clock, NAME_101, NAME_102)
    code, _out, _err = reconcile(root, clock)
    assert code == EXIT_OK
    report = report_of(root)
    assert report["nur_api"] == [] and report["beide"] == 0
    assert report["nur_geraet"] == [f"DEVICE/{NAME_101}", f"DEVICE/{NAME_102}"]


def test_report_contains_ids_names_and_counts_but_no_values(
    tmp_path: Path, root: Path, clock: FakeClock
) -> None:
    make_api_archive(root, clock, [A101, A102])
    import_names(tmp_path, root, clock, NAME_102, "2026-10-20-09-00-00.fit")
    _code, out, _err = reconcile(root, clock)
    text = (root / "reconcile-report.json").read_text(encoding="utf-8")
    report = json.loads(text)
    assert set(report) == {
        "schema_version",
        "generated_at",
        "name_pattern",
        "tolerance_s",
        "name_offset_minutes",
        "counts",
        "nur_api",
        "nur_geraet",
        "beide",
        "nicht_zuordenbar",
    }
    assert isinstance(report["beide"], int)
    # Weder Namen noch Zeitpunkte der API noch Prüfsummen: nur Kennungen, Dateinamen, Zahlen.
    for forbidden in ("Synthetisch", "05:15", "2026-10-09 ", "startTime", "sha256"):
        assert forbidden not in text and forbidden not in out, forbidden
    assert report["nur_api"] == ["101"]


def test_reconcile_only_reads_and_writes_a_derived_report(
    tmp_path: Path, root: Path, clock: FakeClock
) -> None:
    make_api_archive(root, clock, [A101])
    import_names(tmp_path, root, clock, NAME_101)
    manifest = (root / "manifest.jsonl").read_bytes()
    reconcile(root, clock)
    reconcile(root, clock)
    assert (root / "manifest.jsonl").read_bytes() == manifest
    assert Archive(root, lib_version="x", clock=clock).verify() == []


def test_reconcile_against_the_synthetic_account_from_backfill(
    tmp_path: Path, root: Path, token_dir: Path, clock: FakeClock
) -> None:
    """Archiv aus ``backfill`` (Attrappen-Client), dann Gerätekopie und Abgleich."""
    account = Account([A103, A101, A102])
    account.missing_original.add(101)
    cli(["backfill"], account.client(), clock, root=root, token_dir=token_dir)
    import_names(tmp_path, root, clock, NAME_102, NAME_103, "2026-10-30-12-00-00.fit")
    code, _out, _err = reconcile(root, clock)
    assert code == EXIT_OK
    report = report_of(root)
    assert report["nur_api"] == ["101"] and report["beide"] == 2
    assert report["nur_geraet"] == ["DEVICE/2026-10-30-12-00-00.fit"]
    assert Archive(root, lib_version="x", clock=clock).verify() == []


def test_coverage_marks_missing_originals_that_exist_on_the_device(
    tmp_path: Path, root: Path, token_dir: Path, clock: FakeClock
) -> None:
    account = Account([A103, A101, A102])
    account.missing_original.add(101)
    account.missing_original.add(102)
    cli(["backfill"], account.client(), clock, root=root, token_dir=token_dir)

    # Ohne Gerätekopie bleibt der Bericht unverändert (kein Schlüssel "device").
    code, out, _err, _f = cli(["coverage"], account.client(), clock, root=root, token_dir=token_dir)
    assert code == EXIT_OK and "über Gerät" not in out
    plain = json.loads((root / "coverage.json").read_text(encoding="utf-8"))
    assert "device" not in plain
    assert all(item["via_device"] is None for item in plain["activities_without_original_fit"])

    import_names(tmp_path, root, clock, NAME_102)
    code, out, _err, _f = cli(["coverage"], account.client(), clock, root=root, token_dir=token_dir)
    assert code == EXIT_OK
    report = json.loads((root / "coverage.json").read_text(encoding="utf-8"))
    assert report["activities"] == {"known": 3, "without_original_fit": 2}
    via = {
        item["activity_id"]: item["via_device"]
        for item in report["activities_without_original_fit"]
    }
    assert via == {101: None, 102: f"DEVICE/{NAME_102}"}
    assert report["device"] == {"matched": 1, "without_original_fit_via_device": 1}
    line_102 = "ohne original.fit: 102 (2026-10-09), nicht verfügbar (404)"
    assert f"{line_102}, über Gerät vorhanden: DEVICE/{NAME_102}" in out
    assert "ohne original.fit: 101 (2026-10-08), nicht verfügbar (404)\n" in out


def test_command_runs_as_subprocess_against_a_temporary_archive(tmp_path: Path) -> None:
    """Die Befehlsfolge aus SPEC 12 M4c als Unterprozesse."""
    clock = FakeClock(start=1792065600.0)
    target = tmp_path / "target" / "archive"
    make_api_archive(target, clock, [A101, A102])
    device = make_device(tmp_path, activity=(NAME_102, "2026-10-20-09-00-00.fit"))
    env = {k: v for k, v in os.environ.items() if k != "PYTHONPATH"}

    def bridge(*args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, "-m", "lastwerk_bridge", *args],
            capture_output=True,
            text=True,
            timeout=60,
            check=False,
            env=env,
        )

    imported = bridge("import-device", str(device), "--root", str(target))
    assert imported.returncode == EXIT_OK, imported.stderr
    result = bridge("reconcile", "--root", str(target))
    assert result.returncode == EXIT_OK, result.stderr
    assert "beide: 1" in result.stdout and "nur API: 1" in result.stdout
    assert report_of(target)["nur_api"] == ["101"]
    assert bridge("reconcile", "--root", str(tmp_path / "gibt-es-nicht")).returncode == EXIT_USAGE


def test_fixture_files_exist_only_at_runtime(tmp_path: Path) -> None:
    """Die Geräteordner liegen unter tmp_path, nicht im Repository."""
    device = make_device(tmp_path, activity=(NAME_101,))
    assert tmp_path in device.parents
    assert not list(Path(__file__).parent.rglob("*.fit"))
    assert not list(Path(__file__).parent.rglob("*.FIT"))
