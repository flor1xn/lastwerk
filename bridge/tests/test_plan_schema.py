"""Eigene Schemaprüfung gegen ``schema/plan.schema.json`` (SPEC 7.1, 12 M6).

Die Negativfälle entsprechen denen der Rust-Seite (``crates/cli/src/schema_tests.rs``, T08):
jedes Pflichtfeld einzeln, unbekannte Felder auf allen Ebenen, Wertebereiche, Formate.
"""

from __future__ import annotations

import json
from typing import Any

import pytest

from lastwerk_bridge.manifest import (
    ManifestValidationError,
    default_schema_path,
    load_schema,
    unsupported_keywords,
    validate_value,
)
from lastwerk_bridge.push import default_plan_schema_path

from .plan_fixture import PLAN_ID, make_plan, mutated

SCHEMA_DIR = default_schema_path().parent


@pytest.fixture(scope="module")
def schema() -> dict[str, Any]:
    return load_schema(default_plan_schema_path())


def accepts(plan: dict[str, Any], schema: dict[str, Any]) -> bool:
    try:
        validate_value(plan, schema)
    except ManifestValidationError:
        return False
    return True


def test_every_schema_in_the_repository_uses_only_checked_keywords() -> None:
    """Ein Schlüsselwort ohne Prüfung in der eigenen Umsetzung wäre wirkungslos."""
    files = sorted(SCHEMA_DIR.glob("*.schema.json"))
    assert files, "keine Schemas gefunden"
    for file in files:
        if file.name in ("plan-edit.schema.json", "plan-chat.schema.json"):
            continue  # gehören zum Coach (T15, T26), push liest sie nicht
        assert unsupported_keywords(json.loads(file.read_text(encoding="utf-8"))) == [], file.name


def test_unsupported_keyword_detector_finds_unknown_keywords() -> None:
    found = unsupported_keywords(
        {"type": "object", "properties": {"a": {"oneOf": []}, "b": {"format": "email"}}}
    )
    assert found == ["$.properties.a.oneOf", "$.properties.b.format=email"]


def test_valid_plan_is_accepted(schema: dict[str, Any]) -> None:
    assert accepts(make_plan(), schema)


def test_event_goal_with_date_is_accepted(schema: dict[str, Any]) -> None:
    plan = mutated(make_plan(), "goal", value={"kind": "event", "event_date": "2027-05-01"})
    assert accepts(plan, schema)


@pytest.mark.parametrize(
    "key",
    [
        "schema_version",
        "plan_id",
        "created_at",
        "locale",
        "source",
        "rules_version",
        "goal",
        "weeks",
    ],
)
def test_missing_top_level_field_is_rejected(key: str, schema: dict[str, Any]) -> None:
    assert not accepts(mutated(make_plan(), key, value=...), schema)


@pytest.mark.parametrize(
    "path",
    [
        ("goal", "kind"),
        ("weeks", 0, "week_start"),
        ("weeks", 0, "phase"),
        ("weeks", 0, "target_load"),
        ("weeks", 0, "days"),
        ("weeks", 0, "days", 0, "date"),
        ("weeks", 0, "days", 0, "sessions"),
        ("weeks", 0, "days", 1, "sessions", 0, "slot"),
        ("weeks", 0, "days", 1, "sessions", 0, "sport"),
        ("weeks", 0, "days", 1, "sessions", 0, "template_id"),
        ("weeks", 0, "days", 1, "sessions", 0, "duration_min"),
        ("weeks", 0, "days", 1, "sessions", 0, "planned_load"),
        ("weeks", 0, "days", 1, "sessions", 0, "hard"),
    ],
)
def test_missing_nested_required_field_is_rejected(
    path: tuple[str | int, ...], schema: dict[str, Any]
) -> None:
    assert not accepts(mutated(make_plan(), *path, value=...), schema)


@pytest.mark.parametrize(
    "path",
    [
        (),
        ("goal",),
        ("weeks", 0),
        ("weeks", 0, "days", 0),
        ("weeks", 0, "days", 1, "sessions", 0),
        ("warnings",),
    ],
)
def test_unknown_field_is_rejected_on_every_level(
    path: tuple[str | int, ...], schema: dict[str, Any]
) -> None:
    plan = make_plan()
    plan["warnings"] = [{"code": "R01", "severity": "info"}]
    node: Any = plan
    for key in path:
        node = node[key]
    if isinstance(node, list):
        node = node[0]
    node["unexpected"] = 1
    assert not accepts(plan, schema)


def test_event_goal_without_date_is_rejected(schema: dict[str, Any]) -> None:
    assert not accepts(mutated(make_plan(), "goal", value={"kind": "event"}), schema)


