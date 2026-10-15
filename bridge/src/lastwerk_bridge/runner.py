"""Ausführung von ``backfill`` und ``sync`` (SPEC 5.5, Abrufregeln 4 und 8).

Der Lauf geht den Plan (``plan.py``) Schritt für Schritt durch. Jeder Aufruf läuft
über den gedrosselten Aufrufer aus ``throttle.py``, ein zweiter Weg zum Netzwerk
existiert nicht. Das Aufrufbudget (``--max-calls``) liegt im Aufrufer und zählt
Wiederholungen mit, ein Lauf endet daher nie mit mehr als N Aufrufen an den Client.
Nach Budget, Sperre oder ungültiger Anmeldung wird der Zustand geschrieben und der
Lauf lässt sich mit demselben Befehl fortsetzen.
"""

from __future__ import annotations

import json
from collections import Counter
from dataclasses import dataclass, field
from datetime import date
from typing import Any

from . import plan
from .activities import ORIGINAL_ENDPOINT, ActivityIndex, extract_original
from .archive import STATE_FILE_NAME, Archive, write_bytes_atomic
from .fetch import fetch_one
from .manifest import ManifestEntry, make_key
from .plan import Context, stages_for
from .registry import REGISTRY
from .throttle import (
    AuthRequiredError,
    CallBudgetExhausted,
    LockoutError,
    ThrottledCaller,
)

STATE_SCHEMA_VERSION = 1


@dataclass
class RunSummary:
    mode: str
    dry_run: bool = False
    calls: int = 0
    files: int = 0
    empty: int = 0
    not_available: int = 0
    errors: int = 0
    extracted: int = 0
    stop_reason: str = "done"  # done, budget, lockout, auth
    stop_message: str = ""
    per_endpoint: Counter[str] = field(default_factory=Counter)
    extract_pending: int = 0
    remaining: Counter[str] = field(default_factory=Counter)
    notes: list[str] = field(default_factory=list)
    open_ended: list[str] = field(default_factory=list)
    list_complete: bool = False

    @property
    def planned(self) -> int:
        return sum(self.per_endpoint.values())

    @property
    def remaining_total(self) -> int:
        return sum(self.remaining.values())

    def record(self, found: ManifestEntry) -> None:
        if found.status == "ok":
            self.files += 1
        elif found.status == "empty":
            self.empty += 1
        elif found.status == "not_available":
            self.not_available += 1
        else:
            self.errors += 1


def make_context(
    archive: Archive,
    mode: str,
    *,
    dry_run: bool,
    until: date | None = None,
    refresh_days: int = 7,
    latest: dict[str, ManifestEntry] | None = None,
) -> Context:
    index = ActivityIndex.load(archive)
    return Context(
        archive=archive,
        mode=mode,
        today=archive.today(),
        latest=dict(latest) if latest is not None else archive.latest(),
        index=index,
        dry_run=dry_run,
        until=until,
        refresh_days=refresh_days,
        known_before=index.ids(),
    )


def count_plan(ctx: Context) -> RunSummary:
    """Zählt den Plan je Endpunkt, ohne etwas zu senden oder zu schreiben."""
    summary = RunSummary(ctx.mode, dry_run=True)
    for _name, stage in stages_for(ctx.mode):
        for item in stage(ctx):
            if item.action == "extract":
                summary.extract_pending += 1
            else:
                summary.per_endpoint[item.entry.name] += 1
    summary.notes = list(ctx.notes)
    summary.open_ended = sorted(ctx.open_ended)
    return summary


def plan_run(
    archive: Archive, mode: str, *, until: date | None = None, refresh_days: int = 7
) -> RunSummary:
    """Trockenlauf: derselbe Plan wie im echten Lauf, kein Aufruf, keine Datei."""
    ctx = make_context(archive, mode, dry_run=True, until=until, refresh_days=refresh_days)
    return count_plan(ctx)


