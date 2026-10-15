-- Migration 0006 (T37): Bestzeiten über feste Strecken je Lauf.
--
-- Cache wie activity_best_power (SPEC 6.1, 6.3): aus FIT ableitbar, nicht im Backup, beim
-- Wiederaufbau geleert. Eine Zeile je Lauf und erreichter Strecke; Strecken, die der Lauf nicht
-- erreicht, haben keine Zeile. Die Migration legt nur die Tabelle an und löscht nichts.

CREATE TABLE activity_best_effort (
  activity_id INTEGER NOT NULL REFERENCES activity(id),
  distance_m  INTEGER NOT NULL,     -- 1000, 5000, 10000, 21097, 42195
  seconds     INTEGER NOT NULL,
  PRIMARY KEY (activity_id, distance_m)
);
