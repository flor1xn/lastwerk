# bridge

> **Eingefroren seit T20 (15. Oktober 2026).** Die App meldet sich selbst bei Garmin an und holt Aktivitäten und Erholungswerte (SPEC 5.6, `crates/garmin`). Dieses Programm bleibt lauffähig für Vollabzug, Archiv und `push`, bekommt aber keine neuen Funktionen.

Python-Zusatzprogramm `lastwerk-bridge`. Holt Daten aus dem Garmin-Konto in ein lokales Archiv und stellt Dateien für Lastwerk bereit. Entsteht ab M4a. Die Garmin-Anmeldung wird vom Nutzer lokal und interaktiv ausgeführt (L1), nie in einer Cloud-Session.

## Wichtige Hinweise

- **Inoffizieller Zugang.** Das Programm nutzt die Bibliothek `garminconnect` und damit eine nicht dokumentierte Schnittstelle von Garmin. Sie kann sich jederzeit ändern oder gesperrt werden. Der automatisierte Abruf kann gegen Nutzungsbedingungen verstoßen. Nutzung auf eigenes Risiko (SPEC 1.5, 13, 14 O18).
- **Nur lesend, mit einer Ausnahme.** Eine ausdrückliche Allowlist (`src/lastwerk_bridge/allowlist.py`) erlaubt nur lesende Methoden. Einzige Ausnahme ist `push --yes` mit eigener Freigabe für zwei Methoden (Abschnitt "Workouts übertragen"). Schreibende Methoden, die Anmeldung und alle Menstruations- und Schwangerschaftsmethoden stehen in der Sperrliste. Ein Test vergleicht beide Listen mit allen öffentlichen Methoden der installierten Bibliothek: nach einer Aktualisierung schlägt er fehl, bis jede neue Methode eingestuft ist.
- **Getrennt vom Kern.** Kein Import zwischen `bridge/` und `crates/` oder `app/`. Austausch nur über Dateien (SPEC 5.4). Prüfung: `sh scripts/check-isolation.sh`.
- **Zugangsdaten.** Token liegen im Benutzerprofil (`~/.garminconnect/garmin_tokens.json`), nie in diesem Ordner und nie im Repository. Zugangsdaten stehen in keiner Datei.
- **Kein Garmin-Datenexport.** Der Export aus dem Konto ist keine Quelle. Das Werkzeug funktioniert ohne ihn.
- **Tests laufen ohne Netz.** Jeder Test sperrt `socket` und nutzt eine Attrappe der Bibliothek (`tests/fakes.py`). Es gibt keine echten Antworten im Repository.

## Einrichtung

```
cd bridge
uv python install 3.12
uv sync --locked
```

Python 3.12 ist festgelegt (`.python-version`, `requires-python >= 3.12`). Abhängigkeiten sind in `pyproject.toml` gepinnt und in `uv.lock` aufgelöst.

## Befehle

```
uv run python -m lastwerk_bridge auth                          einmalig, interaktiv, MFA, legt Token an (L1)
uv run python -m lastwerk_bridge auth --check                  prüft das Token mit genau einem Aufruf
uv run python -m lastwerk_bridge probe --out ../local/probe    Sonde: je Datenart ein Aufruf, Bericht ohne Werte
uv run python -m lastwerk_bridge backfill --root R [--until D] [--max-calls N] [--dry-run]
uv run python -m lastwerk_bridge sync --root R [--refresh-days 7] [--max-calls N]
uv run python -m lastwerk_bridge coverage --root R
uv run python -m lastwerk_bridge import-device ORDNER --root R [--device NAME]    optional, USB-Kopie
uv run python -m lastwerk_bridge reconcile --root R [--name-pattern RE] [--tolerance S]
uv run python -m lastwerk_bridge normalize --root R --out ../local/exchange/out [--allow-sensitive KATEGORIE]
uv run python -m lastwerk_bridge push --plan ../local/exchange/in/plan.json [--dry-run]    Trockenlauf (Standard), sendet nichts
uv run python -m lastwerk_bridge push --plan P --yes --out ../local/exchange/out --max-workouts N   echter Versand
```

