"""import-device: unveränderte Kopie, Manifest, Prüfbericht, Exit-Code 2 (SPEC 12, M4c).

Die Geräteordner entstehen zur Laufzeit unter ``tmp_path`` (``device_fixture.py``),
es liegt keine ``*.fit``-Datei im Index.
"""

from __future__ import annotations

import hashlib
import io
import json
import os
import subprocess
import sys
from pathlib import Path

from lastwerk_bridge.archive import DERIVED_FILES, Archive
from lastwerk_bridge.cli import EXIT_OK, EXIT_USAGE, main

from .device_fixture import fake_fit, make_device, write
from .fakes import FakeClock

ACTIVITY = ("2026-10-10-06-00-00.fit", "2026-10-11-07-30-00.fit")
MONITOR = ("1A2B3C4D.FIT",)
SLEEP = ("2026-10-11-22-00-00.fit",)


def run(argv: list[str], clock: FakeClock) -> tuple[int, str, str]:
    out, err = io.StringIO(), io.StringIO()
    code = main(argv, clock=clock, stdout=out, stderr=err)
    return code, out.getvalue(), err.getvalue()


def import_device(device: Path, root: Path, clock: FakeClock, *extra: str):
    return run(["import-device", str(device), "--root", str(root), *extra], clock)


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def tree_digest(path: Path) -> dict[str, str]:
    return {p.relative_to(path).as_posix(): sha(p) for p in sorted(path.rglob("*")) if p.is_file()}


def manifest_lines(root: Path) -> list[dict]:
    return [
        json.loads(line)
        for line in (root / "manifest.jsonl").read_text(encoding="utf-8").splitlines()
    ]


def report_of(root: Path) -> dict:
    return json.loads((root / "device-import-report.json").read_text(encoding="utf-8"))


def test_files_are_copied_unchanged_and_noted_in_the_manifest(
    tmp_path: Path, root: Path, clock: FakeClock
) -> None:
    device = make_device(tmp_path, activity=ACTIVITY, monitor=MONITOR, sleep=SLEEP)
    before = tree_digest(device)

    code, out, err = import_device(device, root, clock)
    assert code == EXIT_OK, err

    # Bytes unverändert: SHA-256 der Quelle gleich SHA-256 der Kopie, Quelle unberührt.
    assert tree_digest(device) == before
    for folder, names in (("Activity", ACTIVITY), ("Monitor", MONITOR), ("Sleep", SLEEP)):
        for name in names:
            copy = root / "device_usb" / "DEVICE" / folder / name
            assert copy.is_file(), copy
            assert sha(copy) == before[f"{folder}/{name}"]
            assert copy.read_bytes() == (device / folder / name).read_bytes()

    entries = manifest_lines(root)
    assert len(entries) == 4, "ein Eintrag je Datei"
    for entry in entries:
        assert entry["kind"] == "device_usb" and entry["status"] == "ok"
        folder = entry["endpoint"].split("/", 1)[1]
        name = entry["params"]["file"]
        assert entry["endpoint"] == f"device_usb/{folder}"
        assert entry["params"] == {"device": "DEVICE", "file": name}
        assert entry["path"] == f"device_usb/DEVICE/{folder}/{name}"
        assert entry["sha256"] == before[f"{folder}/{name}"]
        assert entry["bytes"] == (root / entry["path"]).stat().st_size
    assert Archive(root, lib_version="x", clock=clock).verify() == []

    assert "Gesamt: erkannt 4, kopiert 4, übersprungen 0, unbekannt 0" in out
    assert "Activity" in out and "Monitor" in out and "Sleep" in out
    report = report_of(root)
    assert report["total"] == {
        "recognized": 4,
        "copied": 4,
        "skipped": 0,
        "unknown": 0,
        "changed": 0,
    }
    assert report["folders"]["Activity"]["copied"] == 2
    assert report["folders"]["Monitor"]["copied"] == 1
    assert report["folders"]["Sleep"]["copied"] == 1


def test_second_import_skips_everything(tmp_path: Path, root: Path, clock: FakeClock) -> None:
    device = make_device(tmp_path, activity=ACTIVITY, monitor=MONITOR, sleep=SLEEP)
    import_device(device, root, clock)
    manifest = (root / "manifest.jsonl").read_bytes()
    archived = tree_digest(root / "device_usb")

    code, out, _err = import_device(device, root, clock)
    assert code == EXIT_OK
    assert (root / "manifest.jsonl").read_bytes() == manifest, "keine neuen Einträge"
    assert tree_digest(root / "device_usb") == archived, "keine neuen Dateien"
    assert "Gesamt: erkannt 4, kopiert 0, übersprungen 4, unbekannt 0" in out
    assert report_of(root)["total"]["skipped"] == 4
    assert sorted(report_of(root)["skipped_files"]) == sorted(
        [f"Activity/{n}" for n in ACTIVITY] + [f"Monitor/{MONITOR[0]}", f"Sleep/{SLEEP[0]}"]
    )


