"""Plan für ``backfill`` und ``sync`` (SPEC 5.5, Abrufregeln 1, 2 und 4).

Der Plan wird nicht gespeichert, sondern bei jedem Lauf aus Register und
Archivstand (Manifest) berechnet. Dadurch ist jeder Lauf fortsetzbar: ein
Schlüssel mit Eintrag ``ok``, ``empty`` oder ``not_available`` kommt im Plan nicht
mehr vor, ein Schlüssel ohne Eintrag oder mit ``error`` schon.

Die Stufen sind Erzeuger (Generatoren) von ``Item``. Sie laufen in der Reihenfolge
der Abrufregel 1: Konto und Zonen, Aktivitätsliste, je Aktivität Detailsatz und
Original, Tageswerte von neu nach alt, Bereichsabfragen. Eine Stufe liest nach
jedem ``yield`` den vom Lauf nachgeführten Stand (``Context.latest``); im
Trockenlauf bleibt der Stand unverändert, offene Teile werden als solche vermerkt.

Sperrliste: Der Plan enthält nur Einträge des Registers, und das Register nimmt nur
Methoden der Allowlist auf (``RegistryEntry.__post_init__``). Menstruation und
Schwangerschaft stehen weder im Register noch im Plan.
"""

from __future__ import annotations

from collections.abc import Callable, Iterator
from dataclasses import dataclass, field
from datetime import date, timedelta
from typing import Any

from .activities import (
    ORIGINAL_ENDPOINT,
    ORIGINAL_FIT_ENDPOINT,
    ActivityIndex,
    ActivityRef,
    oldest_day_of,
    page_items,
    parse_day,
)
from .archive import Archive
from .fetch import profile_number
from .manifest import ManifestEntry, make_key
from .registry import REGISTRY, RegistryEntry, by_name

# Seitengröße der Aktivitätsliste. Konvention (Faustregel), in L1 zu prüfen (O26).
ACTIVITY_PAGE_SIZE = 100
# Spanne für Bereichsendpunkte ohne gemessenen Wert (``max_span_days`` ist im
# Register ``null``, bis die Sonde in L1 misst, O26). Konvention, bewusst klein.
DEFAULT_RANGE_SPAN_DAYS = 28
# Festes Raster für Bereichsspannen. Die Grenzen hängen so nicht vom Tag des Laufs
# ab, derselbe Schlüssel ergibt sich bei jedem Lauf (Voraussetzung der Fortsetzung).
GRID_ANCHOR = date(2000, 1, 1)
# Felder der Profilantwort, aus denen das Eröffnungsdatum des Kontos gelesen wird.
# Annahme bis zur Sonde in L1 (O9): die Namen sind nicht an einer echten Antwort
# geprüft. Fehlt das Feld, gilt die älteste Aktivität.
PROFILE_DATE_KEYS = ("createDate", "createdDate", "registrationDate")


@dataclass(frozen=True)
class Item:
    """Ein Arbeitsschritt: ein Aufruf (``fetch``) oder das lokale Entpacken (``extract``)."""

    entry: RegistryEntry
    params: dict[str, Any]
    action: str = "fetch"


Stage = Callable[["Context"], Iterator[Item]]


def span_days_of(entry: RegistryEntry) -> int:
    return entry.max_span_days or DEFAULT_RANGE_SPAN_DAYS


def grid_chunks(first: date, last: date, span: int) -> list[tuple[date, date]]:
    """Rasterspannen von der Spanne mit ``first`` bis zu der mit ``last``, aufsteigend.

    Spannen sind ``span`` Tage lang und am Raster ``GRID_ANCHOR`` ausgerichtet. Die
    erste Spanne kann vor ``first`` beginnen, damit ihr Schlüssel stabil bleibt. Die
    letzte wird bei ``last`` gekürzt, sie ist die offene Spanne am neuen Ende.
    """
    k0 = (first - GRID_ANCHOR).days // span
    k1 = (last - GRID_ANCHOR).days // span
    chunks = []
    for k in range(k0, k1 + 1):
        start = GRID_ANCHOR + timedelta(days=k * span)
        chunks.append((start, min(start + timedelta(days=span - 1), last)))
    return chunks


