-- Migration 0005 (T33): Version der abgeleiteten Werte im Cache.
--
-- Eine Zeile `derived_version` hält fest, mit welchem Stand die abgeleiteten Werte (Belastung,
-- Zeit in Zonen, Bestleistungen) berechnet sind. Ist sie kleiner als die des Programms, liest der
-- nächste Import jede bekannte Datei neu ein, auch wenn sie sich nicht geändert hat. Die Tabelle
-- gehört zum Cache (SPEC 6.1), steht nicht im Backup und enthält keine Nutzerdaten. Die
-- Migration legt nur die Tabelle an und löscht nichts; ohne Zeile gilt Version 0.

CREATE TABLE cache_meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