def test_new_files_on_the_second_run_are_copied_and_old_ones_skipped(
    tmp_path: Path, root: Path, clock: FakeClock
) -> None:
    device = make_device(tmp_path, activity=ACTIVITY)
    import_device(device, root, clock)
    write(device / "Activity" / "2026-10-12-08-00-00.fit", fake_fit("neu"))
    _code, out, _err = import_device(device, root, clock)
    assert "Gesamt: erkannt 3, kopiert 1, übersprungen 2, unbekannt 0" in out
    assert len(manifest_lines(root)) == 3


def test_unknown_files_are_counted_and_not_copied(
    tmp_path: Path, root: Path, clock: FakeClock
) -> None:
    device = make_device(tmp_path, activity=ACTIVITY[:1])
    write(device / "Activity" / "notes.txt", b"kein fit")
    write(device / "Activity" / "broken.fit", b"\x0e\x10\x00\x00" + b"x" * 30)
    write(device / "Activity" / "short.fit", b"\x0e")
    write(device / "Music" / "2026-10-10-06-00-00.fit", fake_fit("fremder Ordner"))
    write(device / "Music" / "song.mp3", b"kein fit")
    (device / "Activity" / "link.fit").symlink_to(device / "Activity" / ACTIVITY[0])

    code, out, _err = import_device(device, root, clock)
    assert code == EXIT_OK
    report = report_of(root)
    assert report["folders"]["Activity"] == {
        "recognized": 1,
        "copied": 1,
        "skipped": 0,
        "unknown": 4,
        "changed": 0,
    }
    # Eine .fit-Datei außerhalb der erkannten Ordner ist unbekannt, andere Dateien dort nicht.
    assert report["folders"]["other"]["unknown"] == 1
    assert sorted(report["unknown_files"]) == [
        "Activity/broken.fit",
        "Activity/link.fit",
        "Activity/notes.txt",
        "Activity/short.fit",
        "Music/2026-10-10-06-00-00.fit",
    ]
    assert report["total"]["unknown"] == 5
    assert "unbekannt 5" in out and "unbekannt: Activity/notes.txt" in out

    copied = sorted(p.name for p in (root / "device_usb").rglob("*") if p.is_file())
    assert copied == [ACTIVITY[0]], "unbekannte Dateien werden nicht kopiert"
    assert len(manifest_lines(root)) == 1


def test_missing_folder_exits_2_and_writes_nothing(
    tmp_path: Path, root: Path, clock: FakeClock
) -> None:
    code, out, err = import_device(tmp_path / "nicht-da", root, clock)
    assert code == EXIT_USAGE and out == ""
    assert "Ordner nicht gefunden" in err and "nichts kopiert" in err
    assert not root.exists()


def test_folder_without_device_subfolders_exits_2(
    tmp_path: Path, root: Path, clock: FakeClock
) -> None:
    other = tmp_path / "STICK"
    write(other / "Music" / "a.fit", fake_fit("x"))
    code, _out, err = import_device(other, root, clock)
    assert code == EXIT_USAGE
    assert "Activity, Monitor, Sleep" in err
    assert not root.exists()

    code, _out, err = import_device(other / "Music" / "a.fit", root, clock)
    assert code == EXIT_USAGE, "eine Datei ist kein Ordner"


def test_missing_root_option_exits_2(tmp_path: Path, clock: FakeClock) -> None:
    device = make_device(tmp_path, activity=ACTIVITY)
    code, _out, err = run(["import-device", str(device)], clock)
    assert code == EXIT_USAGE and "--root fehlt" in err


def test_vendor_subfolder_and_any_case_are_found(
    tmp_path: Path, root: Path, clock: FakeClock
) -> None:
    device = make_device(tmp_path, vendor_dir="Garmin")
    write(device / "Garmin" / "ACTIVITY" / "2026-10-10-06-00-00.FIT", fake_fit("a"))
    write(device / "Garmin" / "sleep" / "2026-10-10-22-00-00.fit", fake_fit("b"))
    code, out, _err = import_device(device, root, clock)
    assert code == EXIT_OK
    assert "Gesamt: erkannt 2, kopiert 2" in out
    # Im Archiv gilt die kanonische Schreibweise der Ordner, der Dateiname bleibt.
    assert (root / "device_usb" / "DEVICE" / "Activity" / "2026-10-10-06-00-00.FIT").is_file()
    assert (root / "device_usb" / "DEVICE" / "Sleep" / "2026-10-10-22-00-00.fit").is_file()


def test_device_name_option_and_default(tmp_path: Path, root: Path, clock: FakeClock) -> None:
    device = make_device(tmp_path, activity=ACTIVITY[:1])
    import_device(device, root, clock, "--device", "Edge 840")
    assert (root / "device_usb" / "Edge_840" / "Activity" / ACTIVITY[0]).is_file()
    assert manifest_lines(root)[0]["params"]["device"] == "Edge_840"

    import_device(device, root, clock, "--device", "../../evil")
    assert (root / "device_usb" / "evil" / "Activity" / ACTIVITY[0]).is_file()
    assert not (tmp_path / "evil").exists()

    code, _out, err = import_device(device, root, clock, "--device", "...")
    assert code == EXIT_USAGE and "Gerätename" in err


