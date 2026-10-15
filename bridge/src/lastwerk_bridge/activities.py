"""Aktivitäten im Archiv: Index aus den Listenseiten, FIT-Original aus dem ZIP.

Der Index liest die abgelegten Seiten der Aktivitätsliste (``activities/list/``),
damit ``backfill`` und ``sync`` ohne weiteren Aufruf wissen, welche Aktivitäten
bekannt sind. Die Feldnamen ``activityId``, ``startTimeLocal`` und ``startTimeGMT``
stammen aus der Antwort von ``get_activities`` und sind bis zur Sonde in L1 eine
Annahme. Fehlt das Datum, landet die Aktivität im Ordner ``unknown`` und wird bei
der Datumsgrenze nicht ausgeschlossen.

``download_activity`` mit ``ORIGINAL`` liefert laut Bibliothek ein ZIP. Das Archiv
legt die Bytes unverändert als ``original.zip`` ab und entpackt die FIT-Datei
zusätzlich als ``original.fit`` mit eigenem Manifest-Eintrag (Endpunkt
``original_fit``). Ein Fehler beim Entpacken ergibt den Status ``error`` mit
``error_class``; das ZIP bleibt erhalten. Ein Entpackfehler wird nicht bei jedem
Lauf wiederholt (dieselben Bytes, dasselbe Ergebnis), nur ein neueres ZIP löst
einen neuen Versuch aus. ``coverage`` führt solche Aktivitäten auf.
"""

from __future__ import annotations

import io
import zipfile
from dataclasses import dataclass
from datetime import UTC, date, datetime
from typing import Any

from .archive import Archive
from .manifest import ManifestEntry
from .registry import by_name

LIST_ENDPOINT = "activities_list"
ORIGINAL_ENDPOINT = "original"
ORIGINAL_FIT_ENDPOINT = "original_fit"
# Obergrenze für die entpackte FIT-Datei. Faustregel: Aufzeichnungen langer
# Aktivitäten haben wenige MB, 256 MiB schützt vor einem ZIP mit falschen Größen.
MAX_FIT_BYTES = 256 * 1024 * 1024


@dataclass(frozen=True)
class ActivityRef:
    activity_id: Any
    day: date | None
    year: str
    # Startzeit in UTC aus ``startTimeGMT`` (Annahme bis L1), für den Abgleich mit Gerätedateien.
    start_utc: datetime | None = None

    @property
    def params(self) -> dict[str, Any]:
        return {"activity_id": self.activity_id, "year": self.year}


def parse_day(value: Any) -> date | None:
    """Datum aus den ersten zehn Zeichen einer ISO-ähnlichen Zeichenkette."""
    if isinstance(value, str) and len(value) >= 10:
        try:
            return date.fromisoformat(value[:10])
        except ValueError:
            return None
    return None


def parse_start_gmt(value: Any) -> datetime | None:
    """``startTimeGMT`` als Zeitpunkt in UTC. Ohne Zeitzone gilt UTC, sonst wird umgerechnet."""
    if not isinstance(value, str):
        return None
    try:
        parsed = datetime.fromisoformat(value.strip().replace("Z", "+00:00"))
    except ValueError:
        return None
    return parsed.replace(tzinfo=UTC) if parsed.tzinfo is None else parsed.astimezone(UTC)


def start_gmt_of(item: Any) -> datetime | None:
    """Startzeit aus einer Listenzeile oder einer Zusammenfassung.

    Gesucht wird ``startTimeGMT`` oben und in ``summaryDTO``. Der Ort des Feldes in
    ``get_activity`` ist bis zur Sonde in L1 eine Annahme.
    """
    if not isinstance(item, dict):
        return None
    found = parse_start_gmt(item.get("startTimeGMT"))
    if found is None and isinstance(item.get("summaryDTO"), dict):
        found = parse_start_gmt(item["summaryDTO"].get("startTimeGMT"))
    return found


def activity_ref(item: Any) -> ActivityRef | None:
    if not isinstance(item, dict) or item.get("activityId") is None:
        return None
    day = parse_day(item.get("startTimeLocal")) or parse_day(item.get("startTimeGMT"))
    return ActivityRef(
        item["activityId"], day, str(day.year) if day else "unknown", start_gmt_of(item)
    )


def page_items(value: Any) -> list[Any]:
    """Aktivitäten einer Listenseite. Etwas anderes als eine Liste gilt als leer."""
    return value if isinstance(value, list) else []


def oldest_day_of(items: list[Any]) -> date | None:
    days = [ref.day for ref in map(activity_ref, items) if ref is not None and ref.day]
    return min(days) if days else None


