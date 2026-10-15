"""Normalisierung des Archivs für Lastwerk (SPEC 5.4, 5.5, 12 M4d).

``normalize`` leitet aus dem Archiv die Dateien unter ``exchange/out`` ab:

- ``activities/<id>.fit``: die Bytes von ``original.fit`` unverändert (Endpunkt
  ``original_fit``, Quelle ist der jüngste ``ok``-Eintrag je Aktivität),
- ``recovery/YYYY-MM-DD.json``: ein Erholungstag nach ``schema/recovery-day.schema.json``
  (SPEC 7.3) aus den Tageswerten HRV, Ruhepuls und Schlaf.

Das Programm sendet nichts und braucht keine Anmeldung. Es liest nur Dateien, die das
Manifest für die genannten Endpunkte als ``ok`` führt und deren Pfad unter ``activities/``
oder ``daily/`` liegt. Der Ordner ``sensitive/`` (Klasse S3, SPEC 5.5) wird nie geöffnet,
solange keine Kategorie freigegeben ist. Freigegeben werden Kategorien mit
``--allow-sensitive <kategorie>``, Standard keine. In M4d verarbeitet noch keine Kategorie
etwas, die Option hält nur den Platz und wird im Bericht vermerkt.

Der Lauf ist wiederholbar: Eine Ausgabedatei wird nur geschrieben, wenn sie fehlt oder ihre
SHA-256 abweicht. Ein zweiter Lauf auf unverändertem Archiv schreibt nichts. Der Bericht
``normalize-report.json`` im Archiv ist abgeleitet (SPEC 5.5), enthält Zählungen und
Kennungen, keine Messwerte.

Feldnamen der Garmin-Antworten
------------------------------

Die Zuordnung ``FIELD_MAP`` von Schema-Feld zu Quelle im Archiv beruht auf Annahmen über die
Ausgabe der Bibliothek. Sie sind bis zur Sonde in L1 (``docs/garmin-feldabdeckung.md``,
SPEC 14 O9) nicht gegen das echte Konto geprüft. Zeigt die Sonde andere Namen, genügt es,
``FIELD_MAP`` anzupassen. Ein Wert, der nicht im Bereich des Schemas liegt, wird weggelassen
und im Bericht gezählt, nie auf die Grenze gekürzt.
"""

from __future__ import annotations

import hashlib
import json
import math
import os
import re
from dataclasses import dataclass, field
from datetime import UTC, date, datetime
from pathlib import Path, PurePosixPath
from typing import Any

from . import __version__
from .archive import (
    NORMALIZE_REPORT_NAME,
    Archive,
    sha256_of,
    write_bytes_atomic,
)
from .manifest import ManifestEntry, ManifestValidationError, load_schema, validate_value
from .registry import REGISTRY

SCHEMA_VERSION = 1
RECOVERY_SCHEMA_FILENAME = "recovery-day.schema.json"
ACTIVITIES_OUT = "activities"
RECOVERY_OUT = "recovery"
ORIGINAL_FIT_ENDPOINT = "original_fit"
# Nur unter diesen Ordnern des Archivs liest normalize (SPEC 5.5 Archivlayout).
READABLE_PREFIXES = ("activities", "daily")
SENSITIVE_DIR = "sensitive"
# Obergrenze für eine Tagesdatei beim Lesen. Faustregel: Herzfrequenz über den ganzen Tag
# hat einige hundert KB, 32 MiB schützt vor einer unerwartet großen Antwort.
MAX_DAY_FILE_BYTES = 32 * 1024 * 1024
# Garmin-Kennungen sind Zahlen. Nur sie werden Dateiname unter ``activities/``.
_ACTIVITY_ID = re.compile(r"^[0-9]{1,20}$")
_CHUNK = 1024 * 1024

# Plausibilitätsgrenzen aus SPEC 7.3 (Faustregel). Ein Test hält sie gleich mit dem Schema.
LIMITS: dict[str, tuple[float, float]] = {
    "hrv_overnight_ms": (0, 400),
    "resting_hr_bpm": (20, 120),
    "sleep_duration_s": (0, 86400),
    "sleep_score": (0, 100),
}
INTEGER_FIELDS = ("sleep_duration_s", "sleep_score")