def window_chunks(first: date, last: date, span: int) -> list[tuple[date, date]]:
    """Spannen ab ``first`` bis ``last`` (auch teilweise), aufsteigend, nicht am Raster."""
    chunks = []
    start = first
    while start <= last:
        end = min(start + timedelta(days=span - 1), last)
        chunks.append((start, end))
        start = end + timedelta(days=1)
    return chunks


@dataclass
class Context:
    archive: Archive
    mode: str  # "backfill" oder "sync"
    today: date
    latest: dict[str, ManifestEntry]
    index: ActivityIndex
    dry_run: bool = False
    until: date | None = None
    refresh_days: int = 7
    registry: tuple[RegistryEntry, ...] = REGISTRY
    known_before: set[str] = field(default_factory=set)
    new_ids: set[str] = field(default_factory=set)
    notes: list[str] = field(default_factory=list)
    open_ended: set[str] = field(default_factory=set)
    list_complete: bool = False
    list_failed: bool = False
    profile_pending: bool = False

    # Stand lesen

    def entries_of(self, granularity: str) -> list[RegistryEntry]:
        return [e for e in self.registry if e.granularity == granularity]

    def latest_entry(self, entry: RegistryEntry, params: dict[str, Any]) -> ManifestEntry | None:
        return self.latest.get(make_key(entry.manifest_kind, entry.name, params))

    def need(self, entry: RegistryEntry, params: dict[str, Any]) -> bool:
        """Fortsetzungsregel von ``backfill``: fehlt oder Fehler, sonst nicht (kein Auffrischen)."""
        return self.archive.needs_fetch(
            entry.manifest_kind, entry.name, params, latest=self.latest, refresh_days=-1
        )

    def read_page(self, params: dict[str, Any]) -> tuple[str, list[Any]]:
        """Status und Inhalt einer abgelegten Listenseite: ``ok``, ``empty``, ``not_available``,
        ``missing`` (kein Eintrag) oder ``error`` (Fehler oder Datei nicht lesbar)."""
        found = self.latest.get(make_key("activity", "activities_list", params))
        if found is None:
            return "missing", []
        if found.status != "ok":
            return found.status, []
        try:
            return "ok", page_items(self.archive.read_json(found))
        except (OSError, ValueError):
            return "error", []

    def profile_value(self) -> Any:
        found = self.latest.get(make_key("account", "profile", {}))
        if found is None or found.status != "ok":
            return None
        try:
            return self.archive.read_json(found)
        except (OSError, ValueError):
            return None

    def profile_day(self) -> date | None:
        profile = self.profile_value()
        if not isinstance(profile, dict):
            return None
        found = []
        for source in [profile, *[v for v in profile.values() if isinstance(v, dict)]]:
            for key in PROFILE_DATE_KEYS:
                day = parse_day(source.get(key))
                if day is not None:
                    found.append(day)
        return min(found) if found else None

    def earliest_day(self) -> date | None:
        """Frühestes Datum des Kontos: das ältere von Profil und ältester Aktivität."""
        days = [d for d in (self.profile_day(), self.index.oldest_day()) if d is not None]
        return min(days) if days else None

    def floor_day(self) -> date | None:
        """Untergrenze der Tageswerte: frühestes Datum, durch ``--until`` nach oben begrenzt."""
        earliest = self.earliest_day()
        if self.until is None:
            return earliest
        return max(earliest, self.until) if earliest is not None else self.until

    def newest_dates(self) -> tuple[date | None, dict[str, date]]:
        """Jüngstes Datum der Tageswerte und je Bereichsendpunkt das jüngste Ende (ohne Fehler)."""
        newest_day: date | None = None
        range_ends: dict[str, date] = {}
        for found in self.latest.values():
            if found.status == "error" or not found.params:
                continue
            if found.kind == "daily":
                day = parse_day(found.params.get("date"))
                if day is not None and (newest_day is None or day > newest_day):
                    newest_day = day
            elif found.kind == "range":
                end = parse_day(found.params.get("end"))
                known = range_ends.get(found.endpoint)
                if end is not None and (known is None or end > known):
                    range_ends[found.endpoint] = end
        return newest_day, range_ends

    # Regeln von sync

    def sync_wants(self, ref: ActivityRef) -> bool:
        """Neue Aktivitäten dieses Laufs und solche der letzten ``refresh_days`` Tage."""
        if str(ref.activity_id) in self.new_ids:
            return True
        return ref.day is not None and ref.day >= self.today - timedelta(days=self.refresh_days)

    def sync_due(self, entry: RegistryEntry, params: dict[str, Any], day: date) -> bool:
        """Tageswert holen? Fehlt, Fehler, im Zeitfenster der letzten Tage oder zu früh geholt.

        Zu früh geholt heißt: der jüngste Abruf lag höchstens ``refresh_days`` Tage nach
        dem Tag selbst. Garmin finalisiert Werte nachträglich (SPEC 5.5), ein Wert aus
        dem Zeitfenster bleibt sonst bei einer langen Pause dauerhaft vorläufig.
        """
        if self.archive.needs_fetch(
            entry.manifest_kind,
            entry.name,
            params,
            latest=self.latest,
            day=day,
            refresh_days=self.refresh_days,
            today=self.today,
        ):
            return True
        found = self.latest_entry(entry, params)
        if found is not None and found.status in ("ok", "empty"):
            fetched = parse_day(found.fetched_at)
            return fetched is not None and (fetched - day).days <= self.refresh_days
        return False

    def once_due_sync(self, entry: RegistryEntry, params: dict[str, Any]) -> bool:
        """Kontodaten ändern sich selten (Zonen, FTP): alle ``refresh_days`` Tage neu."""
        found = self.latest_entry(entry, params)
        if found is None or found.status == "error":
            return True
        fetched = parse_day(found.fetched_at)
        return fetched is not None and (self.today - fetched).days >= self.refresh_days

    def fit_extraction_due(self, params: dict[str, Any]) -> bool:
        zip_entry = self.latest.get(make_key("activity", ORIGINAL_ENDPOINT, params))
        if zip_entry is None or zip_entry.status != "ok":
            return False
        fit_entry = self.latest.get(make_key("activity", ORIGINAL_FIT_ENDPOINT, params))
        if fit_entry is None:
            return True
        # Ein Entpackfehler wird nicht bei jedem Lauf wiederholt: dieselben Bytes ergeben
        # dasselbe Ergebnis. Nur ein neueres ZIP löst einen neuen Versuch aus.
        return fit_entry.status == "error" and fit_entry.fetched_at < zip_entry.fetched_at


