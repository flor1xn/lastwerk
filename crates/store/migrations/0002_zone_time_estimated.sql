-- Migration 0002 (T05): Kennzeichnung geschätzter Herzfrequenzzonen.
--
-- Fehlt die maximale Herzfrequenz, rechnet der Speicher die Zonen nach Herzfrequenzreserve
-- mit der Schätzung aus dem Alter (SPEC 8.5, Q7). Die Spalte hält fest, dass die Zeit in
-- Zonen auf einer Schätzung beruht, damit die Oberfläche sie als Schätzung ausweist.
-- Die Tabelle gehört zum Cache (SPEC 6.1) und enthält keine Nutzerdaten.

ALTER TABLE activity_zone_time ADD COLUMN estimated INTEGER NOT NULL DEFAULT 0;
