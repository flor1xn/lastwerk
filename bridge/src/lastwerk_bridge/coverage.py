"""Abdeckungsbericht (SPEC 5.5, Endpunkt-Register; SPEC 12, M4b).

Liest nur das Archiv (Manifest und Listenseiten), sendet nie etwas und braucht
keine Anmeldung. Je Registereintrag steht der Status, das älteste und jüngste Datum
der gelieferten Daten und die Anzahl Dateien. Zusätzlich stehen Aktivitäten ohne
``original.fit`` im Bericht; ist für sie eine Gerätedatei aus der USB-Kopie
zugeordnet (``reconcile``), steht sie als "über Gerät vorhanden" dabei. Ausgabe
als Text auf der Konsole und als ``coverage.json`` im Archiv (abgeleitet, wird
überschrieben).

Status je Eintrag, aus den jüngsten Manifest-Einträgen je Schlüssel:

- ``available`` (verfügbar): mindestens ein Schlüssel mit Datei,
- ``empty`` (leer): kein Schlüssel mit Datei, mindestens einer leer,
- ``not_available`` (nicht verfügbar): nur Antworten 404,
- ``unchecked`` (ungeprüft): kein Eintrag oder nur Fehler. Fehler sind ein
  Zustand des Abrufs, keine Aussage über das Konto, sie stehen in ``keys.error``.
"""

from __future__ import annotations

import json
from collections import Counter
from datetime import date
from typing import Any

from .activities import (
    ORIGINAL_ENDPOINT,
    ORIGINAL_FIT_ENDPOINT,
    ActivityIndex,
    parse_day,
)
from .archive import COVERAGE_FILE_NAME, Archive, write_bytes_atomic
from .manifest import ManifestEntry, make_key
from .reconcile import DeviceFile, device_matches, has_device_copy
from .registry import REGISTRY, RegistryEntry

STATUS_LABEL = {
    "available": "verfügbar",
    "empty": "leer",
    "not_available": "nicht verfügbar",
    "unchecked": "ungeprüft",
}
SCHEMA_VERSION = 1
# So viele Aktivitäten ohne Original stehen auf der Konsole, alle stehen in coverage.json.
CONSOLE_LIST_LIMIT = 50


def _status_of(keys: Counter[str]) -> str:
    if keys["ok"]:
        return "available"
    if keys["empty"]:
        return "empty"
    if keys["not_available"]:
        return "not_available"
    return "unchecked"


def _dates_of(
    entry: RegistryEntry, found: list[ManifestEntry], index: ActivityIndex
) -> tuple[date | None, date | None]:
    """Ältestes und jüngstes Datum der Daten (nur Schlüssel mit Datei)."""
    days: list[date] = []
    for item in found:
        if item.status != "ok" or not item.params:
            continue
        if entry.granularity == "day":
            day = parse_day(item.params.get("date"))
            if day:
                days.append(day)
        elif entry.granularity == "range":
            for name in ("start", "end"):
                day = parse_day(item.params.get(name))
                if day:
                    days.append(day)
        elif entry.granularity == "per_activity":
            ref = index.get(item.params.get("activity_id"))
            if ref is not None and ref.day is not None:
                days.append(ref.day)
    if entry.granularity == "page":
        days = [r.day for r in index.newest_first() if r.day is not None]
    return (min(days), max(days)) if days else (None, None)


def _row(
    name: str,
    meta: dict[str, Any],
    found: list[ManifestEntry],
    files: int,
    dates: tuple[date | None, date | None],
) -> dict[str, Any]:
    keys: Counter[str] = Counter(item.status for item in found)
    status = _status_of(keys)
    return {
        "name": name,
        **meta,
        "status": status,
        "status_label": STATUS_LABEL[status],
        "files": files,
        "oldest": dates[0].isoformat() if dates[0] else None,
        "newest": dates[1].isoformat() if dates[1] else None,
        "keys": {s: keys[s] for s in ("ok", "empty", "not_available", "error")},
    }


def _missing_original(
    index: ActivityIndex,
    latest: dict[str, ManifestEntry],
    on_device: dict[str, DeviceFile],
) -> list[dict[str, Any]]:
    missing = []
    for ref in index.newest_first():
        params = ref.params
        fit = latest.get(make_key("activity", ORIGINAL_FIT_ENDPOINT, params))
        if fit is not None and fit.status == "ok":
            continue
        zip_entry = latest.get(make_key("activity", ORIGINAL_ENDPOINT, params))
        if zip_entry is None:
            reason = "nicht geholt"
        elif zip_entry.status != "ok":
            reason = {
                "empty": "Antwort leer",
                "not_available": "nicht verfügbar (404)",
            }.get(zip_entry.status, f"Abruf fehlgeschlagen ({zip_entry.error_class or 'Fehler'})")
        elif fit is None:
            reason = "Entpacken ausstehend"
        else:
            reason = f"Entpacken fehlgeschlagen ({fit.error_class or 'Fehler'})"
        missing.append(
            {
                "activity_id": ref.activity_id,
                "date": ref.day.isoformat() if ref.day else None,
                "reason": reason,
                "via_device": on_device[str(ref.activity_id)].label
                if str(ref.activity_id) in on_device
                else None,
            }
        )
    return missing


