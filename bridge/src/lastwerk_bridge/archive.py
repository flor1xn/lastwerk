"""Archiv mit unveränderten Antworten und Manifest (SPEC 5.5, Archivlayout und Manifest).

``root`` ist der Ordner, der ``manifest.jsonl`` direkt enthält, im Normalfall
``local/archive/garmin``. Regeln:

- Dateien werden atomar geschrieben (temporäre Datei, dann ``os.replace``).
- Eine geschriebene Datei wird nie überschrieben. Eine Neuabfrage erzeugt eine
  Datei mit Zeitstempel im Namen und einen neuen Manifest-Eintrag.
- Das Manifest wird nur angehängt, der jüngste Eintrag je Schlüssel gilt.
- Leere und nicht verfügbare Antworten werden vermerkt und nicht erneut abgefragt,
  außer für die letzten N Tage (Standard 7, Faustregel), weil Garmin Werte
  nachträglich finalisiert.
"""

from __future__ import annotations

import hashlib
import json
import os
from datetime import UTC, date, datetime
from pathlib import Path
from typing import Any

from .manifest import ManifestEntry, default_schema_path, load_schema, make_key, validate_entry
from .registry import RegistryEntry, to_yaml
from .throttle import Clock, SystemClock, local_today

MANIFEST_NAME = "manifest.jsonl"
REGISTRY_COPY_NAME = "registry.yaml"
STATE_FILE_NAME = "state.json"
COVERAGE_FILE_NAME = "coverage.json"
DEVICE_IMPORT_REPORT_NAME = "device-import-report.json"
RECONCILE_REPORT_NAME = "reconcile-report.json"
NORMALIZE_REPORT_NAME = "normalize-report.json"
# Abgeleitete Dateien im Archivordner: werden neu erzeugt und überschrieben, haben
# deshalb keinen Manifest-Eintrag.
DERIVED_FILES = (
    REGISTRY_COPY_NAME,
    STATE_FILE_NAME,
    COVERAGE_FILE_NAME,
    DEVICE_IMPORT_REPORT_NAME,
    RECONCILE_REPORT_NAME,
    NORMALIZE_REPORT_NAME,
)
STATE_DIR_NAME = "state"
DEFAULT_REFRESH_DAYS = 7


