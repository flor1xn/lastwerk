-- Migration 0007 (T39): Vollabzug aus Garmin und Schwellen mit Herkunft `garmin`.
--
-- 1. `threshold.origin` kennt `garmin` (SPEC 8.6, Entscheidung E1). SQLite kann eine
--    CHECK-Bedingung nicht ändern, deshalb wird die Tabelle neu angelegt und mit denselben IDs
--    gefüllt (Verfahren aus der SQLite-Dokumentation zu ALTER TABLE). `activity_load.threshold_id`
--    verweist auf diese IDs. Die Migration läuft deshalb mit ausgeschalteten Fremdschlüsseln
--    (`Migration::foreign_keys_off`), danach prüft `foreign_key_check`, dass alle Verweise
--    stimmen.
-- 2. Cache: `activity_load.missing` nennt bei `missing_threshold` die fehlende Art.
-- 3. Nutzerdaten (nicht aus FIT ableitbar, im Backup): Tageswerte, Trainingswerte, Garmins Last
--    je Aktivität und S3-Werte in einer eigenen Tabelle (SPEC 5.5, 6.2).
--
-- Die Migration löscht keine Nutzerdaten.

CREATE TABLE threshold_new (
  id          INTEGER PRIMARY KEY,
  sport       TEXT NOT NULL,        -- 'cycling', 'running', ... oder 'all'
  kind        TEXT NOT NULL CHECK (kind IN ('ftp_w','lthr_bpm','hr_max_bpm','hr_rest_bpm','run_speed_mps','css_mps')),
  value       REAL NOT NULL,
  valid_from  TEXT NOT NULL,        -- Datum, YYYY-MM-DD
  origin      TEXT NOT NULL CHECK (origin IN ('manual','suggested_accepted','garmin')),
  note        TEXT
);
INSERT INTO threshold_new (id, sport, kind, value, valid_from, origin, note)
  SELECT id, sport, kind, value, valid_from, origin, note FROM threshold;
DROP TABLE threshold;
ALTER TABLE threshold_new RENAME TO threshold;
CREATE INDEX threshold_lookup ON threshold(kind, valid_from);

ALTER TABLE activity_load ADD COLUMN missing TEXT;

CREATE TABLE wellness_day (
  local_date               TEXT PRIMARY KEY,
  steps                    INTEGER,
  step_distance_m          REAL,
  hr_min_bpm               REAL,
  hr_max_bpm               REAL,
  respiration_waking_brpm  REAL,
  respiration_sleep_brpm   REAL,
  spo2_avg_pct             REAL,
  spo2_low_pct             REAL,
  intensity_moderate_min   INTEGER,
  intensity_vigorous_min   INTEGER,
  stress_avg               INTEGER,
  stress_max               INTEGER,
  source                   TEXT NOT NULL,
  imported_at              TEXT NOT NULL
);

CREATE TABLE training_day (
  local_date         TEXT PRIMARY KEY,
  readiness_score    INTEGER,
  readiness_level    TEXT,
  garmin_acute_load  REAL,
  fitness_age        REAL,
  vo2max_running     REAL,
  vo2max_cycling     REAL,
  endurance_score    INTEGER,
  hill_score         INTEGER,
  source             TEXT NOT NULL,
  imported_at        TEXT NOT NULL
);

CREATE TABLE garmin_activity_load (
  garmin_activity_id  INTEGER PRIMARY KEY,
  local_date          TEXT NOT NULL,
  training_load       REAL,
  training_effect     TEXT,
  source              TEXT NOT NULL,
  imported_at         TEXT NOT NULL
);
CREATE INDEX garmin_activity_load_date ON garmin_activity_load(local_date);

CREATE TABLE sensitive_day (
  local_date    TEXT PRIMARY KEY,
  hydration_ml  REAL,
  source        TEXT NOT NULL,
  imported_at   TEXT NOT NULL
);
