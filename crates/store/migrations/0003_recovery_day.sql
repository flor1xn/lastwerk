-- Migration 0003 (T14): Erholungstage aus dem Zusatzprogramm.
--
-- Tabelle nach SPEC 6.2. Die Werte sind nicht aus FIT ableitbar und gehören deshalb zu den
-- Nutzerdaten: Sie stehen im Backup und `rebuild_cache` lässt sie unberührt. Die Migration
-- legt nur eine neue Tabelle an und löscht nichts.
--
-- Die Feldnamen gelten vorläufig, bis `docs/garmin-feldabdeckung.md` aus L1 vorliegt (SPEC 14
-- O9). `source` nennt den Absender der Datei (zum Beispiel `garmin-bridge 0.1.0`),
-- `imported_at` den Zeitpunkt des Imports (UTC, RFC 3339).

CREATE TABLE recovery_day (
  local_date           TEXT PRIMARY KEY,   -- YYYY-MM-DD
  hrv_overnight_ms     REAL,
  resting_hr_bpm       REAL,
  sleep_duration_s     INTEGER,
  sleep_score          INTEGER,
  source               TEXT NOT NULL,
  imported_at          TEXT NOT NULL
);
