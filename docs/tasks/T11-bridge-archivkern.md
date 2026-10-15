# T11: Zusatzprogramm, Archivkern offline (bridge)

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | M4a (Teil 2) |
| Voraussetzung | T00 |
| cloudfähig | ja (nur offline mit Attrappen-Client) |
| Bereich | `bridge/` |
| SPEC | 5.5, 7.3, 10.2, 11.4 (Bridge), 12 (M4a), 13, 14 (O21, O24 bis O26) |

## Ziel

Der Kern des Zusatzprogramms `lastwerk-bridge`: Allowlist, gedrosselter Aufrufer, Fortsetzung, Manifest, Anmeldeprüfung. Vollständig mit Attrappen getestet, ohne Netzwerk und ohne Garmin-Konto.

## Warum

Der Abruf ist das riskanteste Teilstück (Kontosperre, ungewollte Schreibzugriffe). Er soll durch Tests abgesichert sein, bevor er je ein echtes Konto sieht.

## Umfang

- Python-Projekt in `bridge/`: `pyproject.toml` (Python 3.12, uv), `src/lastwerk_bridge/`, `tests/`, `uv.lock` einchecken. `garminconnect` mit Mindestversion 0.3.17 (SPEC 13) und `curl_cffi` gepinnt. Prüfe in der Cloud, dass beides installierbar ist, und vermerke Versionen.
- Kommandozeile `python -m lastwerk_bridge` mit `auth --check` (verlangt vorherige Anmeldung, sonst Exit-Code 2 und keine Netzwerkanfrage) und Gerüst für die Befehle aus M4b.
- Befehl `probe --out <ordner>` (wird in L1 vom Nutzer ausgeführt): je Datenart des Registers genau ein Aufruf für ein Datum der letzten Woche, danach Messung der größten Bereichsspanne je Bereichsendpunkt und der Historientiefe, Ausgabe nur in den angegebenen Ordner. Hier nur mit Attrappen-Client getestet.
- Allowlist und Sperrliste der Bibliotheksmethoden. Jede öffentliche Methode der installierten Bibliothek steht in genau einer Liste. Menstruations- und Schwangerschaftsmethoden stehen in der Sperrliste und werden nie aufgerufen. Schreibende Methoden stehen in der Sperrliste (sie werden erst in T16 und nur mit eigener Freigabe angefasst).
- Aufrufer mit Mindestabstand, Zeitlimit je Aufruf, begrenzter Wiederholung, Abbruch nach drei Fehlern 429 oder 403 in Folge (Zustand gespeichert, Exit-Code 3, klare Meldung), kein Parallelabruf.
- Archiv unter `<root>/…` mit unveränderten Antworten, atomarem Schreiben, `manifest` mit SHA-256 je Datei. Fortsetzung ohne Doppelabruf, negative Zwischenspeicherung leerer Antworten (die letzten N Tage werden dennoch erneut abgefragt).
- `schema/archive-manifest.schema.json` und ein erstes `bridge/registry.yaml` (Endpunkt-Register aus der Methodenliste, Status "ungeprüft bis L1").
- `bridge/README.md` mit Hinweis auf den inoffiziellen Zugang (SPEC 1.5).

## Nicht im Umfang

Anmeldung am echten Konto und Ausführen der Sonde gegen das echte Konto (L1), `backfill` und `sync` (T12), USB (T13), `normalize` (T14), Workouts übertragen (T16).

## Vorgaben

- **Netzwerk ist im Test gesperrt.** Tests laufen mit einem Attrappen-Client. Ein Test stellt sicher, dass ohne Anmeldung keine Netzwerkanfrage entsteht (zum Beispiel durch Sperren von `socket`).
- Keine Zugangsdaten, keine Tokens, keine echten Antworten im Repository. Token-Verzeichnis ist `~/.garminconnect`, nie im Projekt.
- `bridge/` importiert nichts aus `crates/` oder `app/` (`sh scripts/check-isolation.sh`).
- Standardwerte der Drosselung vorsichtig wählen und als vorläufig kennzeichnen (offen O24, Messung in L1).

## Abnahme

