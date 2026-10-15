"""``push``: Trockenlauf, Nutzlast je Vorlage, Schreibpfad mit Attrappen (SPEC 12 M6).

Quelle der erwarteten Struktur (Kommentar zu allen Nutzlast-Tests): python-garminconnect 0.3.17
(Q21, Q27), Quelltext von ``garminconnect/workout.py`` (``create_warmup_step``,
``create_interval_step``, ``create_recovery_step``, ``create_cooldown_step``,
``create_repeat_group``, ``SportType``, ``StepType``, ``ConditionType``, ``TargetType``) und
``upload_workout``/``schedule_workout`` in ``garminconnect/__init__.py``, gelesen am 14. Oktober
2026. Aufgezeichnete echte Antworten gibt es nicht: die Beispiele sind aus der Dokumentation
abgeleitet und synthetisch. Die Struktur bleibt bis zum ersten echten Versand eine Annahme.
"""

from __future__ import annotations

import inspect
import io
import json
import socket
import subprocess
import sys
from datetime import UTC, date, datetime, timedelta, timezone
from pathlib import Path
from typing import Any

import pytest
from garminconnect import GarminConnectAuthenticationError, GarminConnectTooManyRequestsError

import lastwerk_bridge.push as push
from lastwerk_bridge.allowlist import (
    ALLOWLIST,
    BLOCKLIST,
    WORKOUT_WRITE_ALLOWLIST,
    MethodNotAllowedError,
)
from lastwerk_bridge.cli import EXIT_ERROR, EXIT_LOCKOUT, EXIT_OK, EXIT_USAGE, main
from lastwerk_bridge.throttle import ThrottleConfig, ThrottledCaller

from .fakes import FakeClock, FakeGarmin
from .plan_fixture import make_plan, mutated, write_plan

BRIDGE_DIR = Path(__file__).resolve().parents[1]
REPO_DIR = BRIDGE_DIR.parent


# Hilfen


class Factory:
    """Client-Fabrik der Tests. Zählt, wie oft ein Client entsteht."""

    def __init__(self, client: FakeGarmin | None = None) -> None:
        self.client = client or workout_client()
        self.calls: list[Path] = []

    def __call__(self, token_dir: Path) -> FakeGarmin:
        self.calls.append(token_dir)
        return self.client


def workout_client(**overrides: Any) -> FakeGarmin:
    """Attrappe, die Workout-Kennungen 1001, 1002, ... vergibt."""
    counter = iter(range(1001, 2000))
    responses: dict[str, Any] = {
        "upload_workout": lambda payload: {"workoutId": next(counter), "workoutName": "x"},
        "schedule_workout": lambda workout_id, date_str: {"workoutScheduleId": 7},
    }
    responses.update(overrides)
    return FakeGarmin(responses)


def run(argv: list[str], factory: Factory, clock: FakeClock, **_: Any) -> tuple[int, str, str]:
    out, err = io.StringIO(), io.StringIO()
    code = main(argv, client_factory=factory, clock=clock, stdout=out, stderr=err)
    return code, out.getvalue(), err.getvalue()


@pytest.fixture
def templates() -> dict[str, push.Template]:
    return push.load_templates()


@pytest.fixture
def texts() -> dict[str, Any]:
    return push.load_locale(push.default_locales_dir(), "de")


@pytest.fixture
def plan_file(tmp_path: Path) -> Path:
    return write_plan(tmp_path / "in" / "plan.json")


def dry_run_document(plan_file: Path, clock: FakeClock, *extra: str) -> dict[str, Any]:
    code, out, err = run(["push", "--plan", str(plan_file), "--dry-run", *extra], Factory(), clock)
    assert code == EXIT_OK, err
    return json.loads(out)  # Standardausgabe ist reines JSON