# Stufen


def stage_account(ctx: Context) -> Iterator[Item]:
    for entry in ctx.entries_of("once"):
        params: dict[str, Any] = {}
        if "userProfileNumber" in entry.params:
            number = profile_number(ctx.profile_value())
            if number is None:
                if ctx.dry_run and ctx.profile_pending:
                    # Im Trockenlauf wird das Profil erst geholt, die Nummer fehlt noch.
                    number = "pending"
                else:
                    ctx.notes.append(f"{entry.name}: übersprungen, Profilnummer unbekannt")
                    continue
            params = {"userProfileNumber": number}
        due = ctx.once_due_sync(entry, params) if ctx.mode == "sync" else ctx.need(entry, params)
        if not due:
            continue
        if entry.name == "profile":
            ctx.profile_pending = True
        yield Item(entry, params)


def _page_params(start: int) -> dict[str, Any]:
    return {"start": start, "limit": ACTIVITY_PAGE_SIZE}


def stage_list_backfill(ctx: Context) -> Iterator[Item]:
    """Seiten von neu nach alt bis zur letzten Seite (weniger Einträge als ``limit``)."""
    entry = by_name("activities_list")
    start = 0
    while True:
        params = _page_params(start)
        status, items = ctx.read_page(params)
        if status in ("missing", "error"):
            yield Item(entry, params)
            if ctx.dry_run:
                ctx.open_ended.add(entry.name)
                return
            status, items = ctx.read_page(params)
            if status in ("missing", "error"):
                ctx.list_failed = True
                return
        ctx.index.add_items(items)
        if status != "ok" or len(items) < ACTIVITY_PAGE_SIZE:
            ctx.list_complete = True
            return
        oldest = oldest_day_of(items)
        if ctx.until is not None and oldest is not None and oldest < ctx.until:
            return
        start += ACTIVITY_PAGE_SIZE