```
cd bridge
uv sync --locked
uv run pytest -q tests/test_allowlist.py tests/test_manifest.py tests/test_throttle.py tests/test_resume.py
uv run pytest -q
cd ..
sh scripts/check-isolation.sh
sh scripts/check-privacy.sh
```

Erfolg (SPEC 12, M4a Teil 2): Allowlist-Test deckt alle öffentlichen Methoden ab, 429 oder 403 stoppen den Lauf nach drei Fehlern in Folge mit Exit-Code 3, ein hängender Aufruf wird durch das Zeitlimit beendet, ein zweiter Lauf holt nichts doppelt, jede Archivdatei hat genau einen Manifest-Eintrag mit korrekter SHA-256, ohne `auth` keine Netzwerkanfrage.

## Manuelle Schritte

Der Befehl `auth --check` gegen das echte Konto (genau ein Aufruf) gehört zu L1.

## Blocker oder Vermerke

Keine Blocker. Erledigt am 13. Oktober 2026 in einer Cloud-Session (Linux).

### Abhängigkeiten (Name, Version, Lizenz, Grund)

| Name | Gepinnt | Gelöst | Lizenz | Grund |
|---|---|---|---|---|
| garminconnect | `==0.3.17` | 0.3.17 | MIT | Zugang zur Garmin-Schnittstelle (SPEC 13) |
| curl-cffi | `==0.16.3` | 0.16.3 | MIT | TLS-Profil der Bibliothek gegen Cloudflare-403 (SPEC 5.5, 13). Von garminconnect als `>=0.15.0` verlangt, hier fest gepinnt |
| pytest (dev) | `>=9.1.1` | 9.1.1 | MIT | Tests |
| ruff (dev) | `>=0.17.0` | 0.17.0 | MIT | Lint und Format (`uv run ruff check src tests`, `uv run ruff format --check src tests`) |

Transitiv (aus `uv.lock`): requests 2.34.2, ua-generator 2.1.6, cffi 2.1.1, pycparser 3.11, certifi, charset-normalizer, idna, urllib3. Keine YAML-Bibliothek: das Register wird als Python-Modul geführt und nach `registry.yaml` geschrieben, ein Test hält beide gleich. Keine `jsonschema`-Abhängigkeit: die Manifest-Prüfung ist eine kleine eigene Umsetzung der im Schema verwendeten Schlüsselwörter, die Schemadatei bleibt die Quelle.

### Python-Version (O21)

`uv python install 3.12` lief in der Cloud (CPython 3.12.13). `.python-version` ist `3.12`, `requires-python = ">=3.12"`. Zusätzlich geprüft: Dieselbe gepinnte Kette installiert unter Linux auch mit CPython 3.14.6 (`uv sync --locked --python 3.14`), alle 92 Offline-Tests bestehen dort. Windows ungeprüft. Festlegung bleibt 3.12, in SPEC 14 O21 vermerkt.

### Abweichungen und Festlegungen

- `--root` ist der Ordner, der `manifest.jsonl` direkt enthält (`local/archive/garmin`), passend zu den Befehlen in T12. Der Sperrzustand liegt unter `<root>/state/lockout.json`; ohne `--root` (nur `auth --check`) unter `<token-dir>/state/`.
- Drosselungswerte sind vorläufig (SPEC 14 O24): Mindestabstand 1,5 s plus bis 0,5 s Zufall, Zeitlimit 30 s, 2 Wiederholungen bei Netzfehlern, 30 s Pause nach 429 oder 403, Sperre 6 Stunden.
- Die Sonde schreibt zusätzlich zum Bericht die Rohantworten nach `<out>/responses/`, weil L1 daraus die Feldliste ableitet. Der Bericht selbst enthält keine Werte (Test).
- `original` (`download_activity` mit `ORIGINAL`) liefert laut Bibliothek ein ZIP mit der FIT-Datei. Das Archiv legt die Bytes unverändert als `original.fit` ab, das Entpacken entscheidet T12.
- `auth` ohne `--check` (interaktive Anmeldung) ist umgesetzt, weil L1 es braucht, aber hier nicht gegen das echte Konto getestet.
