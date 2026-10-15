"""Endpunkt-Register (SPEC 5.5, Abschnitt "Endpunkt-Register").

Das Register wird als Python-Modul geführt und nach ``bridge/registry.yaml``
geschrieben. Begründung: Die Einträge sind flach, die Bridge braucht beim Lesen
keine YAML-Bibliothek, und ein Test hält Modul und Datei gleich. Beim Lauf wird die
Datei nur unverändert in das Archiv kopiert.

Je Eintrag: ``name``, ``method`` (Methodenname der Bibliothek, muss in der Allowlist
stehen), ``granularity`` (``once``, ``day``, ``range``, ``page``, ``per_activity``),
``sensitivity`` (``S1``, ``S2``, ``S3`` nach SPEC 5.5), ``params`` (Parameterart),
``category`` (Unterordner für S3), ``consumer`` (liest Lastwerk das?),
``max_span_days`` (Messung in L1, vorher unbekannt), ``status`` ("ungeprüft bis L1"),
``ext`` (``json`` oder ``fit``) und ``note``.

Status und ``max_span_days`` stammen aus der Sonde in L1 (``_PROBE_L1``, unten).
Ein Eintrag ohne Sondenergebnis bleibt ``unchecked_until_L1``.
"""

from __future__ import annotations

from dataclasses import dataclass, field, replace
from pathlib import Path

from .allowlist import ALLOWLIST

GRANULARITIES = ("once", "day", "range", "page", "per_activity")
SENSITIVITIES = ("S1", "S2", "S3")
STATUS_UNCHECKED = "unchecked_until_L1"


@dataclass(frozen=True)
class RegistryEntry:
    name: str
    method: str
    granularity: str
    sensitivity: str
    params: tuple[str, ...] = ()
    category: str | None = None
    consumer: bool = False
    max_span_days: int | None = None
    status: str = STATUS_UNCHECKED
    ext: str = "json"
    note: str = ""
    kwargs: dict[str, object] = field(default_factory=dict, hash=False, compare=False)

    def __post_init__(self) -> None:
        if self.granularity not in GRANULARITIES:
            raise ValueError(f"{self.name}: unbekannte Granularität {self.granularity}")
        if self.sensitivity not in SENSITIVITIES:
            raise ValueError(f"{self.name}: unbekannte Sensibilitätsklasse {self.sensitivity}")
        if self.method not in ALLOWLIST:
            raise ValueError(f"{self.name}: Methode {self.method} steht nicht in der Allowlist")
        if self.sensitivity == "S3" and not self.category:
            raise ValueError(f"{self.name}: S3 braucht eine Kategorie")

    @property
    def manifest_kind(self) -> str:
        """Manifest-Feld ``kind`` (SPEC 5.5, Schema)."""
        if self.granularity in ("once",):
            return "account"
        if self.granularity in ("page", "per_activity"):
            return "activity"
        if self.granularity == "day":
            return "daily"
        return "range"


def _e(
    name: str,
    method: str,
    granularity: str,
    sensitivity: str,
    *,
    params: tuple[str, ...] = (),
    category: str | None = None,
    consumer: bool = False,
    ext: str = "json",
    note: str = "",
    kwargs: dict[str, object] | None = None,
) -> RegistryEntry:
    return RegistryEntry(
        name=name,
        method=method,
        granularity=granularity,
        sensitivity=sensitivity,
        params=params,
        category=category,
        consumer=consumer,
        ext=ext,
        note=note,
        kwargs=kwargs or {},
    )


