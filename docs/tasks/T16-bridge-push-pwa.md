# T16: Workouts übertragen (Trockenlauf) und PWA-Entscheidung

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | M6 |
| Voraussetzung | T08, T11 |
| cloudfähig | ja (nur Trockenlauf und Dokument) |
| Bereich | `bridge/`, `docs/` |
| SPEC | 4.2 (F22), 4.3 (F31, F32), 12 (M6), 13, 14 (O4, O5) |

## Ziel

`push --dry-run` erzeugt je Einheit eines Plans die Nutzlast für Garmin Connect, ohne zu senden. Die PWA-Frage ist entschieden und dokumentiert.

## Umfang

- `python -m lastwerk_bridge push --plan <plan.json> --dry-run`: gibt je Einheit die Nutzlast aus und sendet nichts. Der Standard ist der Trockenlauf.
- Ein echter Versand ist ein eigener, ausdrücklich freigegebener Schreibpfad: eigene Allowlist für genau die Workout-Methoden, Option `--yes`, Protokoll in `exchange/out/push-report.json`. Er wird in diesem Task implementiert und mit Attrappen getestet, aber **nie** gegen ein echtes Konto ausgeführt.
- Nutzlast je Vorlage entspricht der erwarteten Struktur (Test gegen synthetische Beispiele, die aus der Bibliotheksdokumentation abgeleitet sind, Quelle im Testkommentar).
- `exchange/in/plan.json` wird vor jeder Nutzung gegen `schema/plan.schema.json` geprüft.
- `docs/pwa-entscheidung.md`: Varianten (LAN-Server, privater Tunnel, statische Veröffentlichung), Erreichbarkeit, Datenschutzfolge, Empfehlung (offen O5).

## Nicht im Umfang

Eine PWA-Implementierung. FIT-Workout-Dateien zum Kopieren auf das Gerät (F32), solange der Pfad je Gerät ungeklärt ist (O4).

## Abnahme

```
cd bridge
uv run pytest -q
uv run python -m lastwerk_bridge push --plan ../target/plan.json --dry-run
cd ..
cargo run -p lastwerk-cli -- export-ics target/plan.json --out target/plan.ics
sh scripts/check-isolation.sh
```

Erfolg (SPEC 12, M6): `--dry-run` gibt je Einheit die Nutzlast aus und sendet nichts (Test sperrt das Netzwerk), die Nutzlast entspricht der erwarteten Struktur, die Entscheidung zur PWA ist mit Variante, Erreichbarkeit und Datenschutzfolge dokumentiert.

## Manuelle Schritte

Der erste echte Versand eines einzelnen Workouts mit `--yes` durch den Nutzer, danach Kontrolle in Garmin Connect. Vorher O18 (Nutzungsbedingungen) klären.

## Blocker oder Vermerke

Keine Blocker. Erledigt am 14. Oktober 2026 in einer Cloud-Session (Linux), vollständig offline mit synthetischen Daten, Attrappen und gesperrtem Socket. Der echte Versand (`--yes`) wurde nie gegen ein Konto ausgeführt.

### Abhängigkeiten

Keine neuen. `jsonschema` kommt nicht als Abhängigkeit hinzu (siehe Entscheidung). `uv.lock` und `pyproject.toml` ändern sich nicht.

### Entscheidung zur Schemaprüfung

Die eigene Prüfung aus `manifest.py` (T11, T14) wurde um die Schlüsselwörter des Plan-Schemas erweitert (`items`, `minItems`, `maxItems`, `$ref` innerhalb von `$defs`, `if`/`then`/`else`, Format `uuid`; `const`, `enum`, `pattern`, `minimum`, `maximum`, `maxLength` waren da) statt `jsonschema` aufzunehmen. Gründe: `bridge/` bleibt ohne zusätzliche Laufzeitabhängigkeit (Lockfile, Lizenzprüfung, Lieferkette), die benötigten Schlüsselwörter sind wenige, und das Schema bleibt die einzige Quelle, weil die Datei zur Laufzeit gelesen wird. Absicherung: `unsupported_keywords` meldet Schlüsselwörter und Formate, die ein Schema nutzt und die nicht geprüft würden (Test über alle Schemas in `schema/`, außer `plan-edit` aus T15); die Negativfälle der Rust-Seite (T08) sind in `tests/test_plan_schema.py` nachgebildet. Einmaliger Abgleich außerhalb des Repositories mit `jsonschema` 4.26.0 über 6000 zufällige Verfälschungen eines gültigen Plans: kein Fall, in dem die eigene Prüfung etwas annimmt, was `jsonschema` ablehnt. Die eigene Prüfung ist strenger bei `date-time` (RFC 3339 mit Offset, `jsonschema` prüft das Format ohne Zusatzpaket nicht) und bei `integer` (`300.0` gilt nicht als Ganzzahl). Dabei wurden `format: date` und `date-time` auch für das Manifest enger gefasst (Muster vor dem Parsen), `true` ist nicht mehr gleich `1` bei `const` und `enum`; alle T11-Tests bestehen unverändert.