@dataclass(frozen=True)
class FieldSource:
    """Ein Wert in einer Tagesantwort: Endpunkt im Register und Pfad durch das JSON."""

    endpoint: str
    path: tuple[str | int, ...]


# ANNAHME bis L1: Feldnamen aus der Ausgabe von garminconnect 0.3.17 (get_hrv_data,
# get_rhr_day, get_heart_rates, get_sleep_data), nicht gegen das echte Konto geprüft.
# Je Feld gilt die erste Quelle mit gültigem Wert im Bereich des Schemas.
FIELD_MAP: dict[str, tuple[FieldSource, ...]] = {
    "hrv_overnight_ms": (FieldSource("hrv", ("hrvSummary", "lastNightAvg")),),
    "resting_hr_bpm": (
        FieldSource(
            "rhr",
            ("allMetrics", "metricsMap", "WELLNESS_RESTING_HEART_RATE", 0, "value"),
        ),
        FieldSource("heart_rates", ("restingHeartRate",)),
    ),
    "sleep_duration_s": (FieldSource("sleep", ("dailySleepDTO", "sleepTimeSeconds")),),
    "sleep_score": (FieldSource("sleep", ("dailySleepDTO", "sleepScores", "overall", "value")),),
}


def sensitive_categories() -> tuple[str, ...]:
    """Kategorien der Klasse S3 aus dem Register (Ordner unter ``sensitive/``)."""
    return tuple(sorted({e.category for e in REGISTRY if e.sensitivity == "S3" and e.category}))


def sensitive_endpoints() -> frozenset[str]:
    return frozenset(e.name for e in REGISTRY if e.sensitivity == "S3")


def field_map_endpoints(field_map: dict[str, tuple[FieldSource, ...]] | None = None) -> set[str]:
    return {s.endpoint for sources in (field_map or FIELD_MAP).values() for s in sources}


def check_field_map(field_map: dict[str, tuple[FieldSource, ...]] | None = None) -> None:
    """Die Zuordnung darf nur freigegebene Endpunkte (Klasse S2) lesen, nie S3.

    Wirft ``ValueError``, wenn ein Endpunkt unbekannt oder gesperrt abgelegt ist oder ein Feld
    zugeordnet ist, das das Schema nicht kennt.
    """
    by_name = {e.name: e for e in REGISTRY}
    mapping = field_map or FIELD_MAP
    for name, sources in mapping.items():
        if name not in LIMITS:
            raise ValueError(f"FIELD_MAP: Feld {name} steht nicht im Schema")
        for source in sources:
            entry = by_name.get(source.endpoint)
            if entry is None:
                raise ValueError(f"FIELD_MAP: unbekannter Endpunkt {source.endpoint}")
            if entry.sensitivity == "S3":
                raise ValueError(
                    f"FIELD_MAP: {source.endpoint} liegt unter sensitive/ und ist gesperrt"
                )
            if entry.granularity != "day":
                raise ValueError(f"FIELD_MAP: {source.endpoint} ist kein Tagesendpunkt")


class NormalizeError(Exception):
    """Voraussetzung nicht erfüllt (Exit-Code 2). Es wurde nichts geschrieben."""


@dataclass
class Report:
    """Zählungen eines Laufs. Enthält keine Messwerte."""

    activities_total: int = 0
    activities_copied: int = 0
    activities_unchanged: int = 0
    recovery_days: int = 0
    recovery_written: int = 0
    recovery_unchanged: int = 0
    recovery_no_values: int = 0
    fields_out_of_range: int = 0
    problems: list[dict[str, str]] = field(default_factory=list)
    allowed_sensitive: tuple[str, ...] = ()

    def problem(self, kind: str, subject: str) -> None:
        self.problems.append({"kind": kind, "subject": subject})

    def to_dict(self, generated_at: str) -> dict[str, Any]:
        return {
            "schema_version": SCHEMA_VERSION,
            "generated_at": generated_at,
            "source": source_name(),
            "activities": {
                "total": self.activities_total,
                "copied": self.activities_copied,
                "unchanged": self.activities_unchanged,
            },
            "recovery": {
                "days_with_sources": self.recovery_days,
                "written": self.recovery_written,
                "unchanged": self.recovery_unchanged,
                "without_values": self.recovery_no_values,
                "fields_out_of_range": self.fields_out_of_range,
            },
            "sensitive": {"allowed": list(self.allowed_sensitive), "read": 0},
            "field_map": "assumed_until_L1",
            "problems": self.problems,
        }