def test_day_and_session_counts_are_checked(schema: dict[str, Any]) -> None:
    plan = make_plan()
    short = mutated(plan, "weeks", 0, "days", value=plan["weeks"][0]["days"][:6])
    long = mutated(
        plan, "weeks", 0, "days", value=plan["weeks"][0]["days"] + plan["weeks"][1]["days"][:1]
    )
    assert not accepts(short, schema)
    assert not accepts(long, schema)
    three = mutated(
        plan, "weeks", 0, "days", 3, "sessions", value=plan["weeks"][0]["days"][3]["sessions"] * 2
    )
    assert not accepts(three, schema)


def test_week_count_is_checked(schema: dict[str, Any]) -> None:
    plan = make_plan()
    assert not accepts(mutated(plan, "weeks", value=[]), schema)
    assert not accepts(mutated(plan, "weeks", value=plan["weeks"] * 14), schema)  # 28 Wochen
    assert accepts(mutated(plan, "weeks", value=plan["weeks"] * 13), schema)  # 26 Wochen


@pytest.mark.parametrize(
    ("path", "value"),
    [
        (("schema_version",), 2),
        (("schema_version",), True),
        (("schema_version",), "1"),
        (("plan_id",), "kein-uuid"),
        (("created_at",), "gestern"),
        (("created_at",), "2026-10-13"),
        (("created_at",), "2026-10-13T12:00:00"),
        (("locale",), "fr"),
        (("source",), "cloud"),
        (("rules_version",), "1.0"),
        (("goal", "kind"), "weekly"),
        (("weeks", 0, "week_start"), "2027-1-4"),
        (("weeks", 0, "week_start"), "2027-02-30"),
        (("weeks", 0, "phase"), "sprint"),
        (("weeks", 0, "target_load"), -1),
        (("weeks", 0, "target_load"), "300"),
        (("weeks", 0, "days", 0, "date"), "04.01.2027"),
        (("weeks", 0, "days", 1, "sessions", 0, "slot"), "noon"),
        (("weeks", 0, "days", 1, "sessions", 0, "sport"), "golf"),
        (("weeks", 0, "days", 1, "sessions", 0, "template_id"), "bike"),
        (("weeks", 0, "days", 1, "sessions", 0, "template_id"), "Bike.Threshold"),
        (("weeks", 0, "days", 1, "sessions", 0, "duration_min"), 9),
        (("weeks", 0, "days", 1, "sessions", 0, "duration_min"), 361),
        (("weeks", 0, "days", 1, "sessions", 0, "duration_min"), 45.5),
        (("weeks", 0, "days", 1, "sessions", 0, "duration_min"), True),
        (("weeks", 0, "days", 1, "sessions", 0, "hard"), "ja"),
        (("weeks", 0, "days", 1, "sessions", 0, "rpe"), 0),
        (("weeks", 0, "days", 1, "sessions", 0, "rpe"), 11),
        (("explanation",), "x" * 1201),
    ],
)
def test_out_of_range_value_is_rejected(
    path: tuple[str | int, ...], value: Any, schema: dict[str, Any]
) -> None:
    assert not accepts(mutated(make_plan(), *path, value=value), schema)


def test_boundary_values_are_accepted(schema: dict[str, Any]) -> None:
    plan = make_plan()
    plan = mutated(plan, "weeks", 0, "days", 1, "sessions", 0, "duration_min", value=10)
    plan = mutated(plan, "weeks", 0, "days", 2, "sessions", 0, "duration_min", value=360)
    plan = mutated(plan, "weeks", 0, "days", 2, "sessions", 0, "rpe", value=10)
    plan = mutated(plan, "explanation", value="x" * 1200)
    assert accepts(plan, schema)


def test_warning_codes_are_checked(schema: dict[str, Any]) -> None:
    plan = make_plan()
    ok = mutated(
        plan, "warnings", value=[{"code": "R12", "severity": "info", "date": "2027-01-04"}]
    )
    bad_code = mutated(plan, "warnings", value=[{"code": "r12", "severity": "info"}])
    bad_severity = mutated(plan, "warnings", value=[{"code": "R12", "severity": "error"}])
    assert accepts(ok, schema)
    assert not accepts(bad_code, schema)
    assert not accepts(bad_severity, schema)


def test_plan_id_constant_is_a_valid_uuid(schema: dict[str, Any]) -> None:
    assert make_plan()["plan_id"] == PLAN_ID
    assert accepts(make_plan(), schema)


def test_ref_to_unknown_definition_is_an_error() -> None:
    with pytest.raises(ManifestValidationError):
        validate_value(1, {"$ref": "#/$defs/missing", "$defs": {}})
    with pytest.raises(ManifestValidationError):
        validate_value(1, {"$ref": "other.json#/x"})


def test_schema_content_alone_decides() -> None:
    """Das Schema bleibt die einzige Quelle: eine geänderte Kopie ändert das Ergebnis."""
    copy = load_schema(default_plan_schema_path())
    copy["properties"]["locale"] = {"enum": ["de"]}
    assert not accepts(make_plan(locale="en"), copy)
    assert accepts(make_plan(locale="de"), copy)
