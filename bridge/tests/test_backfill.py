"""backfill: Trockenlauf, Budget, Fortsetzung, Reihenfolge, Sperre, sensible Ablage (M4b)."""

from __future__ import annotations

import hashlib
import json
from datetime import date, timedelta
from pathlib import Path

import pytest
from garminconnect import GarminConnectConnectionError

from lastwerk_bridge import plan as plan_module
from lastwerk_bridge.activities import MAX_FIT_BYTES
from lastwerk_bridge.allowlist import BLOCKLIST, MethodNotAllowedError
from lastwerk_bridge.archive import Archive
from lastwerk_bridge.cli import EXIT_ERROR, EXIT_LOCKOUT, EXIT_OK, EXIT_USAGE
from lastwerk_bridge.plan import GRID_ANCHOR, grid_chunks, window_chunks
from lastwerk_bridge.registry import REGISTRY
from lastwerk_bridge.runner import make_context
from lastwerk_bridge.throttle import ThrottleConfig, ThrottledCaller

from .fakes import FakeClock, FakeGarmin
from .synthetic import (
    DAY_METHODS,
    PER_ACTIVITY_METHODS,
    RANGE_METHODS,
    TODAY,
    Account,
    activity,
    cli,
    fit_bytes,
    limited_client,
    make_zip,
    strict_client,
)

YESTERDAY = TODAY - timedelta(days=1)


def young_account(**kwargs) -> Account:
    """Konto seit 2026-10-05 mit drei Aktivitäten (kleine Aufrufzahl im Vollabzug)."""
    return Account(
        [
            activity(103, date(2026, 10, 12)),
            activity(101, date(2026, 10, 8)),
            activity(102, date(2026, 10, 9)),
        ],
        **kwargs,
    )


def call_key(call) -> tuple:
    name, args, kwargs = call
    return (name, args, tuple(sorted((k, str(v)) for k, v in kwargs.items())))


def run_until_done(argv, client, clock, root, token_dir, limit=30):
    codes = []
    for _ in range(limit):
        before = len(client.calls)
        code, out, _err, _f = cli(argv, client, clock, root=root, token_dir=token_dir)
        codes.append(code)
        if len(client.calls) == before:
            return codes, out
    raise AssertionError("Lauf wird nicht fertig")


# Trockenlauf


def test_dry_run_sends_nothing_and_writes_nothing(root: Path, token_dir: Path, clock) -> None:
    client = young_account().client()
    # Erster Lauf holt Konto und Liste, danach kennt der Plan das früheste Datum.
    code, *_ = cli(["backfill", "--max-calls", "30"], client, clock, root=root, token_dir=token_dir)
    assert code == EXIT_OK
    calls_before = len(client.calls)
    files_before = sorted(p for p in root.rglob("*") if p.is_file())
    manifest_before = (root / "manifest.jsonl").read_bytes()

    code, out, _err, factory = cli(
        ["backfill", "--max-calls", "200", "--dry-run"],
        client,
        clock,
        root=root,
        token_dir=token_dir,
    )
    assert code == EXIT_OK
    assert factory.calls == [], "Trockenlauf darf keinen Client erzeugen"
    assert len(client.calls) == calls_before, "Trockenlauf darf nichts senden"
    assert sorted(p for p in root.rglob("*") if p.is_file()) == files_before
    assert (root / "manifest.jsonl").read_bytes() == manifest_before
    assert "nichts wurde gesendet" in out
    # Zehn Tage von 2026-10-05 (Profil) bis gestern, je Tagesendpunkt ein Aufruf.
    line = next(row for row in out.splitlines() if row.strip().startswith("sleep "))
    assert line.split()[1] == "10"
    assert "Summe:" in out and "höchstens 200 Aufrufen" in out


