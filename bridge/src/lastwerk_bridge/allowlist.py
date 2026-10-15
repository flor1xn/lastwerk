"""Allowlist und Sperrliste der Methoden von ``garminconnect.Garmin`` (SPEC 5.5, Regel 7).

Beide Listen sind ausdrücklich, kein Muster. Jede öffentliche Methode der installierten
Bibliothek muss in genau einer Liste stehen. Der Test ``tests/test_allowlist.py``
vergleicht die Listen mit der Bibliothek, damit neue Methoden nach einer
Aktualisierung auffallen, bevor sie je aufgerufen werden.

Eingestuft gegen garminconnect 0.3.17.

Grundsätze der Einstufung:

- Allowlist: nur lesende Methoden (``get_*``, ``count_*``, ``download*``, die freien
  Methoden ``connectapi``, ``connectwebproxy``, ``query_garmin_graphql``).
- Sperrliste: alles Schreibende (``add_``, ``create_``, ``delete_``, ``set_``,
  ``update_``, ``upload_``, ``import_``, ``push_``, ``schedule_``, ``remove_``),
  ``request_reload`` (stößt serverseitig eine Verarbeitung an), die Anmeldung
  (``login``, ``resume_login``, ``logout``: nur der ``auth``-Pfad nutzt ``login``,
  nie der gedrosselte Aufrufer) und alle Menstruations- und
  Schwangerschaftsmethoden, auch die lesenden. Letztere werden nie abgerufen
  (SPEC 2.1, 5.5, 11.4).

Ausnahme für M6 (``push``, SPEC 12): ``WORKOUT_WRITE_ALLOWLIST`` nennt genau die zwei
Methoden, die ein Workout anlegen und in den Kalender setzen. Sie bleiben in der
Sperrliste und fehlen in ``ALLOWLIST``. Nur ein Aufrufer, dem diese Freigabe ausdrücklich
übergeben wird (``push.WorkoutWriteCaller``), darf sie aufrufen, der lesende Aufrufer
lehnt sie weiter ab.
"""

from __future__ import annotations

ALLOWLIST: frozenset[str] = frozenset(
    {
        "connectapi",
        "connectwebproxy",
        "count_activities",
        "download",
        "download_activity",
        "download_health_snapshot",
        "download_workout",
        "get_activities",
        "get_activities_by_date",
        "get_activities_fordate",
        "get_activity",
        "get_activity_details",
        "get_activity_exercise_sets",
        "get_activity_gear",
        "get_activity_hr_in_timezones",
        "get_activity_power_in_timezones",
        "get_activity_split_summaries",
        "get_activity_splits",
        "get_activity_typed_splits",
        "get_activity_types",
        "get_activity_weather",
        "get_adaptive_training_plan_by_id",
        "get_adhoc_challenges",
        "get_all_day_events",
        "get_all_day_stress",
        "get_available_badge_challenges",
        "get_available_badges",
        "get_badge_challenges",
        "get_blood_pressure",
        "get_body_battery",
        "get_body_battery_events",
        "get_body_composition",
        "get_calories_daily",
        "get_cycling_ftp",
        "get_daily_steps",
        "get_daily_training_status",
        "get_daily_weigh_ins",
        "get_device_alarms",
        "get_device_last_used",
        "get_device_settings",
        "get_device_solar_data",
        "get_devices",
        "get_earned_badges",
        "get_endurance_score",
        "get_fitnessage_data",
        "get_floors",
        "get_full_name",
        "get_functional_threshold_power_range",
        "get_gear",
        "get_gear_activities",
        "get_gear_defaults",
        "get_gear_stats",
        "get_goals",
        "get_golf_club_stats",
        "get_golf_scorecard",
        "get_golf_shot_data",
        "get_golf_summary",
        "get_golf_user_stats",
        "get_heart_rate_zones",
        "get_heart_rates",
        "get_hill_score",
        "get_hrv_data",
        "get_hrv_data_range",
        "get_hydration_data",
        "get_in_progress_badges",
        "get_inprogress_virtual_challenges",
        "get_intensity_minutes_data",
        "get_lactate_threshold",
        "get_last_activity",
        "get_lifestyle_logging_data",
        "get_max_metrics",
        "get_max_metrics_range",
        "get_morning_training_readiness",
        "get_next_scheduled_workout",
        "get_non_completed_badge_challenges",
        "get_nutrition_daily_food_log",
        "get_nutrition_daily_meals",
        "get_nutrition_daily_settings",
        "get_personal_record",
        "get_power_zones",
        "get_power_zones_for_sport",
        "get_primary_training_device",
        "get_progress_summary_between_dates",
        "get_race_predictions",
        "get_respiration_data",
        "get_rhr_daily",
        "get_rhr_day",
        "get_running_tolerance",
        "get_scheduled_workout_by_id",
        "get_scheduled_workouts",
        "get_sleep_daily",
        "get_sleep_data",
        "get_spo2_data",
        "get_stats",
        "get_stats_and_body",
        "get_steps_data",
        "get_stress_data",
        "get_training_four_week_load_balance",
        "get_training_load_activities",
        "get_training_plan_by_id",
        "get_training_plans",
        "get_training_readiness",
        "get_training_status",
        "get_unit_system",
        "get_user_profile",
        "get_user_summary",
        "get_userprofile_settings",
        "get_weekly_intensity_minutes",
        "get_weekly_steps",
        "get_weekly_stress",
        "get_weigh_ins",
        "get_workout_by_id",
        "get_workouts",
        "query_garmin_graphql",
    }
)

