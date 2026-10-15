"""sync: nur Neues, Zeitfenster der letzten Tage, nie überschreiben (SPEC 5.5, 12 M4b)."""

from __future__ import annotations

import hashlib
import json
import re
from datetime import date
from pathlib import Path

import pytest

from lastwerk_bridge import plan as plan_module
from lastwerk_bridge.archive import DERIVED_FILES, Archive
from lastwerk_bridge.cli import EXIT_LOCKOUT, EXIT_OK, EXIT_USAGE
from lastwerk_bridge.registry import REGISTRY

from .fakes import FakeClock, FakeGarmin
from .synthetic import (
    DAY,
    DAY_ENTRY_COUNT,
    DAY_METHODS,
    PER_ACTIVITY_METHODS,
    RANGE_METHODS,
    Account,
    activity,
    cli,
    limited_client,
)

ONCE_COUNT = len([e for e in REGISTRY if e.granularity == "once"])
RANGE_COUNT = len([e for e in REGISTRY if e.granularity == "range"])
PER_ACTIVITY_COUNT = len([e for e in REGISTRY if e.granularity == "per_activity"])


def account() -> Account:
    return Account(
        [
            activity(103, date(2026, 10, 12)),
            activity(101, date(2026, 10, 8)),
            activity(102, date(2026, 10, 9)),
        ]
    )


@pytest.fixture
def backfilled(root: Path, token_dir: Path, clock: FakeClock):
    """Vollständiger backfill am 2026-10-15, danach beginnt der Test mit einem neuen Zähler."""
    acc = account()
    client = acc.client()
    code, *_ = cli(["backfill"], client, clock, root=root, token_dir=token_dir)
    assert code == EXIT_OK
    client.calls.clear()
    return acc, client


def snapshot(root: Path) -> dict[str, str]:
    return {
        p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
        for p in root.rglob("*")
        if p.is_file() and p.relative_to(root).as_posix() not in (*DERIVED_FILES, "manifest.jsonl")
    }


def dates_of(client: FakeGarmin, method: str) -> set[date]:
    return {date.fromisoformat(a[0]) for n, a, _ in client.calls if n == method}


def test_sync_without_archive_exits_2_and_sends_nothing(
    root: Path, token_dir: Path, clock: FakeClock
) -> None:
    client = account().client()
    code, _out, err, factory = cli(["sync"], client, clock, root=root, token_dir=token_dir)
    assert code == EXIT_USAGE and "backfill" in err
    assert factory.calls == [] and client.calls == []


