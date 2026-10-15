"""USB-Kopie der Geräteordner in das Archiv (SPEC 4.2 F28, SPEC 5.5, SPEC 12 M4c).

Rückfall für den Fall, dass der Garmin-Zugang gesperrt ist. Das Modul liest nur
vom Gerät und kopiert Dateien unverändert nach ``device_usb/<geraet>/<Ordner>/``.
Es parst kein FIT. Eine Datei zählt als FIT-Datei, wenn sie in einem der Ordner
``Activity``, ``Monitor`` oder ``Sleep`` liegt, die Endung ``.fit`` (Groß- und
Kleinschreibung egal) trägt und der Kopf stimmt: Kopflänge 12 oder 14 und die
Kennung ``.FIT`` an Byte 8 bis 11 (Aufbau nach dem FIT-SDK, Anhang A der SPEC).

Die Ordner liegen je nach Gerät direkt im Datenträger oder unter ``GARMIN/``
(Groß- und Kleinschreibung egal, Konvention bis zur Prüfung am echten Gerät, O4).
Es wird nichts auf das Gerät geschrieben und nichts gelöscht.

Eine Datei wird nie überschrieben:

- gleiche SHA-256 wie der jüngste Manifest-Eintrag unter demselben Schlüssel:
  ``übersprungen``,
- gleicher Name, anderer Inhalt: neue Datei mit Zeitstempel im Namen
  (Archivregel, ``Archive.store``), gezählt als ``kopiert`` und ``abweichend``.
"""

from __future__ import annotations

import json
import os
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from . import __version__
from .archive import (
    DEVICE_IMPORT_REPORT_NAME,
    Archive,
    sha256_of,
    write_bytes_atomic,
)
from .manifest import ManifestEntry, make_key

KIND = "device_usb"
DEVICE_DIR = "device_usb"
# Ordner, die übernommen werden (SPEC 4.2 F28). Die Schreibweise im Archiv ist diese.
FOLDERS = ("Activity", "Monitor", "Sleep")
OTHER = "other"
# Zwischenordner mancher Geräte (Konvention, O4).
VENDOR_DIR = "garmin"
FIT_SIGNATURE = b".FIT"
FIT_HEADER_SIZES = (12, 14)
# Obergrenze je Datei. Faustregel: Aufzeichnungen haben wenige MB, die Grenze
# schützt davor, etwas Fremdes vollständig in den Speicher zu lesen.
MAX_FIT_BYTES = 256 * 1024 * 1024
# So tief unter dem Datenträger wird nach FIT-Dateien in fremden Ordnern gesucht.
MAX_SCAN_DEPTH = 4
SCHEMA_VERSION = 1
CONSOLE_LIST_LIMIT = 50
_DEVICE_NAME_UNSAFE = re.compile(r"[^A-Za-z0-9._-]+")


class DeviceImportError(Exception):
    """Voraussetzung nicht erfüllt (Exit-Code 2). Es wurde nichts geschrieben."""


def lib_version() -> str:
    """Quelle der Einträge ist dieses Programm, nicht die Garmin-Bibliothek."""
    return f"lastwerk-bridge {__version__}"


def sanitize_device_name(name: str) -> str:
    """Gerätename als einzelner Ordnername: keine Trenner, kein führender Punkt."""
    cleaned = _DEVICE_NAME_UNSAFE.sub("_", name.strip()).strip("._")
    if not cleaned:
        raise DeviceImportError(f"Gerätename nicht verwendbar: {name!r}.")
    return cleaned[:64]


def default_device_name(folder: Path) -> str:
    name = folder.resolve().name
    if not name:
        raise DeviceImportError("Gerätename nicht ableitbar, bitte --device angeben.")
    return sanitize_device_name(name)


def has_fit_header(head: bytes) -> bool:
    """Kopf einer FIT-Datei: Länge 12 oder 14 in Byte 0, Kennung ``.FIT`` an Byte 8 bis 11."""
    return len(head) >= 12 and head[0] in FIT_HEADER_SIZES and head[8:12] == FIT_SIGNATURE


def canonical_folder(name: str) -> str | None:
    for folder in FOLDERS:
        if name.lower() == folder.lower():
            return folder
    return None