def source_name() -> str:
    """Wert von ``source`` im Erholungstag (höchstens 64 Zeichen, SPEC 7.3)."""
    return f"garmin-bridge {__version__}"[:64]


# Lesen aus dem Archiv


def _safe_relative(path: str) -> PurePosixPath | None:
    """Relativer Archivpfad aus dem Manifest, nur unter den lesbaren Ordnern.

    ``None`` bei absoluten Pfaden, ``..``, Laufwerken oder Pfaden außerhalb von
    ``READABLE_PREFIXES``. ``sensitive/`` ist damit nie lesbar.
    """
    if not path or "\\" in path or "\x00" in path:
        return None
    rel = PurePosixPath(path)
    if rel.is_absolute() or ".." in rel.parts or not rel.parts:
        return None
    if rel.parts[0] not in READABLE_PREFIXES:
        return None
    return rel


def _archive_file(archive: Archive, entry: ManifestEntry) -> Path | None:
    """Datei eines Manifest-Eintrags im Archiv oder ``None``, wenn der Pfad nicht lesbar ist."""
    if entry.path is None:
        return None
    rel = _safe_relative(entry.path)
    if rel is None:
        return None
    target = archive.root / rel
    root = archive.root.resolve()
    try:
        resolved = target.resolve()
    except OSError:
        return None
    # Symlinks aus dem Archiv heraus oder nach sensitive/ werden nicht gefolgt.
    if root not in resolved.parents or not target.is_file():
        return None
    if resolved.relative_to(root).parts[0] not in READABLE_PREFIXES:
        return None
    return target


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(_CHUNK), b""):
            digest.update(chunk)
    return digest.hexdigest()


def copy_atomic(source: Path, target: Path) -> None:
    """Kopiert ``source`` in Stücken in eine temporäre Datei daneben und benennt um."""
    target.parent.mkdir(parents=True, exist_ok=True)
    tmp = target.with_name(f".{target.name}.tmp")
    with open(source, "rb") as src, open(tmp, "wb") as dst:
        for chunk in iter(lambda: src.read(_CHUNK), b""):
            dst.write(chunk)
        dst.flush()
        os.fsync(dst.fileno())
    os.replace(tmp, target)


def _valid_day(value: Any) -> date | None:
    if not isinstance(value, str):
        return None
    try:
        parsed = date.fromisoformat(value)
    except ValueError:
        return None
    return parsed if parsed.isoformat() == value else None


# Aktivitäten


def _normalize_activities(archive: Archive, out: Path, report: Report) -> None:
    for entry in sorted(archive.latest().values(), key=lambda e: e.key):
        if entry.kind != "activity" or entry.endpoint != ORIGINAL_FIT_ENDPOINT:
            continue
        if entry.status != "ok":
            continue
        params = entry.params or {}
        activity_id = str(params.get("activity_id", ""))
        report.activities_total += 1
        if not _ACTIVITY_ID.match(activity_id):
            report.problem("invalid_activity_id", "original_fit")
            continue
        source = _archive_file(archive, entry)
        if source is None:
            report.problem("source_missing", activity_id)
            continue
        source_sha = sha256_file(source)
        if entry.sha256 is not None and source_sha != entry.sha256:
            report.problem("source_checksum_mismatch", activity_id)
            continue
        target = out / ACTIVITIES_OUT / f"{activity_id}.fit"
        if target.is_file() and not target.is_symlink() and sha256_file(target) == source_sha:
            report.activities_unchanged += 1
            continue
        copy_atomic(source, target)
        report.activities_copied += 1


# Erholungstage


def _dig(value: Any, path: tuple[str | int, ...]) -> Any:
    for key in path:
        if isinstance(key, int):
            if not isinstance(value, list) or key >= len(value):
                return None
            value = value[key]
        else:
            if not isinstance(value, dict) or key not in value:
                return None
            value = value[key]
    return value