def test_dry_run_needs_neither_tokens_nor_archive(root: Path, tmp_path: Path, clock) -> None:
    client = young_account().client()
    code, out, _err, factory = cli(
        ["backfill", "--dry-run"], client, clock, root=root, token_dir=tmp_path / "none"
    )
    assert code == EXIT_OK
    assert factory.calls == [] and client.calls == []
    assert not root.exists()
    # Ohne Konto sind nur die Kontoendpunkte und die erste Listenseite bekannt.
    assert "profile" in out and "activities_list" in out
    assert "mindestens" in out and "frühestes Datum unbekannt" in out


def test_backfill_without_tokens_exits_2_and_sends_nothing(
    root: Path, tmp_path: Path, clock
) -> None:
    client = young_account().client()
    code, _out, err, factory = cli(
        ["backfill"], client, clock, root=root, token_dir=tmp_path / "none"
    )
    assert code == EXIT_USAGE
    assert "Keine Anmeldung" in err
    assert factory.calls == [] and client.calls == []


def test_plan_contains_no_blocklisted_method(root: Path, token_dir: Path, clock) -> None:
    client = strict_client(young_account())
    code, *_ = cli(["backfill"], client, clock, root=root, token_dir=token_dir)
    assert code == EXIT_OK
    archive = Archive(root, lib_version="0.3.17", clock=clock)
    ctx = make_context(archive, "backfill", dry_run=True)
    planned: set[str] = set()
    for _name, stage in plan_module.stages_for("backfill"):
        planned |= {item.entry.method for item in stage(ctx)}
    ctx = make_context(
        Archive(root / "leer", lib_version="x", clock=clock), "backfill", dry_run=True
    )
    for _name, stage in plan_module.stages_for("backfill"):
        planned |= {item.entry.method for item in stage(ctx)}
    assert planned.isdisjoint(BLOCKLIST)
    called = set(client.names_called())
    assert called.isdisjoint(BLOCKLIST)
    assert not any("menstrual" in name or "pregnancy" in name for name in called | planned)
    assert not any("menstrual" in e.method or "pregnancy" in e.method for e in REGISTRY)


def test_blocklisted_method_is_refused_before_the_client(clock, tmp_path: Path) -> None:
    client = strict_client(young_account())
    caller = ThrottledCaller(
        client,
        state_dir=tmp_path / "state",
        config=ThrottleConfig(min_interval_s=0.0, jitter_s=0.0),
        clock=clock,
    )
    with pytest.raises(MethodNotAllowedError):
        caller.call("get_menstrual_data_for_date", "2026-10-01")
    with pytest.raises(AssertionError, match="Sperrlisten-Methode"):
        client.get_pregnancy_summary()
    assert client.calls == []


# Budget und Fortsetzung


def test_max_calls_200_stops_and_resumes_without_duplicates(
    root: Path, token_dir: Path, clock
) -> None:
    client = young_account().client()
    argv = ["backfill", "--max-calls", "200"]

    code, out, _err, _f = cli(argv, client, clock, root=root, token_dir=token_dir)
    assert code == EXIT_OK
    assert len(client.calls) == 200
    assert "Aufrufbudget erreicht" in out and "Aufrufe:          200 (Budget 200)" in out
    assert "Noch offen" in out
    state = json.loads((root / "state.json").read_text(encoding="utf-8"))
    assert state["last_run"]["stop_reason"] == "budget"
    assert state["last_run"]["calls"] == 200

    codes, last_out = run_until_done(argv, client, clock, root, token_dir)
    assert set(codes) == {EXIT_OK}
    assert "Plan vollständig abgearbeitet" in last_out
    keys = [call_key(c) for c in client.calls]
    assert len(keys) == len(set(keys)), "ein Schlüssel wurde doppelt geholt"
    assert Archive(root, lib_version="0.3.17", clock=clock).verify() == []

    # Ein weiterer Lauf holt nichts mehr.
    before = len(client.calls)
    code, out, _err, _f = cli(argv, client, clock, root=root, token_dir=token_dir)
    assert code == EXIT_OK and len(client.calls) == before
    assert "Aufrufe:          0" in out