Gemeinsame Optionen, vor oder nach dem Unterbefehl: `--root` (Archivordner, enthält `manifest.jsonl`, im Normalfall `local/archive/garmin`), `--token-dir` (Standard `~/.garminconnect`), `--min-interval` (Sekunden), `--call-timeout` (Sekunden).

| Exit-Code | Bedeutung |
|---|---|
| 0 | ok |
| 1 | Fehler (Netz, Bibliothek, einzelne 429 oder 403). Bei `backfill` und `sync`: der Lauf lief durch, einzelne Abrufe sind als `error` im Manifest und werden beim nächsten Lauf wiederholt |
| 2 | Nutzungs- oder Voraussetzungsfehler: keine Anmeldung, ungültiges Token, fehlender Archivordner, ungültige Optionen. Es wird keine Anfrage gesendet |
| 3 | Abbruch wegen Sperre: drei Antworten 429 oder 403 in Folge, oder eine gespeicherte Sperre ist noch aktiv |

### Rückwirkender Abruf (`backfill`)

Holt das Konto in der Reihenfolge der SPEC (5.5, Abrufregel 1): Kontodaten und Zonen, Aktivitätsliste seitenweise, je Aktivität (neueste zuerst) Detailsatz und `original`, Tageswerte von gestern bis zum frühesten Datum des Kontos (neu nach alt), danach Bereichsendpunkte in Spannen. Alle Aufrufe laufen über den gedrosselten Aufrufer.

| Option | Wirkung |
|---|---|
| `--root R` | Archivordner, enthält `manifest.jsonl` |
| `--until JJJJ-MM-TT` | älteste Grenze, weiter zurück wird nicht abgerufen (Tage, Aktivitäten, Listenseiten) |
| `--max-calls N` | höchstens N Aufrufe an die Bibliothek je Lauf, Wiederholungen eingerechnet |
| `--dry-run` | berechnet denselben Plan, zeigt die Anzahl je Endpunkt, sendet nichts, braucht weder Anmeldung noch Archiv, schreibt keine Datei |

Das früheste Datum ist das ältere von Profildatum und ältester Aktivität. Die Feldnamen des Profildatums (`createDate`, `createdDate`, `registrationDate`) sind bis zur Sonde in L1 eine Annahme; fehlt das Feld, gilt die älteste Aktivität, fehlt auch sie, braucht der Lauf `--until`. Ein Lauf endet nach dem Budget oder nach vollständigem Plan mit einer Zusammenfassung (Aufrufe, neue Dateien, leere, nicht verfügbare, Fehler, noch offen). Fortsetzen: derselbe Befehl noch einmal, bereits geholte Schlüssel werden übersprungen. Nach einer Sperre (Exit-Code 3) erst nach Ablauf der Frist. Bei vielen Jahren ist der Umfang groß (SPEC 5.5, Abrufregel 2): `--dry-run` zeigt ihn vorab, `--max-calls` teilt ihn auf mehrere Läufe.

Der Plan wird bei jedem Lauf aus Register und Manifest berechnet, `state.json` im Archiv ist nur eine Momentaufnahme der Zeiger. Bereichsendpunkte werden in Spannen von `max_span_days` je Registereintrag geteilt (Konvention 28 Tage, bis die Sonde in L1 misst, SPEC 14 O26), Dateien `ranges/<endpoint>/<start>_<end>.json`. Sensible Klassen (Gewicht, Körperzusammensetzung, Blutdruck, Hydration, Ernährung, Lifestyle) liegen unter `sensitive/<kategorie>/`. Menstruation und Schwangerschaft stehen weder im Register noch im Plan.