def test_same_name_with_other_content_is_never_overwritten(
    tmp_path: Path, root: Path, clock: FakeClock
) -> None:
    device = make_device(tmp_path, activity=ACTIVITY[:1])
    import_device(device, root, clock)
    first = root / "device_usb" / "DEVICE" / "Activity" / ACTIVITY[0]
    first_bytes = first.read_bytes()

    write(device / "Activity" / ACTIVITY[0], fake_fit("anderer Inhalt"))
    code, out, _err = import_device(device, root, clock)
    assert code == EXIT_OK
    assert first.read_bytes() == first_bytes, "die erste Kopie bleibt unverändert"
    assert "davon abweichend 1" in out
    copies = sorted(p.name for p in first.parent.iterdir())
    assert len(copies) == 2 and ACTIVITY[0] in copies
    assert report_of(root)["total"]["changed"] == 1
    assert Archive(root, lib_version="x", clock=clock).verify() == []

    # Dritter Lauf mit demselben neuen Inhalt: übersprungen.
    _code, out, _err = import_device(device, root, clock)
    assert "kopiert 0, übersprungen 1" in out


def test_file_without_manifest_entry_is_noted_not_duplicated(
    tmp_path: Path, root: Path, clock: FakeClock
) -> None:
    """Abbruch zwischen Schreiben und Vermerken: der nächste Lauf vermerkt nur."""
    device = make_device(tmp_path, activity=ACTIVITY[:1])
    import_device(device, root, clock)
    (root / "manifest.jsonl").write_text("", encoding="utf-8")

    code, _out, _err = import_device(device, root, clock)
    assert code == EXIT_OK
    files = [p for p in (root / "device_usb").rglob("*") if p.is_file()]
    assert len(files) == 1
    assert len(manifest_lines(root)) == 1
    assert Archive(root, lib_version="x", clock=clock).verify() == []


def test_report_has_names_and_counts_only(tmp_path: Path, root: Path, clock: FakeClock) -> None:
    device = make_device(tmp_path, activity=ACTIVITY, monitor=MONITOR)
    write(device / "Activity" / "notes.txt", b"kein fit")
    _code, out, _err = import_device(device, root, clock)
    text = (root / "device-import-report.json").read_text(encoding="utf-8")
    report = json.loads(text)
    assert set(report) == {
        "schema_version",
        "generated_at",
        "device",
        "source_name",
        "folders",
        "total",
        "copied_files",
        "skipped_files",
        "unknown_files",
    }
    assert set(report["copied_files"]) == {f"Activity/{n}" for n in ACTIVITY} | {
        f"Monitor/{n}" for n in MONITOR
    }
    # Weder Prüfsummen noch Dateiinhalt, nur Namen und Zählungen.
    for path in (device / "Activity").glob("*.fit"):
        digest = sha(path)
        assert digest not in text and digest not in out
        assert path.read_bytes().hex() not in text


def test_archive_inside_the_device_folder_is_refused(tmp_path: Path, clock: FakeClock) -> None:
    device = make_device(tmp_path, activity=ACTIVITY[:1])
    code, _out, err = import_device(device, device / "archiv", clock)
    assert code == EXIT_USAGE and "nicht im Geräteordner" in err
    assert not (device / "archiv").exists()


def test_report_file_is_derived_and_no_verify_problem(
    tmp_path: Path, root: Path, clock: FakeClock
) -> None:
    assert "device-import-report.json" in DERIVED_FILES
    device = make_device(tmp_path, activity=ACTIVITY)
    import_device(device, root, clock)
    import_device(device, root, clock)
    assert Archive(root, lib_version="x", clock=clock).verify() == []


def _bridge(*args: str) -> subprocess.CompletedProcess[str]:
    env = {k: v for k, v in os.environ.items() if k != "PYTHONPATH"}
    return subprocess.run(
        [sys.executable, "-m", "lastwerk_bridge", *args],
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
        env=env,
    )


def test_command_runs_as_subprocess_against_a_temporary_folder(tmp_path: Path) -> None:
    """Der Befehl aus SPEC 12 M4c als Unterprozess, ohne Anmeldung und ohne Netz."""
    device = make_device(tmp_path, activity=ACTIVITY, sleep=SLEEP)
    target = tmp_path / "target" / "archive"
    result = _bridge("import-device", str(device), "--root", str(target))
    assert result.returncode == EXIT_OK, result.stderr
    assert "Gesamt: erkannt 3, kopiert 3, übersprungen 0, unbekannt 0" in result.stdout
    assert (target / "device-import-report.json").is_file()

    missing = _bridge("import-device", str(tmp_path / "x"), "--root", str(target))
    assert missing.returncode == EXIT_USAGE and "Traceback" not in missing.stderr