def test_budget_includes_retries(root: Path, token_dir: Path, clock) -> None:
    broken = FakeGarmin(default=GarminConnectConnectionError("Verbindung getrennt"))
    code, out, _err, _f = cli(
        ["backfill", "--max-calls", "4"], broken, clock, root=root, token_dir=token_dir
    )
    # Erster Schlüssel: 1 Aufruf plus 2 Wiederholungen, zweiter Schlüssel: nur noch 1 Aufruf.
    assert len(broken.calls) == 4
    assert code == EXIT_ERROR
    assert "Aufrufbudget erreicht" in out
    entries = Archive(root, lib_version="x", clock=clock).entries()
    assert [e.status for e in entries] == ["error"], "abgebrochene Wiederholung bleibt ohne Eintrag"


def test_archive_state_continues_after_other_clock_day(root: Path, token_dir: Path, clock) -> None:
    client = young_account().client()
    cli(["backfill", "--max-calls", "120"], client, clock, root=root, token_dir=token_dir)
    first = len(client.calls)
    clock.advance(3 * 3600)
    cli(["backfill", "--max-calls", "120"], client, clock, root=root, token_dir=token_dir)
    keys = [call_key(c) for c in client.calls]
    assert len(client.calls) > first
    assert len(keys) == len(set(keys))


# Reihenfolge von neu nach alt