def stage_list_sync(ctx: Context) -> Iterator[Item]:
    """Seiten ab der ersten, immer neu, bis eine Seite eine bekannte Aktivität enthält."""
    entry = by_name("activities_list")
    start = 0
    while True:
        params = _page_params(start)
        yield Item(entry, params)
        if ctx.dry_run:
            ctx.open_ended.add(entry.name)
            return
        status, items = ctx.read_page(params)
        if status in ("missing", "error"):
            ctx.list_failed = True
            return
        for ref in ctx.index.add_items(items):
            ctx.new_ids.add(str(ref.activity_id))
        ids = {str(i.get("activityId")) for i in items if isinstance(i, dict)}
        if status != "ok" or len(items) < ACTIVITY_PAGE_SIZE or ids & ctx.known_before:
            ctx.list_complete = True
            return
        start += ACTIVITY_PAGE_SIZE


def stage_activities(ctx: Context) -> Iterator[Item]:
    per_activity = ctx.entries_of("per_activity")
    for ref in ctx.index.newest_first():
        if ctx.until is not None and ref.day is not None and ref.day < ctx.until:
            continue
        if ctx.mode == "sync" and not ctx.sync_wants(ref):
            continue
        params = ref.params
        for entry in per_activity:
            if ctx.need(entry, params):
                yield Item(entry, params)
        if ctx.fit_extraction_due(params):
            yield Item(by_name(ORIGINAL_ENDPOINT), params, action="extract")


def stage_daily(ctx: Context) -> Iterator[Item]:
    entries = ctx.entries_of("day")
    if ctx.mode == "backfill":
        floor = ctx.floor_day()
        if floor is None:
            ctx.notes.append(
                "Tageswerte nicht geplant: frühestes Datum unbekannt (Profil ohne Datum, "
                "keine Aktivität bekannt). Mit --until DATUM festlegen."
            )
            return
        last, first = ctx.today - timedelta(days=1), floor
    else:
        newest, _ends = ctx.newest_dates()
        last = ctx.today
        first = (newest or ctx.today) - timedelta(days=ctx.refresh_days)
        first = min(first, ctx.today - timedelta(days=ctx.refresh_days))
    day = last
    while day >= first:
        for entry in entries:
            params = {"date": day.isoformat()}
            due = (
                ctx.sync_due(entry, params, day) if ctx.mode == "sync" else ctx.need(entry, params)
            )
            if due:
                yield Item(entry, params)
        day -= timedelta(days=1)


def stage_ranges(ctx: Context) -> Iterator[Item]:
    entries = ctx.entries_of("range")
    work: list[tuple[date, int, RegistryEntry, date]] = []
    if ctx.mode == "backfill":
        floor = ctx.floor_day()
        if floor is None:
            return  # Hinweis steht schon bei den Tageswerten
        yesterday = ctx.today - timedelta(days=1)
        for order, entry in enumerate(entries):
            for start, end in grid_chunks(floor, yesterday, span_days_of(entry)):
                work.append((end, -order, entry, start))
    else:
        _day, ends = ctx.newest_dates()
        window_start = ctx.today - timedelta(days=ctx.refresh_days)
        for order, entry in enumerate(entries):
            known = ends.get(entry.name)
            # Die letzten ``refresh_days`` Tage vor dem bisherigen Ende werden neu geholt,
            # weil Garmin Werte nachträglich finalisiert (SPEC 5.5).
            first = (
                min(known + timedelta(days=1 - ctx.refresh_days), window_start)
                if known
                else window_start
            )
            for start, end in window_chunks(first, ctx.today, span_days_of(entry)):
                work.append((end, -order, entry, start))
    # Von neu nach alt, bei gleichem Ende in Registerreihenfolge.
    work.sort(key=lambda w: (w[0], w[1]), reverse=True)
    for end, _order, entry, start in work:
        params = {"start": start.isoformat(), "end": end.isoformat()}
        if ctx.mode == "sync" or ctx.need(entry, params):
            yield Item(entry, params)


def stages_for(mode: str) -> list[tuple[str, Stage]]:
    listing = stage_list_backfill if mode == "backfill" else stage_list_sync
    return [
        ("account", stage_account),
        ("activities_list", listing),
        ("activities", stage_activities),
        ("daily", stage_daily),
        ("ranges", stage_ranges),
    ]