`original`: Garmin liefert ein ZIP. Es bleibt unverändert als `original.zip` liegen, die erste `.fit`-Datei wird als `original.fit` entpackt (eigener Manifest-Eintrag `original_fit`, Größenlimit 256 MiB, Pfade im ZIP werden ignoriert). Schlägt das Entpacken fehl, steht `error` mit `error_class` im Manifest und `coverage` führt die Aktivität auf.

### Inkrementeller Abruf (`sync`)

Holt nur Neues seit dem letzten Lauf und braucht ein vorhandenes Archiv (zuerst `backfill`, sonst Exit-Code 2).

| Option | Wirkung |
|---|---|
| `--root R` | Archivordner |
| `--refresh-days N` | so viele letzte Tage immer neu holen, Standard 7 (Faustregel) |
| `--max-calls N` | wie bei `backfill` |

Aktivitätsliste ab Seite 0, immer neu, bis eine Seite eine bekannte Aktivität enthält; für neue Aktivitäten (und solche der letzten N Tage mit fehlendem Schlüssel) der Detailsatz. Tageswerte der letzten N Tage und heute werden immer neu geholt, ebenso Tage, die beim letzten Abruf noch vorläufig waren. Bereichsendpunkte laufen von N Tage vor ihrem bisherigen Ende bis heute. Kontodaten (Zonen, FTP, Geräte) werden alle N Tage erneuert. Neu Geholtes erzeugt neue Dateien mit Zeitstempel im Namen und neue Manifest-Einträge, nichts wird überschrieben, der jüngste Eintrag je Schlüssel gilt.

### Abdeckung (`coverage`)

Liest nur das Archiv, sendet nichts und braucht keine Anmeldung. Je Registereintrag: Status (`verfügbar`, `leer`, `nicht verfügbar`, `ungeprüft`), ältestes und jüngstes Datum der Daten, Anzahl Dateien und Schlüssel je Status. Dazu Aktivitäten ohne `original.fit` mit Grund. Ausgabe als Text auf der Konsole und als `coverage.json` im Archiv (abgeleitet, wird überschrieben). Fehler beim Abruf zählen als `ungeprüft` und erscheinen unter den Schlüsseln. Exit-Code 2, wenn der Archivordner fehlt.

### USB-Kopie (`import-device`, optional)

Rückfall, wenn der Garmin-Zugang gesperrt ist (SPEC 4.2 F28, 12 M4c). Braucht weder Anmeldung noch Netz und schreibt nie auf das Gerät. `ORDNER` ist der Datenträger oder ein Ordner, der `Activity`, `Monitor` oder `Sleep` enthält, direkt oder unter `GARMIN/` (Groß- und Kleinschreibung egal). Fehlt der Ordner oder keiner der drei Unterordner, endet der Befehl mit Exit-Code 2 und schreibt nichts.

| Option | Wirkung |
|---|---|
| `ORDNER` | Geräteordner, Pflicht |
| `--root R` | Archivordner, Pflicht (wird bei Bedarf angelegt) |
| `--device NAME` | Name unter `device_usb/`, Standard: Name des Ordners (nur `A-Z a-z 0-9 . _ -`, anderes wird zu `_`) |

Die Bridge parst kein FIT. Eine Datei gilt als FIT-Datei, wenn sie in einem der drei Ordner liegt, auf `.fit` endet (Groß- und Kleinschreibung egal) und der Kopf stimmt (Byte 0 gleich 12 oder 14, Kennung `.FIT` an Byte 8 bis 11). Alles andere in diesen Ordnern, jede `.fit`-Datei in anderen Ordnern und jeder Symlink zählen als `unbekannt` und werden nicht kopiert. Dateien in anderen Ordnern ohne Endung `.fit` (Musik und Ähnliches) werden nicht gezählt.