def _execute(ctx: Context, caller: ThrottledCaller, summary: RunSummary) -> None:
    archive = ctx.archive
    for _name, stage in stages_for(ctx.mode):
        for item in stage(ctx):
            if item.action == "extract":
                zip_entry = ctx.latest[make_key("activity", ORIGINAL_ENDPOINT, item.params)]
                _record_extract(ctx, summary, extract_original(archive, zip_entry, item.params))
                continue
            if caller.max_calls is not None and caller.stats.calls >= caller.max_calls:
                raise CallBudgetExhausted(f"Aufrufbudget von {caller.max_calls} erreicht")
            outcome = fetch_one(
                archive,
                caller,
                item.entry,
                item.params,
                latest=ctx.latest,
                today=ctx.today,
                force=True,
            )
            found = outcome.entry
            if found is None:
                continue
            ctx.latest[found.key] = found
            summary.record(found)
            summary.per_endpoint[item.entry.name] += 1
            if item.entry.name == ORIGINAL_ENDPOINT and found.status == "ok":
                _record_extract(ctx, summary, extract_original(archive, found, item.params))


def _record_extract(ctx: Context, summary: RunSummary, found: ManifestEntry) -> None:
    ctx.latest[found.key] = found
    if found.status == "ok":
        summary.extracted += 1
        summary.files += 1
    else:
        summary.errors += 1


def run(
    archive: Archive,
    caller: ThrottledCaller,
    mode: str,
    *,
    until: date | None = None,
    refresh_days: int = 7,
) -> RunSummary:
    """Führt den Plan aus. Sperre, Budget und ungültige Anmeldung beenden den Lauf geordnet."""
    ctx = make_context(archive, mode, dry_run=False, until=until, refresh_days=refresh_days)
    summary = RunSummary(mode)
    started = archive.now()
    try:
        _execute(ctx, caller, summary)
    except CallBudgetExhausted as exc:
        summary.stop_reason, summary.stop_message = "budget", str(exc)
    except LockoutError as exc:
        summary.stop_reason, summary.stop_message = "lockout", str(exc)
    except AuthRequiredError as exc:
        summary.stop_reason, summary.stop_message = "auth", str(exc)
    summary.calls = caller.stats.calls
    summary.notes = list(ctx.notes)
    summary.list_complete = ctx.list_complete
    if ctx.list_failed:
        summary.notes.append(
            "Aktivitätsliste unvollständig (Fehler bei einer Seite), nächster Lauf setzt fort"
        )
    if summary.stop_reason != "done":
        # Was nach dem Abbruch noch offen ist, aus dem nachgeführten Stand berechnet.
        rest = make_context(
            archive,
            mode,
            dry_run=True,
            until=until,
            refresh_days=refresh_days,
            latest=ctx.latest,
        )
        counted = count_plan(rest)
        summary.remaining = counted.per_endpoint
        summary.open_ended = counted.open_ended
    write_state(archive, ctx, summary, started.isoformat(timespec="seconds"))
    return summary


# state.json


def list_progress(ctx: Context) -> tuple[int, bool]:
    """Anzahl abgelegter Listenseiten ab Seite 0 und ob die letzte Seite erreicht ist."""
    pages = 0
    while True:
        status, items = ctx.read_page(
            {"start": pages * plan.ACTIVITY_PAGE_SIZE, "limit": plan.ACTIVITY_PAGE_SIZE}
        )
        if status in ("missing", "error"):
            return pages, False
        pages += 1
        if status != "ok" or len(items) < plan.ACTIVITY_PAGE_SIZE:
            return pages, True


def _span(values: list[date]) -> dict[str, str | None]:
    return {
        "oldest": min(values).isoformat() if values else None,
        "newest": max(values).isoformat() if values else None,
    }


