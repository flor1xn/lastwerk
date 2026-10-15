"""Abgleich der API-Aktivitäten im Archiv mit den Gerätedateien (SPEC 4.2 F28, SPEC 12 M4c).

Liest nur das Archiv (Manifest, ``summary.json``, Listenseiten), sendet nichts. Der
Bericht enthält Kennungen, Dateinamen und Zählungen, keine Werte, auch keine
Zeitpunkte.

Zuordnung (Konvention, bis zur Prüfung am echten Gerät, O4):

1. Der Dateiname trägt die Aktivitätskennung (Gruppe ``activity_id`` im Muster):
   Zuordnung über die Kennung.
2. Der Dateiname trägt einen Zeitstempel (Gruppen ``year``, ``month``, ``day``,
   ``hour``, ``minute``, ``second``, so benennt Garmin Aktivitätsdateien auf dem
   Gerät, zum Beispiel ``2026-05-10-06-00-00.fit``): Zuordnung zur API-Aktivität,
   deren ``startTimeGMT`` höchstens ``tolerance_s`` Sekunden abweicht (Konvention
   120 s). Der Zeitstempel gilt als UTC, ``name_offset_minutes`` rechnet eine
   Ortszeit um (UTC = Namenszeit minus Versatz). Je Aktivität und je Datei höchstens
   eine Zuordnung, kleinste Abweichung zuerst.

Dateien, deren Name nicht zum Muster passt, stehen unter ``nicht_zuordenbar`` und
nicht unter ``nur_geraet``: von ihnen ist nicht bekannt, ob die API sie hat.
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass, field
from datetime import UTC, datetime, timedelta
from typing import Any

from .activities import ActivityIndex, start_gmt_of
from .archive import RECONCILE_REPORT_NAME, Archive, write_bytes_atomic
from .usb import DEVICE_DIR, KIND

SCHEMA_VERSION = 1
ACTIVITY_FOLDER = "Activity"
# Garmin benennt Aktivitätsdateien auf dem Gerät nach dem Startzeitpunkt. Alternativ
# trägt der Name die Kennung (zum Beispiel ``12345678901.fit`` oder
# ``12345678901_ACTIVITY.fit``). Das Muster ist über ``--name-pattern`` änderbar.
DEFAULT_NAME_PATTERN = (
    r"^(?:(?P<year>\d{4})-(?P<month>\d{2})-(?P<day>\d{2})"
    r"-(?P<hour>\d{2})-(?P<minute>\d{2})-(?P<second>\d{2})"
    r"|(?P<activity_id>\d{6,}))(?:_ACTIVITY)?\.fit$"
)
# Konvention: Uhren von Gerät und Konto weichen um wenige Sekunden ab, 120 s fängt das ab.
DEFAULT_TOLERANCE_S = 120
CONSOLE_LIST_LIMIT = 50
_TIME_GROUPS = ("year", "month", "day", "hour", "minute", "second")


class ReconcileError(Exception):
    """Voraussetzung nicht erfüllt (Exit-Code 2)."""


@dataclass(frozen=True)
class MatchConfig:
    pattern: str = DEFAULT_NAME_PATTERN
    tolerance_s: int = DEFAULT_TOLERANCE_S
    name_offset_minutes: int = 0

    def compiled(self) -> re.Pattern[str]:
        try:
            return re.compile(self.pattern, re.IGNORECASE)
        except re.error as exc:
            raise ReconcileError(f"Ungültiges Namensmuster: {exc}") from exc


@dataclass(frozen=True)
class DeviceFile:
    device: str
    name: str

    @property
    def label(self) -> str:
        return f"{self.device}/{self.name}"


@dataclass
class Reconciliation:
    both: dict[str, DeviceFile] = field(default_factory=dict)
    only_api: list[str] = field(default_factory=list)
    only_device: list[DeviceFile] = field(default_factory=list)
    unassignable: list[DeviceFile] = field(default_factory=list)
    api_count: int = 0
    device_count: int = 0


def device_activity_files(archive: Archive) -> list[DeviceFile]:
    """Aktivitätsdateien aus dem Manifest (jüngster Eintrag je Gerät und Dateiname)."""
    endpoint = f"{DEVICE_DIR}/{ACTIVITY_FOLDER}"
    found: list[DeviceFile] = []
    for entry in archive.latest().values():
        if entry.kind != KIND or entry.endpoint != endpoint or entry.status != "ok":
            continue
        params = entry.params or {}
        device, name = params.get("device"), params.get("file")
        if isinstance(device, str) and isinstance(name, str):
            found.append(DeviceFile(device, name))
    return sorted(found, key=lambda f: (f.device, f.name))


def has_device_copy(archive: Archive) -> bool:
    return any(e.kind == KIND for e in archive.latest().values())


def api_start_times(archive: Archive) -> dict[str, datetime | None]:
    """Kennung auf Startzeit (UTC) für alle bekannten API-Aktivitäten.

    Quellen: ``summary.json`` je Aktivität, sonst die Zeile der Listenseite. Eine
    Aktivität ohne lesbare Startzeit bleibt mit ``None`` erhalten.
    """
    index = ActivityIndex.load(archive)
    times: dict[str, datetime | None] = {
        str(ref.activity_id): ref.start_utc for ref in index.newest_first()
    }
    for entry in archive.latest().values():
        if entry.kind != "activity" or entry.endpoint != "summary" or entry.status != "ok":
            continue
        activity_id = (entry.params or {}).get("activity_id")
        if activity_id is None:
            continue
        try:
            start = start_gmt_of(archive.read_json(entry))
        except (OSError, ValueError):
            start = None
        if start is not None:
            times[str(activity_id)] = start
        else:
            times.setdefault(str(activity_id), None)
    return times


def _name_time(match: re.Match[str], config: MatchConfig) -> tuple[str | None, datetime | None]:
    groups = match.groupdict()
    activity_id = groups.get("activity_id")
    if activity_id:
        return activity_id, None
    if not all(groups.get(g) for g in _TIME_GROUPS):
        return None, None
    try:
        local = datetime(*(int(groups[g]) for g in _TIME_GROUPS), tzinfo=UTC)  # type: ignore[arg-type]
    except ValueError:
        return None, None
    return None, local - timedelta(minutes=config.name_offset_minutes)


def reconcile(
    api: dict[str, datetime | None],
    files: list[DeviceFile],
    config: MatchConfig | None = None,
) -> Reconciliation:
    config = config or MatchConfig()
    pattern = config.compiled()
    result = Reconciliation(api_count=len(api), device_count=len(files))
    matched_ids: set[str] = set()
    matched_files: set[DeviceFile] = set()

    parsed: dict[DeviceFile, tuple[str | None, datetime | None]] = {}
    for item in files:
        match = pattern.match(item.name)
        if match is None:
            result.unassignable.append(item)
            continue
        parsed[item] = _name_time(match, config)
        if parsed[item] == (None, None):
            result.unassignable.append(item)
            del parsed[item]

    for item, (activity_id, _when) in parsed.items():
        if activity_id is not None and activity_id in api and activity_id not in matched_ids:
            result.both[activity_id] = item
            matched_ids.add(activity_id)
            matched_files.add(item)

    tolerance = timedelta(seconds=config.tolerance_s)
    candidates: list[tuple[timedelta, str, str, str, DeviceFile]] = []
    for item, (_activity_id, when) in parsed.items():
        if when is None or item in matched_files:
            continue
        for activity_id, start in api.items():
            if start is None or activity_id in matched_ids:
                continue
            diff = abs(start - when)
            if diff <= tolerance:
                candidates.append((diff, item.device, item.name, activity_id, item))
    for _diff, _device, _name, activity_id, item in sorted(candidates, key=lambda c: c[:4]):
        if activity_id in matched_ids or item in matched_files:
            continue
        result.both[activity_id] = item
        matched_ids.add(activity_id)
        matched_files.add(item)

    result.only_api = sorted(set(api) - matched_ids, key=_id_order)
    result.only_device = sorted(
        (item for item in parsed if item not in matched_files), key=lambda f: (f.device, f.name)
    )
    return result


def _id_order(value: str) -> tuple[int, int | str]:
    return (0, int(value)) if value.isdigit() else (1, value)


def run_reconcile(archive: Archive, config: MatchConfig | None = None) -> Reconciliation:
    """Liest das Archiv und gleicht ab. Wirft ``ReconcileError`` ohne Geräteordner im Archiv."""
    files = device_activity_files(archive)
    if not files:
        raise ReconcileError(
            "Keine Dateien unter device_usb/*/Activity im Archiv. Zuerst `import-device` ausführen."
        )
    return reconcile(api_start_times(archive), files, config)


def device_matches(archive: Archive, config: MatchConfig | None = None) -> dict[str, DeviceFile]:
    """Kennung auf zugeordnete Gerätedatei. Leer, wenn das Archiv keine Gerätekopie hat."""
    files = device_activity_files(archive)
    if not files:
        return {}
    return reconcile(api_start_times(archive), files, config).both


def build_report(
    archive: Archive, result: Reconciliation, config: MatchConfig | None = None
) -> dict[str, Any]:
    config = config or MatchConfig()
    return {
        "schema_version": SCHEMA_VERSION,
        "generated_at": archive.now().isoformat(timespec="seconds"),
        "name_pattern": config.pattern,
        "tolerance_s": config.tolerance_s,
        "name_offset_minutes": config.name_offset_minutes,
        "counts": {
            "api": result.api_count,
            "device": result.device_count,
            "beide": len(result.both),
            "nur_api": len(result.only_api),
            "nur_geraet": len(result.only_device),
            "nicht_zuordenbar": len(result.unassignable),
        },
        "nur_api": result.only_api,
        "nur_geraet": [f.label for f in result.only_device],
        "beide": len(result.both),
        "nicht_zuordenbar": [f.label for f in result.unassignable],
    }


def write_report(archive: Archive, report: dict[str, Any]) -> None:
    payload = json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    write_bytes_atomic(archive.root / RECONCILE_REPORT_NAME, payload.encode("utf-8"))


def format_report(report: dict[str, Any], *, console_limit: int = CONSOLE_LIST_LIMIT) -> list[str]:
    counts = report["counts"]
    lines = [
        f"Abgleich API gegen Gerät (Stand {report['generated_at']}): "
        f"{counts['api']} Aktivitäten in der API, {counts['device']} Dateien auf dem Gerät",
        f"  beide: {counts['beide']}",
        f"  nur API: {counts['nur_api']}",
    ]
    lines += _listing("nur API", report["nur_api"], console_limit)
    lines.append(f"  nur Gerät: {counts['nur_geraet']}")
    lines += _listing("nur Gerät", report["nur_geraet"], console_limit)
    if counts["nicht_zuordenbar"]:
        lines.append(
            f"  nicht zuordenbar (Name passt nicht zum Muster): {counts['nicht_zuordenbar']}"
        )
        lines += _listing("nicht zuordenbar", report["nicht_zuordenbar"], console_limit)
    return lines


def _listing(label: str, items: list[Any], limit: int) -> list[str]:
    lines = [f"    {label}: {item}" for item in items[:limit]]
    if len(items) > limit:
        lines.append(f"    ... und {len(items) - limit} weitere, alle in {RECONCILE_REPORT_NAME}")
    return lines