def _number(value: Any) -> float | None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    return float(value) if math.isfinite(value) else None


def extract_fields(
    responses: dict[str, Any],
    field_map: dict[str, tuple[FieldSource, ...]] | None = None,
) -> tuple[dict[str, float | int], int]:
    """Werte eines Tages aus den Antworten je Endpunkt. Gibt auch die Zahl der Werte außerhalb
    des Bereichs zurück, die weggelassen wurden."""
    values: dict[str, float | int] = {}
    out_of_range = 0
    for name, sources in (field_map or FIELD_MAP).items():
        low, high = LIMITS[name]
        for source in sources:
            if source.endpoint not in responses:
                continue
            number = _number(_dig(responses[source.endpoint], source.path))
            if number is None:
                continue
            if name in INTEGER_FIELDS:
                number = int(round(number))
            if not low <= number <= high:
                out_of_range += 1
                continue
            values[name] = number
            break
    return values, out_of_range


def _parse_fetched(text: str) -> datetime | None:
    try:
        parsed = datetime.fromisoformat(text.replace("Z", "+00:00"))
    except ValueError:
        return None
    return parsed.replace(tzinfo=UTC) if parsed.tzinfo is None else parsed.astimezone(UTC)


def _day_sources(
    archive: Archive,
    field_map: dict[str, tuple[FieldSource, ...]],
) -> dict[str, dict[str, ManifestEntry]]:
    """Jüngste ``ok``-Einträge der benötigten Tagesendpunkte, je Tag und Endpunkt."""
    wanted = field_map_endpoints(field_map)
    days: dict[str, dict[str, ManifestEntry]] = {}
    for entry in archive.latest().values():
        if entry.kind != "daily" or entry.endpoint not in wanted or entry.status != "ok":
            continue
        day = _valid_day((entry.params or {}).get("date"))
        if day is None:
            continue
        days.setdefault(day.isoformat(), {})[entry.endpoint] = entry
    return days