def build_state(
    archive: Archive, ctx: Context, summary: RunSummary, started_at: str
) -> dict[str, Any]:
    """Zeiger je Quelle (SPEC 5.5, Archivlayout). Abgeleitet aus dem Manifest, nicht maßgeblich:
    der nächste Lauf berechnet seinen Plan aus dem Manifest selbst."""
    pages, complete = list_progress(ctx)
    activity_days = [r.day for r in ctx.index.newest_first() if r.day is not None]
    daily: list[date] = []
    ranges: dict[str, list[tuple[date, date]]] = {}
    for found in ctx.latest.values():
        if found.status == "error" or not found.params:
            continue
        if found.kind == "daily" and isinstance(found.params.get("date"), str):
            daily.append(date.fromisoformat(found.params["date"]))
        elif found.kind == "range" and "start" in found.params and "end" in found.params:
            ranges.setdefault(found.endpoint, []).append(
                (date.fromisoformat(found.params["start"]), date.fromisoformat(found.params["end"]))
            )
    earliest = ctx.earliest_day()
    return {
        "schema_version": STATE_SCHEMA_VERSION,
        "updated_at": archive.now().isoformat(timespec="seconds"),
        "lib_version": archive.lib_version,
        "earliest_date": earliest.isoformat() if earliest else None,
        "activities_list": {
            "page_size": plan.ACTIVITY_PAGE_SIZE,
            "pages_stored": pages,
            "next_start": pages * plan.ACTIVITY_PAGE_SIZE,
            "complete": complete,
            "activities_known": len(ctx.index),
            **_span(activity_days),
        },
        "daily": _span(daily),
        "ranges": {
            name: {
                "oldest_start": min(s for s, _ in spans).isoformat(),
                "newest_end": max(e for _, e in spans).isoformat(),
            }
            for name, spans in sorted(ranges.items())
        },
        "last_run": {
            "mode": summary.mode,
            "started_at": started_at,
            "calls": summary.calls,
            "files": summary.files,
            "empty": summary.empty,
            "not_available": summary.not_available,
            "errors": summary.errors,
            "stop_reason": summary.stop_reason,
        },
    }


def write_state(archive: Archive, ctx: Context, summary: RunSummary, started_at: str) -> None:
    state = build_state(archive, ctx, summary, started_at)
    payload = (json.dumps(state, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode()
    write_bytes_atomic(archive.root / STATE_FILE_NAME, payload)


# Ausgabe

STOP_TEXT = {
    "done": "Plan vollständig abgearbeitet.",
    "budget": "Aufrufbudget erreicht. Mit demselben Befehl fortsetzen.",
    "lockout": "Abbruch wegen Sperre (drei Antworten 429 oder 403 in Folge). Zustand gespeichert,"
    " später mit demselben Befehl fortsetzen, keine Umgehung.",
    "auth": "Abbruch: Anmeldung ungültig. `auth` erneut ausführen, danach fortsetzen.",
}


def format_plan(summary: RunSummary, *, max_calls: int | None = None) -> list[str]:
    lines = [f"Trockenlauf {summary.mode}: geplante Aufrufe je Endpunkt (nichts wurde gesendet)"]
    width = max([len(name) for name in summary.per_endpoint] + [8])
    for name in sorted(summary.per_endpoint, key=_registry_order):
        mark = " (mindestens, Länge unbekannt)" if name in summary.open_ended else ""
        lines.append(f"  {name:<{width}}  {summary.per_endpoint[name]:>7}{mark}")
    lines.append(f"Summe: {summary.planned} Aufrufe in {len(summary.per_endpoint)} Endpunkten")
    if summary.extract_pending:
        lines.append(
            f"Dazu {summary.extract_pending} lokale Entpackungen von original.fit (kein Aufruf)"
        )
    if max_calls is not None:
        lines.append(
            f"Ein Lauf mit --max-calls {max_calls} endet nach höchstens {max_calls} Aufrufen."
        )
    lines.extend(f"Hinweis: {note}" for note in summary.notes)
    if summary.open_ended:
        lines.append(
            "Hinweis: Der Plan ist unvollständig, weil Listenseiten und davon abhängige Abrufe "
            "erst nach dem Abruf bekannt sind."
        )
    return lines


def _registry_order(name: str) -> int:
    names = [e.name for e in REGISTRY]
    return names.index(name) if name in names else len(names)


def format_summary(summary: RunSummary, *, max_calls: int | None = None) -> list[str]:
    budget = f" (Budget {max_calls})" if max_calls is not None else ""
    lines = [
        f"Zusammenfassung {summary.mode}",
        f"  Aufrufe:          {summary.calls}{budget}",
        f"  Neue Dateien:     {summary.files}"
        + (f" (davon {summary.extracted} entpackte original.fit)" if summary.extracted else ""),
        f"  Leer:             {summary.empty}",
        f"  Nicht verfügbar:  {summary.not_available}",
        f"  Fehler:           {summary.errors}",
        f"  Ende:             {STOP_TEXT[summary.stop_reason]}",
    ]
    if summary.stop_reason != "done":
        extra = " (mindestens)" if summary.open_ended else ""
        lines.append(f"  Noch offen:       {summary.remaining_total} Aufrufe{extra}")
    lines.extend(f"  Hinweis:          {note}" for note in summary.notes)
    return lines
