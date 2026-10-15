# T12: Zusatzprogramm, backfill, sync und coverage (bridge)

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | M4b |
| Voraussetzung | T11 |
| cloudfähig | ja (offline), Live-Lauf manuell |
| Bereich | `bridge/` |
| SPEC | 5.5, 12 (M4b), 14 (O26 bis O28) |

## Ziel

`backfill` (rückwirkend, gedrosselt, fortsetzbar), `sync` (inkrementell) und `coverage` (Abdeckungsbericht) für Aktivitäten und Tageswerte, vollständig offline gegen einen Attrappen-Client getestet.

## Umfang

- `backfill --root <archiv> [--max-calls N] [--dry-run]`: arbeitet von neu nach alt, erreicht das früheste Datum des Kontos (aus Profil und ältester Aktivität), endet nach höchstens N Aufrufen mit Zusammenfassung und ist fortsetzbar.
- `--dry-run`: listet die geplanten Aufrufe mit Anzahl je Endpunkt und sendet nichts.
- `sync --root <archiv>`: holt nur Neues seit dem letzten Lauf.
- `coverage --root <archiv>`: je Registereintrag Status ("verfügbar", "leer", "nicht verfügbar"), ältestes und jüngstes Datum, Anzahl Dateien. Aktivitäten ohne `original.fit` stehen im Bericht.
- Sensible Kategorien (Gewicht, Körperzusammensetzung, Blutdruck, Hydration, Ernährung, Lifestyle) in `sensitive/` getrennt ablegen (SPEC 2.1). Menstruation und Schwangerschaft werden nie abgerufen.
- Bereichsendpunkte in Spannen teilen (größte zulässige Spanne ist ein Parameter, Messung in L1, O26).

## Nicht im Umfang

Echter Abruf (manuell), USB (T13), `normalize` (T14), Aufrufe über `connectapi` ohne Wrapper (O27, erst nach Auswertung des Abdeckungsberichts).

## Vorgaben

- Alle Läufe über den gedrosselten Aufrufer aus T11. Kein zweiter Weg zum Netzwerk.
- Tests mit einem Attrappen-Client, dessen Antworten synthetisch sind.

## Abnahme

```
cd bridge
uv run pytest -q tests/test_backfill.py tests/test_sync.py tests/test_coverage.py
uv run pytest -q
cd ..
sh scripts/check-isolation.sh
```

Erfolg (SPEC 12, M4b, offline): `--dry-run` sendet nichts, ein Lauf mit `--max-calls 200` endet nach höchstens 200 Aufrufen und lässt sich fortsetzen, `backfill` arbeitet von neu nach alt, `coverage` listet jeden Registereintrag mit Status, Daten ohne Inhalt stehen als "leer" oder "nicht verfügbar".

## Manuelle Schritte (nur lokal, Nutzer)

```
uv run python -m lastwerk_bridge backfill --root local/archive/garmin --max-calls 200 --dry-run
uv run python -m lastwerk_bridge backfill --root local/archive/garmin --max-calls 200
uv run python -m lastwerk_bridge sync --root local/archive/garmin
uv run python -m lastwerk_bridge coverage --root local/archive/garmin
```

Vorher O18 (Nutzungsbedingungen) und O28 (Sicherung des Archivs) entscheiden.

## Blocker oder Vermerke

Keine Blocker. Erledigt am 13. Oktober 2026 in einer Cloud-Session (Linux), vollständig offline gegen einen synthetischen Attrappen-Client mit gesperrtem Socket.

### Abhängigkeiten

Keine neuen. Das Entpacken nutzt `zipfile` der Standardbibliothek.

### Festlegungen und Abweichungen (in SPEC 5.5 und 14 O26 nachgetragen)

