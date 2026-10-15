-- Lastwerk, Migration 0001: Ausgangsschema nach SPEC 6.2.
--
-- Cache (aus FIT ableitbar, SPEC 6.3): source_file, activity, activity_load,
-- activity_zone_time, activity_best_power.
-- Nutzerdaten (nicht ableitbar, im Backup): threshold, goal, availability, plan,
-- manual_session, setting.
-- Nicht enthalten: recovery_day (kommt mit M4, Task T14).
--
-- Abweichungen von SPEC 6.2, jeweils dort vermerkt:
--   * source_file.sha256 ist nicht UNIQUE, dafür ist rel_path UNIQUE. Der Import
--     überspringt Dateien mit gleichem Pfad und gleicher Prüfsumme. Zwei Dateien mit
--     gleichem Inhalt in verschiedenen Ordnern werden zu zwei Aktivitäten, die die
--     Duplikatregel (SPEC 6.4) zusammenführt.
--   * activity.source_file_id ist UNIQUE (eine Aktivität je Datei, Mehrkampf ist nicht
--     Teil von Version 1).
--   * threshold.sport kennt zusätzlich den Wert 'all' für sportartübergreifende Werte
--     (Ruhepuls, maximale Herzfrequenz).
--   * activity_load.method kennt für Fehlerzeilen den Wert 'none', weil bei einem Fehler
--     keine Methode feststeht.

-- Cache ----------------------------------------------------------------------------

CREATE TABLE source_file (
  id            INTEGER PRIMARY KEY,
  rel_path      TEXT NOT NULL UNIQUE,
  sha256        TEXT NOT NULL,
  size_bytes    INTEGER NOT NULL,
  mtime_utc     TEXT NOT NULL,
  imported_at   TEXT NOT NULL,
  status        TEXT NOT NULL CHECK (status IN ('ok','not_activity','error')),
  error_code    TEXT
);
CREATE INDEX source_file_sha256 ON source_file(sha256);

-- threshold wird von activity_load referenziert und steht deshalb davor.
CREATE TABLE threshold (
  id          INTEGER PRIMARY KEY,
  sport       TEXT NOT NULL,        -- 'cycling', 'running', ... oder 'all'
  kind        TEXT NOT NULL CHECK (kind IN ('ftp_w','lthr_bpm','hr_max_bpm','hr_rest_bpm','run_speed_mps','css_mps')),
  value       REAL NOT NULL,
  valid_from  TEXT NOT NULL,        -- Datum, YYYY-MM-DD
  origin      TEXT NOT NULL CHECK (origin IN ('manual','suggested_accepted')),
  note        TEXT
);
CREATE INDEX threshold_lookup ON threshold(kind, valid_from);

CREATE TABLE activity (
  id              INTEGER PRIMARY KEY,
  source_file_id  INTEGER NOT NULL UNIQUE REFERENCES source_file(id),
  sport           TEXT NOT NULL,
  sub_sport       TEXT,
  start_utc       TEXT NOT NULL,
  local_offset_s  INTEGER NOT NULL,
  local_date      TEXT NOT NULL,
  duration_s      INTEGER NOT NULL,
  moving_s        INTEGER,
  distance_m      REAL,
  ascent_m        REAL,
  avg_hr          REAL,
  max_hr          REAL,
  avg_power_w     REAL,
  np_w            REAL,
  device_serial   INTEGER,
  device_product  TEXT,
  has_power       INTEGER NOT NULL DEFAULT 0,
  has_hr          INTEGER NOT NULL DEFAULT 0,
  is_primary      INTEGER NOT NULL DEFAULT 1,
  duplicate_of    INTEGER REFERENCES activity(id)
);
CREATE INDEX activity_local_date ON activity(local_date);

CREATE TABLE activity_load (
  activity_id     INTEGER NOT NULL REFERENCES activity(id),
  method          TEXT NOT NULL,
  value           REAL,
  intensity_factor REAL,
  threshold_id    INTEGER REFERENCES threshold(id),
  status          TEXT NOT NULL CHECK (status IN ('ok','missing_threshold','missing_channel','implausible')),
  computed_at     TEXT NOT NULL,
  PRIMARY KEY (activity_id, method)
);

CREATE TABLE activity_zone_time (
  activity_id INTEGER NOT NULL REFERENCES activity(id),
  scheme      TEXT NOT NULL,        -- z. B. 'power7', 'hr_lthr5'
  zone        INTEGER NOT NULL,
  seconds     INTEGER NOT NULL,
  PRIMARY KEY (activity_id, scheme, zone)
);

CREATE TABLE activity_best_power (
  activity_id INTEGER NOT NULL REFERENCES activity(id),
  duration_s  INTEGER NOT NULL,     -- 5, 60, 300, 1200, 3600
  watts       REAL NOT NULL,
  PRIMARY KEY (activity_id, duration_s)
);

-- Nutzerdaten ------------------------------------------------------------------------

CREATE TABLE goal (
  id        INTEGER PRIMARY KEY,
  kind      TEXT NOT NULL CHECK (kind IN ('event','continuous')),
  sport     TEXT,
  event_date TEXT,
  label     TEXT,                   -- Freitext, nie an ein LLM
  priority  TEXT CHECK (priority IN ('A','B','C')),
  active    INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE availability (
  weekday      INTEGER PRIMARY KEY CHECK (weekday BETWEEN 0 AND 6),  -- 0 = Montag
  max_minutes  INTEGER NOT NULL,
  allow_hard   INTEGER NOT NULL DEFAULT 1,
  fixed_rest   INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE plan (
  id            TEXT PRIMARY KEY,   -- UUID
  created_at    TEXT NOT NULL,
  goal_id       INTEGER REFERENCES goal(id),
  source        TEXT NOT NULL CHECK (source IN ('rules','llm')),
  rules_version TEXT NOT NULL,
  locale        TEXT NOT NULL,
  json          TEXT NOT NULL,      -- gegen plan.schema.json validiert
  status        TEXT NOT NULL CHECK (status IN ('draft','active','archived'))
);

CREATE TABLE manual_session (         -- Kraft ohne FIT, Nachträge
  id          INTEGER PRIMARY KEY,
  local_date  TEXT NOT NULL,
  sport       TEXT NOT NULL,
  duration_s  INTEGER NOT NULL,
  rpe         INTEGER CHECK (rpe BETWEEN 1 AND 10)
);

CREATE TABLE setting (key TEXT PRIMARY KEY, value TEXT NOT NULL);
