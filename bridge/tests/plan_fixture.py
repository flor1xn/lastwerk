"""Synthetischer Plan nach ``schema/plan.schema.json`` für die Tests von ``push``.

Alles erfunden: zwei Wochen, mehrere Vorlagen aus allen Sportarten, ein Tag mit zwei
Einheiten, Ruhetage als Tage ohne Einheit. Die Lasten sind Platzhalter, ``push`` rechnet nicht
damit.
"""

from __future__ import annotations

import copy
import json
from datetime import date, timedelta
from pathlib import Path
from typing import Any

PLAN_ID = "3f2b8c1e-7a4d-4e55-9c0a-1d2e3f4a5b6c"

# (Wochentag 0 = Montag, Platz, Sportart, Vorlage, Dauer in Minuten, hart, RPE oder None)
WEEK_1: list[tuple[int, str, str, str, int, bool, int | None]] = [
    (1, "am", "cycling", "bike.threshold.2x20", 90, True, None),
    (2, "am", "running", "run.easy.steady", 40, False, None),
    (3, "am", "cycling", "bike.vo2.5x4", 80, True, None),
    (3, "pm", "strength", "strength.general.a", 30, False, 6),
    (4, "am", "swimming", "swim.technique.easy", 30, False, None),
    (5, "am", "cycling", "bike.endurance.steady", 150, False, None),
    (6, "am", "running", "run.long.steady", 90, False, None),
]
WEEK_2: list[tuple[int, str, str, str, int, bool, int | None]] = [
    (1, "am", "running", "run.intervals.6x3", 50, True, None),
    (2, "am", "cycling", "bike.tempo.blocks", 75, False, None),
    (3, "am", "swimming", "swim.threshold.sets", 50, True, None),
    (5, "am", "cycling", "bike.recovery.spin", 45, False, None),
    (6, "am", "running", "run.tempo.continuous", 50, True, None),
    (6, "pm", "strength", "strength.general.b", 40, False, 7),
]


def make_plan(
    start: date = date(2027, 1, 4), *, locale: str = "de", plan_id: str = PLAN_ID
) -> dict[str, Any]:
    """Gültiger Plan über zwei Wochen ab ``start`` (ein Montag)."""
    weeks = []
    for index, rows in enumerate((WEEK_1, WEEK_2)):
        week_start = start + timedelta(days=7 * index)
        days = []
        for offset in range(7):
            sessions = []
            for weekday, slot, sport, template_id, minutes, hard, rpe in rows:
                if weekday != offset:
                    continue
                session: dict[str, Any] = {
                    "slot": slot,
                    "sport": sport,
                    "template_id": template_id,
                    "duration_min": minutes,
                    "planned_load": round(minutes * 0.8, 1),
                    "hard": hard,
                }
                if rpe is not None:
                    session["rpe"] = rpe
                sessions.append(session)
            days.append(
                {"date": (week_start + timedelta(days=offset)).isoformat(), "sessions": sessions}
            )
        weeks.append(
            {
                "week_start": week_start.isoformat(),
                "phase": "build",
                "target_load": 300.0,
                "planned_load": 290.0,
                "days": days,
            }
        )
    return {
        "schema_version": 1,
        "plan_id": plan_id,
        "created_at": "2026-10-13T12:00:00Z",
        "locale": locale,
        "source": "rules",
        "rules_version": "1.0.0",
        "goal": {"kind": "continuous"},
        "weeks": weeks,
        "warnings": [],
    }


def write_plan(path: Path, plan: dict[str, Any] | None = None) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(plan or make_plan(), indent=2), encoding="utf-8")
    return path


def mutated(plan: dict[str, Any], *path: str | int, value: Any) -> dict[str, Any]:
    """Kopie des Plans, bei der der Wert unter ``path`` ersetzt (oder bei ``...`` entfernt) ist."""
    copy_ = copy.deepcopy(plan)
    node: Any = copy_
    for key in path[:-1]:
        node = node[key]
    if value is ...:
        del node[path[-1]]
    else:
        node[path[-1]] = value
    return copy_
