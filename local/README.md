# local

Private Daten. **Nichts in diesem Ordner wird versioniert**, nur diese README. Der Ordner ist in `.gitignore` ausgeschlossen und vom Hook `.githooks/pre-commit` geschützt.

| Ordner | Inhalt |
|---|---|
| fit/ | Echte FIT-Dateien, Eingabeordner für Lastwerk |
| archive/garmin/ | Archiv des Zusatzprogramms (unveränderte Antworten, Manifest) |
| exchange/out/ | Dateien vom Zusatzprogramm an Lastwerk |
| exchange/in/ | Dateien von Lastwerk an das Zusatzprogramm |
| probe/ | Antworten der Sonde, nie weitergeben |

## Warnungen

- `git clean -x` und `git clean -X` löschen ignorierte Dateien und damit alles in diesem Ordner. Nie mit `-x` oder `-X` ausführen.
- Dieser Ordner ist nicht gesichert. Regelmäßig auf einen anderen Datenträger kopieren (SPEC O28).
- Der Projektordner darf nicht in einen Cloud-Sync-Ordner verschoben werden.
- Die Daten enthalten Gesundheits- und Standortinformationen. Nicht teilen, nicht hochladen.
- Garmin-Tokens liegen nicht hier, sondern im Benutzerprofil (`~/.garminconnect`).