Kopiert wird unverändert und atomar nach `device_usb/<geraet>/<Ordner>/<dateiname>`, nie überschrieben. Je Datei ein Manifest-Eintrag (`kind: device_usb`, `endpoint: device_usb/<Ordner>`, `params` mit `device` und `file`, `path`, `bytes`, `sha256`). Ein zweiter Lauf überspringt Dateien mit gleicher SHA-256. Hat eine Datei gleichen Namens anderen Inhalt, entsteht eine neue Datei mit Zeitstempel im Namen, die erste bleibt; der Bericht zählt sie als `abweichend`.

Der Prüfbericht steht auf der Konsole und als `device-import-report.json` im Archiv (abgeleitet, wird überschrieben): je Ordner erkannt, kopiert, übersprungen, unbekannt, dazu die Dateinamen. Keine Messwerte, keine Prüfsummen.

### Abgleich (`reconcile`, optional)

Liest nur das Archiv und sendet nichts. Vergleicht die Aktivitäten der API (Kennung und `startTimeGMT` aus `activities/<yyyy>/<id>/summary.json`, sonst aus der Listenseite) mit den Dateien unter `device_usb/*/Activity/` und meldet `nur_api` (Kennungen), `nur_geraet` (Dateinamen) und `beide` (Anzahl), auf der Konsole und als `reconcile-report.json` (abgeleitet). Ohne Archivordner oder ohne Aktivitätsdateien der USB-Kopie: Meldung und Exit-Code 2. Lücken sind kein Fehler, der Befehl endet dann mit 0.

Zuordnung (Konvention bis zur Prüfung am echten Gerät, SPEC 14 O4): Garmin benennt Aktivitätsdateien auf dem Gerät nach dem Startzeitpunkt, zum Beispiel `2026-05-10-06-00-00.fit`; manche Geräte nutzen die Aktivitätskennung (`12345678901.fit` oder `12345678901_ACTIVITY.fit`, mindestens sechs Ziffern). Der Zeitstempel gilt als UTC und muss höchstens 120 s von `startTimeGMT` abweichen. Je Aktivität und je Datei gibt es höchstens eine Zuordnung, die kleinste Abweichung zuerst. Dateien mit unpassendem Namen stehen unter `nicht_zuordenbar`, nicht unter `nur_geraet`.

| Option | Wirkung |
|---|---|
| `--name-pattern RE` | regulärer Ausdruck für den Dateinamen mit den Gruppen `year`, `month`, `day`, `hour`, `minute`, `second` oder `activity_id` (Groß- und Kleinschreibung egal) |
| `--tolerance S` | Toleranz in Sekunden, Standard 120 |
| `--name-offset-minutes M` | Versatz der Namenszeit gegenüber UTC, Standard 0 (Ortszeit UTC+2: `120`) |

`coverage` nutzt dieselbe Zuordnung mit den Standardwerten: Aktivitäten ohne `original.fit`, zu denen eine Gerätedatei gehört, stehen dort mit dem Zusatz "über Gerät vorhanden". Ohne USB-Kopie im Archiv ändert sich der Bericht nicht.

Die Tests erzeugen die Geräteordner zur Laufzeit (`tests/device_fixture.py`), es liegt keine `*.fit`-Datei im Repository.

### Normalisierung (`normalize`)

Leitet aus dem Archiv die Dateien für Lastwerk ab (SPEC 5.4, 12 M4d). Braucht weder Anmeldung noch Netz und ruft die Bibliothek nie auf. Lastwerk startet das Programm nie, der Nutzer führt den Befehl selbst aus und liest danach mit `lastwerk import` und `lastwerk import-recovery` ein.

| Option | Wirkung |
|---|---|
| `--root R` | Archivordner, Pflicht |
| `--out O` | Austauschordner `exchange/out`, Pflicht. Darf sich nicht mit dem Archiv überlappen (Exit-Code 2) |
| `--allow-sensitive KATEGORIE` | gibt eine Kategorie aus `sensitive/` frei, wiederholbar, Standard keine |

Ausgabe unter `--out`:

| Pfad | Inhalt |
|---|---|
| `activities/<id>.fit` | die Bytes von `original.fit`, unverändert (Prüfsumme gegen das Manifest geprüft) |
| `recovery/JJJJ-MM-TT.json` | ein Erholungstag nach `schema/recovery-day.schema.json` (SPEC 7.3): `schema_version` 1, `date`, `source` (`garmin-bridge <Version>`), `fetched_at` (jüngster Abruf der benutzten Quellen laut Manifest) und die Werte, die das Archiv liefert |

Quellen sind die jüngsten `ok`-Einträge des Manifests: `original_fit` je Aktivität und die Tagesendpunkte `hrv`, `rhr`, `heart_rates` und `sleep`. Ein Tag ohne einen verwertbaren Wert bekommt keine Datei, fehlende Werte fehlen in der Datei. Ein Wert außerhalb der Bereiche des Schemas wird weggelassen und im Bericht gezählt, nie gekürzt. **Die Feldnamen der Garmin-Antworten sind Annahmen bis zur Sonde in L1** (`FIELD_MAP` in `normalize.py`, SPEC 14 O9). Zeigt die Sonde andere Namen, genügt es, die Tabelle anzupassen.

`sensitive/` (Klasse S3, SPEC 5.5) wird nie geöffnet, solange keine Kategorie freigegeben ist. Auch bei Freigabe leitet M4d daraus noch nichts ab, die Option hält nur den Platz und steht im Bericht. Das Programm liest nur Pfade des Manifests unter `activities/` und `daily/`, folgt keinem Symlink aus dem Archiv heraus oder nach `sensitive/` und lehnt `..` und absolute Pfade ab.

Wiederholbar: Eine Ausgabedatei wird nur geschrieben, wenn sie fehlt oder ihre SHA-256 abweicht, ein zweiter Lauf auf unverändertem Archiv schreibt nichts. Geschrieben wird atomar. Bericht auf der Konsole und als `normalize-report.json` im Archiv (abgeleitet, wird überschrieben, nur Zählungen und Kennungen, keine Messwerte). Exit-Code 1, wenn Quellen fehlen, beschädigt sind (Prüfsumme weicht ab) oder keine gültige Kennung tragen, der Lauf läuft trotzdem zu Ende. Exit-Code 2 bei fehlendem Archivordner, überlappenden Ordnern oder unbekannter Kategorie.

Die Tests bauen das Archiv zur Laufzeit mit `backfill` gegen die Attrappe des Kontos (`tests/synthetic.py`), mit Markerwerten in `sensitive/`, die in keiner Ausgabedatei vorkommen dürfen. Ein Test schreibt nach `../target/exchange/out`, damit `lastwerk import-recovery --dir target/exchange/out/recovery --db target/m4.db` aus dem Repository-Wurzelordner läuft.

### Workouts übertragen (`push`)

Legt aus den Einheiten eines Plans Workouts in Garmin Connect an und plant sie (SPEC 4.2 F22, 12 M6). **Der Trockenlauf ist der Standard.** Ohne `--yes` wird nichts gesendet, es entsteht kein Client und es braucht keine Anmeldung.

| Option | Wirkung |
|---|---|
| `--plan P` | `plan.json` (Schema `../schema/plan.schema.json`), Pflicht. Wird vor jeder Nutzung geprüft: Größe (1 MiB), JSON, Schema, `schema_version` 1. Ungültig: Exit-Code 2, nichts gesendet |
| `--dry-run` | Trockenlauf, auch ohne die Option der Standard |
| `--yes` | echter Versand, schließt `--dry-run` aus. Verlangt `--out` und `--max-workouts` |
| `--out O` | Ordner `exchange/out`. Trockenlauf: schreibt zusätzlich `push-dry-run.json`. Mit `--yes`: dort liegt `push-report.json` |
| `--max-workouts N` | nur die ersten N gewählten Einheiten in Planreihenfolge |
| `--from D`, `--to D` | nur Einheiten im Zeitraum. Standard für `--from`: im Trockenlauf keine Grenze, mit `--yes` heute |
| `--templates T` | Vorlagenkatalog, Standard `catalog/templates.json` im Repository (relativ zur Paketdatei) |
| `--locales L` | Ordner mit `de.json` und `en.json`, Standard `locales` neben dem Katalog |