def _subdirs(path: Path) -> list[Path]:
    try:
        with os.scandir(path) as it:
            found = [Path(e.path) for e in it if e.is_dir(follow_symlinks=False)]
    except OSError:
        return []
    return sorted(found)


def find_device_dirs(root: Path) -> dict[Path, str]:
    """Erkannte Ordner (Pfad auf kanonischen Namen): direkt im Datenträger und unter ``GARMIN``."""
    found: dict[Path, str] = {}
    parents = [root] + [d for d in _subdirs(root) if d.name.lower() == VENDOR_DIR]
    for parent in parents:
        for child in _subdirs(parent):
            canon = canonical_folder(child.name)
            if canon is not None:
                found[child] = canon
    return found


@dataclass
class FolderCount:
    recognized: int = 0
    copied: int = 0
    skipped: int = 0
    unknown: int = 0
    changed: int = 0

    def to_dict(self) -> dict[str, int]:
        return {
            "recognized": self.recognized,
            "copied": self.copied,
            "skipped": self.skipped,
            "unknown": self.unknown,
            "changed": self.changed,
        }


@dataclass
class ImportReport:
    device: str
    source_name: str
    folders: dict[str, FolderCount] = field(default_factory=dict)
    unknown_files: list[str] = field(default_factory=list)
    copied_files: list[str] = field(default_factory=list)
    skipped_files: list[str] = field(default_factory=list)
    generated_at: str = ""

    def count(self, label: str) -> FolderCount:
        return self.folders.setdefault(label, FolderCount())

    def total(self) -> FolderCount:
        total = FolderCount()
        for item in self.folders.values():
            for name in total.to_dict():
                setattr(total, name, getattr(total, name) + getattr(item, name))
        return total

    def to_dict(self) -> dict[str, Any]:
        labels = [*FOLDERS, OTHER]
        return {
            "schema_version": SCHEMA_VERSION,
            "generated_at": self.generated_at,
            "device": self.device,
            "source_name": self.source_name,
            "folders": {label: self.count(label).to_dict() for label in labels},
            "total": self.total().to_dict(),
            "copied_files": self.copied_files,
            "skipped_files": self.skipped_files,
            "unknown_files": self.unknown_files,
        }


def _read_fit(path: Path) -> bytes | None:
    """Inhalt, wenn die Datei eine FIT-Datei ist (Endung, Größe, Kopf), sonst ``None``.
    Liest die Datei genau einmal, damit Prüfung und Kopie dieselben Bytes sehen."""
    if path.is_symlink() or not path.is_file() or not path.name.lower().endswith(".fit"):
        return None
    try:
        if path.stat().st_size > MAX_FIT_BYTES:
            return None
        data = path.read_bytes()
    except OSError:
        return None
    return data if has_fit_header(data[:14]) else None


def _relative(path: Path, root: Path) -> str:
    return path.relative_to(root).as_posix()


def _walk(root: Path, recognized: dict[Path, str]) -> tuple[list[Path], list[Path]]:
    """Dateien in erkannten Ordnern und ``.fit``-Dateien an anderer Stelle (nicht rekursiv,
    ohne Symlinks zu folgen, Tiefe begrenzt)."""
    inside: list[Path] = []
    outside: list[Path] = []
    for current, dirs, files in os.walk(root, followlinks=False):
        here = Path(current)
        depth = len(here.relative_to(root).parts)
        dirs.sort()
        if depth >= MAX_SCAN_DEPTH:
            dirs[:] = []
        for name in sorted(files):
            path = here / name
            if here in recognized:
                inside.append(path)
            elif name.lower().endswith(".fit"):
                outside.append(path)
    return inside, outside