def build_coverage(
    archive: Archive, registry: tuple[RegistryEntry, ...] = REGISTRY
) -> dict[str, Any]:
    entries = archive.entries()
    latest = archive.latest()
    index = ActivityIndex.load(archive)
    by_endpoint: dict[str, list[ManifestEntry]] = {}
    for item in latest.values():
        by_endpoint.setdefault(item.endpoint, []).append(item)
    files: Counter[str] = Counter(e.endpoint for e in entries if e.path)

    rows = []
    for entry in registry:
        meta = {
            "method": entry.method,
            "granularity": entry.granularity,
            "class": entry.sensitivity,
            "consumer": entry.consumer,
        }
        found = by_endpoint.get(entry.name, [])
        rows.append(
            _row(entry.name, meta, found, files[entry.name], _dates_of(entry, found, index))
        )

    fit_found = by_endpoint.get(ORIGINAL_FIT_ENDPOINT, [])
    fit_days = [
        r.day
        for r in index.newest_first()
        if r.day is not None
        and (f := latest.get(make_key("activity", ORIGINAL_FIT_ENDPOINT, r.params))) is not None
        and f.status == "ok"
    ]
    derived = [
        _row(
            ORIGINAL_FIT_ENDPOINT,
            {"method": "(aus original entpackt)", "granularity": "per_activity", "class": "S1"},
            fit_found,
            files[ORIGINAL_FIT_ENDPOINT],
            (min(fit_days), max(fit_days)) if fit_days else (None, None),
        )
    ]
    on_device = device_matches(archive)
    missing = _missing_original(index, latest, on_device)
    report = {
        "schema_version": SCHEMA_VERSION,
        "generated_at": archive.now().isoformat(timespec="seconds"),
        "lib_version": archive.lib_version,
        "manifest_entries": len(entries),
        "entries": rows,
        "derived": derived,
        "activities": {"known": len(index), "without_original_fit": len(missing)},
        "activities_without_original_fit": missing,
    }
    if has_device_copy(archive):
        report["device"] = {
            "matched": len(on_device),
            "without_original_fit_via_device": sum(1 for m in missing if m["via_device"]),
        }
    return report


def write_coverage(archive: Archive, report: dict) -> None:
    payload = (json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode()
    write_bytes_atomic(archive.root / COVERAGE_FILE_NAME, payload)


def format_coverage(report: dict, *, console_limit: int = CONSOLE_LIST_LIMIT) -> list[str]:
    lines = [
        f"Abdeckung des Archivs (Stand {report['generated_at']}, "
        f"{report['manifest_entries']} Manifest-Einträge)",
    ]
    rows = report["entries"] + report["derived"]
    width = max(len(r["name"]) for r in rows)
    lines.append(
        f"  {'Endpunkt':<{width}}  {'Klasse':<6}  {'Status':<15}  {'Dateien':>7}  "
        f"{'Ältestes':<10}  {'Jüngstes':<10}  Schlüssel ok/leer/n.v./Fehler"
    )
    for row in rows:
        keys = row["keys"]
        lines.append(
            f"  {row['name']:<{width}}  {row['class']:<6}  {row['status_label']:<15}  "
            f"{row['files']:>7}  {row['oldest'] or '-':<10}  {row['newest'] or '-':<10}  "
            f"{keys['ok']}/{keys['empty']}/{keys['not_available']}/{keys['error']}"
        )
    counts = Counter(r["status"] for r in report["entries"])
    lines.append(
        "Einträge: "
        + ", ".join(f"{STATUS_LABEL[s]} {counts[s]}" for s in STATUS_LABEL)
        + f" (von {len(report['entries'])})"
    )
    info = report["activities"]
    lines.append(
        f"Aktivitäten: {info['known']} bekannt, {info['without_original_fit']} ohne original.fit"
    )
    missing = report["activities_without_original_fit"]
    if "device" in report:
        lines.append(
            f"USB-Kopie: {report['device']['matched']} Aktivitäten einer Gerätedatei zugeordnet, "
            f"{report['device']['without_original_fit_via_device']} davon ohne original.fit"
        )
    for item in missing[:console_limit]:
        via = f", über Gerät vorhanden: {item['via_device']}" if item.get("via_device") else ""
        lines.append(
            f"  ohne original.fit: {item['activity_id']} ({item['date'] or '?'}), "
            f"{item['reason']}{via}"
        )
    if len(missing) > console_limit:
        lines.append(
            f"  ... und {len(missing) - console_limit} weitere, alle in {COVERAGE_FILE_NAME}"
        )
    return lines