### Festlegungen und Abweichungen (in SPEC 5.4, 5.5, 10.2, 12 M6 und 14 O4, O5, O18 nachgetragen)

- **Katalog über Dateizugriff.** `push` liest `catalog/templates.json` (Option `--templates`, Standard relativ zur Paketdatei) und die Texte aus `locales/<locale>.json` daneben (Option `--locales`). Das ist ein Dateizugriff auf `catalog/`, kein Import, die Isolationsprüfung besteht. In SPEC 5.4 vermerkt.
- **Trockenlauf.** Standardausgabe ist reines JSON (Meldungen gehen nach stderr). Fehlt eine Textdatei der Plan-Sprache, ist der Titel die Vorlagen-ID. Eine Dauer außerhalb des Fensters der Vorlage ergibt die Warnung `duration_outside_template_window`, keinen Fehler (der Planer erzeugt das nie).
- **Dauer.** Konvention des Planers (SPEC 7.4), auf ganze Minuten, Rest an den größten Nachkommaanteil; die Summe der Schritte ist genau `duration_min`. Weil der Planer nur die Pausen *zwischen* den Wiederholungen zählt, ist eine Wiederholung eine `RepeatGroupDTO` über (Arbeit, Pause) mit `reps - 1` Durchläufen, danach die letzte Arbeit ohne Pause; bei zwei Wiederholungen (zum Beispiel `bike.threshold.2x20`) stehen die Schritte flach. Alternativ wäre die gängige Form "Gruppe mit Pause am Ende" gewesen, sie hätte die Gesamtdauer um die letzte Pause verlängert und bei Nenndauer von den Minuten des Katalogs abweichen lassen.
- **Annahmen bis zum ersten echten Versand (O4 analog):** Kennungen und Schlüssel aus `garminconnect/workout.py` (0.3.17); `pace.zone` mit `zoneNumber` für Lauf und Schwimmen (die Bibliothek kennt bei `pace.zone` nur Grenzwerte in m/s, ob Garmin die Zonennummer annimmt, ist offen); `heart.rate.zone` für Herzfrequenzschemata (kommt in keiner Vorlage vor); `zoneNumber` zeigt auf die Zonen im Garmin-Profil, nicht auf `catalog/zones.json`; Beschreibung `Lastwerk <id>, <Minuten> min`; Kraft als ein Zeitschritt ohne Ziel mit RPE im Titel.
- **Sicherheit des Schreibpfads.** `WorkoutWriteCaller` (Unterklasse von `ThrottledCaller`) erlaubt nur `upload_workout` und `schedule_workout`, keine lesende Methode. Der Aufrufer prüft über den neuen Haken `_ensure_method_allowed`, das Verhalten des lesenden Aufrufers ist unverändert. Keine automatische Wiederholung (`retries=0`), Mindestabstand 3 s, Aufrufbudget `2 * N`, Abbruch beim ersten Fehler. `--yes` verlangt `--out` und `--max-workouts` (Zusatz zum Auftrag, damit Umfang und Protokoll ausdrücklich feststehen). Termine vor heute werden mit `--yes` nicht geplant (Zusatz). Das Protokoll ist kumulativ und verhindert doppeltes Anlegen (Zusatz): `ok`, `uploaded`, `error`, `unknown`, siehe README. Ein unbrauchbares Protokoll stoppt vor jeder Anfrage (Exit-Code 2).
- **`pyproject.toml` und `uv.lock`** unverändert.

### Prüfstand

`uv run pytest -q` (bridge) 322 Tests bestanden, davon 75 in `test_push.py` und 68 in `test_plan_schema.py`; `ruff check` und `ruff format --check` sauber; `sh scripts/check-isolation.sh`, `check-privacy.sh`, `check-i18n.sh` bestehen. `push --plan ../target/plan.json --dry-run` gegen einen echten Plan der Rust-CLI (17 Wochen, 77 Einheiten): Exit-Code 0, jede Nutzlast summiert sich auf `duration_min`. `export-ics` schreibt 77 Veranstaltungen.

### Offen für den Nutzer (manuell)

- Erster echter Versand eines einzelnen Workouts mit `--yes --max-workouts 1`, danach Kontrolle in Garmin Connect (Name, Schritte, Zonen, Datum). Vorher O18 (Nutzungsbedingungen) klären. Zeigt Garmin eine andere Struktur, genügen Änderungen in `push.py` (Tabellen `SPORT_TYPES`, `STEP_TYPES`, `TARGET_TYPES`, `SCHEME_TARGET`).
- Bestätigung oder Änderung der PWA-Empfehlung (O5, `docs/pwa-entscheidung.md`).