def import_device(
    folder: Path,
    archive: Archive,
    *,
    device: str | None = None,
) -> ImportReport:
    """Kopiert die Geräteordner unter ``device_usb/<geraet>/``. Wirft ``DeviceImportError``,
    wenn der Ordner fehlt oder keinen der Ordner ``Activity``, ``Monitor``, ``Sleep`` enthält."""
    root = Path(folder)
    if not root.is_dir():
        raise DeviceImportError(f"Ordner nicht gefunden: {folder}.")
    recognized = find_device_dirs(root)
    if not recognized:
        raise DeviceImportError(
            f"Keiner der Ordner {', '.join(FOLDERS)} unter {folder} (auch nicht unter GARMIN/)."
        )
    resolved_root = archive.root.resolve()
    if resolved_root == root.resolve() or root.resolve() in resolved_root.parents:
        raise DeviceImportError("Das Archiv darf nicht im Geräteordner liegen.")
    name = sanitize_device_name(device) if device else default_device_name(root)
    report = ImportReport(device=name, source_name=root.resolve().name, generated_at=_now(archive))
    for label in (*FOLDERS, OTHER):
        report.count(label)

    latest = archive.latest()
    inside, outside = _walk(root, recognized)
    for path in outside:
        report.count(OTHER).unknown += 1
        report.unknown_files.append(_relative(path, root))
    for path in inside:
        label = recognized[path.parent]
        counts = report.count(label)
        data = _read_fit(path)
        if data is None:
            counts.unknown += 1
            report.unknown_files.append(_relative(path, root))
            continue
        counts.recognized += 1
        outcome = _copy_one(archive, latest, name, label, path.name, data)
        if outcome == "skipped":
            counts.skipped += 1
            report.skipped_files.append(_relative(path, root))
        else:
            counts.copied += 1
            if outcome == "changed":
                counts.changed += 1
            report.copied_files.append(_relative(path, root))
    write_report(archive, report)
    return report


def _now(archive: Archive) -> str:
    return archive.now().isoformat(timespec="seconds")


def _copy_one(
    archive: Archive,
    latest: dict[str, ManifestEntry],
    device: str,
    folder: str,
    filename: str,
    data: bytes,
) -> str:
    """``copied``, ``changed`` oder ``skipped``. Nie überschreiben."""
    endpoint = f"{DEVICE_DIR}/{folder}"
    params = {"device": device, "file": filename}
    relative = Path(DEVICE_DIR) / device / folder / filename
    digest = sha256_of(data)
    key = make_key(KIND, endpoint, params)
    previous = latest.get(key)
    target = archive.root / relative
    if (
        previous is not None
        and previous.status == "ok"
        and previous.sha256 == digest
        and previous.path is not None
        and (archive.root / previous.path).is_file()
    ):
        return "skipped"
    if target.is_file() and sha256_of(target.read_bytes()) == digest:
        # Datei liegt schon da, nur der Manifest-Eintrag fehlt (Abbruch zwischen
        # Schreiben und Vermerken). Nichts neu schreiben, nur vermerken.
        entry = archive.record(
            ManifestEntry(
                kind=KIND,
                endpoint=endpoint,
                params=params,
                status="ok",
                fetched_at=_now(archive),
                lib_version=archive.lib_version,
                path=relative.as_posix(),
                bytes=len(data),
                sha256=digest,
            )
        )
        latest[key] = entry
        return "copied"
    entry = archive.store(KIND, endpoint, params, data, relative)
    latest[key] = entry
    changed = previous is not None and previous.status == "ok" and previous.sha256 != digest
    return "changed" if changed else "copied"


def write_report(archive: Archive, report: ImportReport) -> None:
    payload = json.dumps(report.to_dict(), ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    write_bytes_atomic(archive.root / DEVICE_IMPORT_REPORT_NAME, payload.encode("utf-8"))


def format_report(report: ImportReport, *, console_limit: int = CONSOLE_LIST_LIMIT) -> list[str]:
    lines = [f"USB-Kopie von {report.source_name} als Gerät {report.device}"]
    for label in (*FOLDERS, OTHER):
        item = report.count(label)
        shown = "andere Ordner" if label == OTHER else label
        lines.append(
            f"  {shown:<13} erkannt {item.recognized}, kopiert {item.copied}, "
            f"übersprungen {item.skipped}, unbekannt {item.unknown}"
            + (f", davon abweichend {item.changed}" if item.changed else "")
        )
    total = report.total()
    lines.append(
        f"Gesamt: erkannt {total.recognized}, kopiert {total.copied}, "
        f"übersprungen {total.skipped}, unbekannt {total.unknown}"
    )
    for name in report.unknown_files[:console_limit]:
        lines.append(f"  unbekannt: {name}")
    if len(report.unknown_files) > console_limit:
        lines.append(
            f"  ... und {len(report.unknown_files) - console_limit} weitere, "
            f"alle in {DEVICE_IMPORT_REPORT_NAME}"
        )
    return lines