BLOCKLIST: frozenset[str] = frozenset(
    {
        "add_body_composition",
        "add_gear_to_activity",
        "add_hydration_data",
        "add_weigh_in",
        "add_weigh_in_with_timestamps",
        "confirm_menstrual_period_start",
        "create_gear",
        "create_manual_activity",
        "create_manual_activity_from_json",
        "delete_activity",
        "delete_blood_pressure",
        "delete_weigh_in",
        "delete_weigh_ins",
        "delete_workout",
        "get_menstrual_calendar_data",
        "get_menstrual_cycle_summary",
        "get_menstrual_data_for_date",
        "get_menstrual_last_confirmed",
        "get_menstrual_reports",
        "get_pregnancy_summary",
        "import_activity",
        "init_menstrual_cycle_setup",
        "login",
        "logout",
        "push_workout_to_device",
        "remove_gear_from_activity",
        "request_reload",
        "resume_login",
        "schedule_workout",
        "set_activity_description",
        "set_activity_exercise_sets",
        "set_activity_name",
        "set_activity_type",
        "set_blood_pressure",
        "set_gear_default",
        "unschedule_workout",
        "update_menstrual_calendar",
        "update_menstrual_daily_log",
        "update_menstrual_settings",
        "update_workout",
        "upload_activity",
        "upload_cycling_workout",
        "upload_hiking_workout",
        "upload_running_workout",
        "upload_strength_workout",
        "upload_swimming_workout",
        "upload_walking_workout",
        "upload_workout",
    }
)

# Freigabe nur für den Schreibpfad von ``push --yes`` (SPEC 5.5, 12 M6). Ausdrücklich, kein
# Muster. Nicht enthalten: ``update_workout``, ``delete_workout``, ``unschedule_workout``
# (Löschen und Ändern gehören nicht zu M6) und ``push_workout_to_device`` (F32, O4 offen).
# Alle Einträge müssen in ``BLOCKLIST`` stehen (Test).
WORKOUT_WRITE_ALLOWLIST: frozenset[str] = frozenset({"schedule_workout", "upload_workout"})

# Namensbestandteile, die in der Allowlist nie vorkommen dürfen. Das ist eine
# zweite Sicherung neben der ausdrücklichen Liste, geprüft im Test.
FORBIDDEN_ALLOWLIST_PATTERNS: tuple[str, ...] = (
    "menstrual",
    "pregnancy",
    "add_",
    "create_",
    "delete_",
    "set_",
    "update_",
    "upload",
    "import_",
    "push_",
    "schedule_",
    "remove_",
    "request_",
    "login",
    "logout",
)


class MethodNotAllowedError(PermissionError):
    """Die Methode steht nicht in der Allowlist und wird nicht aufgerufen."""


def is_allowed(method_name: str) -> bool:
    return method_name in ALLOWLIST


def ensure_write_allowed(method_name: str) -> None:
    """Wirft ``MethodNotAllowedError``, wenn die Methode nicht zum Schreibpfad gehört."""
    if method_name not in WORKOUT_WRITE_ALLOWLIST:
        raise MethodNotAllowedError(
            f"Methode gehört nicht zur Freigabe des Schreibpfads: {method_name}"
        )


def ensure_allowed(method_name: str) -> None:
    """Wirft ``MethodNotAllowedError``, wenn die Methode nicht erlaubt ist."""
    if method_name not in ALLOWLIST:
        if method_name in BLOCKLIST:
            raise MethodNotAllowedError(f"Methode steht in der Sperrliste: {method_name}")
        raise MethodNotAllowedError(f"Methode ist nicht eingestuft: {method_name}")