def recovery_document(
    day: str, values: dict[str, float | int], fetched_at: datetime
) -> dict[str, Any]:
    doc: dict[str, Any] = {
        "schema_version": 1,
        "date": day,
        "source": source_name(),
        "fetched_at": fetched_at.astimezone(UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
    }
    doc.update(values)
    return doc


def encode_document(doc: dict[str, Any]) -> bytes:
    """Feste Darstellung: Schlüssel sortiert, eingerückt, ein Zeilenende am Schluss."""
    return (json.dumps(doc, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode("utf-8")


def recovery_schema_path() -> Path:
    return Path(__file__).resolve().parents[3] / "schema" / RECOVERY_SCHEMA_FILENAME


def load_recovery_schema() -> dict[str, Any]:
    """Lädt ``schema/recovery-day.schema.json``. Ohne Schema wird nichts geschrieben.

    Die Prüfung gegen das Schema ist die Grenze zu Lastwerk (SPEC 10.2, "Zusatzprogramm liefert
    manipulierte Dateien"). Fehlt die Datei, etwa weil das Programm außerhalb des Repositories
    liegt, darf ``normalize`` nicht ungeprüft schreiben.
    """
    path = recovery_schema_path()
    if not path.is_file():
        raise NormalizeError(
            f"Schemadatei fehlt: {path}. Ohne sie lassen sich die Erholungsdateien nicht "
            "prüfen (Programm im Repository starten, nicht aus einer Kopie)."
        )
    try:
        return load_schema(path)
    except (OSError, ValueError) as exc:  # ManifestValidationError ist ein ValueError
        raise NormalizeError(f"Schemadatei nicht lesbar: {path} ({type(exc).__name__}).") from exc


def _normalize_recovery(
    archive: Archive,
    out: Path,
    report: Report,
    field_map: dict[str, tuple[FieldSource, ...]],
    schema: dict[str, Any],
) -> None:
    for day, entries in sorted(_day_sources(archive, field_map).items()):
        report.recovery_days += 1
        responses: dict[str, Any] = {}
        fetched: list[datetime] = []
        for endpoint, entry in sorted(entries.items()):
            source = _archive_file(archive, entry)
            if source is None:
                report.problem("source_missing", f"{endpoint} {day}")
                continue
            if source.stat().st_size > MAX_DAY_FILE_BYTES:
                report.problem("source_too_large", f"{endpoint} {day}")
                continue
            data = source.read_bytes()
            if entry.sha256 is not None and sha256_of(data) != entry.sha256:
                report.problem("source_checksum_mismatch", f"{endpoint} {day}")
                continue
            try:
                responses[endpoint] = json.loads(data.decode("utf-8"))
            except (UnicodeDecodeError, ValueError):
                report.problem("source_not_json", f"{endpoint} {day}")
                continue
            stamp = _parse_fetched(entry.fetched_at)
            if stamp is not None:
                fetched.append(stamp)
        values, skipped = extract_fields(responses, field_map)
        report.fields_out_of_range += skipped
        if not values or not fetched:
            report.recovery_no_values += 1
            continue
        doc = recovery_document(day, values, max(fetched))
        try:
            validate_value(doc, schema)
        except ManifestValidationError:
            report.problem("schema_violation", day)
            continue
        payload = encode_document(doc)
        target = out / RECOVERY_OUT / f"{day}.json"
        if (
            target.is_file()
            and not target.is_symlink()
            and sha256_of(target.read_bytes()) == (sha256_of(payload))
        ):
            report.recovery_unchanged += 1
            continue
        write_bytes_atomic(target, payload)
        report.recovery_written += 1


# Lauf


def check_paths(root: Path, out: Path) -> None:
    """Ausgabeordner und Archiv dürfen sich nicht überlappen."""
    root_r, out_r = root.resolve(), out.resolve()
    if root_r == out_r or root_r in out_r.parents or out_r in root_r.parents:
        raise NormalizeError(
            "Archiv und Ausgabeordner dürfen sich nicht überlappen (--root und --out)."
        )


def run_normalize(
    archive: Archive,
    out: Path,
    *,
    allow_sensitive: tuple[str, ...] = (),
    field_map: dict[str, tuple[FieldSource, ...]] | None = None,
) -> Report:
    """Leitet ``exchange/out`` aus dem Archiv ab und schreibt den Bericht ins Archiv."""
    mapping = field_map or FIELD_MAP
    check_field_map(mapping)
    unknown = sorted(set(allow_sensitive) - set(sensitive_categories()))
    if unknown:
        raise NormalizeError(f"Unbekannte Kategorie: {', '.join(unknown)}.")
    check_paths(archive.root, out)
    schema = load_recovery_schema()  # vor dem ersten Schreiben, damit "nichts geschrieben" stimmt
    report = Report(allowed_sensitive=tuple(sorted(set(allow_sensitive))))
    _normalize_activities(archive, out, report)
    _normalize_recovery(archive, out, report, mapping, schema)
    payload = (
        json.dumps(
            report.to_dict(archive.now().isoformat(timespec="seconds")),
            ensure_ascii=False,
            indent=2,
            sort_keys=True,
        )
        + "\n"
    ).encode("utf-8")
    write_bytes_atomic(archive.root / NORMALIZE_REPORT_NAME, payload)
    return report


def format_report(report: Report) -> list[str]:
    lines = [
        f"Aktivitäten: {report.activities_total} mit original.fit, "
        f"{report.activities_copied} kopiert, {report.activities_unchanged} unverändert",
        f"Erholungstage: {report.recovery_days} Tage mit Quelle, "
        f"{report.recovery_written} geschrieben, {report.recovery_unchanged} unverändert, "
        f"{report.recovery_no_values} ohne Werte",
    ]
    if report.fields_out_of_range:
        lines.append(f"Werte außerhalb des Bereichs weggelassen: {report.fields_out_of_range}")
    if report.allowed_sensitive:
        lines.append(
            "Freigegebene Kategorien: "
            + ", ".join(report.allowed_sensitive)
            + " (in M4d wird daraus noch nichts abgeleitet)"
        )
    for item in report.problems[:50]:
        lines.append(f"  Problem {item['kind']}: {item['subject']}")
    if len(report.problems) > 50:
        lines.append(
            f"  ... und {len(report.problems) - 50} weitere, alle in {NORMALIZE_REPORT_NAME}"
        )
    return lines