- **`original`:** `download_activity` mit `ORIGINAL` liefert ein ZIP. Die Bytes bleiben unverändert als `original.zip` (Registereintrag `original`, `ext: zip`, `registry.yaml` neu erzeugt). Daraus wird die erste `.fit`-Datei als `original.fit` entpackt, mit eigenem Manifest-Eintrag und Endpunkt `original_fit` (jede Archivdatei hat genau einen Eintrag). Größenlimit 256 MiB (Faustregel), Pfade und Namen im ZIP werden ignoriert, es wird nur gelesen. Fehler ergeben `error` mit `error_class` (`BadZipFile`, `NoFitInZip`, `FitTooLarge`, `EmptyFit`). Das weicht vom Archivlayout in SPEC 5.5 ab (dort nur `original.fit`) und ist dort vermerkt. Die T11-Tests, die `original.fit` für die Bytes des Abrufs erwarteten, wurden auf `original.zip` angepasst.
- **Aufrufbudget:** `ThrottledCaller(max_calls=N)` zählt jeden Aufruf an den Client einschließlich Wiederholungen und wirft `CallBudgetExhausted`. So sendet ein Lauf nie mehr als N Aufrufe. Eine durch das Budget abgebrochene Wiederholung hinterlässt keinen Manifest-Eintrag und wird im nächsten Lauf neu versucht. `fetch_one` hat dafür den Parameter `force` und reicht die Ausnahme durch.
- **Plan:** `plan.py` (Stufen als Erzeuger) und `runner.py` (Ausführung, Zusammenfassung, `state.json`). Der Plan wird bei jedem Lauf aus Register und Manifest berechnet; `state.json` ist eine Momentaufnahme der Zeiger je Quelle, nicht maßgeblich.
- **Seitengröße der Aktivitätsliste:** 100, Konvention, in L1 zu prüfen. Eine Seite mit weniger Einträgen gilt als letzte.
- **Bereichsspannen:** `max_span_days` im Register bleibt `null` (Messung in L1, O26). Bis dahin gelten 28 Tage (Konvention, `DEFAULT_RANGE_SPAN_DAYS`). Raster ab 2000-01-01, damit die Schlüssel von Lauf zu Lauf gleich bleiben. Die letzte Spanne wird bei gestern gekürzt und von `backfill` bei geändertem Ende einmal erneut geholt.
- **Tag- und Bereichsendpunkte nebeneinander:** SPEC 5.5, Regel 2 bevorzugt Bereiche, wo sie dieselben Werte liefern. Ob sie es tun, zeigt erst L1. Bis dahin holt `backfill` beide (Tagesendpunkte und Bereichsendpunkte des Registers), die Aufrufzahl ist also die obere Schätzung. `--dry-run` zeigt sie vorab.
- **Frühestes Datum:** das ältere von Profildatum und ältester Aktivität. Die Feldnamen des Profildatums (`createDate`, `createdDate`, `registrationDate`) und die der Aktivitätsliste (`activityId`, `startTimeLocal`, `startTimeGMT`) sind Annahmen bis zur Sonde in L1 und in `plan.py` und `activities.py` als solche gekennzeichnet. Ohne beides braucht `backfill` die Option `--until`.
- **`sync`, Auffrischen:** Zusätzlich zu den letzten N Tagen wird ein Tageswert einmal erneuert, der höchstens N Tage nach dem Tag selbst geholt wurde (sonst bliebe er nach einer langen Pause dauerhaft vorläufig). Bereichsendpunkte laufen ab N Tage vor ihrem bisherigen Ende. Kontodaten werden alle N Tage erneuert (Zonen und FTP ändern sich). Aktivitäten werden nicht aufgefrischt. `sync` ohne Archiv endet mit Exit-Code 2.
- **Exit-Codes:** `backfill` und `sync` enden mit 1, wenn einzelne Abrufe als `error` vermerkt wurden (der Lauf lief durch und ist fortsetzbar), mit 3 bei Sperre, mit 2 bei ungültiger Anmeldung. `--dry-run` und `coverage` brauchen keine Anmeldung.
- **Offene Punkte für L1:** Feldnamen (siehe oben), Seitengröße, `max_span_days`, Historientiefe, ob Bereichsendpunkte die Tagesaufrufe ersetzen können, und das Verhalten beim Löschen einer Aktivität (Seitenversatz kann dann eine Aktivität überspringen; `sync` fängt neue Aktivitäten ab, ein Löschen ohne Neuabruf nicht).
- `scripts/check-isolation.sh` und `scripts/check-privacy.sh` bestehen, `uv run ruff check` und `ruff format --check` ebenfalls.