**Trockenlauf.** Die Standardausgabe ist ein JSON-Dokument: je Einheit (ohne Ruhetage) Datum, Platz, Vorlage, Titel, Dauer, Warnungen und die vollständige Nutzlast für `upload_workout`. Meldungen gehen auf die Fehlerausgabe, damit die Standardausgabe reines JSON bleibt. Der Titel kommt aus `template.<id>.name` der Plan-Sprache.

**Nutzlast.** `workoutName`, `sportType` (Rad 2, Lauf 1, Schwimmen 4, Kraft 5), ein Segment mit `workoutSteps`: `ExecutableStepDTO` (`warmup`, `interval`, `recovery`, `cooldown`) mit Zeit in Sekunden und Ziel `power.zone` (Rad), `pace.zone` (Lauf, Schwimmen) oder `heart.rate.zone` samt `zoneNumber`, Wiederholungen als `RepeatGroupDTO`. Dauern wie im Planer: Intervallblöcke bleiben, die übrigen Schritte werden auf ganze Minuten gedehnt, die Summe ist `duration_min`. Kraft ist ein Schritt mit Zeitvorgabe, der RPE steht im Titel. Einzelheiten und Annahmen: SPEC 5.5, "Festlegungen aus M6".

**Die Struktur ist bis zum ersten echten Versand eine Annahme.** Sie ist aus Dokumentation und Quelltext von `garminconnect` 0.3.17 abgeleitet (Q21, Q27), nicht an echten Workouts geprüft. Die Zonennummern zeigen auf die Zonen im Garmin-Profil, nicht auf die Grenzen aus `catalog/zones.json`. Ob Garmin `zoneNumber` bei `pace.zone` annimmt, ist offen.

**Echter Versand.** Eigener Schreibpfad `WorkoutWriteCaller` mit der Freigabe `WORKOUT_WRITE_ALLOWLIST` (`upload_workout`, `schedule_workout`, sonst nichts). Beide Methoden bleiben in der Sperrliste, der lesende Aufrufer lehnt sie weiter ab. Je Einheit zwei Aufrufe, gedrosselt (3 s Mindestabstand, Faustregel), **ohne automatische Wiederholung** (ein zweiter POST nach einem Zeitlimit würde Workouts doppelt anlegen), Abbruch beim ersten Fehler, höchstens `2 * N` Aufrufe. Ohne Token-Verzeichnis: Exit-Code 2 ohne Anfrage. Bei aktiver Sperre: Exit-Code 3 ohne Anfrage. Termine vor heute werden nicht geplant.

`exchange/out/push-report.json` hält je Einheit Datum, Vorlage, Titel, `workout_id` und den Zustand, aber keine Nutzlast und keine Zugangsdaten:

| Zustand | Bedeutung | Nächster Lauf |
|---|---|---|
| `ok` | angelegt und geplant | übersprungen |
| `uploaded` | angelegt, Planen fehlgeschlagen | plant nur noch |
| `error` | Anfrage sicher nicht angekommen (zum Beispiel 4xx) | versucht es noch einmal |
| `unknown` | Zeitlimit, Netzfehler, 5xx oder Antwort ohne Kennung: das Workout kann angelegt sein | wird nie automatisch wiederholt. In Garmin Connect prüfen, danach den Eintrag aus der Datei löschen |

Exit-Code 1 bei Fehler oder unklarem Ergebnis, 2 bei ungültiger Anmeldung, 3 bei Sperre. Löschen und Ändern von Workouts sind nicht Teil des Programms: ein geänderter Plan (neue `plan_id`) legt neue Workouts an, die alten sind in Garmin Connect von Hand zu entfernen.

