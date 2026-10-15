-- Migration 0004 (T22): Body Battery je Tag aus dem Abgleich mit Garmin.
--
-- Zwei neue Spalten in `recovery_day`, beide leer erlaubt: Höchst- und Tiefstwert des Tages
-- auf der Skala 0 bis 100 (Skala des Herstellers, Plausibilitätsgrenze Faustregel). Alte Zeilen
-- behalten alle Werte und bekommen NULL. Die Migration löscht nichts.

ALTER TABLE recovery_day ADD COLUMN body_battery_high INTEGER;
ALTER TABLE recovery_day ADD COLUMN body_battery_low  INTEGER;