def flat_steps(steps: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Alle ausführbaren Schritte in Reihenfolge, Wiederholungen aufgelöst (einmal)."""
    result: list[dict[str, Any]] = []
    for step in steps:
        if step["type"] == "RepeatGroupDTO":
            result.extend(flat_steps(step["workoutSteps"]))
        else:
            result.append(step)
    return result


def all_orders(steps: list[dict[str, Any]]) -> list[int]:
    orders: list[int] = []
    for step in steps:
        orders.append(step["stepOrder"])
        if step["type"] == "RepeatGroupDTO":
            orders.extend(all_orders(step["workoutSteps"]))
    return orders


def summary(step: dict[str, Any]) -> tuple[str, float, str, int | None]:
    """(Schritttyp, Sekunden, Zieltyp, Zonennummer) eines ausführbaren Schritts."""
    return (
        step["stepType"]["stepTypeKey"],
        step["endConditionValue"],
        step["targetType"]["workoutTargetTypeKey"],
        step.get("zoneNumber"),
    )


def build(
    templates: dict[str, push.Template], template_id: str, minutes: int, title: str = "T"
) -> dict[str, Any]:
    return push.build_payload(templates[template_id], minutes, title)


# Nutzlast: handgerechnete Referenzwerte
#
# Dauer: Intervallblöcke bleiben, die übrigen Schritte teilen die Restzeit im Verhältnis ihrer
# Nennzeit (SPEC 7.4), ganze Minuten, Rest an den größten Nachkommaanteil.


def test_threshold_2x20_at_nominal_duration_matches_the_catalog(templates) -> None:
    # Katalog: Aufwärmen 15 (Z2), 2 mal 20 (Z4) mit 5 Pause, Abwärmen 10 (Z1) = 70 Minuten.
    # Dauer 70: fester Block 2*20 + 1*5 = 45, dehnbar 25 von 25, Faktor 1. Zwei Wiederholungen
    # ergeben nach "Wiederholungen minus eins" nur eine Pause, also keine Gruppe: A, P, A.
    payload = build(templates, "bike.threshold.2x20", 70)
    steps = payload["workoutSegments"][0]["workoutSteps"]
    assert [summary(s) for s in steps] == [
        ("warmup", 900.0, "power.zone", 2),
        ("interval", 1200.0, "power.zone", 4),
        ("recovery", 300.0, "power.zone", 1),
        ("interval", 1200.0, "power.zone", 4),
        ("cooldown", 600.0, "power.zone", 1),
    ]
    assert payload["estimatedDurationInSecs"] == 70 * 60


def test_threshold_2x20_at_90_minutes_stretches_warmup_and_cooldown(templates) -> None:
    # Dauer 90: dehnbar 90 - 45 = 45 Minuten, Faktor 45/25 = 1,8. Aufwärmen 15 * 1,8 = 27,
    # Abwärmen 10 * 1,8 = 18, beide ganzzahlig, Summe 45.
    steps = build(templates, "bike.threshold.2x20", 90)["workoutSegments"][0]["workoutSteps"]
    assert [s["endConditionValue"] for s in steps] == [1620.0, 1200.0, 300.0, 1200.0, 1080.0]


def test_vo2_5x4_uses_a_repeat_group_and_rounds_by_largest_remainder(templates) -> None:
    # Dauer 80: fester Block 5*4 + 4*4 = 36, dehnbar 44 bei Nennzeit 30 (20 + 10).
    # Aufwärmen 20 * 44/30 = 29,33, Abwärmen 10 * 44/30 = 14,67. Abgerundet 29 + 14 = 43,
    # eine Minute bleibt: der größere Nachkommaanteil (0,67) bekommt sie, Abwärmen 15.
    # Struktur: 4 mal (4 min Z5, 4 min Pause Z1), danach die letzte Arbeit ohne Pause.
    payload = build(templates, "bike.vo2.5x4", 80)
    steps = payload["workoutSegments"][0]["workoutSteps"]
    assert [s["type"] for s in steps] == [
        "ExecutableStepDTO",
        "RepeatGroupDTO",
        "ExecutableStepDTO",
        "ExecutableStepDTO",
    ]
    assert steps[0]["endConditionValue"] == 29 * 60.0
    group = steps[1]
    assert group["numberOfIterations"] == 4
    assert group["stepType"] == {"stepTypeId": 6, "stepTypeKey": "repeat", "displayOrder": 6}
    assert group["endCondition"]["conditionTypeKey"] == "iterations"
    assert group["endConditionValue"] == 4.0
    assert [summary(s) for s in group["workoutSteps"]] == [
        ("interval", 240.0, "power.zone", 5),
        ("recovery", 240.0, "power.zone", 1),
    ]
    assert summary(steps[2]) == ("interval", 240.0, "power.zone", 5)
    assert steps[3]["endConditionValue"] == 15 * 60.0
    assert payload["estimatedDurationInSecs"] == 80 * 60


def test_run_intervals_6x3_uses_pace_zones(templates) -> None:
    # Dauer 50: fester Block 6*3 + 5*2 = 28, dehnbar 22 bei Nennzeit 20 (12 + 8).
    # Aufwärmen 12 * 22/20 = 13,2, Abwärmen 8 * 22/20 = 8,8. Abgerundet 13 + 8 = 21, eine Minute
    # bleibt, der Anteil 0,8 gewinnt: Abwärmen 9. Gruppe: 5 mal (3 min Z5, 2 min Pause Z1).
    payload = build(templates, "run.intervals.6x3", 50)
    steps = payload["workoutSegments"][0]["workoutSteps"]
    assert payload["sportType"] == {"sportTypeId": 1, "sportTypeKey": "running", "displayOrder": 1}
    assert summary(steps[0]) == ("warmup", 13 * 60.0, "pace.zone", 2)
    assert steps[1]["numberOfIterations"] == 5
    assert [summary(s) for s in steps[1]["workoutSteps"]] == [
        ("interval", 180.0, "pace.zone", 5),
        ("recovery", 120.0, "pace.zone", 1),
    ]
    assert summary(steps[2]) == ("interval", 180.0, "pace.zone", 5)
    assert summary(steps[3]) == ("cooldown", 9 * 60.0, "pace.zone", 1)
    assert payload["estimatedDurationInSecs"] == 50 * 60


def test_swim_easy_distributes_the_remainder_to_the_longest_step(templates) -> None:
    # Dauer 30, dehnbar 30 bei Nennzeit 35 (5 + 25 + 5). Anteile 4,29 / 21,43 / 4,29, abgerundet
    # 4 + 21 + 4 = 29, eine Minute bleibt. Nachkommaanteil 0,43 ist der größte: gleichmäßig 22.
    payload = build(templates, "swim.technique.easy", 30)
    steps = payload["workoutSegments"][0]["workoutSteps"]
    assert payload["sportType"] == {
        "sportTypeId": 4,
        "sportTypeKey": "swimming",
        "displayOrder": 3,
    }
    assert [summary(s) for s in steps] == [
        ("warmup", 240.0, "pace.zone", 1),
        ("interval", 1320.0, "pace.zone", 2),
        ("cooldown", 240.0, "pace.zone", 1),
    ]


def test_recovery_spin_is_one_step(templates) -> None:
    # Ein dehnbarer Schritt: bekommt die ganze Dauer (45 Minuten = 2700 s), Zone 1.
    steps = build(templates, "bike.recovery.spin", 45)["workoutSegments"][0]["workoutSteps"]
    assert [summary(s) for s in steps] == [("interval", 2700.0, "power.zone", 1)]


def test_strength_is_one_timed_step_without_target(templates) -> None:
    # Kraft: ein Schritt mit Zeitvorgabe (30 min = 1800 s), kein Ziel. RPE steht im Titel.
    payload = build(templates, "strength.general.a", 30, "Kraft, Programm A (RPE 6)")
    assert payload["sportType"] == {
        "sportTypeId": 5,
        "sportTypeKey": "strength_training",
        "displayOrder": 5,
    }
    steps = payload["workoutSegments"][0]["workoutSteps"]
    assert len(steps) == 1
    assert summary(steps[0]) == ("interval", 1800.0, "no.target", None)
    assert "zoneNumber" not in steps[0]
    assert payload["workoutName"] == "Kraft, Programm A (RPE 6)"


def test_step_has_the_documented_fields(templates) -> None:
    # Feldnamen und Kennungen nach workout.py: ExecutableStepDTO mit stepType, endCondition
    # (conditionTypeId 2 "time"), endConditionValue in Sekunden, targetType (2 = power.zone).
    step = build(templates, "bike.endurance.steady", 60)["workoutSegments"][0]["workoutSteps"][0]
    assert step == {
        "type": "ExecutableStepDTO",
        "stepOrder": 1,
        "stepType": {"stepTypeId": 1, "stepTypeKey": "warmup", "displayOrder": 1},
        "endCondition": {
            "conditionTypeId": 2,
            "conditionTypeKey": "time",
            "displayOrder": 2,
            "displayable": True,
        },
        "endConditionValue": 600.0,
        "targetType": {
            "workoutTargetTypeId": 2,
            "workoutTargetTypeKey": "power.zone",
            "displayOrder": 1,
        },
        "zoneNumber": 1,
    }


def test_payload_header(templates) -> None:
    payload = build(templates, "run.easy.steady", 40, "Lockerer Dauerlauf")
    assert set(payload) == {
        "workoutName",
        "description",
        "sportType",
        "estimatedDurationInSecs",
        "workoutSegments",
    }
    assert payload["workoutName"] == "Lockerer Dauerlauf"
    segment = payload["workoutSegments"][0]
    assert segment["segmentOrder"] == 1
    assert segment["sportType"] == payload["sportType"]
    assert payload["description"] == "Lastwerk run.easy.steady, 40 min"


# Nutzlast: Eigenschaften über den ganzen Katalog


def endurance_ids(templates: dict[str, push.Template]) -> list[str]:
    return [tid for tid, t in templates.items() if t.sport != "rest"]


def test_every_template_sums_to_the_duration_over_its_whole_window(templates) -> None:
    checked = 0
    for tid in endurance_ids(templates):
        template = templates[tid]
        for minutes in range(template.min_minutes, template.max_minutes + 1):
            payload = push.build_payload(template, minutes, tid)
            steps = payload["workoutSegments"][0]["workoutSteps"]
            assert payload["estimatedDurationInSecs"] == minutes * 60, (tid, minutes)
            orders = all_orders(steps)
            assert orders == list(range(1, len(orders) + 1)), (tid, minutes)
            assert all(s["endConditionValue"] > 0 for s in flat_steps(steps)), (tid, minutes)
            json.dumps(payload)  # serialisierbar
            checked += 1
    assert checked > 500


def test_nominal_duration_reproduces_the_catalog_minutes(templates) -> None:
    for tid in endurance_ids(templates):
        template = templates[tid]
        if template.sport == "strength":
            continue
        nominal = template.fixed_minutes + template.flexible_minutes
        flexible = [s.minutes for s in template.steps if s.kind != "interval"]
        assert push.stretched_minutes(template, nominal) == flexible, tid


def test_every_zone_of_the_catalog_reaches_the_payload(templates) -> None:
    for tid in endurance_ids(templates):
        template = templates[tid]
        if template.sport == "strength":
            continue
        steps = build(templates, tid, template.max_minutes)["workoutSegments"][0]["workoutSteps"]
        zones = {s.get("zoneNumber") for s in flat_steps(steps)}
        expected = {s.zone for s in template.steps} | (
            {1} if any(s.rest_minutes for s in template.steps) else set()
        )
        assert zones == expected, tid


def test_sport_types_follow_the_library_ids(templates) -> None:
    ids = {
        "bike.endurance.steady": (2, "cycling"),
        "run.easy.steady": (1, "running"),
        "swim.technique.easy": (4, "swimming"),
        "strength.general.b": (5, "strength_training"),
    }
    for tid, (type_id, key) in ids.items():
        sport = build(templates, tid, 45)["sportType"]
        assert (sport["sportTypeId"], sport["sportTypeKey"]) == (type_id, key)


def test_duration_below_the_interval_blocks_drops_the_flexible_steps(templates) -> None:
    # Dauer 40 < fester Block 45 von 2x20: dehnbar 0, die Summe überschreitet die Dauer.
    # Der Planer erzeugt das nie (Mindestdauer 60), der Trockenlauf meldet es als Warnung.
    template = templates["bike.threshold.2x20"]
    assert push.stretched_minutes(template, 40) == [0, 0]
    steps = push.build_steps(template, 40)
    assert [s["stepType"]["stepTypeKey"] for s in steps] == ["interval", "recovery", "interval"]


def test_rest_between_two_blocks_is_flat_and_three_or_more_use_a_group(templates) -> None:
    two = push.build_steps(templates["bike.threshold.2x20"], 70)
    three = push.build_steps(templates["bike.tempo.blocks"], 75)
    assert all(s["type"] == "ExecutableStepDTO" for s in two)
    groups = [s for s in three if s["type"] == "RepeatGroupDTO"]
    assert len(groups) == 1
    assert groups[0]["numberOfIterations"] == 2  # 3 Wiederholungen: 2 mal (A, P), dann A


# Titel


def test_title_comes_from_the_plan_locale(texts) -> None:
    assert push.template_title(texts, "bike.threshold.2x20") == "Schwelle 2 mal 20"
    assert push.template_title(texts, "run.long.steady") == "Langer Lauf"
    en = push.load_locale(push.default_locales_dir(), "en")
    assert push.template_title(en, "run.long.steady") != "Langer Lauf"
    assert push.template_title(en, "run.long.steady") != "run.long.steady"


def test_unknown_title_falls_back_to_the_template_id(texts) -> None:
    assert push.template_title(texts, "bike.unknown.thing") == "bike.unknown.thing"
    assert push.template_title({}, "bike.endurance.steady") == "bike.endurance.steady"


# Trockenlauf


def test_dry_run_prints_the_payload_per_session_and_sends_nothing(
    plan_file: Path, clock, monkeypatch: pytest.MonkeyPatch
) -> None:
    import garminconnect

    def forbidden(*_a: object, **_k: object) -> None:
        raise AssertionError("Instanz der Bibliothek im Trockenlauf")

    monkeypatch.setattr(garminconnect.Garmin, "__init__", forbidden)
    factory = Factory()
    code, out, err = run(["push", "--plan", str(plan_file), "--dry-run"], factory, clock)
    assert code == EXIT_OK, err
    document = json.loads(out)
    assert document["dry_run"] is True
    assert document["sends_nothing"] is True
    assert document["counts"] == {"workouts": 13, "skipped": 0, "outside_selection": 0}
    assert factory.calls == []  # kein Client erzeugt
    assert factory.client.calls == []
    first = document["workouts"][0]
    assert first["date"] == "2027-01-05"
    assert first["template_id"] == "bike.threshold.2x20"
    assert first["title"] == "Schwelle 2 mal 20"
    for item in document["workouts"]:
        assert item["payload"]["estimatedDurationInSecs"] == item["duration_min"] * 60
        assert item["warnings"] == []
    # Zwei Einheiten am selben Tag bleiben getrennt.
    thursday = [w for w in document["workouts"] if w["date"] == "2027-01-07"]
    assert [w["slot"] for w in thursday] == ["am", "pm"]
    assert "assumption" in document


def test_dry_run_is_the_default_without_flags(plan_file: Path, clock) -> None:
    factory = Factory()
    code, out, _err = run(["push", "--plan", str(plan_file)], factory, clock)
    assert code == EXIT_OK
    assert json.loads(out)["dry_run"] is True
    assert factory.calls == []


def test_dry_run_never_touches_the_network_layer(
    plan_file: Path, clock, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Zusätzlich zur Sperre aus conftest: ein Zähler zeigt, dass nie ein Versuch stattfindet."""
    attempts: list[str] = []

    def counting(name: str):
        def refuse(*_a: object, **_k: object) -> None:
            attempts.append(name)
            raise RuntimeError("Netzwerkzugriff im Test gesperrt")

        return refuse

    for name in ("socket", "create_connection", "getaddrinfo"):
        monkeypatch.setattr(socket, name, counting(name))
    code, _out, err = run(["push", "--plan", str(plan_file), "--dry-run"], Factory(), clock)
    assert code == EXIT_OK, err
    assert attempts == []


def test_dry_run_with_out_writes_push_dry_run_json(tmp_path: Path, plan_file: Path, clock) -> None:
    out_dir = tmp_path / "exchange" / "out"
    document = dry_run_document(plan_file, clock, "--out", str(out_dir))
    written = json.loads((out_dir / "push-dry-run.json").read_text(encoding="utf-8"))
    assert written == document
    assert not (out_dir / "push-report.json").exists()


def test_dry_run_selection_options(plan_file: Path, clock) -> None:
    document = dry_run_document(
        plan_file, clock, "--from", "2027-01-07", "--to", "2027-01-09", "--max-workouts", "2"
    )
    assert [w["date"] for w in document["workouts"]] == ["2027-01-07", "2027-01-07"]
    assert document["selection"] == {"from": "2027-01-07", "to": "2027-01-09", "max_workouts": 2}
    assert document["counts"]["outside_selection"] == 11
    only_range = dry_run_document(plan_file, clock, "--from", "2027-01-07", "--to", "2027-01-09")
    assert [w["date"] for w in only_range["workouts"]] == [
        "2027-01-07",
        "2027-01-07",
        "2027-01-08",
        "2027-01-09",
    ]


def test_dry_run_marks_sessions_in_the_past(tmp_path: Path, clock) -> None:
    past = write_plan(tmp_path / "past.json", make_plan(date(2026, 10, 5)))
    document = dry_run_document(past, clock)
    today = datetime.fromtimestamp(clock.now(), tz=UTC).date().isoformat()
    for item in document["workouts"]:
        assert item["in_past"] == (item["date"] < today)
    assert any(item["in_past"] for item in document["workouts"])


def _clock_at(utc: datetime, offset_hours: int) -> FakeClock:
    return FakeClock(start=utc.timestamp(), tz=timezone(timedelta(hours=offset_hours)))


def test_today_is_the_local_calendar_day_west_of_utc(tmp_path: Path) -> None:
    # 2026-10-15 03:00 UTC ist in UTC-5 noch der 14. Oktober, 22:00 Uhr. Die Einheit am 14.
    # liegt dort nicht in der Vergangenheit. Mittwoch 14.10. ist die Einheit run.easy.steady.
    plan = write_plan(tmp_path / "p.json", make_plan(date(2026, 10, 12)))
    clock = _clock_at(datetime(2026, 10, 15, 3, 0, tzinfo=UTC), -5)
    by_date = {w["date"]: w for w in dry_run_document(plan, clock)["workouts"]}
    assert by_date["2026-10-13"]["in_past"] is True
    assert by_date["2026-10-14"]["in_past"] is False  # mit UTC-Datum wäre es True
    assert by_date["2026-10-15"]["in_past"] is False


def test_today_is_the_local_calendar_day_east_of_utc(tmp_path: Path) -> None:
    # 2026-10-14 22:30 UTC ist in UTC+2 schon der 15. Oktober, 00:30 Uhr. Die Einheit am 14.
    # ist dort vergangen.
    plan = write_plan(tmp_path / "p.json", make_plan(date(2026, 10, 12)))
    clock = _clock_at(datetime(2026, 10, 14, 22, 30, tzinfo=UTC), 2)
    by_date = {w["date"]: w for w in dry_run_document(plan, clock)["workouts"]}
    assert by_date["2026-10-14"]["in_past"] is True  # mit UTC-Datum wäre es False
    assert by_date["2026-10-15"]["in_past"] is False


@pytest.mark.parametrize(
    ("utc", "offset", "first_date"),
    [
        (datetime(2026, 10, 15, 3, 0, tzinfo=UTC), -5, "2026-10-14"),
        (datetime(2026, 10, 14, 22, 30, tzinfo=UTC), 2, "2026-10-15"),
    ],
)
def test_yes_starts_at_the_local_day_by_default(
    utc: datetime, offset: int, first_date: str, tmp_path: Path, token_dir: Path
) -> None:
    plan = write_plan(tmp_path / "p.json", make_plan(date(2026, 10, 12)))
    factory = Factory()
    clock = _clock_at(utc, offset)
    code, _out, err = run(send_args(plan, tmp_path / "out", token_dir, 1), factory, clock)
    assert code == EXIT_OK, err
    scheduled = [c[1][1] for c in factory.client.calls if c[0] == "schedule_workout"]
    assert scheduled == [first_date]


def test_dry_run_warns_about_a_duration_outside_the_template_window(tmp_path: Path, clock) -> None:
    plan = mutated(make_plan(), "weeks", 0, "days", 1, "sessions", 0, "duration_min", value=300)
    document = dry_run_document(write_plan(tmp_path / "p.json", plan), clock)
    assert document["workouts"][0]["warnings"] == ["duration_outside_template_window"]


def test_dry_run_uses_the_titles_of_the_plan_language(tmp_path: Path, clock) -> None:
    en = write_plan(tmp_path / "en.json", make_plan(locale="en"))
    document = dry_run_document(en, clock)
    assert document["locale"] == "en"
    assert document["workouts"][0]["title"] != "Schwelle 2 mal 20"
    assert document["workouts"][0]["title"] != "bike.threshold.2x20"


def test_custom_templates_option_is_used(tmp_path: Path, plan_file: Path, clock) -> None:
    catalog = json.loads(push.default_templates_path().read_text(encoding="utf-8"))
    for template in catalog["templates"]:
        if template["id"] == "run.easy.steady":
            template["steps"] = [
                {"kind": "steady", "minutes": 30, "zone_scheme": "run_speed5", "zone": 3}
            ]
    path = tmp_path / "templates.json"
    path.write_text(json.dumps(catalog), encoding="utf-8")
    document = dry_run_document(plan_file, clock, "--templates", str(path))
    run_item = next(w for w in document["workouts"] if w["template_id"] == "run.easy.steady")
    steps = run_item["payload"]["workoutSegments"][0]["workoutSteps"]
    assert [(s["endConditionValue"], s["zoneNumber"]) for s in steps] == [(2400.0, 3)]
    # Die Texte kommen von locales neben dem Katalog; dort fehlen sie: Titel ist die ID.
    assert run_item["title"] == "run.easy.steady"


def test_missing_locale_file_falls_back_to_template_ids(
    tmp_path: Path, plan_file: Path, clock
) -> None:
    empty = tmp_path / "no-locales"
    empty.mkdir()
    document = dry_run_document(plan_file, clock, "--locales", str(empty))
    assert document["workouts"][0]["title"] == "bike.threshold.2x20"


def test_dry_run_in_a_subprocess_without_the_library_and_without_sockets(
    plan_file: Path,
) -> None:
    """Die Bibliothek ist nicht einmal importierbar und jeder Socket wirft: es klappt trotzdem."""
    code = (
        "import socket, sys\n"
        "sys.modules['garminconnect'] = None\n"  # import garminconnect -> ImportError
        "def refuse(*a, **k):\n"
        "    raise RuntimeError('Socket gesperrt')\n"
        "socket.socket = refuse\n"
        "socket.create_connection = refuse\n"
        "socket.getaddrinfo = refuse\n"
        "from lastwerk_bridge.cli import main\n"
        f"sys.exit(main(['push', '--plan', {str(plan_file)!r}, '--dry-run']))\n"
    )
    result = subprocess.run(
        [sys.executable, "-c", code],
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
    assert result.returncode == EXIT_OK, result.stderr
    assert json.loads(result.stdout)["counts"]["workouts"] == 13


def test_acceptance_command_from_the_task_runs_as_a_subprocess() -> None:
    """Befehl aus T16 und SPEC 12 M6, mit einem Plan, den dieser Test selbst anlegt.

    ``../target/plan.json`` entsteht sonst in der Rust-Abnahme (``plan --out target/plan.json``).
    Existiert die Datei schon, bleibt sie unverändert und wird genutzt.
    """
    plan_path = REPO_DIR / "target" / "plan.json"
    if not plan_path.exists():
        write_plan(plan_path)
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "lastwerk_bridge",
            "push",
            "--plan",
            "../target/plan.json",
            "--dry-run",
        ],
        cwd=BRIDGE_DIR,
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
    assert result.returncode == EXIT_OK, result.stderr
    document = json.loads(result.stdout)
    assert document["sends_nothing"] is True
    assert document["counts"]["workouts"] == len(document["workouts"]) > 0
    for item in document["workouts"]:
        steps = item["payload"]["workoutSegments"][0]["workoutSteps"]
        assert steps
        assert item["payload"]["estimatedDurationInSecs"] == item["duration_min"] * 60


# Ungültige Eingaben: Exit-Code 2, nichts gesendet


@pytest.mark.parametrize("mode", ["--dry-run", "--yes"])
def test_invalid_plan_exits_2_and_sends_nothing(
    mode: str, tmp_path: Path, token_dir: Path, clock
) -> None:
    bad = mutated(make_plan(), "weeks", 0, "days", 1, "sessions", 0, "hard", value=...)
    plan = write_plan(tmp_path / "bad.json", bad)
    factory = Factory()
    argv = ["push", "--plan", str(plan), mode, "--token-dir", str(token_dir)]
    if mode == "--yes":
        argv += ["--out", str(tmp_path / "out"), "--max-workouts", "1"]
    code, out, err = run(argv, factory, clock)
    assert code == EXIT_USAGE
    assert out == ""
    assert "Schema" in err and "nichts gesendet" in err
    assert factory.calls == []
    assert not (tmp_path / "out").exists()


@pytest.mark.parametrize(
    "plan_bad",
    [
        mutated(make_plan(), "schema_version", value=2),
        mutated(make_plan(), "weeks", 0, "days", 1, "sessions", 0, "unknown", value=1),
        mutated(make_plan(), "plan_id", value="abc"),
        mutated(make_plan(), "weeks", value=[]),
    ],
)
def test_schema_violations_exit_2(plan_bad: dict[str, Any], tmp_path: Path, clock) -> None:
    plan = write_plan(tmp_path / "p.json", plan_bad)
    code, out, err = run(["push", "--plan", str(plan)], Factory(), clock)
    assert (code, out) == (EXIT_USAGE, "")
    assert "plan.json" in err


def test_missing_file_garbage_nan_and_oversize_exit_2(tmp_path: Path, clock) -> None:
    cases = {
        "missing.json": None,
        "garbage.json": "kein json",
        "nan.json": '{"schema_version": NaN}',
        "array.json": "[]",
        "huge.json": "[" + "0," * push.MAX_INPUT_BYTES + "0]",
    }
    for name, content in cases.items():
        path = tmp_path / name
        if content is not None:
            path.write_text(content, encoding="utf-8")
        code, out, err = run(["push", "--plan", str(path)], Factory(), clock)
        assert (code, out) == (EXIT_USAGE, ""), name
        assert "plan.json" in err, name


def test_unknown_template_and_sport_mismatch_exit_2(tmp_path: Path, clock) -> None:
    unknown = mutated(
        make_plan(), "weeks", 0, "days", 1, "sessions", 0, "template_id", value="bike.nope.x"
    )
    code, _out, err = run(
        ["push", "--plan", str(write_plan(tmp_path / "a.json", unknown))], Factory(), clock
    )
    assert code == EXIT_USAGE and "fehlt im Katalog" in err
    mismatch = mutated(make_plan(), "weeks", 0, "days", 1, "sessions", 0, "sport", value="running")
    code, _out, err = run(
        ["push", "--plan", str(write_plan(tmp_path / "b.json", mismatch))], Factory(), clock
    )
    assert code == EXIT_USAGE and "Sportart" in err


def test_unreadable_catalog_exits_2(tmp_path: Path, plan_file: Path, clock) -> None:
    broken = tmp_path / "t.json"
    broken.write_text('{"format_version": 2, "templates": []}', encoding="utf-8")
    code, _out, err = run(
        ["push", "--plan", str(plan_file), "--templates", str(broken)], Factory(), clock
    )
    assert code == EXIT_USAGE and "Katalog" in err


def test_rest_days_and_rest_template_are_skipped(tmp_path: Path, clock) -> None:
    plan = make_plan()
    plan["weeks"][0]["days"][0]["sessions"] = [
        {
            "slot": "am",
            "sport": "rest",
            "template_id": "rest.day",
            "duration_min": 10,
            "planned_load": 0,
            "hard": False,
        }
    ]
    document = dry_run_document(write_plan(tmp_path / "p.json", plan), clock)
    assert document["counts"]["workouts"] == 13
    assert document["skipped"] == [
        {"date": "2027-01-04", "slot": "am", "template_id": "rest.day", "reason": "rest"}
    ]


# Schreibpfad: Freigabe


def library_methods() -> set[str]:
    from garminconnect import Garmin

    return {
        n for n, m in inspect.getmembers(Garmin) if not n.startswith("_") and inspect.isfunction(m)
    }


def test_write_allowlist_is_explicit_and_stays_in_the_blocklist() -> None:
    assert WORKOUT_WRITE_ALLOWLIST == {"schedule_workout", "upload_workout"}
    assert WORKOUT_WRITE_ALLOWLIST <= BLOCKLIST
    assert not (WORKOUT_WRITE_ALLOWLIST & ALLOWLIST)
    assert WORKOUT_WRITE_ALLOWLIST <= library_methods()


def test_library_signatures_match_the_calls_of_push() -> None:
    from garminconnect import Garmin

    upload = list(inspect.signature(Garmin.upload_workout).parameters)
    schedule = list(inspect.signature(Garmin.schedule_workout).parameters)
    assert upload == ["self", "workout_json"]
    assert schedule == ["self", "workout_id", "date_str"]


@pytest.mark.parametrize("method", sorted(WORKOUT_WRITE_ALLOWLIST))
def test_reading_caller_still_refuses_the_write_methods(method: str, tmp_path: Path) -> None:
    fake = FakeGarmin()
    caller = ThrottledCaller(fake, state_dir=tmp_path)
    with pytest.raises(MethodNotAllowedError):
        caller.call(method, {})
    assert fake.calls == []


@pytest.mark.parametrize(
    "method",
    [
        "get_user_profile",
        "get_workouts",
        "delete_workout",
        "update_workout",
        "unschedule_workout",
        "push_workout_to_device",
        "upload_running_workout",
        "upload_activity",
        "delete_activity",
        "login",
        "connectapi",
        "get_menstrual_data_for_date",
    ],
)
def test_write_caller_allows_nothing_but_its_two_methods(method: str, tmp_path: Path) -> None:
    fake = FakeGarmin()
    caller = push.WorkoutWriteCaller(fake, state_dir=tmp_path)
    with pytest.raises(MethodNotAllowedError):
        caller.call(method)
    assert fake.calls == []


def test_write_caller_calls_upload_and_schedule(tmp_path: Path, clock) -> None:
    fake = workout_client()
    caller = push.WorkoutWriteCaller(
        fake, state_dir=tmp_path, clock=clock, config=ThrottleConfig(retries=0)
    )
    assert caller.call("upload_workout", {"workoutName": "x"}) == {
        "workoutId": 1001,
        "workoutName": "x",
    }
    caller.call("schedule_workout", 1001, "2027-01-05")
    assert fake.names_called() == ["upload_workout", "schedule_workout"]


# Schreibpfad: Ablauf mit Attrappe


def send_args(
    plan_file: Path, out: Path, token_dir: Path, count: int = 3, *extra: str
) -> list[str]:
    return [
        "push",
        "--plan",
        str(plan_file),
        "--yes",
        "--out",
        str(out),
        "--max-workouts",
        str(count),
        "--token-dir",
        str(token_dir),
        *extra,
    ]


@pytest.fixture
def future_plan(tmp_path: Path) -> Path:
    # Beginnt weit nach der Uhr der Tests (Oktober 2026), damit kein Termin als vergangen gilt.
    return write_plan(tmp_path / "plan.json")


def test_yes_calls_only_write_methods_and_writes_the_report(
    tmp_path: Path, future_plan: Path, token_dir: Path, clock
) -> None:
    factory = Factory()
    out_dir = tmp_path / "exchange" / "out"
    code, out, err = run(send_args(future_plan, out_dir, token_dir), factory, clock)
    assert code == EXIT_OK, err
    names = factory.client.names_called()
    assert names == ["upload_workout", "schedule_workout"] * 3
    assert set(names) <= WORKOUT_WRITE_ALLOWLIST
    # Planung mit Kennung aus der Antwort und dem Datum der Einheit.
    schedules = [c for c in factory.client.calls if c[0] == "schedule_workout"]
    assert [c[1] for c in schedules] == [
        (1001, "2027-01-05"),
        (1002, "2027-01-06"),
        (1003, "2027-01-07"),
    ]
    # Angelegt wird genau die Nutzlast des Trockenlaufs.
    uploads = [c[1][0] for c in factory.client.calls if c[0] == "upload_workout"]
    dry = dry_run_document(future_plan, clock, "--max-workouts", "3")
    assert uploads == [w["payload"] for w in dry["workouts"]]

    report_path = out_dir / "push-report.json"
    report = json.loads(report_path.read_text(encoding="utf-8"))
    assert report["report_version"] == 1
    assert report["plan_id"] == make_plan()["plan_id"]
    assert [e["status"] for e in report["entries"]] == ["ok"] * 3
    assert [e["workout_id"] for e in report["entries"]] == [1001, 1002, 1003]
    first = report["entries"][0]
    assert first["date"] == "2027-01-05"
    assert first["title"] == "Schwelle 2 mal 20"
    assert first["template_id"] == "bike.threshold.2x20"
    # Weder Nutzlast noch Zugangsdaten im Protokoll.
    text = report_path.read_text(encoding="utf-8")
    for forbidden in ("workoutSteps", "endConditionValue", "garmin_tokens", "password", "token"):
        assert forbidden not in text
    assert "gesendet und geplant" in out
    assert "Achtung: schreibender Zugriff" in err


def test_yes_is_idempotent_and_continues_with_a_larger_selection(
    tmp_path: Path, future_plan: Path, token_dir: Path, clock
) -> None:
    out_dir = tmp_path / "out"
    factory = Factory()
    assert run(send_args(future_plan, out_dir, token_dir, 2), factory, clock)[0] == EXIT_OK
    assert len(factory.client.calls) == 4
    # Dieselbe Auswahl noch einmal: nichts wird erneut angelegt.
    code, out, _err = run(send_args(future_plan, out_dir, token_dir, 2), factory, clock)
    assert code == EXIT_OK
    assert len(factory.client.calls) == 4
    assert out.count("übersprungen (schon gesendet)") == 2
    # Größere Auswahl: nur die neuen.
    assert run(send_args(future_plan, out_dir, token_dir, 4), factory, clock)[0] == EXIT_OK
    assert len(factory.client.calls) == 8
    entries = json.loads((out_dir / "push-report.json").read_text(encoding="utf-8"))["entries"]
    assert len(entries) == 4
    assert len({e["workout_id"] for e in entries}) == 4


def test_yes_skips_sessions_in_the_past_by_default(tmp_path: Path, token_dir: Path, clock) -> None:
    today = datetime.fromtimestamp(clock.now(), tz=UTC).date()
    # Plan, der eine Woche vor der Uhr beginnt: die erste Woche liegt ganz in der Vergangenheit,
    # die zweite zum Teil.
    monday = today - timedelta(days=today.weekday() + 7)
    plan_path = write_plan(tmp_path / "past.json", make_plan(monday))
    factory = Factory()
    code, _out, err = run(send_args(plan_path, tmp_path / "out", token_dir, 50), factory, clock)
    assert code == EXIT_OK, err
    scheduled = [c[1][1] for c in factory.client.calls if c[0] == "schedule_workout"]
    assert scheduled
    assert all(date >= today.isoformat() for date in scheduled)
    # Mit ausdrücklichem --from werden auch frühere Termine geplant.
    factory2 = Factory()
    run(
        send_args(plan_path, tmp_path / "out2", token_dir, 50, "--from", monday.isoformat()),
        factory2,
        clock,
    )
    assert len(factory2.client.calls) > len(factory.client.calls)


def test_yes_requires_out_and_max_workouts(
    tmp_path: Path, future_plan: Path, token_dir: Path, clock
) -> None:
    factory = Factory()
    base = ["push", "--plan", str(future_plan), "--yes", "--token-dir", str(token_dir)]
    for extra in ([], ["--out", str(tmp_path / "o")], ["--max-workouts", "1"]):
        code, _out, err = run(base + extra, factory, clock)
        assert code == EXIT_USAGE
        assert "Pflicht" in err
    assert factory.calls == []
    assert not (tmp_path / "o").exists()


def test_yes_and_dry_run_exclude_each_other(future_plan: Path, clock) -> None:
    code, _out, _err = run(
        ["push", "--plan", str(future_plan), "--yes", "--dry-run"], Factory(), clock
    )
    assert code == EXIT_USAGE


def test_yes_without_tokens_exits_2_without_any_request(
    tmp_path: Path, future_plan: Path, clock
) -> None:
    factory = Factory()
    code, _out, err = run(
        send_args(future_plan, tmp_path / "out", tmp_path / "no-tokens"), factory, clock
    )
    assert code == EXIT_USAGE
    assert "Keine Anmeldung" in err and "keine Anfrage" in err
    assert factory.calls == []
    assert factory.client.calls == []
    assert not (tmp_path / "out" / "push-report.json").exists()


def test_yes_with_active_lockout_exits_3_without_a_call(
    tmp_path: Path, future_plan: Path, token_dir: Path, clock
) -> None:
    state = token_dir / "state"
    state.mkdir()
    (state / "lockout.json").write_text(
        json.dumps({"until": "2099-01-01T00:00:00+00:00", "until_epoch": clock.now() + 3600}),
        encoding="utf-8",
    )
    factory = Factory()
    code, _out, err = run(send_args(future_plan, tmp_path / "out", token_dir), factory, clock)
    assert code == EXIT_LOCKOUT
    assert "Sperre aktiv" in err
    assert factory.calls == []


def test_yes_with_a_corrupt_report_exits_2_without_a_call(
    tmp_path: Path, future_plan: Path, token_dir: Path, clock
) -> None:
    out_dir = tmp_path / "out"
    out_dir.mkdir()
    (out_dir / "push-report.json").write_text('{"report_version": 99, "entries": []}')
    factory = Factory()
    code, _out, err = run(send_args(future_plan, out_dir, token_dir), factory, clock)
    assert code == EXIT_USAGE
    assert "push-report.json" in err
    assert factory.calls == []


def test_budget_never_exceeds_two_calls_per_selected_session(
    tmp_path: Path, future_plan: Path, token_dir: Path, clock
) -> None:
    factory = Factory()
    run(send_args(future_plan, tmp_path / "out", token_dir, 5), factory, clock)
    assert len(factory.client.calls) == 10


def test_a_failed_scheduling_leaves_an_uploaded_entry_and_the_next_run_only_schedules(
    tmp_path: Path, future_plan: Path, token_dir: Path, clock
) -> None:
    out_dir = tmp_path / "out"
    state = {"fail": True}

    def schedule(workout_id: int, date_str: str) -> dict[str, Any]:
        if state["fail"] and workout_id == 1002:
            raise HttpError(422)  # Ablehnung der Anfrage: sicher nicht geplant
        return {"workoutScheduleId": 1}

    factory = Factory(workout_client(schedule_workout=schedule))
    code, out, _err = run(send_args(future_plan, out_dir, token_dir), factory, clock)
    assert code == EXIT_ERROR
    # Beim ersten Fehler wird abgebrochen: die dritte Einheit wurde nicht angefasst.
    assert factory.client.names_called() == [
        "upload_workout",
        "schedule_workout",
        "upload_workout",
        "schedule_workout",
    ]
    entries = json.loads((out_dir / "push-report.json").read_text(encoding="utf-8"))["entries"]
    assert [e["status"] for e in entries] == ["ok", "uploaded"]
    assert entries[1]["workout_id"] == 1002
    assert entries[1]["error_class"] == "HttpError"
    assert entries[1]["http_status"] == 422
    assert "Noch offen: 2" in out

    state["fail"] = False
    factory.client.calls.clear()
    code, _out, _err = run(send_args(future_plan, out_dir, token_dir), factory, clock)
    assert code == EXIT_OK
    # 1002 wird nur noch geplant (kein zweiter Upload), 1003 ganz neu.
    assert factory.client.names_called() == [
        "schedule_workout",
        "upload_workout",
        "schedule_workout",
    ]
    assert factory.client.calls[0][1] == (1002, "2027-01-06")
    entries = json.loads((out_dir / "push-report.json").read_text(encoding="utf-8"))["entries"]
    assert [e["status"] for e in entries] == ["ok", "ok", "ok"]


class HttpError(Exception):
    def __init__(self, status: int) -> None:
        super().__init__(f"HTTP {status}")
        self.response = type("R", (), {"status_code": status})()


def test_a_rejected_upload_is_an_error_and_is_retried_by_the_next_run(
    tmp_path: Path, future_plan: Path, token_dir: Path, clock
) -> None:
    out_dir = tmp_path / "out"
    calls = {"n": 0}

    def upload(payload: dict[str, Any]) -> dict[str, Any]:
        calls["n"] += 1
        if calls["n"] == 1:
            raise HttpError(400)
        return {"workoutId": 5000 + calls["n"]}

    factory = Factory(workout_client(upload_workout=upload))
    code, _out, _err = run(send_args(future_plan, out_dir, token_dir, 1), factory, clock)
    assert code == EXIT_ERROR
    assert factory.client.names_called() == ["upload_workout"]  # keine Wiederholung im Lauf
    entry = json.loads((out_dir / "push-report.json").read_text())["entries"][0]
    assert entry["status"] == "error"
    assert entry["http_status"] == 400
    assert "workout_id" not in entry
    code, _out, _err = run(send_args(future_plan, out_dir, token_dir, 1), factory, clock)
    assert code == EXIT_OK
    entry = json.loads((out_dir / "push-report.json").read_text())["entries"][0]
    assert entry["status"] == "ok"


@pytest.mark.parametrize("failure", [ConnectionError("Verbindung"), HttpError(502)])
def test_an_unclear_upload_is_never_repeated_automatically(
    failure: Exception, tmp_path: Path, future_plan: Path, token_dir: Path, clock
) -> None:
    out_dir = tmp_path / "out"
    factory = Factory(workout_client(upload_workout=failure))
    code, _out, _err = run(send_args(future_plan, out_dir, token_dir, 1), factory, clock)
    assert code == EXIT_ERROR
    assert factory.client.names_called() == ["upload_workout"]  # genau ein Versuch
    entry = json.loads((out_dir / "push-report.json").read_text())["entries"][0]
    assert entry["status"] == "unknown"
    assert entry["error_class"] == type(failure).__name__
    # Ein neuer Lauf legt diese Einheit nicht noch einmal an.
    factory2 = Factory()
    code, out, _err = run(send_args(future_plan, out_dir, token_dir, 1), factory2, clock)
    assert code == EXIT_ERROR  # braucht Aufmerksamkeit
    assert factory2.client.calls == []
    assert "Ergebnis unklar" in out


def _decode_errors() -> list[Exception]:
    """Alle Klassen, die ``resp.json()`` nach einem erfolgreichen POST werfen kann."""
    import requests.exceptions
    from curl_cffi.requests.exceptions import JSONDecodeError as CurlJSONDecodeError

    # Position 403 ergibt die Meldung "column 404": die darf nicht als Status 404 gelesen werden.
    doc = "x" * 600
    return [
        json.JSONDecodeError("Expecting value", doc, 403),
        requests.exceptions.JSONDecodeError("Expecting value", doc, 403),
        CurlJSONDecodeError("Expecting value", doc, 403),
        UnicodeDecodeError("utf-8", b"\xff", 0, 1, "invalid start byte"),
    ]


@pytest.mark.parametrize("index", range(4))
def test_decode_error_classes_inherit_from_value_error_and_are_found(index: int) -> None:
    from lastwerk_bridge.throttle import http_status_of, is_json_decode_error

    exc = _decode_errors()[index]
    assert isinstance(exc, ValueError)
    assert is_json_decode_error(exc)
    assert http_status_of(exc) is None
    assert push._classify_failure(exc) == "unknown"


@pytest.mark.parametrize("index", range(4))
def test_upload_with_a_response_that_is_not_json_is_unknown_and_not_sent_again(
    index: int, tmp_path: Path, future_plan: Path, token_dir: Path, clock
) -> None:
    out_dir = tmp_path / "out"
    failure = _decode_errors()[index]
    server: list[dict[str, Any]] = []  # Was bei Garmin angekommen ist

    def upload(payload: dict[str, Any]) -> dict[str, Any]:
        server.append(payload)  # Der POST gelingt, erst das Lesen der Antwort scheitert.
        raise failure

    factory = Factory(workout_client(upload_workout=upload))
    code, _out, _err = run(send_args(future_plan, out_dir, token_dir, 1), factory, clock)
    assert code == EXIT_ERROR
    assert len(server) == 1
    entry = json.loads((out_dir / "push-report.json").read_text())["entries"][0]
    assert entry["status"] == "unknown"
    assert entry["error_class"] == type(failure).__name__
    assert "http_status" not in entry
    # Der zweite Lauf sendet nichts und verlangt Aufmerksamkeit.
    code, out, _err = run(send_args(future_plan, out_dir, token_dir, 1), factory, clock)
    assert code == EXIT_ERROR
    assert len(server) == 1
    assert factory.client.names_called() == ["upload_workout"]
    assert "Ergebnis unklar" in out


@pytest.mark.parametrize("index", range(4))
def test_schedule_with_a_response_that_is_not_json_is_unknown_and_not_sent_again(
    index: int, tmp_path: Path, future_plan: Path, token_dir: Path, clock
) -> None:
    out_dir = tmp_path / "out"
    failure = _decode_errors()[index]
    scheduled: list[tuple[int, str]] = []

    def schedule(workout_id: int, date_str: str) -> dict[str, Any]:
        scheduled.append((workout_id, date_str))
        raise failure

    factory = Factory(workout_client(schedule_workout=schedule))
    code, _out, _err = run(send_args(future_plan, out_dir, token_dir, 1), factory, clock)
    assert code == EXIT_ERROR
    entry = json.loads((out_dir / "push-report.json").read_text())["entries"][0]
    assert entry["status"] == "unknown"
    assert entry["workout_id"] == 1001
    assert entry["error_class"] == type(failure).__name__
    code, out, _err = run(send_args(future_plan, out_dir, token_dir, 1), factory, clock)
    assert code == EXIT_ERROR
    assert factory.client.names_called() == ["upload_workout", "schedule_workout"]
    assert len(scheduled) == 1
    assert "Ergebnis unklar" in out


@pytest.mark.parametrize(
    "failure",
    [
        ValueError("irgendein Wert"),
        TypeError("irgendein Typ"),
        ConnectionError("HTTPSConnectionPool(host='x', port=443): Max retries exceeded"),
        OSError("Failed to connect to connectapi.garmin.com port 443 after 10 ms"),
    ],
)
def test_after_sending_only_a_clear_rejection_counts_as_error(failure: Exception) -> None:
    # ValueError und TypeError sind kein Beleg fuer "nicht gesendet", und "port 443" ist kein
    # HTTP-Status 443.
    assert push._classify_failure(failure) == "unknown"


def test_a_tampered_workout_id_is_stopped_before_the_call(
    tmp_path: Path, future_plan: Path, token_dir: Path, clock
) -> None:
    out_dir = tmp_path / "out"
    factory = Factory()
    run(send_args(future_plan, out_dir, token_dir, 1), factory, clock)
    report = out_dir / "push-report.json"
    data = json.loads(report.read_text())
    data["entries"][0].update(status="uploaded", workout_id="abc")
    report.write_text(json.dumps(data))
    factory.client.calls.clear()
    code, _out, _err = run(send_args(future_plan, out_dir, token_dir, 1), factory, clock)
    assert code == EXIT_ERROR
    assert factory.client.calls == []  # Die Bibliothek haette ValueError geworfen.
    entry = json.loads(report.read_text())["entries"][0]
    assert (entry["status"], entry["error_class"]) == ("uploaded", "PreSendError")


def test_a_response_without_workout_id_is_unclear(
    tmp_path: Path, future_plan: Path, token_dir: Path, clock
) -> None:
    out_dir = tmp_path / "out"
    factory = Factory(workout_client(upload_workout={"unexpected": True}))
    code, _out, _err = run(send_args(future_plan, out_dir, token_dir, 2), factory, clock)
    assert code == EXIT_ERROR
    assert factory.client.names_called() == ["upload_workout"]
    entry = json.loads((out_dir / "push-report.json").read_text())["entries"][0]
    assert (entry["status"], entry["error_class"]) == ("unknown", "NoWorkoutId")


def test_a_single_429_stops_the_run_and_counts_towards_the_lockout(
    tmp_path: Path, future_plan: Path, token_dir: Path, clock
) -> None:
    out_dir = tmp_path / "out"
    factory = Factory(workout_client(upload_workout=GarminConnectTooManyRequestsError("zu viele")))
    code, _out, _err = run(send_args(future_plan, out_dir, token_dir, 3), factory, clock)
    assert code == EXIT_ERROR
    assert factory.client.names_called() == ["upload_workout"]
    entry = json.loads((out_dir / "push-report.json").read_text())["entries"][0]
    assert (entry["status"], entry["http_status"]) == ("error", 429)
    assert entry["error_class"] == "BlockedResponseError"


def test_an_invalid_token_exits_2(
    tmp_path: Path, future_plan: Path, token_dir: Path, clock
) -> None:
    out_dir = tmp_path / "out"
    factory = Factory(workout_client(upload_workout=GarminConnectAuthenticationError("401")))
    code, _out, _err = run(send_args(future_plan, out_dir, token_dir, 2), factory, clock)
    assert code == EXIT_USAGE
    entry = json.loads((out_dir / "push-report.json").read_text())["entries"][0]
    assert entry["status"] == "error"
    assert entry["error_class"] == "AuthRequiredError"


def test_writes_are_spaced_by_the_throttle(
    tmp_path: Path, future_plan: Path, token_dir: Path, clock
) -> None:
    factory = Factory()
    run(send_args(future_plan, tmp_path / "out", token_dir, 2), factory, clock)
    # Vier Aufrufe, dazwischen mindestens 3 s (Schreibpfad, vorläufig, Faustregel).
    assert len(clock.sleeps) >= 3
    assert all(s >= 3.0 for s in clock.sleeps[:3])


def test_report_accumulates_across_plans(tmp_path: Path, token_dir: Path, clock) -> None:
    out_dir = tmp_path / "out"
    first = write_plan(
        tmp_path / "a.json", make_plan(plan_id="11111111-1111-4111-8111-111111111111")
    )
    second = write_plan(
        tmp_path / "b.json", make_plan(plan_id="22222222-2222-4222-8222-222222222222")
    )
    factory = Factory()
    run(send_args(first, out_dir, token_dir, 1), factory, clock)
    run(send_args(second, out_dir, token_dir, 1), factory, clock)
    entries = json.loads((out_dir / "push-report.json").read_text())["entries"]
    assert [e["plan_id"][0] for e in entries] == ["1", "2"]
    assert len(factory.client.calls) == 4


def test_nothing_selected_sends_nothing_and_writes_no_report(
    tmp_path: Path, future_plan: Path, token_dir: Path, clock
) -> None:
    factory = Factory()
    code, _out, err = run(
        send_args(future_plan, tmp_path / "out", token_dir, 3, "--from", "2030-01-01"),
        factory,
        clock,
    )
    assert code == EXIT_OK
    assert "keine Einheit gewählt" in err
    assert factory.calls == []
    assert not (tmp_path / "out").exists()