# Reihenfolge wie die Abrufregel 1 in SPEC 5.5: Konto und Zonen, Aktivitätsliste,
# je Aktivität, Tageswerte, Bereiche, danach die gesperrt abgelegten Klassen.
REGISTRY: tuple[RegistryEntry, ...] = (
    # Konto, Zonen, Schwellen (S1, einmalig)
    _e("profile", "get_user_profile", "once", "S1", consumer=True),
    _e("settings", "get_userprofile_settings", "once", "S1", consumer=True),
    _e("devices", "get_devices", "once", "S1", consumer=True),
    _e("device_last_used", "get_device_last_used", "once", "S1"),
    _e("primary_training_device", "get_primary_training_device", "once", "S1"),
    _e("zones", "get_heart_rate_zones", "once", "S1", consumer=True),
    _e("power_zones", "get_power_zones", "once", "S1", consumer=True),
    _e("ftp", "get_cycling_ftp", "once", "S1", consumer=True),
    _e("lactate_threshold", "get_lactate_threshold", "once", "S1", consumer=True),
    _e("personal_records", "get_personal_record", "once", "S1"),
    _e("goals", "get_goals", "once", "S1", note="start zählt ab 1 (Bruch in 0.3.17)"),
    _e("workouts", "get_workouts", "once", "S1"),
    _e("training_plans", "get_training_plans", "once", "S1"),
    _e("activity_types", "get_activity_types", "once", "S1"),
    _e(
        "gear",
        "get_gear",
        "once",
        "S1",
        params=("userProfileNumber",),
        note="braucht die Profilnummer aus profile",
    ),
    # Aktivitätsliste (S1, seitenweise)
    _e("activities_list", "get_activities", "page", "S1", params=("start", "limit"), consumer=True),
    # Je Aktivität (S1)
    _e("summary", "get_activity", "per_activity", "S1", params=("activity_id",), consumer=True),
    _e(
        "details",
        "get_activity_details",
        "per_activity",
        "S1",
        params=("activity_id",),
        note="auf maxchart 2000 und maxpoly 4000 begrenzt, Streams aus original.fit",
    ),
    _e("splits", "get_activity_splits", "per_activity", "S1", params=("activity_id",)),
    _e("typed_splits", "get_activity_typed_splits", "per_activity", "S1", params=("activity_id",)),
    _e(
        "split_summaries",
        "get_activity_split_summaries",
        "per_activity",
        "S1",
        params=("activity_id",),
    ),
    _e("weather", "get_activity_weather", "per_activity", "S1", params=("activity_id",)),
    _e("hr_zones", "get_activity_hr_in_timezones", "per_activity", "S1", params=("activity_id",)),
    _e(
        "power_zones_activity",
        "get_activity_power_in_timezones",
        "per_activity",
        "S1",
        params=("activity_id",),
    ),
    _e(
        "exercise_sets", "get_activity_exercise_sets", "per_activity", "S1", params=("activity_id",)
    ),
    _e("gear_activity", "get_activity_gear", "per_activity", "S1", params=("activity_id",)),
    _e(
        "original",
        "download_activity",
        "per_activity",
        "S1",
        params=("activity_id",),
        consumer=True,
        ext="zip",
        note="dl_fmt ORIGINAL liefert ein ZIP, original.fit wird daraus entpackt (T12)",
        kwargs={"dl_fmt": "ORIGINAL"},
    ),
    # Tageswerte (S2)
    _e("user_summary", "get_user_summary", "day", "S2", params=("date",)),
    _e("steps", "get_steps_data", "day", "S2", params=("date",)),
    _e("floors", "get_floors", "day", "S2", params=("date",)),
    _e("heart_rates", "get_heart_rates", "day", "S2", params=("date",), consumer=True),
    _e("rhr", "get_rhr_day", "day", "S2", params=("date",), consumer=True),
    _e("stress", "get_stress_data", "day", "S2", params=("date",), consumer=True),
    _e("all_day_stress", "get_all_day_stress", "day", "S2", params=("date",)),
    _e("body_battery_events", "get_body_battery_events", "day", "S2", params=("date",)),
    _e("respiration", "get_respiration_data", "day", "S2", params=("date",)),
    _e("spo2", "get_spo2_data", "day", "S2", params=("date",)),
    _e("intensity_minutes", "get_intensity_minutes_data", "day", "S2", params=("date",)),
    _e("sleep", "get_sleep_data", "day", "S2", params=("date",), consumer=True),
    _e("hrv", "get_hrv_data", "day", "S2", params=("date",), consumer=True),
    _e(
        "training_readiness", "get_training_readiness", "day", "S2", params=("date",), consumer=True
    ),
    _e("training_status", "get_training_status", "day", "S2", params=("date",), consumer=True),
    _e("daily_training_status", "get_daily_training_status", "day", "S2", params=("date",)),
    _e(
        "morning_training_readiness",
        "get_morning_training_readiness",
        "day",
        "S2",
        params=("date",),
    ),
    _e("fitness_age", "get_fitnessage_data", "day", "S2", params=("date",)),
    _e("max_metrics", "get_max_metrics", "day", "S2", params=("date",), consumer=True),
    _e(
        "four_week_load_balance",
        "get_training_four_week_load_balance",
        "day",
        "S2",
        params=("date",),
    ),
    # Bereiche (S1 und S2)
    _e(
        "activities_by_date",
        "get_activities_by_date",
        "range",
        "S1",
        params=("start", "end"),
        consumer=True,
    ),
    _e("body_battery", "get_body_battery", "range", "S2", params=("start", "end")),
    _e("hrv_range", "get_hrv_data_range", "range", "S2", params=("start", "end"), consumer=True),
    _e("endurance_score", "get_endurance_score", "range", "S2", params=("start", "end")),
    _e("hill_score", "get_hill_score", "range", "S2", params=("start", "end")),
    _e("running_tolerance", "get_running_tolerance", "range", "S2", params=("start", "end")),
    _e("race_predictions", "get_race_predictions", "range", "S2", params=("start", "end")),
    _e("max_metrics_range", "get_max_metrics_range", "range", "S2", params=("start", "end")),
    _e("daily_steps", "get_daily_steps", "range", "S2", params=("start", "end")),
    _e("sleep_daily", "get_sleep_daily", "range", "S2", params=("start", "end"), consumer=True),
    _e("rhr_daily", "get_rhr_daily", "range", "S2", params=("start", "end"), consumer=True),
    _e("calories_daily", "get_calories_daily", "range", "S2", params=("start", "end")),
    _e(
        "training_load_activities",
        "get_training_load_activities",
        "range",
        "S2",
        params=("start", "end"),
        consumer=True,
    ),
    _e(
        "weekly_intensity_minutes",
        "get_weekly_intensity_minutes",
        "range",
        "S2",
        params=("start", "end"),
    ),
    # Gesperrt abgelegt (S3), Ordner sensitive/<category>/
    _e("weigh_ins", "get_weigh_ins", "range", "S3", params=("start", "end"), category="weight"),
    _e(
        "body_composition",
        "get_body_composition",
        "range",
        "S3",
        params=("start", "end"),
        category="weight",
    ),
    _e(
        "blood_pressure",
        "get_blood_pressure",
        "range",
        "S3",
        params=("start", "end"),
        category="blood_pressure",
    ),
    _e("hydration", "get_hydration_data", "day", "S3", params=("date",), category="hydration"),
    _e(
        "nutrition_food_log",
        "get_nutrition_daily_food_log",
        "day",
        "S3",
        params=("date",),
        category="nutrition",
    ),
    _e(
        "nutrition_meals",
        "get_nutrition_daily_meals",
        "day",
        "S3",
        params=("date",),
        category="nutrition",
    ),
    _e(
        "nutrition_settings",
        "get_nutrition_daily_settings",
        "day",
        "S3",
        params=("date",),
        category="nutrition",
    ),
    _e(
        "lifestyle_logging",
        "get_lifestyle_logging_data",
        "day",
        "S3",
        params=("date",),
        category="lifestyle",
    ),
)


