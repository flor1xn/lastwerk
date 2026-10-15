"""Workouts aus einem Plan nach Garmin Connect übertragen (SPEC 4.2 F22, 5.4, 5.5, 12 M6).

Zwei Wege, strikt getrennt:

- **Trockenlauf** (Standard): ``plan.json`` prüfen, je Einheit die Nutzlast für ein Garmin-Connect-
  Workout aus der Vorlage berechnen und als JSON ausgeben. Kein Client, keine Anmeldung, keine
  Netzwerkanfrage. Dieses Modul importiert dafür nichts aus ``garminconnect``.
- **Echter Versand** (``--yes``): ``WorkoutWriteCaller`` mit der eigenen Freigabe
  ``WORKOUT_WRITE_ALLOWLIST`` (``upload_workout``, ``schedule_workout``), gedrosselt, ohne
  automatische Wiederholung, Protokoll in ``exchange/out/push-report.json``. Er ist nur mit
  Attrappen getestet und wurde nie gegen ein echtes Konto ausgeführt.

Die Struktur der Nutzlast folgt der Dokumentation und dem Quelltext von python-garminconnect
0.3.17 (Q21, Q27: ``garminconnect/workout.py``, ``upload_workout``, ``schedule_workout``,
Stand 14. Oktober 2026). **Sie ist bis zum ersten echten Versand eine Annahme** (O4 analog):
Garmin dokumentiert die Schnittstelle nicht, und die Zonennummern verweisen auf die im
Garmin-Profil des Nutzers hinterlegten Zonen, nicht auf die Grenzen aus ``catalog/zones.json``.

Dauern (Konvention wie im Planer, SPEC 7.4): Intervallblöcke (Arbeit und Pausen zwischen den
Wiederholungen) behalten ihre Länge, Aufwärmen, gleichmäßige Schritte und Abwärmen werden im
Verhältnis ihrer Nennzeit auf ``duration_min`` gedehnt oder gestaucht, auf ganze Minuten
gerundet (Rest an die größten Nachkommaanteile). So ist die Summe der Schritte genau
``duration_min``. Wiederholungen sind eine ``RepeatGroupDTO`` über Arbeit und Pause, die letzte
Arbeit steht ohne Pause dahinter, weil der Planer nur die Pausen *zwischen* den Wiederholungen
zählt.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from datetime import UTC, date, datetime
from fractions import Fraction
from math import floor
from pathlib import Path
from typing import Any

from .allowlist import MethodNotAllowedError, ensure_write_allowed
from .archive import write_bytes_atomic
from .manifest import ManifestValidationError, load_schema, validate_value
from .manifest import default_schema_path as _manifest_schema_path
from .throttle import (
    AuthRequiredError,
    BlockedResponseError,
    BridgeError,
    CallBudgetExhausted,
    Clock,
    LockoutError,
    ThrottledCaller,
    http_status_of,
    is_json_decode_error,
)

PLAN_SCHEMA_FILENAME = "plan.schema.json"
DRY_RUN_REPORT_NAME = "push-dry-run.json"
PUSH_REPORT_NAME = "push-report.json"
REPORT_VERSION = 1

# Größenlimit für Eingaben (Faustregel). Ein Plan über 26 Wochen mit zwei Einheiten je Tag
# hat rund 150 KB.
MAX_INPUT_BYTES = 1024 * 1024

ASSUMPTION_NOTE = (
    "Struktur der Nutzlast nach python-garminconnect 0.3.17 (Q21, Q27), bis zum ersten echten "
    "Versand eine Annahme. Zonennummern verweisen auf die Zonen im Garmin-Profil."
)

DEFAULT_RPE = 6  # nur Rückfall, falls weder Einheit noch Vorlage einen Wert tragen (Konvention)


def write_json(path: Path, data: dict[str, Any]) -> None:
    """Atomar, UTF-8, lesbar eingerückt."""
    text = json.dumps(data, indent=2, ensure_ascii=False) + "\n"
    write_bytes_atomic(Path(path), text.encode("utf-8"))


class PushInputError(BridgeError):
    """Plan, Katalog oder Optionen sind unbrauchbar. Exit-Code 2, nichts wird gesendet."""


class PreSendError(BridgeError):
    """Eigene Prüfung vor dem Senden schlägt fehl. Die Anfrage wurde sicher nicht gesendet."""


# Pfade


def repo_root() -> Path:
    """Wurzel des Repositories relativ zur Paketdatei (``bridge/src/lastwerk_bridge/``)."""
    return Path(__file__).resolve().parents[3]


def default_templates_path() -> Path:
    return repo_root() / "catalog" / "templates.json"


def default_locales_dir(templates_path: Path | None = None) -> Path:
    """``locales/`` neben dem Katalog (``catalog/locales``)."""
    return (templates_path or default_templates_path()).parent / "locales"


def default_plan_schema_path() -> Path:
    return _manifest_schema_path().with_name(PLAN_SCHEMA_FILENAME)


# Eingaben lesen und prüfen


def _read_json(path: Path, what: str) -> Any:
    """Liest eine JSON-Datei mit Größenlimit. NaN und Infinity sind kein JSON."""
    try:
        with open(path, "rb") as fh:
            raw = fh.read(MAX_INPUT_BYTES + 1)
    except OSError as exc:
        raise PushInputError(f"{what} nicht lesbar: {path} ({type(exc).__name__})") from exc
    if len(raw) > MAX_INPUT_BYTES:
        raise PushInputError(f"{what} ist größer als {MAX_INPUT_BYTES} Byte: {path}")

    def reject_constant(name: str) -> Any:
        raise ValueError(f"unzulässige Zahl {name}")

    try:
        return json.loads(raw.decode("utf-8"), parse_constant=reject_constant)
    except (UnicodeDecodeError, ValueError) as exc:
        raise PushInputError(f"{what} ist kein gültiges JSON: {path} ({exc})") from exc


def load_plan(path: Path, schema_path: Path | None = None) -> dict[str, Any]:
    """Liest ``plan.json`` und prüft es gegen ``schema/plan.schema.json``.

    Geprüft werden Größe, JSON, Schema (Pflichtfelder, unbekannte Felder, Wertebereiche,
    Formate) und ``schema_version`` 1. Bei jedem Verstoß ``PushInputError``.
    """
    plan = _read_json(Path(path), "plan.json")
    schema_file = schema_path or default_plan_schema_path()
    try:
        schema = load_schema(schema_file)
    except (OSError, ValueError) as exc:
        raise PushInputError(f"Plan-Schema nicht lesbar: {schema_file} ({exc})") from exc
    try:
        validate_value(plan, schema)
    except ManifestValidationError as exc:
        raise PushInputError(f"plan.json entspricht nicht dem Schema: {exc}") from exc
    version = plan.get("schema_version")
    if isinstance(version, bool) or version != 1:
        raise PushInputError(f"plan.json: schema_version {version!r} wird nicht unterstützt")
    return plan  # type: ignore[no-any-return]


ENDURANCE_SCHEMES = {"power7", "run_speed5", "swim_css5", "hr_lthr5", "hr_karvonen"}
STEP_KINDS = ("warmup", "steady", "interval", "cooldown")
TEMPLATE_SPORTS = ("cycling", "running", "swimming", "strength", "rest")


@dataclass(frozen=True)
class TemplateStep:
    kind: str
    minutes: int
    zone_scheme: str | None = None
    zone: int | None = None
    reps: int = 1
    rest_minutes: int = 0
    rpe: int | None = None


@dataclass(frozen=True)
class Template:
    id: str
    sport: str
    min_minutes: int
    max_minutes: int
    steps: tuple[TemplateStep, ...]
    rpe: int | None = None

    @property
    def fixed_minutes(self) -> int:
        """Minuten der Intervallblöcke: Arbeit mal Wiederholungen plus Pausen dazwischen."""
        return sum(
            s.reps * s.minutes + max(s.reps - 1, 0) * s.rest_minutes
            for s in self.steps
            if s.kind == "interval"
        )

    @property
    def flexible_minutes(self) -> int:
        return sum(s.minutes for s in self.steps if s.kind != "interval")


def _int(value: Any, what: str, *, low: int = 0, high: int = 10_000) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not low <= value <= high:
        raise PushInputError(f"Katalog: {what} muss eine ganze Zahl von {low} bis {high} sein")
    return value


def _parse_template(raw: Any) -> Template:
    if not isinstance(raw, dict) or not isinstance(raw.get("id"), str):
        raise PushInputError("Katalog: Vorlage ohne id")
    tid = raw["id"]
    sport = raw.get("sport")
    if sport not in TEMPLATE_SPORTS:
        raise PushInputError(f"Katalog: {tid}: unbekannte Sportart {sport!r}")
    window = raw.get("duration_min")
    if not isinstance(window, dict):
        raise PushInputError(f"Katalog: {tid}: duration_min fehlt")
    steps_raw = raw.get("steps")
    if not isinstance(steps_raw, list):
        raise PushInputError(f"Katalog: {tid}: steps fehlt")
    steps: list[TemplateStep] = []
    for item in steps_raw:
        if not isinstance(item, dict) or item.get("kind") not in STEP_KINDS:
            raise PushInputError(f"Katalog: {tid}: unbekannter Schritt")
        scheme = item.get("zone_scheme")
        if scheme is not None and scheme not in ENDURANCE_SCHEMES:
            raise PushInputError(f"Katalog: {tid}: unbekanntes Zonenschema {scheme!r}")
        zone = item.get("zone")
        steps.append(
            TemplateStep(
                kind=item["kind"],
                minutes=_int(item.get("minutes"), f"{tid}: minutes", low=1),
                zone_scheme=scheme,
                zone=None if zone is None else _int(zone, f"{tid}: zone", low=1, high=7),
                reps=_int(item.get("reps", 1), f"{tid}: reps", low=1, high=100),
                rest_minutes=_int(item.get("rest_minutes", 0), f"{tid}: rest_minutes"),
                rpe=None
                if item.get("rpe") is None
                else _int(item["rpe"], f"{tid}: rpe", low=1, high=10),
            )
        )
    rpe = raw.get("rpe")
    return Template(
        id=tid,
        sport=sport,
        min_minutes=_int(window.get("min"), f"{tid}: duration_min.min"),
        max_minutes=_int(window.get("max"), f"{tid}: duration_min.max"),
        steps=tuple(steps),
        rpe=None if rpe is None else _int(rpe, f"{tid}: rpe", low=1, high=10),
    )


def load_templates(path: Path | None = None) -> dict[str, Template]:
    """Liest ``catalog/templates.json`` (Format ``{format_version, templates}``)."""
    data = _read_json(Path(path or default_templates_path()), "Vorlagenkatalog")
    if not isinstance(data, dict) or data.get("format_version") != 1:
        raise PushInputError("Katalog: format_version 1 erwartet")
    entries = data.get("templates")
    if not isinstance(entries, list):
        raise PushInputError("Katalog: templates fehlt")
    templates: dict[str, Template] = {}
    for entry in entries:
        template = _parse_template(entry)
        if template.id in templates:
            raise PushInputError(f"Katalog: doppelte id {template.id}")
        templates[template.id] = template
    return templates


def load_locale(locales_dir: Path, locale: str) -> dict[str, Any]:
    """Texte der Plan-Sprache. Fehlt die Datei, bleibt der Titel die Vorlagen-ID."""
    path = Path(locales_dir) / f"{locale}.json"
    if not path.is_file():
        return {}
    data = _read_json(path, f"Texte ({locale})")
    return data if isinstance(data, dict) else {}


def template_title(texts: dict[str, Any], template_id: str) -> str:
    """Titel aus ``template.<id>.name``. Die Schlüssel sind verschachtelt, je Punkt eine Ebene."""
    node: Any = texts
    for part in ("template", *template_id.split("."), "name"):
        if not isinstance(node, dict) or part not in node:
            return template_id
        node = node[part]
    return node if isinstance(node, str) and node else template_id


# Nutzlast

# Sportarten: Kennung, Schlüssel, Anzeigereihenfolge nach garminconnect/workout.py (``SportType``,
# ``RunningWorkout`` usw.). Annahme bis zum ersten Versand.
SPORT_TYPES: dict[str, dict[str, Any]] = {
    "cycling": {"sportTypeId": 2, "sportTypeKey": "cycling", "displayOrder": 2},
    "running": {"sportTypeId": 1, "sportTypeKey": "running", "displayOrder": 1},
    "swimming": {"sportTypeId": 4, "sportTypeKey": "swimming", "displayOrder": 3},
    "strength": {"sportTypeId": 5, "sportTypeKey": "strength_training", "displayOrder": 5},
}

# Schritttypen nach ``StepType`` in workout.py (Kennung, Schlüssel, Anzeigereihenfolge).
STEP_TYPES: dict[str, dict[str, Any]] = {
    "warmup": {"stepTypeId": 1, "stepTypeKey": "warmup", "displayOrder": 1},
    "cooldown": {"stepTypeId": 2, "stepTypeKey": "cooldown", "displayOrder": 2},
    "interval": {"stepTypeId": 3, "stepTypeKey": "interval", "displayOrder": 3},
    "recovery": {"stepTypeId": 4, "stepTypeKey": "recovery", "displayOrder": 4},
    "rest": {"stepTypeId": 5, "stepTypeKey": "rest", "displayOrder": 5},
    "repeat": {"stepTypeId": 6, "stepTypeKey": "repeat", "displayOrder": 6},
}

END_CONDITION_TIME: dict[str, Any] = {
    "conditionTypeId": 2,
    "conditionTypeKey": "time",
    "displayOrder": 2,
    "displayable": True,
}
END_CONDITION_ITERATIONS: dict[str, Any] = {
    "conditionTypeId": 7,
    "conditionTypeKey": "iterations",
    "displayOrder": 7,
    "displayable": False,
}

# Zieltypen nach ``TargetType`` in workout.py. Die Bibliothek setzt displayOrder 1.
TARGET_TYPES: dict[str, dict[str, Any]] = {
    "no.target": {"workoutTargetTypeId": 1, "workoutTargetTypeKey": "no.target", "displayOrder": 1},
    "power.zone": {
        "workoutTargetTypeId": 2,
        "workoutTargetTypeKey": "power.zone",
        "displayOrder": 1,
    },
    "heart.rate.zone": {
        "workoutTargetTypeId": 4,
        "workoutTargetTypeKey": "heart.rate.zone",
        "displayOrder": 1,
    },
    "pace.zone": {
        "workoutTargetTypeId": 6,
        "workoutTargetTypeKey": "pace.zone",
        "displayOrder": 1,
    },
}

# Zonenschema der Vorlage (``catalog/zones.json``) auf den Zieltyp. Annahme: Rad Leistungszone,
# Lauf und Schwimmen Tempozone, Herzfrequenzschemata Herzfrequenzzone. Ob Garmin bei
# ``pace.zone`` die Zonennummer annimmt, zeigt erst der erste echte Versand.
SCHEME_TARGET: dict[str, str] = {
    "power7": "power.zone",
    "run_speed5": "pace.zone",
    "swim_css5": "pace.zone",
    "hr_lthr5": "heart.rate.zone",
    "hr_karvonen": "heart.rate.zone",
}

# Intervallpausen zählen im Planer als Zone 1 (SPEC 7.4).
RECOVERY_ZONE = 1


class _Builder:
    """Vergibt ``stepOrder`` fortlaufend über alle Schritte, auch in Wiederholungen."""

    def __init__(self) -> None:
        self.order = 0

    def _next(self) -> int:
        self.order += 1
        return self.order

    def step(
        self,
        kind: str,
        seconds: int,
        scheme: str | None = None,
        zone: int | None = None,
    ) -> dict[str, Any]:
        target_key = SCHEME_TARGET.get(scheme or "", "no.target") if zone is not None else None
        step: dict[str, Any] = {
            "type": "ExecutableStepDTO",
            "stepOrder": self._next(),
            "stepType": dict(STEP_TYPES[kind]),
            "endCondition": dict(END_CONDITION_TIME),
            "endConditionValue": float(seconds),
            "targetType": dict(TARGET_TYPES[target_key or "no.target"]),
        }
        if zone is not None and target_key is not None and target_key != "no.target":
            step["zoneNumber"] = zone
        return step

    def repeat(self, iterations: int, build_children: Any) -> dict[str, Any]:
        order = self._next()
        children = build_children()
        return {
            "type": "RepeatGroupDTO",
            "stepOrder": order,
            "stepType": dict(STEP_TYPES["repeat"]),
            "numberOfIterations": iterations,
            "workoutSteps": children,
            "endCondition": dict(END_CONDITION_ITERATIONS),
            "endConditionValue": float(iterations),
        }


def stretched_minutes(template: Template, duration_min: int) -> list[int]:
    """Minuten der dehnbaren Schritte (alle außer Intervall) bei ``duration_min``.

    Konvention des Planers (SPEC 7.4): Anteile an der Nennzeit der dehnbaren Schritte, Summe
    ``duration_min - fixed``. Ganze Minuten, Rest an die größten Nachkommaanteile (bei
    Gleichstand an den längeren, dann den früheren Schritt). Reicht ``duration_min`` nicht
    für die Intervallblöcke, bleiben die dehnbaren Schritte bei 0.
    """
    flexible = [s.minutes for s in template.steps if s.kind != "interval"]
    nominal = sum(flexible)
    if nominal == 0:
        return []
    available = max(duration_min - template.fixed_minutes, 0)
    raw = [Fraction(m * available, nominal) for m in flexible]
    base = [floor(r) for r in raw]
    rest = available - sum(base)
    order = sorted(range(len(raw)), key=lambda i: (-(raw[i] - base[i]), -flexible[i], i))
    for i in order[:rest]:
        base[i] += 1
    return base


def _total_seconds(steps: list[dict[str, Any]]) -> int:
    total = 0
    for step in steps:
        if step["type"] == "RepeatGroupDTO":
            total += step["numberOfIterations"] * _total_seconds(step["workoutSteps"])
        else:
            total += int(step["endConditionValue"])
    return total


def build_steps(template: Template, duration_min: int) -> list[dict[str, Any]]:
    """Schritte der Nutzlast für eine Einheit nach ``template`` mit ``duration_min``."""
    builder = _Builder()
    if template.sport == "strength":
        # Kraft: ein Schritt mit Zeitvorgabe, ohne Ziel (der RPE steht im Titel).
        return [builder.step("interval", duration_min * 60)]
    steps: list[dict[str, Any]] = []
    flex_iter = iter(stretched_minutes(template, duration_min))
    for tstep in template.steps:
        if tstep.kind != "interval":
            minutes = next(flex_iter)
            if minutes > 0:
                kind = "interval" if tstep.kind == "steady" else tstep.kind
                steps.append(builder.step(kind, minutes * 60, tstep.zone_scheme, tstep.zone))
            continue
        work = (tstep.minutes * 60, tstep.zone_scheme, tstep.zone)
        pause = (tstep.rest_minutes * 60, tstep.zone_scheme, RECOVERY_ZONE)
        if tstep.reps == 1:
            steps.append(builder.step("interval", *work))
        elif tstep.rest_minutes == 0:
            steps.append(builder.repeat(tstep.reps, lambda w=work: [builder.step("interval", *w)]))
        elif tstep.reps - 1 == 1:
            steps.append(builder.step("interval", *work))
            steps.append(builder.step("recovery", *pause))
            steps.append(builder.step("interval", *work))
        else:
            steps.append(
                builder.repeat(
                    tstep.reps - 1,
                    lambda w=work, p=pause: [
                        builder.step("interval", *w),
                        builder.step("recovery", *p),
                    ],
                )
            )
            steps.append(builder.step("interval", *work))
    return steps


def build_payload(template: Template, duration_min: int, title: str) -> dict[str, Any]:
    """Nutzlast für ``upload_workout`` (Struktur: Annahme bis zum ersten Versand, Q21, Q27)."""
    sport = SPORT_TYPES[template.sport]
    steps = build_steps(template, duration_min)
    return {
        "workoutName": title,
        "description": f"Lastwerk {template.id}, {duration_min} min",
        "sportType": dict(sport),
        "estimatedDurationInSecs": _total_seconds(steps),
        "workoutSegments": [
            {"segmentOrder": 1, "sportType": dict(sport), "workoutSteps": steps},
        ],
    }


# Einheiten des Plans auswählen


@dataclass(frozen=True)
class PlannedWorkout:
    key: str
    date: str
    slot: str
    template_id: str
    sport: str
    duration_min: int
    title: str
    payload: dict[str, Any]
    warnings: tuple[str, ...] = ()

    def summary(self) -> dict[str, Any]:
        return {
            "date": self.date,
            "slot": self.slot,
            "template_id": self.template_id,
            "sport": self.sport,
            "duration_min": self.duration_min,
            "title": self.title,
            "warnings": list(self.warnings),
            "payload": self.payload,
        }


@dataclass
class Selection:
    workouts: list[PlannedWorkout] = field(default_factory=list)
    skipped: list[dict[str, str]] = field(default_factory=list)
    outside: int = 0  # gültige Einheiten außerhalb von --from, --to oder --max-workouts


def _to_date(text: str) -> date:
    return datetime.strptime(text, "%Y-%m-%d").date()


def select_workouts(
    plan: dict[str, Any],
    templates: dict[str, Template],
    texts: dict[str, Any],
    *,
    from_date: date | None = None,
    to_date: date | None = None,
    max_workouts: int | None = None,
) -> Selection:
    """Wählt Einheiten (ohne Ruhetage) in Planreihenfolge und baut ihre Nutzlast.

    Eine unbekannte Vorlage oder eine Vorlage mit anderer Sportart als die Einheit ist ein
    Fehler des ganzen Plans (``PushInputError``), bevor etwas gesendet wird.
    """
    selection = Selection()
    plan_id = plan["plan_id"]
    days = [day for week in plan["weeks"] for day in week["days"]]
    days.sort(key=lambda d: d["date"])
    for day in days:
        for position, session in enumerate(day["sessions"]):
            tid = session["template_id"]
            if session["sport"] == "rest" or tid == "rest.day":
                selection.skipped.append(
                    {
                        "date": day["date"],
                        "slot": session["slot"],
                        "template_id": tid,
                        "reason": "rest",
                    }
                )
                continue
            template = templates.get(tid)
            if template is None:
                raise PushInputError(f"Vorlage {tid} ({day['date']}) fehlt im Katalog")
            if template.sport != session["sport"]:
                raise PushInputError(
                    f"Vorlage {tid} ({day['date']}) gehört zur Sportart {template.sport}, "
                    f"die Einheit nennt {session['sport']}"
                )
            when = _to_date(day["date"])
            if (from_date and when < from_date) or (to_date and when > to_date):
                selection.outside += 1
                continue
            if max_workouts is not None and len(selection.workouts) >= max_workouts:
                selection.outside += 1
                continue
            duration = session["duration_min"]
            rpe = session.get("rpe") or template.rpe or DEFAULT_RPE
            title = template_title(texts, tid)
            if template.sport == "strength":
                title = f"{title} (RPE {rpe})"
            warnings: list[str] = []
            if not template.min_minutes <= duration <= template.max_minutes:
                warnings.append("duration_outside_template_window")
            payload = build_payload(template, duration, title)
            if payload["estimatedDurationInSecs"] != duration * 60:
                warnings.append("duration_mismatch")
            selection.workouts.append(
                PlannedWorkout(
                    key=f"{plan_id}:{day['date']}:{position}",
                    date=day["date"],
                    slot=session["slot"],
                    template_id=tid,
                    sport=session["sport"],
                    duration_min=duration,
                    title=title,
                    payload=payload,
                    warnings=tuple(warnings),
                )
            )
    return selection


# Trockenlauf


def dry_run_document(
    plan: dict[str, Any],
    selection: Selection,
    *,
    from_date: date | None,
    to_date: date | None,
    max_workouts: int | None,
    today: date,
) -> dict[str, Any]:
    """Ausgabe des Trockenlaufs. Enthält die vollständigen Nutzlasten, sendet nichts."""
    workouts = []
    for item in selection.workouts:
        entry = item.summary()
        entry["in_past"] = _to_date(item.date) < today
        workouts.append(entry)
    return {
        "dry_run": True,
        "sends_nothing": True,
        "plan_id": plan["plan_id"],
        "locale": plan["locale"],
        "assumption": ASSUMPTION_NOTE,
        "selection": {
            "from": from_date.isoformat() if from_date else None,
            "to": to_date.isoformat() if to_date else None,
            "max_workouts": max_workouts,
        },
        "counts": {
            "workouts": len(selection.workouts),
            "skipped": len(selection.skipped),
            "outside_selection": selection.outside,
        },
        "workouts": workouts,
        "skipped": selection.skipped,
    }


# Echter Versand


class WorkoutWriteCaller(ThrottledCaller):
    """Gedrosselter Aufrufer mit der Freigabe ``WORKOUT_WRITE_ALLOWLIST``.

    Er erlaubt genau ``upload_workout`` und ``schedule_workout``, auch keine lesenden Methoden.
    Der lesende ``ThrottledCaller`` lehnt beide weiterhin ab (Sperrliste).
    """

    def _ensure_method_allowed(self, method: str) -> None:
        ensure_write_allowed(method)


@dataclass
class SendResult:
    uploaded: int = 0
    scheduled: int = 0
    already_sent: int = 0
    blocked_unknown: int = 0
    errors: int = 0
    stop_reason: str | None = None  # None, "lockout", "auth", "error", "budget"
    stop_message: str | None = None
    pending: int = 0  # Einheiten, die wegen des Abbruchs nicht mehr versucht wurden


def _iso_now(clock: Clock) -> str:
    return datetime.fromtimestamp(clock.now(), tz=UTC).isoformat(timespec="seconds")


def load_report(path: Path) -> dict[str, Any]:
    """Liest ein vorhandenes ``push-report.json``. Ist es unbrauchbar, wird nichts überschrieben."""
    if not path.exists():
        return {"report_version": REPORT_VERSION, "entries": []}
    data = _read_json(path, "push-report.json")
    if (
        not isinstance(data, dict)
        or data.get("report_version") != REPORT_VERSION
        or not isinstance(data.get("entries"), list)
    ):
        raise PushInputError(
            f"{path} ist kein Bericht dieser Version. Datei prüfen oder verschieben, "
            "es wurde nichts gesendet."
        )
    return data


def _classify_failure(exc: BaseException) -> str:
    """``error`` (die Anfrage ist sicher nicht angekommen) oder ``unknown`` (kann angekommen sein).

    Grundsatz: im Zweifel ``unknown``, denn ``error`` wird beim nächsten Lauf erneut gesendet
    und kann ein Workout doppelt anlegen (SPEC 10.2). ``error`` gilt nur, wenn der Fehler vor
    dem Senden entstand (eigene Prüfung ``PreSendError``, Freigabeliste, Sperre, Budget) oder die
    Antwort eine Ablehnung war (401 als ``AuthRequiredError``, 429 und 403 als
    ``BlockedResponseError``, übrige 4xx).

    ``ValueError`` und ``TypeError`` sind kein Beleg für "nicht gesendet": die Bibliothek ruft
    nach einem erfolgreichen POST ``resp.json()`` auf, und eine 2xx-Antwort ohne JSON wirft dort
    ``JSONDecodeError`` (``ValueError``) oder die gleichnamigen Klassen von ``requests`` und
    ``curl_cffi``. Das Workout existiert dann schon. Die Prüfung auf Dekodierfehler steht vor
    jeder Statusauswertung, weil deren Meldung Spaltenzahlen enthält.
    """
    if isinstance(
        exc,
        (
            BlockedResponseError,
            LockoutError,
            AuthRequiredError,
            MethodNotAllowedError,
            PreSendError,
        ),
    ):
        return "error"
    if is_json_decode_error(exc):
        return "unknown"
    status = http_status_of(exc)
    if status is not None and 400 <= status < 500:
        return "error"
    return "unknown"


def _check_before_send(stage: str, item: PlannedWorkout, entry: dict[str, Any]) -> None:
    """Prüft vor dem Aufruf, was die Bibliothek mit ``ValueError`` oder ``TypeError`` ablehnt.

    So bleibt jedes ``ValueError`` aus dem Aufruf selbst ein Zeichen für "kann gesendet sein".
    Das Protokoll kann von Hand verändert worden sein, deshalb wird die Kennung aus der Datei
    ebenfalls geprüft.
    """
    if stage == "upload":
        if not isinstance(item.payload, dict) or not item.payload:
            raise PreSendError("Nutzlast ist kein JSON-Objekt")
        return
    if _workout_id({"workoutId": entry.get("workout_id")}) is None:
        raise PreSendError("workout_id im Protokoll ist keine positive ganze Zahl")
    try:
        valid = _to_date(item.date).isoformat() == item.date
    except ValueError:
        valid = False
    if not valid:
        raise PreSendError(f"Datum {item.date!r} ist nicht im Format JJJJ-MM-TT")


def _workout_id(response: Any) -> int | None:
    value = response.get("workoutId") if isinstance(response, dict) else None
    if isinstance(value, bool):
        return None
    if isinstance(value, int) and value > 0:
        return value
    if isinstance(value, str) and value.isdigit() and int(value) > 0:
        return int(value)
    return None


def send_workouts(
    selection: Selection,
    caller: ThrottledCaller,
    report_path: Path,
    *,
    plan_id: str,
    lib_version: str,
    clock: Clock,
    line: Any = None,
) -> SendResult:
    """Legt die Workouts an und plant sie. Läuft beim ersten Fehler aus, ohne weiterzumachen.

    Je Einheit zwei Aufrufe: ``upload_workout`` (Antwort enthält ``workoutId``), dann
    ``schedule_workout(workoutId, datum)``. Der Zustand je Einheit steht nach jedem Aufruf im
    Bericht: ``uploaded`` (angelegt, nicht geplant, ein erneuter Lauf plant nur noch), ``ok``,
    ``error`` (sicher nicht angekommen, ein erneuter Lauf versucht es noch einmal) und
    ``unknown`` (Ergebnis unklar, in Garmin Connect prüfen, wird nie automatisch wiederholt).
    """
    say = line or (lambda _text: None)
    report = load_report(report_path)
    entries: dict[str, dict[str, Any]] = {e["key"]: e for e in report["entries"] if "key" in e}
    result = SendResult()
    total = len(selection.workouts)
    handled = 0  # Einheiten, die fertig sind (geplant oder bewusst übersprungen)

    def save() -> None:
        report["entries"] = list(entries.values())
        report["plan_id"] = plan_id
        report["updated_at"] = _iso_now(clock)
        report["lib_version"] = lib_version
        write_json(report_path, report)

    for item in selection.workouts:
        previous = entries.get(item.key)
        status = previous.get("status") if previous else None
        if status == "ok":
            result.already_sent += 1
            handled += 1
            say(f"übersprungen (schon gesendet): {item.date} {item.title}")
            continue
        if status == "unknown":
            result.blocked_unknown += 1
            handled += 1
            say(
                f"übersprungen (Ergebnis unklar, in Garmin Connect prüfen): "
                f"{item.date} {item.title}"
            )
            continue
        entry = previous or {
            "key": item.key,
            "plan_id": plan_id,
            "date": item.date,
            "slot": item.slot,
            "template_id": item.template_id,
            "title": item.title,
        }
        entries[item.key] = entry
        entry.pop("error_class", None)
        entry.pop("http_status", None)
        stage = "upload" if not entry.get("workout_id") else "schedule"
        try:
            _check_before_send(stage, item, entry)
            if stage == "upload":
                response = caller.call("upload_workout", item.payload)
                workout_id = _workout_id(response)
                if workout_id is None:
                    # Angelegt ist es wahrscheinlich, die Kennung fehlt aber. Nicht wiederholen.
                    entry.update(status="unknown", error_class="NoWorkoutId", at=_iso_now(clock))
                    result.errors += 1
                    result.stop_reason = "error"
                    result.stop_message = "Antwort ohne workoutId"
                    break
                entry.update(status="uploaded", workout_id=workout_id, at=_iso_now(clock))
                result.uploaded += 1
                save()
                stage = "schedule"
            caller.call("schedule_workout", entry["workout_id"], item.date)
            entry.update(status="ok", scheduled_for=item.date, at=_iso_now(clock))
            result.scheduled += 1
            handled += 1
            save()
            say(f"gesendet und geplant: {item.date} {item.title} (Kennung {entry['workout_id']})")
        except (LockoutError, AuthRequiredError, CallBudgetExhausted) as exc:
            reason = {
                LockoutError: "lockout",
                AuthRequiredError: "auth",
                CallBudgetExhausted: "budget",
            }[type(exc)]
            # Beim Budget ist nichts schiefgegangen, der Stand bleibt wie er ist.
            if reason != "budget":
                _mark_failure(entry, exc, stage, clock)
                result.errors += 1
            elif previous is None:
                del entries[item.key]
            result.stop_reason = reason
            result.stop_message = str(exc)
            break
        except Exception as exc:  # noqa: BLE001, Fehlerklasse wird protokolliert
            _mark_failure(entry, exc, stage, clock)
            result.errors += 1
            result.stop_reason = "error"
            result.stop_message = type(exc).__name__
            break
    result.pending = total - handled
    if total:
        save()
    return result


def _mark_failure(entry: dict[str, Any], exc: BaseException, stage: str, clock: Clock) -> None:
    kind = _classify_failure(exc)
    # Angelegt, aber sicher nicht geplant: der nächste Lauf plant nur noch.
    entry["status"] = "uploaded" if stage == "schedule" and kind == "error" else kind
    entry["error_class"] = type(exc).__name__
    code = http_status_of(exc)
    if code is not None:
        entry["http_status"] = code
    entry["at"] = _iso_now(clock)


def format_send_summary(result: SendResult, *, requested: int) -> list[str]:
    lines = [
        f"Einheiten gewählt: {requested}, angelegt: {result.uploaded}, "
        f"geplant: {result.scheduled}, schon gesendet: {result.already_sent}, "
        f"Ergebnis unklar: {result.blocked_unknown}, Fehler: {result.errors}"
    ]
    if result.stop_reason:
        lines.append(
            f"Abbruch ({result.stop_reason}): {result.stop_message}. Noch offen: {result.pending}"
        )
    return lines
