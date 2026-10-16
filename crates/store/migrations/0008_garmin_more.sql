-- Migration 0008 (T40): weitere Werte aus Garmin.
--
-- 1. `wellness_day`: Etagen auf und ab (Summe der Viertelstunden eines Tages).
-- 2. `training_day`: Trainingsstatus (Schlüssel aus Garmins Phrase), chronische Last und
--    Verhältnis akut zu chronisch, Belastungsfokus der letzten vier Wochen mit Zielspannen,
--    Rennprognosen in Sekunden, Lauftoleranz (gleitend über 7 Tage).
-- 3. S3-Werte je Bereich in eigenen Tabellen, damit Freigabe und Löschen je Bereich gehen
--    (Entscheidung E7): Körper und Gewicht, Blutdruck, Ernährung. Hydration bleibt in
--    `sensitive_day`.
--
-- Alles Nutzerdaten (nicht aus FIT ableitbar, im Backup). Nur neue Spalten und Tabellen, keine
-- Umschreibung bestehender Zeilen.

ALTER TABLE wellness_day ADD COLUMN floors_up REAL;
ALTER TABLE wellness_day ADD COLUMN floors_down REAL;

ALTER TABLE training_day ADD COLUMN training_status TEXT;
ALTER TABLE training_day ADD COLUMN garmin_chronic_load REAL;
ALTER TABLE training_day ADD COLUMN garmin_acwr REAL;
ALTER TABLE training_day ADD COLUMN load_aerobic_low REAL;
ALTER TABLE training_day ADD COLUMN load_aerobic_low_min REAL;
ALTER TABLE training_day ADD COLUMN load_aerobic_low_max REAL;
ALTER TABLE training_day ADD COLUMN load_aerobic_high REAL;
ALTER TABLE training_day ADD COLUMN load_aerobic_high_min REAL;
ALTER TABLE training_day ADD COLUMN load_aerobic_high_max REAL;
ALTER TABLE training_day ADD COLUMN load_anaerobic REAL;
ALTER TABLE training_day ADD COLUMN load_anaerobic_min REAL;
ALTER TABLE training_day ADD COLUMN load_anaerobic_max REAL;
ALTER TABLE training_day ADD COLUMN load_balance TEXT;
ALTER TABLE training_day ADD COLUMN race_5k_s REAL;
ALTER TABLE training_day ADD COLUMN race_10k_s REAL;
ALTER TABLE training_day ADD COLUMN race_half_s REAL;
ALTER TABLE training_day ADD COLUMN race_marathon_s REAL;
ALTER TABLE training_day ADD COLUMN run_acute_distance_m REAL;
ALTER TABLE training_day ADD COLUMN run_impact_load REAL;
ALTER TABLE training_day ADD COLUMN run_tolerance_m REAL;
ALTER TABLE training_day ADD COLUMN run_tolerance_level TEXT;

CREATE TABLE body_day (
  local_date      TEXT PRIMARY KEY,
  weight_kg       REAL,
  bmi             REAL,
  body_fat_pct    REAL,
  body_water_pct  REAL,
  bone_kg         REAL,
  muscle_kg       REAL,
  source          TEXT NOT NULL,
  imported_at     TEXT NOT NULL
);

CREATE TABLE bp_day (
  local_date      TEXT PRIMARY KEY,
  systolic_mmhg   REAL,
  diastolic_mmhg  REAL,
  pulse_bpm       REAL,
  source          TEXT NOT NULL,
  imported_at     TEXT NOT NULL
);

CREATE TABLE nutrition_day (
  local_date          TEXT PRIMARY KEY,
  calories_kcal       REAL,
  carbs_g             REAL,
  fat_g               REAL,
  protein_g           REAL,
  calories_goal_kcal  REAL,
  source              TEXT NOT NULL,
  imported_at         TEXT NOT NULL
);