**Nur mit Attrappen getestet, nie gegen ein echtes Konto ausgeführt.** Der erste echte Versand ist ein manueller Schritt des Nutzers, nach Klärung der Nutzungsbedingungen (SPEC 14 O18):

```
uv run python -m lastwerk_bridge push --plan P --dry-run > trockenlauf.json    prüfen
uv run python -m lastwerk_bridge push --plan P --yes --out ../local/exchange/out --max-workouts 1
```

Danach das eine Workout in Garmin Connect prüfen (Name, Schritte, Zonen, Datum) und erst dann mehr senden.

### Sonde (`probe`)

Schreibt nur in `--out`: `probe-report.json` (Status je Datenart, Feldnamen und Typen, gemessene Bereichsspanne und Historientiefe, Aufrufzahlen, keine Werte) und `responses/` mit den Rohantworten. Die Rohantworten enthalten Gesundheits- und Standortdaten und bleiben unter `local/`. Reihenfolge: je Registereintrag ein Aufruf für ein Datum der letzten Woche, dann je Bereichsendpunkt die größte Spanne ohne Fehler (verdoppeln, dann binär eingrenzen, höchstens 8 Aufrufe), dann für Einträge mit `consumer: true` das früheste Datum mit Daten (Jahresschritte, dann eingrenzen auf 30 Tage, höchstens 10 Aufrufe).

## Drosselung (vorläufig, SPEC 14 O24)

Alle Aufrufe laufen über `ThrottledCaller` (`src/lastwerk_bridge/throttle.py`). Startwerte sind Faustregeln bis zur Messung in L1 und werden nur bei Erfolg gelockert:

| Wert | Standard |
|---|---|
| Mindestabstand | 1,5 s plus Zufallsanteil bis 0,5 s |
| Zeitlimit je Aufruf | 30 s |
| Wiederholungen bei Netzfehlern und Zeitüberschreitung | 2, Backoff 2 s und 4 s |
| Pause nach 429 oder 403 | 30 s |
| Abbruch | nach 3 Antworten 429 oder 403 in Folge, Zustand in `<root>/state/lockout.json` |
| Sperrfrist | 6 Stunden, folgende Läufe brechen sofort mit Exit-Code 3 ab |

Kein Parallelabruf, keine Umgehung, kein Neuversuch der Anmeldung in einer Schleife. Die Sperre endet mit der Frist, die Datei kann nach Ablauf gelöscht werden.

## Archiv

Layout nach SPEC 5.5 unter `--root` (`account/`, `activities/`, `daily/`, `ranges/`, `sensitive/`, optional `device_usb/`, dazu `state.json`, `coverage.json`, `registry.yaml`, `device-import-report.json`, `reconcile-report.json`, `normalize-report.json`, die abgeleitet sind und keinen Manifest-Eintrag haben). Antworten werden unverändert und atomar geschrieben, nie überschrieben. `manifest.jsonl` wird nur angehängt und gegen `../schema/archive-manifest.schema.json` geprüft. Leere und nicht verfügbare Antworten werden vermerkt und nicht erneut abgefragt, die letzten 7 Tage immer. `registry.yaml` ist das Endpunkt-Register; es wird aus `src/lastwerk_bridge/registry.py` erzeugt und beim Lauf ins Archiv kopiert.

## Prüfen

```
cd bridge
uv run pytest -q
uv run ruff check src tests
uv run ruff format --check src tests
```

## Ablageorte

| Zweck | Ort |
|---|---|
| Archiv (unveränderte Antworten, Manifest) | `local/archive/garmin/` |
| Austausch mit Lastwerk | `local/exchange/out` und `local/exchange/in` |
| Antworten der Sonde | `local/probe/` |
| Token | `~/.garminconnect/` (Benutzerprofil, außerhalb des Projekts) |

Vorgaben und Befehle: [SPEC 5.5 und 13](../docs/SPEC.md).