class ActivityIndex:
    """Bekannte Aktivitäten, aus den abgelegten Listenseiten und neu geholten Seiten."""

    def __init__(self) -> None:
        self._by_id: dict[str, ActivityRef] = {}

    @classmethod
    def load(cls, archive: Archive) -> ActivityIndex:
        index = cls()
        for entry in archive.entries():
            if (
                entry.kind == "activity"
                and entry.endpoint == LIST_ENDPOINT
                and entry.status == "ok"
                and entry.path
            ):
                try:
                    index.add_items(page_items(archive.read_json(entry)))
                except (OSError, ValueError):
                    continue
        return index

    def add_items(self, items: list[Any]) -> list[ActivityRef]:
        """Fügt Aktivitäten hinzu und liefert die bisher unbekannten."""
        new: list[ActivityRef] = []
        for item in items:
            ref = activity_ref(item)
            if ref is None:
                continue
            key = str(ref.activity_id)
            if key not in self._by_id:
                new.append(ref)
            self._by_id[key] = ref
        return new

    def __contains__(self, activity_id: Any) -> bool:
        return str(activity_id) in self._by_id

    def __len__(self) -> int:
        return len(self._by_id)

    def ids(self) -> set[str]:
        return set(self._by_id)

    def get(self, activity_id: Any) -> ActivityRef | None:
        return self._by_id.get(str(activity_id))

    def newest_first(self) -> list[ActivityRef]:
        """Neu nach alt. Ohne Datum ans Ende, bei gleichem Datum die höhere Kennung zuerst."""
        return sorted(
            self._by_id.values(),
            key=lambda r: (r.day is not None, r.day or date.min, str(r.activity_id)),
            reverse=True,
        )

    def oldest_day(self) -> date | None:
        days = [r.day for r in self._by_id.values() if r.day is not None]
        return min(days) if days else None


class OriginalExtractError(Exception):
    """Entpacken fehlgeschlagen. ``error_class`` landet im Manifest."""

    def __init__(self, error_class: str, message: str) -> None:
        super().__init__(message)
        self.error_class = error_class


def extract_fit(zip_bytes: bytes, *, max_bytes: int | None = None) -> bytes:
    """Erste ``.fit``-Datei aus dem ZIP. Pfade und Namen im ZIP werden ignoriert.

    Es wird nur gelesen, nie auf Pfade des ZIP geschrieben (kein Zip-Slip). Die
    Größe wird vorab aus dem Verzeichnis und beim Lesen begrenzt.
    """
    limit = MAX_FIT_BYTES if max_bytes is None else max_bytes
    try:
        with zipfile.ZipFile(io.BytesIO(zip_bytes)) as archive:
            member = next(
                (
                    info
                    for info in archive.infolist()
                    if not info.is_dir() and info.filename.lower().endswith(".fit")
                ),
                None,
            )
            if member is None:
                raise OriginalExtractError("NoFitInZip", "keine .fit-Datei im ZIP")
            if member.file_size > limit:
                raise OriginalExtractError("FitTooLarge", f"FIT-Datei größer als {limit} Bytes")
            with archive.open(member) as fh:
                data = fh.read(limit + 1)
    except OriginalExtractError:
        raise
    except Exception as exc:  # noqa: BLE001, jede Störung im ZIP ist ein Entpackfehler
        raise OriginalExtractError(type(exc).__name__, str(exc)) from exc
    if len(data) > limit:
        raise OriginalExtractError("FitTooLarge", f"FIT-Datei größer als {limit} Bytes")
    if not data:
        raise OriginalExtractError("EmptyFit", "FIT-Datei im ZIP ist leer")
    return data


def extract_original(
    archive: Archive, zip_entry: ManifestEntry, params: dict[str, Any]
) -> ManifestEntry:
    """Entpackt ``original.zip`` nach ``original.fit`` und vermerkt das Ergebnis."""
    try:
        data = extract_fit(archive.read_bytes(zip_entry))
    except OriginalExtractError as exc:
        return archive.note(
            "activity", ORIGINAL_FIT_ENDPOINT, params, "error", error_class=exc.error_class
        )
    except OSError as exc:
        return archive.note(
            "activity", ORIGINAL_FIT_ENDPOINT, params, "error", error_class=type(exc).__name__
        )
    target = archive.layout_path(by_name(ORIGINAL_ENDPOINT), params).with_name("original.fit")
    return archive.store("activity", ORIGINAL_FIT_ENDPOINT, params, data, target)
