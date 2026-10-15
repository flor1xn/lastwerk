# L1: Garmin-Anmeldung und Sonde (nur lokal)

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | M4a (Teil 1) |
| Voraussetzung | T11 |
| cloudfähig | **nein** |
| Bereich | lokaler Rechner des Nutzers |
| SPEC | 12 (M4a Teil 1), 13, 14 (O7, O9, O18, O21 bis O27) |

## Ziel

Die Anmeldung am echten Garmin-Konto funktioniert, und ein Bericht über Felder und Abdeckung (ohne Werte) liegt im Repository. Daraus entstehen die endgültigen Felder von `recovery-day`, das geprüfte Endpunkt-Register und der Startwert der Drosselung.

## Warum nicht in der Cloud

Die Anmeldung ist interaktiv (MFA), der Token liegt im Benutzerprofil, die Antworten enthalten Gesundheits- und Standortdaten, und der Zugang ist inoffiziell. Das gehört weder in eine Cloud-Session noch in ein Repository.

## Schritte (Nutzer, lokal, PowerShell)

1. Vorab entscheiden: O18 (Nutzungsbedingungen von Garmin gelesen, Risiko für das Konto akzeptiert), O28 (Sicherung des Archivs, Ort `local/archive/garmin/`).
2. `uv python install 3.12`, dann in `bridge/`: `uv sync`.
3. `uv run python -m lastwerk_bridge auth` (einmalig, MFA). Token liegen in `~/.garminconnect`, nicht im Projekt.
4. `uv run python -m lastwerk_bridge probe --out ../local/probe`: je Datenart genau ein Aufruf, möglichst für ein Datum der letzten Woche, danach Messung der größten Bereichsspanne und der Historientiefe. Gedrosselt. Echte Antworten bleiben in `local/probe/`.
5. Aus den Antworten (ohne Werte) schreiben: `docs/garmin-feldabdeckung.md` mit je Datenart "verfügbar", "leer" oder "nicht verfügbar", Feldnamen und Datentypen, dazu aktualisiertes `bridge/registry.yaml` und Startwert der Drosselung (O24).
6. Prüfen: `git status` zeigt nichts unter `local/`, `sh scripts/check-privacy.sh` ist grün. Dann committen und pushen.

## Abnahme

Der Bericht `docs/garmin-feldabdeckung.md` existiert, enthält je Datenart die Angabe "verfügbar", "leer" oder "nicht verfügbar" mit Feldnamen, und `git ls-files local` nennt nur `local/README.md`.

## Danach

Remote-Sessions können T12 und T14 mit den echten Feldern fertigstellen. Offene Fragen O9, O22 bis O26 werden hier beantwortet und in SPEC 14 nachgetragen.

## Blocker oder Vermerke

- 14. Oktober 2026: Anmeldung und Sonde liefen lokal durch (271 Aufrufe, 255 ok, 14 Fehler, keine Sperre). Bericht: `docs/garmin-feldabdeckung.md`. Offen aus der Sonde: `race_predictions` scheitert mit `ValueError`, `FIELD_MAP` (O9) ist gegen die Feldnamen zu prüfen.
