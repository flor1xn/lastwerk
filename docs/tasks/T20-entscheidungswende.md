# T20: Entscheidungswende für Version 1.0

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | 1.0 |
| Voraussetzung | T19 |
| cloudfähig | ja |
| Bereich | `docs`, `CLAUDE.md`, `bridge/README.md` |
| SPEC | 2.1, 2.3, 4.2, 5.4, 5.5, 5.6, 9.7, 10.2, 13, 14 |

## Ziel

Die Entscheidungen für Version 1.0 stehen in SPEC und Regeln, bevor Code entsteht, und T21 bis T27 sind geschnitten.

## Warum

Der Nutzer bricht bewusst die frühere Festlegung "Garmin nur im Zusatzprogramm, App nimmt keine Zugangsdaten an". Ohne Vermerk in der SPEC würde jede spätere Session den neuen Code als Regelverstoß lesen.

## Entscheidungen (Nutzer, 15. Oktober 2026)

1. Garmin-Anmeldung und Abruf in Rust in der App (`crates/garmin`, SPEC 5.6). Anmeldung über die echte Garmin-Seite in einem eigenen Fenster, Lastwerk sieht Passwort und MFA nie.
2. Token im Schlüsselbund des Betriebssystems.
3. `bridge/` eingefroren, nicht gelöscht.
4. Planer mit drei Fragen (Ziel, Stunden je Woche, Trainingstage), alles Weitere im Gespräch mit dem lokalen Coach (SPEC 9.7).
5. Schrift Cairo überall, Dashboard ohne Standardkacheln, mehr Kennzahlen mit Deutung.

## Umfang

SPEC-Vermerke an allen betroffenen Stellen, neuer Abschnitt 5.6 und 9.7, Bedrohungen in 10.2, O30 und O31 in 14, Funktionen F29, F36, F37. `CLAUDE.md` (Isolation, Garmin-Token), `bridge/README.md` (eingefroren). Task-Dateien T21 bis T27 und Tabelle in `README.md`.

## Abnahme

`sh scripts/check-privacy.sh`, `sh scripts/check-isolation.sh`, `sh scripts/check-i18n.sh` grün. Keine Code-Änderung.