# Ergebnis der Sonde aus L1 (14. Oktober 2026, garminconnect 0.3.17): Name -> (Status, größte
# getestete Spanne in Tagen). Status ok, empty (Aufruf gelang, aber keine Daten im Testzeitraum)
# oder error. Spanne 1792 ist die größte geprüfte, kein Limit gefunden; kleinere Werte sind die
# Grenze, über der der Aufruf scheiterte. Keine Werte aus dem Konto, nur Status und Zahlen.
_PROBE_L1: dict[str, tuple[str, int | None]] = {
    "profile": ("ok", None),
    "settings": ("ok", None),
    "devices": ("ok", None),
    "device_last_used": ("ok", None),
    "primary_training_device": ("ok", None),
    "zones": ("ok", None),
    "power_zones": ("ok", None),
    "ftp": ("ok", None),
    "lactate_threshold": ("ok", None),
    "personal_records": ("ok", None),
    "goals": ("empty", None),
    "workouts": ("ok", None),
    "training_plans": ("ok", None),
    "activity_types": ("ok", None),
    "gear": ("ok", None),
    "activities_list": ("ok", None),
    "summary": ("ok", None),
    "details": ("ok", None),
    "splits": ("ok", None),
    "typed_splits": ("ok", None),
    "split_summaries": ("ok", None),
    "weather": ("ok", None),
    "hr_zones": ("ok", None),
    "power_zones_activity": ("empty", None),
    "exercise_sets": ("ok", None),
    "gear_activity": ("ok", None),
    "original": ("ok", None),
    "user_summary": ("ok", None),
    "steps": ("ok", None),
    "floors": ("ok", None),
    "heart_rates": ("ok", None),
    "rhr": ("ok", None),
    "stress": ("ok", None),
    "all_day_stress": ("ok", None),
    "body_battery_events": ("ok", None),
    "respiration": ("ok", None),
    "spo2": ("ok", None),
    "intensity_minutes": ("ok", None),
    "sleep": ("ok", None),
    "hrv": ("ok", None),
    "training_readiness": ("ok", None),
    "training_status": ("ok", None),
    "daily_training_status": ("ok", None),
    "morning_training_readiness": ("ok", None),
    "fitness_age": ("ok", None),
    "max_metrics": ("empty", None),
    "four_week_load_balance": ("ok", None),
    "activities_by_date": ("ok", 1792),
    "body_battery": ("ok", 31),
    "hrv_range": ("ok", 336),
    "endurance_score": ("ok", 392),
    "hill_score": ("ok", 392),
    "running_tolerance": ("empty", 336),
    "race_predictions": ("error", None),
    "max_metrics_range": ("ok", 1792),
    "daily_steps": ("ok", 1792),
    "sleep_daily": ("ok", 1792),
    "rhr_daily": ("ok", 1792),
    "calories_daily": ("ok", 1792),
    "training_load_activities": ("ok", 1792),
    "weekly_intensity_minutes": ("ok", 336),
    "weigh_ins": ("ok", 1792),
    "body_composition": ("ok", 1792),
    "blood_pressure": ("ok", 1792),
    "hydration": ("ok", None),
    "nutrition_food_log": ("ok", None),
    "nutrition_meals": ("ok", None),
    "nutrition_settings": ("empty", None),
    "lifestyle_logging": ("ok", None),
}