def test_next_day_fetches_only_new_activity_and_recent_days(
    root: Path, token_dir: Path, clock: FakeClock, backfilled
) -> None:
    acc, client = backfilled
    before = snapshot(root)
    manifest_before = (root / "manifest.jsonl").read_bytes()
    clock.advance(DAY)  # 2026-10-16
    acc.activities.append(activity(104, date(2026, 10, 16), hour=6))

    code, out, _err, _f = cli(["sync"], client, clock, root=root, token_dir=token_dir)
    assert code == EXIT_OK

    names = client.names_called()
    # Liste: eine Seite, sie enthält schon bekannte Aktivitäten.
    assert client.calls[0][:2] == ("get_activities", (0, 100))
    assert names.count("get_activities") == 1
    # Kontodaten sind jünger als sieben Tage: nicht neu geholt.
    assert "get_user_profile" not in names
    # Je neue Aktivität ein Satz, sonst keine Aktivität.
    activity_calls = [(n, a[0]) for n, a, _ in client.calls if n in PER_ACTIVITY_METHODS]
    assert len(activity_calls) == PER_ACTIVITY_COUNT
    assert {a for _n, a in activity_calls} == {104}
    # Tage: die letzten sieben Tage und heute (9. bis 16. Oktober), dazu der 8. Oktober,
    # der am 15. Oktober noch innerhalb des Zeitfensters geholt wurde.
    expected = {date(2026, 10, d) for d in range(8, 17)}
    assert dates_of(client, "get_sleep_data") == expected
    assert sum(1 for n in names if n in DAY_METHODS) == len(expected) * DAY_ENTRY_COUNT
    # Bereiche: ein Aufruf je Endpunkt, ab dem Ende des letzten Abrufs minus sieben Tage.
    ranges = [(a[0], a[1]) for n, a, _ in client.calls if n == "get_sleep_daily"]
    assert ranges == [("2026-10-08", "2026-10-16")]
    assert sum(1 for n in names if n in RANGE_METHODS) == RANGE_COUNT
    assert (
        len(client.calls) == 1 + PER_ACTIVITY_COUNT + len(expected) * DAY_ENTRY_COUNT + RANGE_COUNT
    )
    assert "Zusammenfassung sync" in out

    # Nichts Vorhandenes wurde verändert oder überschrieben.
    after = snapshot(root)
    assert all(after[path] == digest for path, digest in before.items())
    assert len(after) > len(before)
    # Das Manifest wurde nur angehängt.
    assert (root / "manifest.jsonl").read_bytes().startswith(manifest_before)
    archive = Archive(root, lib_version="x", clock=clock)
    assert archive.verify() == []
    # Neu geholte Tage tragen den Zeitstempel im Namen, der jüngste Eintrag gilt.
    refreshed = sorted((root / "daily" / "2026" / "2026-10-14").glob("sleep*.json"))
    names = [p.name for p in refreshed]
    assert len(names) == 2 and "sleep.json" in names
    assert re.fullmatch(
        r"sleep\.20261016T\d{6}Z\.json", next(n for n in names if n != "sleep.json")
    )
    entry = archive.latest()['daily|sleep|{"date":"2026-10-14"}']
    assert entry.path != "daily/2026/2026-10-14/sleep.json"
    assert entry.path.startswith("daily/2026/2026-10-14/sleep.2026")
    # Neue Aktivität samt entpacktem Original.
    assert (root / "activities" / "2026" / "104" / "original.fit").is_file()
    state = json.loads((root / "state.json").read_text(encoding="utf-8"))
    assert state["last_run"]["mode"] == "sync"
    assert state["activities_list"]["activities_known"] == 4
    assert state["daily"]["newest"] == "2026-10-16"


def test_long_pause_refreshes_unfinalized_days_and_account(
    root: Path, token_dir: Path, clock: FakeClock, backfilled
) -> None:
    _acc, client = backfilled
    clock.advance(30 * DAY)  # 2026-11-14
    code, *_ = cli(["sync"], client, clock, root=root, token_dir=token_dir)
    assert code == EXIT_OK
    # Kontodaten: älter als sieben Tage, alle neu.
    once = [
        n
        for n in client.names_called()
        if n in {e.method for e in REGISTRY if e.granularity == "once"}
    ]
    assert len(once) == ONCE_COUNT
    # Tage: vom 8. Oktober (am 15. Oktober noch vorläufig geholt) bis heute. Der 7. Oktober
    # wurde am 15. Oktober acht Tage nach dem Tag geholt und gilt als final.
    days = dates_of(client, "get_sleep_data")
    assert min(days) == date(2026, 10, 8) and max(days) == date(2026, 11, 14)
    assert len(days) == (date(2026, 11, 14) - date(2026, 10, 8)).days + 1
    assert date(2026, 10, 7) not in days
    # Bereiche: lückenlos vom 8. Oktober bis heute. get_sleep_daily trug in der Sonde (L1)
    # bis 1792 Tage ohne Limit, die 38 Tage gehen deshalb in einem Aufruf.
    spans = [(a[0], a[1]) for n, a, _ in client.calls if n == "get_sleep_daily"]
    assert spans == [("2026-10-08", "2026-11-14")]
    assert Archive(root, lib_version="x", clock=clock).verify() == []