def test_pages_activities_and_days_run_from_new_to_old(
    root: Path, token_dir: Path, clock, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(plan_module, "ACTIVITY_PAGE_SIZE", 2)
    account = Account(
        [activity(10 + i, date(2026, 10, 14) - timedelta(days=i)) for i in range(5)],
    )
    client = account.client()
    code, *_ = cli(["backfill"], client, clock, root=root, token_dir=token_dir)
    assert code == EXIT_OK

    pages = [args for name, args, _ in client.calls if name == "get_activities"]
    assert pages == [(0, 2), (2, 2), (4, 2)], "Seiten von vorn nach hinten, bis zur kurzen Seite"

    ids = []
    for name, args, _ in client.calls:
        if name in PER_ACTIVITY_METHODS and args[0] not in ids:
            ids.append(args[0])
    assert ids == [10, 11, 12, 13, 14], "neueste Aktivität zuerst"

    day_dates = [date.fromisoformat(a[0]) for n, a, _ in client.calls if n in DAY_METHODS]
    assert day_dates[0] == YESTERDAY
    assert day_dates == sorted(day_dates, reverse=True), "Tageswerte von neu nach alt"

    ends = [date.fromisoformat(a[1]) for n, a, _ in client.calls if n in RANGE_METHODS]
    assert ends == sorted(ends, reverse=True), "Bereiche von neu nach alt"
    # Reihenfolge der Abrufregel 1: Konto, Liste, Aktivitäten, Tage, Bereiche.
    names = client.names_called()
    last = {
        "account": names.index("get_user_profile"),
        "list": max(i for i, n in enumerate(names) if n == "get_activities"),
        "activity": max(i for i, n in enumerate(names) if n in PER_ACTIVITY_METHODS),
        "day": max(i for i, n in enumerate(names) if n in DAY_METHODS),
    }
    first_day = min(i for i, n in enumerate(names) if n in DAY_METHODS)
    first_range = min(i for i, n in enumerate(names) if n in RANGE_METHODS)
    assert last["account"] < last["list"] < last["activity"] < first_day
    assert last["day"] < first_range


@pytest.mark.parametrize(
    ("profile_created", "expected_floor"),
    [
        # Konto älter als jede Aktivität: das Profil bestimmt das früheste Datum.
        ("2026-09-30T08:00:00.0", date(2026, 9, 30)),
        # Aktivität älter als das Profildatum: die älteste Aktivität bestimmt es.
        ("2026-10-10T08:00:00.0", date(2026, 10, 8)),
        # Profil ohne Datum: nur die älteste Aktivität.
        (None, date(2026, 10, 8)),
    ],
)
def test_backfill_reaches_earliest_date_of_the_account(
    root: Path, token_dir: Path, clock, profile_created: str | None, expected_floor: date
) -> None:
    client = young_account(profile_created=profile_created).client()
    code, *_ = cli(["backfill"], client, clock, root=root, token_dir=token_dir)
    assert code == EXIT_OK
    days = {date.fromisoformat(a[0]) for n, a, _ in client.calls if n == "get_sleep_data"}
    assert min(days) == expected_floor
    assert max(days) == YESTERDAY
    assert len(days) == (YESTERDAY - expected_floor).days + 1, "keine Lücke"
    state = json.loads((root / "state.json").read_text(encoding="utf-8"))
    assert state["earliest_date"] == expected_floor.isoformat()
    assert state["daily"] == {"oldest": expected_floor.isoformat(), "newest": YESTERDAY.isoformat()}


def test_until_limits_days_activities_and_pages(root: Path, token_dir: Path, clock) -> None:
    client = young_account().client()
    code, *_ = cli(
        ["backfill", "--until", "2026-10-09"], client, clock, root=root, token_dir=token_dir
    )
    assert code == EXIT_OK
    days = {date.fromisoformat(a[0]) for n, a, _ in client.calls if n == "get_sleep_data"}
    assert min(days) == date(2026, 10, 9)
    fetched = {a[0] for n, a, _ in client.calls if n == "get_activity"}
    assert fetched == {102, 103}, "Aktivität 101 (8. Oktober) liegt vor --until"


def test_without_any_date_nothing_is_planned_for_days(root: Path, token_dir: Path, clock) -> None:
    client = Account([], profile_created=None).client()
    code, out, _err, _f = cli(["backfill"], client, clock, root=root, token_dir=token_dir)
    assert code == EXIT_OK
    assert "frühestes Datum unbekannt" in out
    assert not any(n in DAY_METHODS for n in client.names_called())


# Leere, nicht verfügbare, fehlerhafte Antworten


def test_empty_and_missing_answers_are_noted_and_not_refetched(
    root: Path, token_dir: Path, clock
) -> None:
    account = young_account(empty_sleep=(date(2026, 10, 10),), missing_hrv=(date(2026, 10, 11),))
    client = account.client()
    cli(["backfill"], client, clock, root=root, token_dir=token_dir)
    latest = Archive(root, lib_version="x", clock=clock).latest()
    sleep = latest['daily|sleep|{"date":"2026-10-10"}']
    hrv = latest['daily|hrv|{"date":"2026-10-11"}']
    assert (sleep.status, sleep.path) == ("empty", None)
    assert (hrv.status, hrv.http_status) == ("not_available", 404)
    weather = latest['activity|weather|{"activity_id":101,"year":"2026"}']
    assert weather.status == "not_available"
    sets = latest['activity|exercise_sets|{"activity_id":101,"year":"2026"}']
    assert sets.status == "empty"

    before = len(client.calls)
    cli(["backfill"], client, clock, root=root, token_dir=token_dir)
    assert len(client.calls) == before


# Sperre


def test_three_429_in_a_row_exit_3_and_state_stays_resumable(
    root: Path, token_dir: Path, clock
) -> None:
    account = young_account()
    client = limited_client(account, healthy_calls=40)
    code, out, err, _f = cli(["backfill"], client, clock, root=root, token_dir=token_dir)
    assert code == EXIT_LOCKOUT
    assert "Abbruch wegen Sperre" in out and "Sperre" in err
    assert len(client.calls) == 43, "40 gute Aufrufe, dann genau drei Antworten 429"
    lockout = json.loads((root / "state" / "lockout.json").read_text(encoding="utf-8"))
    assert lockout["http_status"] == 429 and lockout["consecutive"] == 3
    state = json.loads((root / "state.json").read_text(encoding="utf-8"))
    assert state["last_run"]["stop_reason"] == "lockout"

    archive = Archive(root, lib_version="x", clock=clock)
    errors = [e for e in archive.entries() if e.status == "error"]
    assert len(errors) == 2, "die ersten beiden 429 sind vermerkt, die dritte beendet den Lauf"
    assert {(e.http_status, e.error_class) for e in errors} == {(429, "Blocked")}
    done_before = {e.key for e in archive.entries() if e.status != "error"}

    # Solange die Sperre gilt: sofort Exit 3, kein Client, kein Aufruf.
    seen = len(client.calls)
    code, _out, err, factory = cli(["backfill"], client, clock, root=root, token_dir=token_dir)
    assert code == EXIT_LOCKOUT and "Sperre aktiv" in err
    assert factory.calls == [] and len(client.calls) == seen

    # Nach Ablauf der Frist setzt derselbe Befehl fort, ohne etwas Erledigtes zu wiederholen.
    clock.advance(7 * 3600)
    healthy_client = account.client()
    codes, _ = run_until_done(["backfill"], healthy_client, clock, root, token_dir)
    assert set(codes) == {EXIT_OK}
    fetched_again = {
        e.key for e in Archive(root, lib_version="x", clock=clock).entries()[len(done_before) :]
    }
    assert fetched_again.isdisjoint(done_before)
    latest = Archive(root, lib_version="x", clock=clock).latest()
    assert all(e.status != "error" for e in latest.values())


def test_single_errors_do_not_stop_the_run_but_exit_1(root: Path, token_dir: Path, clock) -> None:
    account = young_account()
    client = account.client()
    client.set("get_activity_splits", GarminConnectConnectionError("client error (400)"))
    code, out, _err, _f = cli(["backfill"], client, clock, root=root, token_dir=token_dir)
    assert code == EXIT_ERROR
    assert "Fehler:           3" in out
    latest = Archive(root, lib_version="x", clock=clock).latest()
    errors = [e for e in latest.values() if e.status == "error"]
    assert {e.error_class for e in errors} == {"GarminConnectConnectionError"}
    # Der nächste Lauf holt genau diese drei erneut.
    healthy = account.client()
    code, _out, _err, _f = cli(["backfill"], healthy, clock, root=root, token_dir=token_dir)
    assert code == EXIT_OK
    assert healthy.names_called() == ["get_activity_splits"] * 3


# Sensible Ablage


def test_sensitive_categories_land_under_sensitive_only(root: Path, token_dir: Path, clock) -> None:
    client = young_account().client()
    cli(["backfill"], client, clock, root=root, token_dir=token_dir)
    s3 = {e.name: e for e in REGISTRY if e.sensitivity == "S3"}
    entries = Archive(root, lib_version="x", clock=clock).entries()
    seen_categories = set()
    for entry in entries:
        if entry.path is None:
            continue
        if entry.endpoint in s3:
            category = s3[entry.endpoint].category
            seen_categories.add(category)
            assert entry.path.startswith(f"sensitive/{category}/{entry.endpoint}/"), entry.path
        else:
            assert not entry.path.startswith("sensitive/"), entry.path
    assert seen_categories == {"weight", "blood_pressure", "hydration", "nutrition", "lifestyle"}
    for folder in ("daily", "ranges"):
        for path in (root / folder).rglob("*.json"):
            assert not any(name in path.name or name in path.parts for name in s3), path
    day_file = root / "sensitive" / "hydration" / "hydration" / "2026-10-14.json"
    assert day_file.is_file()
    range_files = list((root / "sensitive" / "weight" / "weigh_ins").glob("*_*.json"))
    assert range_files


# Bereichsspannen


def test_grid_chunks_are_stable_contiguous_and_not_longer_than_span() -> None:
    span = 28
    first, last = date(2026, 1, 3), date(2026, 10, 14)
    chunks = grid_chunks(first, last, span)
    assert chunks[0][0] <= first <= chunks[0][1]
    assert chunks[-1][1] == last, "offene Spanne am neuen Ende"
    for (_s1, e1), (s2, _e2) in zip(chunks, chunks[1:], strict=False):
        assert s2 == e1 + timedelta(days=1)
    for start, end in chunks:
        assert (end - start).days + 1 <= span
        assert (start - GRID_ANCHOR).days % span == 0
    # Spätere Läufe erzeugen dieselben Schlüssel für die älteren Spannen.
    later = grid_chunks(first, date(2026, 12, 31), span)
    assert later[: len(chunks) - 1] == chunks[:-1]
    # Ein Ende genau am Rasterende ergibt eine volle Spanne ohne Rest.
    full_end = GRID_ANCHOR + timedelta(days=span * 10 - 1)
    assert grid_chunks(full_end, full_end, span) == [
        (full_end - timedelta(days=span - 1), full_end)
    ]


def test_window_chunks_cover_the_window_without_gap() -> None:
    chunks = window_chunks(date(2026, 10, 1), date(2026, 10, 15), 6)
    assert chunks[0][0] == date(2026, 10, 1) and chunks[-1][1] == date(2026, 10, 15)
    assert all((e - s).days + 1 <= 6 for s, e in chunks)
    assert [(s, e) for s, e in chunks][1][0] == chunks[0][1] + timedelta(days=1)


def test_range_endpoints_are_split_by_max_span_days(
    root: Path, token_dir: Path, clock, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(plan_module, "span_days_of", lambda entry: 5)
    account = Account([activity(1, date(2026, 9, 20))], profile_created=None)
    client = account.client()
    code, *_ = cli(["backfill"], client, clock, root=root, token_dir=token_dir)
    assert code == EXIT_OK
    spans = [
        (date.fromisoformat(a[0]), date.fromisoformat(a[1]))
        for n, a, _ in client.calls
        if n == "get_sleep_daily"
    ]
    assert len(spans) >= 5
    assert all((e - s).days + 1 <= 5 for s, e in spans)
    assert spans[0][1] == YESTERDAY
    files = sorted(p.name for p in (root / "ranges" / "sleep_daily").glob("*.json"))
    assert files == sorted(f"{s.isoformat()}_{e.isoformat()}.json" for s, e in spans)


# FIT-Original aus dem ZIP


def test_original_is_kept_as_zip_and_unpacked_to_fit(root: Path, token_dir: Path, clock) -> None:
    account = young_account()
    # Pfadangaben im ZIP werden ignoriert, es gilt die erste .fit-Datei.
    account.zips[103] = make_zip(
        [
            ("../../outside.txt", b"nicht schreiben"),
            ("../../outside.fit", fit_bytes(103)),
            ("zweite.fit", b"zweite"),
        ]
    )
    client = account.client()
    cli(["backfill"], client, clock, root=root, token_dir=token_dir)

    folder = root / "activities" / "2026" / "103"
    assert (folder / "original.zip").read_bytes() == account.zips[103]
    assert (folder / "original.fit").read_bytes() == fit_bytes(103)
    other = root / "activities" / "2026" / "101"
    assert (other / "original.fit").read_bytes() == fit_bytes(101)
    assert not (root.parent / "outside.fit").exists() and not (root / "outside.fit").exists()
    assert not list(root.parent.rglob("outside*"))

    archive = Archive(root, lib_version="x", clock=clock)
    latest = archive.latest()
    zip_entry = latest['activity|original|{"activity_id":103,"year":"2026"}']
    fit_entry = latest['activity|original_fit|{"activity_id":103,"year":"2026"}']
    assert zip_entry.path.endswith("original.zip") and fit_entry.path.endswith("original.fit")
    assert fit_entry.sha256 == hashlib.sha256(fit_bytes(103)).hexdigest()
    assert archive.verify() == []


def test_broken_zip_is_noted_as_error_and_not_retried(root: Path, token_dir: Path, clock) -> None:
    account = young_account()
    account.zips[101] = b"kein zip"
    account.zips[102] = make_zip([("readme.txt", b"ohne fit")])
    client = account.client()
    code, out, _err, _f = cli(["backfill"], client, clock, root=root, token_dir=token_dir)
    assert code == EXIT_ERROR and "Fehler:           2" in out
    latest = Archive(root, lib_version="x", clock=clock).latest()
    first = latest['activity|original_fit|{"activity_id":101,"year":"2026"}']
    second = latest['activity|original_fit|{"activity_id":102,"year":"2026"}']
    assert (first.status, first.error_class) == ("error", "BadZipFile")
    assert (second.status, second.error_class) == ("error", "NoFitInZip")
    assert (root / "activities" / "2026" / "101" / "original.zip").is_file()
    assert not (root / "activities" / "2026" / "101" / "original.fit").exists()

    before = len(client.calls)
    code, out, _err, _f = cli(["backfill"], client, clock, root=root, token_dir=token_dir)
    assert code == EXIT_OK and len(client.calls) == before, (
        "kein erneuter Abruf, keine Wiederholung"
    )


def test_fit_size_limit_is_enforced(
    root: Path, token_dir: Path, clock, monkeypatch: pytest.MonkeyPatch
) -> None:
    from lastwerk_bridge import activities

    assert MAX_FIT_BYTES == 256 * 1024 * 1024
    monkeypatch.setattr(activities, "MAX_FIT_BYTES", 10)
    client = young_account().client()
    cli(["backfill"], client, clock, root=root, token_dir=token_dir)
    latest = Archive(root, lib_version="x", clock=clock).latest()
    fit = latest['activity|original_fit|{"activity_id":101,"year":"2026"}']
    assert (fit.status, fit.error_class) == ("error", "FitTooLarge")


# state.json und Register


def test_state_json_has_pointers_per_source(root: Path, token_dir: Path, clock) -> None:
    client = young_account().client()
    cli(["backfill"], client, clock, root=root, token_dir=token_dir)
    state = json.loads((root / "state.json").read_text(encoding="utf-8"))
    assert state["activities_list"]["complete"] is True
    assert state["activities_list"]["activities_known"] == 3
    assert state["activities_list"]["oldest"] == "2026-10-08"
    assert state["activities_list"]["newest"] == "2026-10-12"
    assert state["activities_list"]["pages_stored"] == 1
    assert state["activities_list"]["next_start"] == 100
    assert state["daily"]["newest"] == YESTERDAY.isoformat()
    assert "sleep_daily" in state["ranges"]
    assert state["last_run"]["stop_reason"] == "done"
    assert (root / "registry.yaml").is_file()
    assert Archive(root, lib_version="x", clock=clock).verify() == []


def test_max_calls_must_be_positive(root: Path, token_dir: Path, clock) -> None:
    client = young_account().client()
    code, _out, _err, _f = cli(
        ["backfill", "--max-calls", "0"], client, clock, root=root, token_dir=token_dir
    )
    assert code == EXIT_USAGE and client.calls == []


def test_archive_today_is_the_local_day_while_timestamps_stay_utc() -> None:
    from datetime import UTC, datetime, timedelta, timezone

    utc = datetime(2026, 10, 15, 3, 0, tzinfo=UTC)
    west = FakeClock(start=utc.timestamp(), tz=timezone(timedelta(hours=-5)))
    archive = Archive(Path("."), lib_version="x", clock=west)
    assert archive.today().isoformat() == "2026-10-14"
    assert archive.now().isoformat() == "2026-10-15T03:00:00+00:00"
    east = FakeClock(start=utc.timestamp() - 5 * 3600, tz=timezone(timedelta(hours=2)))
    assert Archive(Path("."), lib_version="x", clock=east).today().isoformat() == "2026-10-15"


def test_clock_fixture_is_the_day_the_tests_assume(clock: FakeClock) -> None:
    assert Archive(Path("."), lib_version="x", clock=clock).today() == TODAY