def sha256_of(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def encode_payload(value: Any) -> bytes:
    """Antwort als Bytes: Rohbytes unverändert, sonst JSON ohne Umformung der Werte."""
    if isinstance(value, (bytes, bytearray)):
        return bytes(value)
    return (json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n").encode("utf-8")


def is_empty(value: Any) -> bool:
    """Leere Antwort (Faustregel): None, leere Bytes oder Zeichenketten, leere
    Sammlungen und Objekte, deren Werte alle leer sind. Zahlen zählen als Inhalt,
    auch 0 (ein Tag mit 0 Schritten ist ein Wert)."""
    if value is None:
        return True
    if isinstance(value, (bytes, bytearray, str)):
        return len(value) == 0
    if isinstance(value, dict):
        return all(is_empty(v) for v in value.values())
    if isinstance(value, (list, tuple)):
        return all(is_empty(v) for v in value)
    return False


def write_bytes_atomic(path: Path, data: bytes) -> None:
    """Schreibt in eine temporäre Datei daneben und benennt dann um."""
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(f".{path.name}.tmp")
    with open(tmp, "wb") as fh:
        fh.write(data)
        fh.flush()
        os.fsync(fh.fileno())
    os.replace(tmp, path)


class Archive:
    def __init__(
        self,
        root: Path,
        *,
        lib_version: str,
        clock: Clock | None = None,
        schema_path: Path | None = None,
    ) -> None:
        self.root = Path(root)
        self.lib_version = lib_version
        self.clock = clock or SystemClock()
        path = schema_path or default_schema_path()
        self._schema = load_schema(path) if path.is_file() else None

    # Pfade

    @property
    def manifest_path(self) -> Path:
        return self.root / MANIFEST_NAME

    @property
    def state_dir(self) -> Path:
        return self.root / STATE_DIR_NAME

    def now(self) -> datetime:
        return datetime.fromtimestamp(self.clock.now(), tz=UTC)

    def today(self) -> date:
        """Heute als lokaler Kalendertag: Garmin ordnet Tageswerte dem Tag des Nutzers zu."""
        return local_today(self.clock)

    def layout_path(self, entry: RegistryEntry, params: dict[str, Any]) -> Path:
        """Relativer Ablagepfad nach SPEC 5.5 Archivlayout."""
        ext = entry.ext
        if entry.sensitivity == "S3":
            base = Path("sensitive") / str(entry.category) / entry.name
            if entry.granularity == "day":
                return base / f"{params['date']}.{ext}"
            if entry.granularity == "range":
                return base / f"{params['start']}_{params['end']}.{ext}"
            return base / f"{entry.name}.{ext}"
        if entry.granularity == "once":
            return Path("account") / f"{entry.name}.{ext}"
        if entry.granularity == "page":
            return Path("activities") / "list" / f"page-{int(params['start']):04d}.{ext}"
        if entry.granularity == "per_activity":
            year = str(params.get("year", "unknown"))
            return Path("activities") / year / str(params["activity_id"]) / f"{entry.name}.{ext}"
        if entry.granularity == "day":
            day = str(params["date"])
            return Path("daily") / day[:4] / day / f"{entry.name}.{ext}"
        return Path("ranges") / entry.name / f"{params['start']}_{params['end']}.{ext}"

    def _unique_path(self, relative: Path) -> Path:
        """Nie überschreiben: bei Kollision Zeitstempel in den Namen."""
        target = self.root / relative
        if not target.exists():
            return target
        stamp = self.now().strftime("%Y%m%dT%H%M%SZ")
        candidate = target.with_name(f"{target.stem}.{stamp}{target.suffix}")
        counter = 1
        while candidate.exists():
            candidate = target.with_name(f"{target.stem}.{stamp}-{counter}{target.suffix}")
            counter += 1
        return candidate

    # Manifest

    def entries(self) -> list[ManifestEntry]:
        if not self.manifest_path.is_file():
            return []
        result: list[ManifestEntry] = []
        with open(self.manifest_path, encoding="utf-8") as fh:
            for line in fh:
                line = line.strip()
                if line:
                    result.append(ManifestEntry.from_dict(json.loads(line)))
        return result

    def latest(self) -> dict[str, ManifestEntry]:
        current: dict[str, ManifestEntry] = {}
        for entry in self.entries():
            current[entry.key] = entry
        return current

    def record(self, entry: ManifestEntry) -> ManifestEntry:
        data = entry.to_dict()
        if self._schema is not None:
            validate_entry(data, self._schema)
        self.root.mkdir(parents=True, exist_ok=True)
        line = json.dumps(data, ensure_ascii=False, sort_keys=True) + "\n"
        with open(self.manifest_path, "a", encoding="utf-8") as fh:
            fh.write(line)
            fh.flush()
            os.fsync(fh.fileno())
        return entry

    def store(
        self,
        kind: str,
        endpoint: str,
        params: dict[str, Any] | None,
        payload: bytes,
        relative_path: Path,
        *,
        http_status: int | None = None,
    ) -> ManifestEntry:
        """Schreibt die Antwort atomar und vermerkt sie mit SHA-256 im Manifest."""
        target = self._unique_path(relative_path)
        write_bytes_atomic(target, payload)
        entry = ManifestEntry(
            kind=kind,
            endpoint=endpoint,
            params=params or None,
            status="ok",
            http_status=http_status,
            fetched_at=self.now().isoformat(timespec="seconds"),
            lib_version=self.lib_version,
            path=target.relative_to(self.root).as_posix(),
            bytes=len(payload),
            sha256=sha256_of(payload),
        )
        return self.record(entry)

    def note(
        self,
        kind: str,
        endpoint: str,
        params: dict[str, Any] | None,
        status: str,
        *,
        http_status: int | None = None,
        error_class: str | None = None,
    ) -> ManifestEntry:
        """Vermerkt eine leere, nicht verfügbare oder fehlgeschlagene Antwort."""
        entry = ManifestEntry(
            kind=kind,
            endpoint=endpoint,
            params=params or None,
            status=status,
            http_status=http_status,
            fetched_at=self.now().isoformat(timespec="seconds"),
            lib_version=self.lib_version,
            error_class=error_class[:64] if error_class else None,
        )
        return self.record(entry)

    # Fortsetzung

    def needs_fetch(
        self,
        kind: str,
        endpoint: str,
        params: dict[str, Any] | None,
        *,
        latest: dict[str, ManifestEntry] | None = None,
        day: date | None = None,
        refresh_days: int = DEFAULT_REFRESH_DAYS,
        today: date | None = None,
    ) -> bool:
        """Entscheidet, ob ein Schlüssel abgefragt wird.

        Kein Eintrag oder letzter Eintrag ``error``: ja. Liegt ``day`` höchstens
        ``refresh_days`` Tage vor ``today`` (gestern bis vor N Tagen, dazu heute):
        ja, auch bei ``ok`` oder ``empty``. Sonst nein.
        """
        current = latest if latest is not None else self.latest()
        entry = current.get(make_key(kind, endpoint, params))
        if entry is None or entry.status == "error":
            return True
        if day is not None:
            today = today or self.today()
            if (today - day).days <= refresh_days:
                return True
        return False

    def read_bytes(self, entry: ManifestEntry) -> bytes:
        """Inhalt der Archivdatei eines Manifest-Eintrags (nur Einträge mit ``path``)."""
        if entry.path is None:
            raise ValueError(f"Manifest-Eintrag ohne Datei: {entry.key}")
        return (self.root / entry.path).read_bytes()

    def read_json(self, entry: ManifestEntry) -> Any:
        return json.loads(self.read_bytes(entry).decode("utf-8"))

    # Register

    def copy_registry(self) -> Path:
        """Legt den Stand des Registers beim Lauf ab (darf überschrieben werden)."""
        target = self.root / REGISTRY_COPY_NAME
        write_bytes_atomic(target, to_yaml().encode("utf-8"))
        return target

    # Prüfung

    def verify(self) -> list[str]:
        """Prüft jede Archivdatei gegen das Manifest: genau ein Eintrag, SHA-256 passt."""
        problems: list[str] = []
        seen: dict[str, int] = {}
        for entry in self.entries():
            if entry.path is None:
                continue
            seen[entry.path] = seen.get(entry.path, 0) + 1
            file_path = self.root / entry.path
            if not file_path.is_file():
                problems.append(f"fehlt: {entry.path}")
                continue
            data = file_path.read_bytes()
            if entry.bytes != len(data):
                problems.append(f"Größe weicht ab: {entry.path}")
            if entry.sha256 != sha256_of(data):
                problems.append(f"SHA-256 weicht ab: {entry.path}")
        for path, count in seen.items():
            if count != 1:
                problems.append(f"{count} Einträge für {path}")
        for file_path in self.root.rglob("*"):
            if not file_path.is_file():
                continue
            rel = file_path.relative_to(self.root).as_posix()
            if rel == MANIFEST_NAME or rel in DERIVED_FILES or rel.startswith(STATE_DIR_NAME + "/"):
                continue
            if rel not in seen:
                problems.append(f"ohne Manifest-Eintrag: {rel}")
        return problems