def test_list_stops_at_first_known_activity(
    root: Path, token_dir: Path, clock: FakeClock, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(plan_module, "ACTIVITY_PAGE_SIZE", 2)
    acc = Account([activity(10 + i, date(2026, 10, 14 - i)) for i in range(5)])
    client = acc.client()
    cli(["backfill"], client, clock, root=root, token_dir=token_dir)
    client.calls.clear()

    clock.advance(DAY)
    acc.activities.append(activity(20, date(2026, 10, 15)))
    cli(["sync"], client, clock, root=root, token_dir=token_dir)
    pages = [a for n, a, _ in client.calls if n == "get_activities"]
    assert pages == [(0, 2)], "Seite 0 enthält Aktivität 10, die schon bekannt ist"

    client.calls.clear()
    clock.advance(DAY)
    acc.activities += [activity(21, date(2026, 10, 16)), activity(22, date(2026, 10, 17))]
    acc.activities.append(activity(23, date(2026, 10, 17), hour=9))
    cli(["sync"], client, clock, root=root, token_dir=token_dir)
    pages = [a for n, a, _ in client.calls if n == "get_activities"]
    assert pages == [(0, 2), (2, 2)], "Seite 0 ist ganz neu, Seite 1 enthält die bekannte 20"
    fetched = {a[0] for n, a, _ in client.calls if n == "get_activity"}
    assert fetched == {21, 22, 23}


def test_sync_with_nothing_new_is_small_and_idempotent_for_old_data(
    root: Path, token_dir: Path, clock: FakeClock, backfilled
) -> None:
    _acc, client = backfilled
    clock.advance(3600)  # derselbe Tag
    cli(["sync"], client, clock, root=root, token_dir=token_dir)
    days = dates_of(client, "get_sleep_data")
    # Zeitfenster: heute minus sieben Tage ist der 8. Oktober, nichts Älteres.
    assert min(days) == date(2026, 10, 8) and max(days) == date(2026, 10, 15)
    assert not any(n in PER_ACTIVITY_METHODS for n in client.names_called())
    assert "get_user_profile" not in client.names_called()


def test_refresh_days_option_changes_the_window(
    root: Path, token_dir: Path, clock: FakeClock, backfilled
) -> None:
    _acc, client = backfilled
    clock.advance(DAY)
    cli(["sync", "--refresh-days", "2"], client, clock, root=root, token_dir=token_dir)
    days = dates_of(client, "get_sleep_data")
    # Fenster 14. bis 16. Oktober, dazu der 13., der am 15. Oktober noch vorläufig geholt wurde.
    assert min(days) == date(2026, 10, 13) and max(days) == date(2026, 10, 16)


def test_sync_max_calls_and_resume(
    root: Path, token_dir: Path, clock: FakeClock, backfilled
) -> None:
    acc, client = backfilled
    clock.advance(DAY)
    acc.activities.append(activity(104, date(2026, 10, 16), hour=6))
    code, out, _err, _f = cli(
        ["sync", "--max-calls", "25"], client, clock, root=root, token_dir=token_dir
    )
    assert code == EXIT_OK and len(client.calls) == 25
    assert "Aufrufbudget erreicht" in out


def test_three_429_during_sync_exit_3(
    root: Path, token_dir: Path, clock: FakeClock, backfilled
) -> None:
    acc, _client = backfilled
    clock.advance(DAY)
    acc.activities.append(activity(104, date(2026, 10, 16), hour=6))
    limited = limited_client(acc, healthy_calls=5)
    code, out, err, _f = cli(["sync"], limited, clock, root=root, token_dir=token_dir)
    assert code == EXIT_LOCKOUT
    assert "Sperre" in out and "Sperre" in err
    assert len(limited.calls) == 8
    assert (root / "state" / "lockout.json").is_file()
    # Frist abwarten, mit gesundem Client fortsetzen.
    clock.advance(7 * 3600)
    healthy = acc.client()
    code, *_ = cli(["sync"], healthy, clock, root=root, token_dir=token_dir)
    assert code == EXIT_OK
    assert Archive(root, lib_version="x", clock=clock).verify() == []