def _apply_probe(entry: RegistryEntry) -> RegistryEntry:
    result = _PROBE_L1.get(entry.name)
    if result is None:
        return entry
    status, span = result
    return replace(entry, status=status, max_span_days=span)


REGISTRY = tuple(_apply_probe(e) for e in REGISTRY)


def by_name(name: str) -> RegistryEntry:
    for entry in REGISTRY:
        if entry.name == name:
            return entry
    raise KeyError(name)


def registry_yaml_path() -> Path:
    return Path(__file__).resolve().parents[2] / "registry.yaml"


def _yaml_scalar(value: object) -> str:
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    text = str(value)
    # Nur Zeichenketten ohne Sonderzeichen bleiben unquotiert.
    if text and all(ch.isalnum() or ch in "_-." for ch in text):
        return text
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"') + '"'


def to_yaml() -> str:
    """Schreibt das Register als einfaches YAML (nur Skalare und Listen)."""
    lines = [
        "# Endpunkt-Register der Lastwerk-Bridge (SPEC 5.5).",
        "# Erzeugt aus src/lastwerk_bridge/registry.py, nicht von Hand ändern.",
        "# status: ok, empty oder error laut Sonde (L1), unchecked_until_L1 ohne Sondenergebnis.",
        "# max_span_days: größte geprüfte Spanne, 1792 heißt kein Limit gefunden (SPEC 14, O26).",
        "schema_version: 1",
        "library: garminconnect",
        "entries:",
    ]
    for entry in REGISTRY:
        lines.append(f"  - name: {_yaml_scalar(entry.name)}")
        lines.append(f"    method: {_yaml_scalar(entry.method)}")
        lines.append(f"    granularity: {_yaml_scalar(entry.granularity)}")
        lines.append(f"    class: {_yaml_scalar(entry.sensitivity)}")
        if entry.params:
            lines.append("    params: [" + ", ".join(_yaml_scalar(p) for p in entry.params) + "]")
        else:
            lines.append("    params: []")
        if entry.kwargs:
            lines.append("    kwargs:")
            for key, value in entry.kwargs.items():
                lines.append(f"      {key}: {_yaml_scalar(value)}")
        if entry.category:
            lines.append(f"    category: {_yaml_scalar(entry.category)}")
        lines.append(f"    consumer: {_yaml_scalar(entry.consumer)}")
        lines.append(f"    max_span_days: {_yaml_scalar(entry.max_span_days)}")
        lines.append(f"    status: {_yaml_scalar(entry.status)}")
        lines.append(f"    ext: {_yaml_scalar(entry.ext)}")
        if entry.note:
            lines.append(f"    note: {_yaml_scalar(entry.note)}")
    return "\n".join(lines) + "\n"
