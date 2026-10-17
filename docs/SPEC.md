# Lastwerk: Spezifikation

Stand: 13. Oktober 2026
Status: Entwurf 0.5 (Arbeitsaufträge für Remote-Sessions in `docs/tasks/`, Gerüst M0 angelegt), Grundlage für M0 bis M3
Lizenz des Projekts: MIT OR Apache-2.0

Lastwerk ist ein lokales Desktop-Werkzeug für Ausdauertraining. Es liest FIT-Dateien aus einem Ordner, berechnet Belastung, Fitness, Ermüdung und Form, erzeugt regelbasierte Wochenpläne und exportiert sie. Ein optionales lokales Sprachmodell darf Pläne anpassen und erklären, es rechnet nie.

Die Meilensteine aus Abschnitt 12 sind für die Umsetzung in Arbeitsaufträge geschnitten: [tasks/README.md](tasks/README.md). Regeln für Sessions stehen in `CLAUDE.md`, die Einrichtung der Cloud-Sessions in [remote-sessions.md](remote-sessions.md).

Dieses Dokument hält Entscheidungen, Schnittstellen und Prüfkriterien fest. Zahlen, Formeln und Regeln tragen eine Quelle oder die Kennzeichnung **Faustregel**. Eine Quelle ist im Anhang A mit ihrem Prüfstatus geführt. Wo nur eine Sekundärquelle vorlag, steht das dort.

Lesehinweis zu Kennzeichnungen:

| Kennzeichen | Bedeutung |
|---|---|
| Q1 bis Q22 | Quelle im Anhang A, mit Prüfstatus |
| Faustregel | Verbreitete Praxisregel ohne belastbaren Beleg. Parameter ist einstellbar und im Plan als Faustregel sichtbar |
| Konvention | Festlegung dieses Projekts, damit Berechnungen reproduzierbar sind |
| offen | Wert oder Verhalten ist noch nicht entschieden, siehe Abschnitt 14 |

---

## 1. Entwicklungsumgebung

### 1.1 Entwicklungsrechner (Stand 13. Oktober 2026)

| Programm | Stand | Vorgehen, falls nötig |
|---|---|---|
| VS Code (`code`) | 1.141.0 | vorhanden |
| git | 2.54.0 | vorhanden |
| GitHub CLI (`gh`) | 2.97.0, angemeldet | vorhanden |
| winget | 1.29.380 | vorhanden |
| WebView2 Runtime | 154.0.4258.62 | vorhanden |
| Ollama (Client) | 0.31.2, Dienst läuft nicht | vorhanden, Dienst vor M5 starten |
| rustup, rustc, cargo | fehlt | rustup-init von rustup.rs, alternativ `winget install Rustlang.Rustup`. Toolchain `stable-x86_64-pc-windows-msvc` |
| Node.js, npm | fehlt | LTS-Installer von nodejs.org, alternativ `winget install OpenJS.NodeJS.LTS` |
| pnpm | fehlt | Anleitung auf pnpm.io/installation, setzt Node voraus |
| MSVC Build Tools | fehlt | Visual Studio Build Tools, Workload "Desktop development with C++" (visualstudio.microsoft.com/downloads) |
| Python | 3.14.5 (Standard), 3.12.7 vorhanden | Für das Zusatzprogramm Python 3.12 über `uv` festlegen, weil `garminconnect` Python 3.12 oder neuer verlangt und Abhängigkeiten mit 3.14 ungeprüft sind |
| uv | 0.13.0 | vorhanden |

Reihenfolge der Installation (nur nötig, wer lokal bauen will; Cloud-Sessions bringen Rust, Node und pnpm mit): MSVC Build Tools, rustup, Node.js, pnpm. Python und uv sind für Phase 0 und M4 schon vorhanden.

Hinweis: `uv`/`uvx` kann laut README von garmin_mcp unter Windows durch Smart App Control blockiert werden (Fehler CodeIntegrity 3077). Auf diesem Rechner nicht geprüft (O23).

Hardware des Entwicklungsrechners: AMD Ryzen AI 7 350 (8 Kerne, 16 Threads), 31,3 GB RAM, integrierte Radeon 860M mit 0,5 GB dediziertem Speicher, keine NVIDIA-GPU, 472 GB frei auf C:. Folge: Lokale Modelle laufen auf CPU oder integrierter Grafik. Sinnvoll sind kleine quantisierte Modelle im Bereich weniger Milliarden Parameter. Die Antwortgeschwindigkeit ist in M5 zu messen und nicht vorab zugesichert.

### 1.2 VS-Code-Erweiterungen

Installiert am 13. Oktober 2026 über `code --install-extension`. Vor der Installation wurde jede ID gegen den Marketplace geprüft.

| ID | Herausgeber | Installierte Version | Domain verifiziert | Zweck |
|---|---|---|---|---|
| rust-lang.rust-analyzer | The Rust Programming Language | 0.3.3081 | ja | Rust, clippy beim Speichern |
| tauri-apps.tauri-vscode | Tauri | 0.2.9 | nein | Tauri |
| svelte.svelte-vscode | Svelte | 110.3.1 | ja | Svelte |
| tamasfe.even-better-toml | tamasfe | 0.21.2 | nein | Cargo.toml |
| dbaeumer.vscode-eslint | Microsoft | 3.0.34 | ja | ESLint |
| esbenp.prettier-vscode | Prettier | 12.4.0 | ja | Formatierung |
| GitHub.vscode-pull-request-github | GitHub | 0.168.0 | ja | Pull Requests |
| GitHub.vscode-github-actions | GitHub | 0.32.3 | ja | Workflows |

Hinweise:

- Bei tauri-apps.tauri-vscode und tamasfe.even-better-toml ist die Herausgeber-Domain im Marketplace nicht verifiziert. Herausgebername und ID entsprechen den in der Tauri- und Rust-Dokumentation genannten Werkzeugen. Die Erweiterungen bleiben auf der Liste, die Einstufung ist hier festgehalten.
- Der Marketplace lieferte für rust-analyzer und die Pull-Request-Erweiterung Vorabversionen (0.4.3082 und 0.171.x). Installiert wurde die stabile Linie.
- Die Tauri-Erweiterung ist klein und wenig aktiv gepflegt. Sie ist Komfort, keine Voraussetzung.

Optional, nicht installiert, nur nach Rückfrage:

| ID | Begründung |
|---|---|
| ms-vscode.cpptools (Microsoft) | Debugger `cppvsdbg` für MSVC-Binaries mit PDB. Unter Windows MSVC die robustere Wahl gegenüber CodeLLDB |
| usernamehw.errorlens | Zeigt Compilerfehler und Clippy-Warnungen in der Zeile |
| qwtel.sqlite-viewer | Schreibgeschützter Betrachter der lokalen Datenbank, Herausgeber eine Einzelperson, Domain nicht verifiziert |

### 1.3 Dateien für das Repository

`.vscode/extensions.json`:

```json
{
  "recommendations": [
    "rust-lang.rust-analyzer",
    "tauri-apps.tauri-vscode",
    "svelte.svelte-vscode",
    "tamasfe.even-better-toml",
    "dbaeumer.vscode-eslint",
    "esbenp.prettier-vscode",
    "GitHub.vscode-pull-request-github",
    "GitHub.vscode-github-actions"
  ]
}
```

`.vscode/settings.json`:

```json
{
  "editor.formatOnSave": true,
  "files.eol": "\n",
  "files.watcherExclude": {
    "**/target/**": true,
    "**/node_modules/**": true,
    "**/local/**": true
  },
  "search.exclude": {
    "**/local/**": true
  },
  "rust-analyzer.check.command": "clippy",
  "[rust]": {
    "editor.defaultFormatter": "rust-lang.rust-analyzer"
  },
  "[toml]": {
    "editor.defaultFormatter": "tamasfe.even-better-toml"
  },
  "[svelte]": {
    "editor.defaultFormatter": "esbenp.prettier-vscode"
  },
  "[typescript]": {
    "editor.defaultFormatter": "esbenp.prettier-vscode"
  },
  "[json]": {
    "editor.defaultFormatter": "esbenp.prettier-vscode"
  },
  "[jsonc]": {
    "editor.defaultFormatter": "esbenp.prettier-vscode"
  }
}
```

`local/` ist aus Watcher und Suche ausgeschlossen, damit private Dateien nicht indexiert werden.

### 1.4 `.gitignore` (Mindestinhalt)

```
/target/
node_modules/
/local/*
!/local/README.md
*.fit
*.FIT
*.db
*.db-wal
*.db-shm
*.sqlite
.env
.env.*
garmin_tokens.json
__pycache__/
.venv/
```

`bridge/` ist der Quellordner des Zusatzprogramms und wird nicht ignoriert. Der Austauschordner heißt `exchange/` und liegt standardmäßig unter `local/exchange/` (5.4).

Ergänzend prüft CI, dass kein `*.fit` im Git-Index steht (Abschnitt 11).

### 1.5 Repository, Git und GitHub

| Punkt | Festlegung |
|---|---|
| Arbeitsordner | `<Benutzerordner>\Desktop\Code\lastwerk`, der Ordnername ist identisch mit dem Repository-Namen |
| Repository | Ein Repository für Kern, App, Zusatzprogramm, Schemas, Kataloge und Dokumentation |
| GitHub | `lastwerk`, öffentlich seit dem 15. Oktober 2026 nach der Checkliste unten. Neuanfang mit einem Anfangscommit ohne Vorgänger (T38, Weg b): Die alte Geschichte enthielt Personenbezug, sie liegt im privaten Archiv `lastwerk-archiv`. Commit-Hashes in älteren Aufträgen und in der Spezifikation beziehen sich auf dieses Archiv. Commits nur mit noreply-Adresse. Issues sind abgeschaltet, Interaktionen nur für Collaborators. Regeln für `main`: kein Force-Push, kein Löschen |
| Standardzweig | `main` |
| Hooks | `core.hooksPath = .githooks`, lokal gesetzt. `pre-commit` blockiert FIT-Dateien, Dateien unter `local/` außer `local/README.md`, Token-Dateien und Datenbanken |
| Commits | Betreffzeile auf Deutsch im Imperativ (Konvention), Rumpf erklärt das Warum |
| Isolation | `bridge/` importiert nichts aus `crates/` und `app/`, und umgekehrt. Austausch nur über Dateien nach 5.4. Eigene CI-Datei mit Pfadfilter. Die README in `bridge/` weist auf den inoffiziellen Zugang hin. Seit T20 ist `bridge/` eingefroren; der Garmin-Zugang der App liegt in `crates/garmin` und übernimmt aus `bridge/` nur Wissen (Endpunkte, Feldzuordnung, Drosselwerte), keinen Code |
| Ignorierte Daten | `local/` ist nicht versioniert. `git clean -x` und `git clean -X` löschen ignorierte Dateien einschließlich des Archivs und sind zu vermeiden |
| Veröffentlichung | Checkliste: Markenprüfung (O6), Lizenzdateien vorhanden, Secret-Scan ohne Treffer, `git ls-files "*.fit" "*.FIT"` leer, `git ls-files local` nur `local/README.md`, README mit Datenschutz- und Markenhinweis, `sh scripts/check-history.sh` auf einem Spiegelklon (`git clone --mirror`) ohne Fund, `reuse lint` ohne Befund, `zizmor` ohne Befund ab Stufe medium, `SECURITY.md` vorhanden |

---

## 2. Ziele und Nicht-Ziele

### 2.1 Entscheidungen aus der Anforderungsklärung

| Thema | Entscheidung | Anmerkung |
|---|---|---|
| Zielgruppe | Nur der Autor, öffentlich als Schaufenster | Kein Support-Versprechen. macOS und Linux nur "kompiliert in CI" |
| Zeitbudget | Mehr als 10 Stunden pro Woche | |
| Sportarten | Rad und Lauf vollständig. Schwimmen und Kraft als Bausteine | Datenlage für Schwimmen und Kraft in FIT-Dateien schwächer |
| Erholung (HRV, Schlaf) | Über den Garmin-Zugang der App (seit T20, 5.6), früher nur über das Zusatzprogramm | Aktivitäts-FIT-Dateien enthalten diese Daten nicht |
| Name | Lastwerk | crates.io frei, 0 GitHub-Treffer am 13. Oktober 2026. Markenregister (DPMA, EUIPO) nicht geprüft |
| Lizenz | MIT OR Apache-2.0 | |
| Oberfläche | Dicht, datenorientiert, dunkel zuerst, Deutsch und Englisch | |
| Speicherung | FIT-Ordner ist Quelle, SQLite ist neu aufbaubarer Cache | Nutzerdaten per JSON-Export sichern |
| Version 1 | M1 bis M3: Import, Auswertung, Planer, ICS-Export. Ohne Netzwerk und ohne LLM | |
| Handy | ICS-Datei zum manuellen Import. Garmin Connect über das Zusatzprogramm (eingefroren seit T20) | Kein Server in V1. PWA offen |
| Repository | Ein Repository `lastwerk` auf GitHub, zunächst privat, danach öffentlich (1.5). Das Zusatzprogramm liegt als Unterordner `bridge/` | Entscheidung des Projekts, Isolation per Regel (1.5) |
| Zusatzprogramm | Python-Programm `lastwerk-bridge` im Ordner `bridge/` auf Basis von python-garminconnect ab 0.3.17. **Seit T20 (15. Oktober 2026) eingefroren**: bleibt im Repository und lauffähig, die App braucht es nicht mehr | Q21, Q27 |
| Ablageort der privaten Daten | Alles im Projektordner unter `local/` (nicht versioniert). Einzige Ausnahme sind die Garmin-Tokens im Benutzerprofil (`~/.garminconnect`), weil sie wie ein Passwort wirken | Beantwortet O28 zum Ort, Sicherung bleibt offen |
| Umfang des Abrufs | Vollabzug aller lesbaren Datenarten, rückwirkend ab Kontoerstellung | Menstruation und Schwangerschaft werden nie abgerufen |
| Sensible Kategorien | Gewicht und Körperzusammensetzung, Blutdruck, Hydration, Ernährung, Lifestyle-Logging werden geholt, getrennt abgelegt und für Lastwerk und MCP gesperrt, bis sie je Kategorie freigegeben sind | |
| Archiv | Unveränderte Antworten (JSON, FIT) mit Manifest unter `local/archive/garmin/`, nicht versioniert, nicht in Cloud-Sync-Ordnern | Alles Weitere ist daraus neu aufbaubar |
| Ablauf des Abrufs | Manuell per Kommandozeile, in Etappen (`backfill`, `sync`) | Kein Hintergrunddienst |
| Quellen für Garmin-Daten | Bibliothek (Hauptweg), freie Methode `connectapi`, optional USB-Kopie der Geräteordner | Der Garmin-Datenexport (ZIP aus dem Konto) wird nicht genutzt. Das Werkzeug funktioniert ohne ihn |
| LLM in Lastwerk | Nur lokal: Ollama und lokale OpenAI-kompatible Server | Lastwerk ruft keine Cloud-API auf und speichert keinen API-Schlüssel |
| Claude | Nur als MCP-Client über `lastwerk-mcp`, auf Initiative des Nutzers | Ändert die frühere Entscheidung "kein Cloud-LLM", Rahmen in 9.6 |
| Datenumfang für Claude | Aggregierte Kennzahlen und einzelne Aktivitäten ohne GPS | Erholungswerte standardmäßig nicht freigegeben |
| Garmin-Zugang | **Seit T20 (Entscheidung vom 15. Oktober 2026): in Rust in der App**, Crate `lastwerk-garmin`, Anmeldung über die Garmin-Seite in einem eigenen Fenster, Token im Schlüsselbund des Betriebssystems (5.6). Vorher: eigener schlanker Kern im Ordner `bridge/` | Nur lesend (Allowlist). Grund der Wende: Python, `uv` und Kommandozeile sind für den Alltag zu viel Einrichtung |
| Plan-Ziel | Mit Wettkampftermin (rückwärts, mit Taper) und fortlaufend (Blöcke) | |
| Schwellenwerte | **Seit T39 (Entscheidung E1 vom 16. Oktober 2026): standardmäßig automatisch aus Garmin** mit Herkunft `garmin` und dem Datum aus Garmin, manuelle Werte gehen vor (bei gleichem Datum gewinnt manuell, 8.6). Vorher: manuell mit Gültigkeitsdatum, Schätzung aus FIT nur als Vorschlag. Die Schätzung aus FIT bleibt Vorschlag für Werte, die Garmin nicht liefert | Grund der Wende: Werte galten erst ab dem Eintragstag, frühere Aktivitäten zeigten "Schwellenwert fehlt" |
| Netzwerk | Keine Telemetrie. Kartenkacheln sind als Option erlaubt | Umsetzung: Standard aus, siehe 10.3 |
| Geräte | Edge-Radcomputer und Uhr | Zwei Dateiquellen, Duplikate möglich |

### 2.2 Ziele

1. FIT-Dateien aus einem Ordner einlesen, ohne Konto und ohne Netzwerk.
2. Belastung pro Aktivität berechnen, mit nachvollziehbarer Methode je Sportart.
3. Fitness, Ermüdung und Form als Verlauf darstellen und für Planwochen vorausberechnen.
4. Zonen aus Schwellenwerten ableiten, Zeit in Zonen ausweisen.
5. Wochenpläne deterministisch und regelbasiert erzeugen, jeder Planschritt mit Begründungsschlüssel.
6. Pläne exportieren (ICS, JSON, später Workout-Dateien über das Zusatzprogramm).
7. Ein lokales Sprachmodell darf optional aus erlaubten Vorlagen auswählen und den Plan erklären. Jede Ausgabe wird validiert. Bei Fehlschlag bleibt der Regelplan.
8. Jede Berechnung ist durch handgerechnete Referenzwerte und synthetische Daten testbar.
9. Keine Daten verlassen den Rechner, außer durch eine ausdrückliche Handlung des Nutzers.
10. Ein MCP-Client (zum Beispiel Claude Desktop) darf über einen schreibgeschützten Server nur gefilterte Kennzahlen abrufen. Die Filtergrenze liegt im Server und ist getestet.

### 2.3 Nicht-Ziele

- Aufruf einer Cloud-API durch Lastwerk (Anthropic, OpenAI-Cloud) und Speichern von API-Schlüsseln. Cloud-Modelle kommen nur als MCP-Client auf Initiative des Nutzers ins Spiel (9.6).
- Schreibzugriff auf das Garmin-Konto über einen MCP-Server.
- Der Garmin-Datenexport (ZIP aus dem Konto) als Datenquelle. Das Werkzeug muss ohne ihn funktionieren.
- Telemetrie, Absturzberichte, Updates im Hintergrund, Nutzerkonten. Ein Update auf ausdrücklichen Klick ist ab T17 erlaubt (10.1).
- Native Mobile-App.
- Garmin-Anmeldung in `lastwerk-core`. `core` bleibt rein (kein Netzwerk). Die Anmeldung liegt seit T20 in `lastwerk-garmin` und der App (5.6), das frühere Nicht-Ziel "Garmin-Anmeldung im Kern" ist damit für die App aufgehoben.
- Strava in Version 1 (offene Rechtslage, Abschnitt 14).
- Medizinische Aussagen, Diagnosen, Überlastungs- oder Verletzungsprognosen.
- Ersatz für Trainerbetreuung, Wettkampfprognosen, Ernährungsplanung.
- Mehrere Athletenprofile, Mehrbenutzerbetrieb, Synchronisation zwischen Rechnern.
- Aufzeichnung von Aktivitäten, Live-Daten, Sensorverwaltung.
- Signierte Installer und Support für macOS und Linux in Version 1.

---

## 3. Nutzer

**Primärnutzer:** der Autor. Hobby-Ausdauersportler, Rad mit Leistungsmesser (Edge-Radcomputer), Lauf und Schwimmen mit Uhr, ergänzend Krafttraining. Tagesdaten liegen als FIT-Dateien vor, auf zwei Geräten. Deutsch als Arbeitssprache, Englisch für das öffentliche Repository.

**Sekundärnutzer:** Leser des Repositories, die den Code oder die Methoden nachvollziehen. Sie bekommen keinen Support.

Anforderungen aus dieser Nutzung:

- Mehrere Quellen für dieselbe Aktivität müssen zusammengeführt werden.
- Ein falscher Schwellenwert verfälscht jede Belastung. Schwellenwerte tragen ein Gültigkeitsdatum und wirken rückwirkend nur für ihren Zeitraum.
- Die App ist nicht der Ort für Rohdatenanalyse. Sie zeigt Kennzahlen, Verläufe und Pläne.

Pflegeaufwand: Release-Prozess minimal. Tag setzt CI-Build für drei Plattformen an. Issues werden nach Möglichkeit beantwortet. Das README sagt das ausdrücklich.

---

## 4. Funktionen nach Priorität

### 4.1 Muss (Version 1, Meilensteine M0 bis M3)

| ID | Funktion |
|---|---|
| F1 | FIT-Dateien aus dem festen Ablageordner von Lastwerk rekursiv einlesen, inkrementell per SHA-256 der Datei. Der Nutzer wählt keinen Ordner (Entscheidung vom 15. Oktober 2026, siehe unten); eigene Dateien kommen über "FIT-Dateien hinzufügen" dazu |
| F2 | Aktivitäten aus mehreren Quellen zusammenführen (Duplikatregel 6.4) |
| F3 | Belastung je Aktivität nach Sportart (Abschnitt 8.1) |
| F4 | Schwellenwerte je Sportart mit Gültigkeitsdatum erfassen, Vorschläge aus FIT anzeigen |
| F5 | Zonen je Schwellenwert, Zeit in Zonen je Aktivität und Woche |
| F6 | Fitness (CTL), Ermüdung (ATL), Form (TSB) als Verlauf, mit Vorausberechnung für Planwochen |
| F7 | Übersichten: Woche, Monat, Jahr, Aktivitätsliste, Aktivitätsdetail ohne Karte |
| F8 | Regelbasierter Planer für Wettkampfziel und fortlaufenden Zyklus (Abschnitt 8.3) |
| F9 | Planvorlagenkatalog mit Texten in Deutsch und Englisch |
| F10 | Validierung jedes Plans gegen Schema und Leitplanken |
| F11 | Export: Plan als ICS und JSON, Nutzerdaten als Backup-JSON |
| F12 | Datenbank vollständig aus FIT-Ordner und Backup-JSON wiederherstellbar |
| F13 | Kommandozeile `lastwerk` für Import, Neuberechnung, Plan, Validierung, Export |
| F14 | Oberfläche Deutsch und Englisch, dunkles Thema |

### 4.2 Soll (nach Version 1)

| ID | Funktion | Meilenstein |
|---|---|---|
| F20 | Erholungsdaten (HRV, Ruhepuls, Schlaf) importieren und anzeigen, seit T20 direkt über den Garmin-Zugang der App (5.6) | M4d, T22, T25 |
| F21 | Planregel: Erholungsdaten dämpfen die nächste harte Einheit | M4d |
| F22 | Zusatzprogramm: Workouts nach Garmin Connect übertragen und planen (mit Trockenlauf) | M6 |
| F23 | LLM-Anpassung und Erklärung über Ollama und lokale OpenAI-kompatible Server | M5 |
| F24 | Vorschau des Modelleingangs vor jedem Aufruf | M5 |
| F25 | MCP-Server `lastwerk-mcp`: nur lesend, gefilterte Ausgabe für Claude und andere MCP-Clients | M7 |
| F27 | Vollabzug: alle lesbaren Datenarten rückwirkend ab Kontoerstellung in ein unverändertes Archiv holen, gedrosselt, fortsetzbar, mit Manifest und Abdeckungsbericht | M4a, M4b |
| F28 | USB-Kopie der Geräteordner (Activity, Monitor, Sleep) als optionale Quelle in das Archiv übernehmen. Rückfall, wenn der Garmin-Zugang gesperrt ist | M4c |
| F29 | Garmin-Verbindung in der App: Anmeldung im Garmin-Fenster, Abruf von Aktivitäten (FIT) und Erholungswerten auf Klick und einmal beim Start | T21 bis T23 |
| F36 | Dashboard mit Bereitschaft, Formverlauf, Rampe, Intensitätsverteilung, Planerfüllung, Effizienz, Leistungskurve und Belastungskalender, jede Deutung aus `core` mit Quelle | T25 |
| F37 | Planer mit drei Fragen und Gespräch mit dem lokalen Coach, der nur geprüfte Änderungsvorschläge macht | T26 |
| F38 | Aktivitätsdetail mit aerober Entkopplung, Effizienzfaktor, Runden, Zeit in Zonen als Balken und Tempo | T31 |
| F39 | Fortschritt nach Woche, Monat und Jahr je Sportart, Jahresvergleich zum selben Stichtag (erfüllt F7 für Monat und Jahr) | T32 |
| F40 | Seite "Leistung" mit Leistungskurve von 1 s bis 2 h je Zeitraum gegen den Vorzeitraum (setzt F35 um) | T33 |
| F41 | Seite "Erholung" mit HRV, Ruhepuls, Schlaf und Body Battery über Monate, Basislinie und Band aus R12, Tageslast darunter | T34 |
| F42 | Trainingskalender je Monat mit Plan gegen Ist | T35 |
| F43 | Verlauf der Schwellenwerte mit FTP-Vorschlag je Monat | T36 |
| F44 | Bestzeiten im Laufen über 1 km bis Marathon | T37 |
| F45 | Vollabzug in der App: Schwellen und Profil, Tageswerte, Trainingswerte und Garmins Last je Aktivität aus Garmin, Schwellen ohne Eingabe (5.6, T39) | T39 |

### 4.3 Kann

| ID | Funktion |
|---|---|
| F30 | Kartenansicht der Strecke mit Kacheln (Standard aus) |
| F31 | PWA für Handy, setzt erreichbare Adresse voraus |
| F32 | FIT-Workout-Dateien zum Kopieren auf das Gerät, Pfad je Gerät zu prüfen |
| F33 | Strava-Import, abhängig von der Klärung der API-Bedingungen |
| F34 | Neuplanung ab Datum X nach verpassten Einheiten |
| F35 | Leistungskurve (beste Leistung je Dauer) als Verlauf |

---

## 5. Architektur

### 5.1 Überblick

```
Garmin Connect (inoffiziell)            USB-Kopie Edge/Uhr (optional)
      |                                          |
      +--------------------+---------------------+
                           v
        Zusatzprogramm lastwerk-bridge (Ordner bridge/, Python 3.12)
                     |
        Archiv: unveränderte Antworten + Manifest (5.5)
                     |
        exchange/out: FIT, recovery-*.json     exchange/in: plan.json
                     |                                ^
                     v                                |
FIT-Ordner (Nutzer) ---> lastwerk-fit ---> lastwerk-core <--- lastwerk-coach (lokales LLM, optional)
                              |                 ^
                              v                 |
                        lastwerk-store ---------+---- lastwerk-app (Tauri 2) + Svelte-UI
                              |                 |
                              |            lastwerk-cli
                              v
                        lastwerk-mcp (nur lesend, gefilterte Ausgabe)
                              |
                        MCP-Client (zum Beispiel Claude Desktop, Claude Code)
```

Kernprinzip: `core` enthält reine Logik ohne Ein- und Ausgabe, ohne Systemzeit und ohne Zufall. Alles mit Seiteneffekten liegt in `fit`, `store`, `coach`, `cli`, `mcp` und `app`.

Datenschutzgrenze: Das Zusatzprogramm sieht Rohdaten aus dem Garmin-Konto. Ein MCP-Client wird nie mit dem Zusatzprogramm verbunden, sondern nur mit `lastwerk-mcp`. Dessen Ausgabetypen enthalten keine Koordinaten, keine Freitexte und keine Gerätekennungen (9.6).

### 5.2 Verzeichnisbaum

```
lastwerk/                    Git-Repository (GitHub: lastwerk)
  README.md                  Überblick, Status, Datenschutz, Struktur
  LICENSE-MIT
  LICENSE-APACHE
  .gitignore
  .gitattributes
  .editorconfig
  .githooks/
    pre-commit               blockiert FIT, local/, Tokens, Datenbanken
  .github/workflows/         ci.yml, bridge.yml (ab M0), release.yml (ab M3)
  .claude/settings.json      SessionStart-Hook für Cloud-Sessions
  CLAUDE.md                  Regeln für Claude-Code-Sessions
  scripts/                   Datenschutz-, Isolations-, Sprachprüfung, Cloud-Skripte
  .vscode/                   extensions.json, settings.json
  Cargo.toml                 Workspace (ab M0)
  rust-toolchain.toml        (ab M0)
  crates/
    core/                    lastwerk-core
    fit/                     lastwerk-fit
    store/                   lastwerk-store
    coach/                   lastwerk-coach
    garmin/                  lastwerk-garmin (Garmin-Zugang der App: Token, Schlüsselbund, lesender Client, Drosselung, seit T21)
    cli/                     lastwerk-cli (Bibliothek und Binary: lastwerk)
    mcp/                     lastwerk-mcp (MCP-Server, stdio, nur lesend)
    testkit/                 lastwerk-testkit (nur dev: FIT-Encoder, Generatoren)
  app/
    src-tauri/               lastwerk-app (ab M2b, eigenes Cargo-Projekt außerhalb des Wurzel-Workspace)
    ui/                      Svelte 5, Vite, TypeScript
  bridge/                    Python-Zusatzprogramm (uv, Python 3.12), ab M4a
    pyproject.toml
    src/lastwerk_bridge/
    tests/
    registry.yaml            Endpunkt-Register
  schema/
    plan.schema.json
    plan-edit.schema.json
    recovery-day.schema.json
    archive-manifest.schema.json
  catalog/
    templates.json           Vorlagenkatalog
    locales/de.json
    locales/en.json
    rules.default.json       Leitplankenparameter
  docs/
    README.md                Index der Dokumente
    SPEC.md
    remote-sessions.md       Einrichtung und Start von Cloud-Sessions
    tasks/                   Arbeitsaufträge je Session (T00 bis T16, L1)
  local/                     nicht versioniert, nur README.md ist im Repository
    fit/                     echte FIT-Dateien (Eingabeordner für Lastwerk)
    archive/garmin/          Archiv des Zusatzprogramms (5.5)
    exchange/out/            Zusatzprogramm nach Lastwerk
    exchange/in/             Lastwerk nach Zusatzprogramm
    probe/                   Antworten der Sonde, nie committen
```

Die Garmin-Tokens des Zusatzprogramms liegen nicht im Projektordner, sondern im Benutzerprofil (`~/.garminconnect`). Die Token der App liegen im Schlüsselbund des Betriebssystems (5.6).

### 5.3 Technologieentscheidung und Einwände

Entscheidung: Rust-Workspace, Tauri 2, Svelte 5 mit Vite und TypeScript, ohne SvelteKit.

Gründe:

- Die UI wird für eine spätere PWA wiederverwendbar.
- Dichte Dashboards, Tabellen und Diagramme sind im Web mit geringerem Aufwand zu bauen als in einer reinen Rust-GUI.
- Deutsch und Englisch laufen über einen Schlüsselkatalog.

Geprüfte Alternative: egui (reines Rust, keine WebView, kein Node). Verworfen wegen des Aufwands für dichte Diagramme, Tabellen und Text, und weil eine PWA dann keine UI-Wiederverwendung hätte.

Einwände gegen die Ausgangsfassung:

- Die vier Crates des Ausgangsvorschlags bleiben. Ergänzt werden `cli` (Import und Berechnung ohne UI, Golden-Tests) und `testkit` (FIT-Encoder für synthetische Dateien, nur Entwicklung).
- Der Planer liegt in `core`, nicht in `coach`. `coach` enthält nur die LLM-Anbindung. Damit bleibt der Plan ohne LLM vollständig.
- SvelteKit entfällt, weil Server-Rendering hier nichts bringt.

Abhängigkeiten (Stand crates.io am 13. Oktober 2026):

| Crate | Version | Lizenz | Verwendung |
|---|---|---|---|
| (eigener Decoder) | | MIT OR Apache-2.0 | FIT-Dekodierung in `fit` ohne Fremdbibliothek, Entscheidung zu O2 am 13. Oktober 2026 |
| rusqlite | 0.40.2 | MIT | SQLite in `store`, Feature `bundled` |
| tauri | 2.12.3 | Apache-2.0 OR MIT | App-Hülle |
| tauri-plugin-dialog | 2.8.2 | MIT OR Apache-2.0 | Dateidialoge, nur aus Rust (T06, seit T28 Auswahl von FIT-Dateien statt Ordnerwahl) |
| uplot (npm) | 1.6.32 | MIT | Diagramme der Oberfläche (O11, T06) |
| @fontsource-variable/cairo (npm) | 5.3.0 | OFL-1.1 (SIL Open Font License) | Schrift Cairo der Oberfläche als variable woff2-Datei im Build, nur latin und latin-ext (T24). Die Lizenz erlaubt Einbettung und Weitergabe, der Lizenztext liegt im Paket (`LICENSE`) |
| jsonschema | 0.58.6 | MIT | Schema-Validierung |
| icalendar | 0.17.14 | MIT/Apache-2.0 | ICS-Erzeugung |
| jsonschema | 0.58.6 | MIT | Schema-Validierung in `cli`, ohne Standardfeatures (keine Netzwerk- und Dateiauflösung von Verweisen), T08 |
| icalendar | 0.17.14 | MIT/Apache-2.0 | ICS-Erzeugung in `cli` ohne Standardfeatures, der Parser (Feature `parser`) nur in den Tests, T08 |
| ureq | 3.4.2 | MIT OR Apache-2.0 | HTTP zu Ollama und lokalen OpenAI-kompatiblen Servern in `coach`, ohne Standardfeatures (kein TLS, keine Kompression), blockierend, T15. Neue Pakete der Kette, alle MIT OR Apache-2.0: `ureq-proto` 0.6.4, `http` 1.5.0, `httparse` 1.10.1, `base64` 0.23.1, `utf8-zero` 0.8.1 |
| ureq (mit TLS) | 3.4.2 | MIT OR Apache-2.0 | HTTP zu Garmin in `garmin` (T21), blockierend, mit den Funktionen `rustls-no-provider` und `platform-verifier`, weiter ohne Standardfeatures: keine mitgelieferten Wurzelzertifikate (`webpki-roots` kommt nicht in die Kette), keine Kompression. Die Funktionen gelten im Workspace für alle Mitglieder; `coach` lehnt `https` weiter ab |
| rustls | 0.23.46 | Apache-2.0 OR ISC OR MIT | TLS in `garmin` (T21), Funktionen `ring`, `std`, `tls12`. Der Kryptoanbieter `ring` 0.17.14 (Apache-2.0 AND ISC, braucht beim Bau einen C-Compiler) wird ureq ausdrücklich übergeben |
| rustls-platform-verifier | 0.7.1 | MIT OR Apache-2.0 | Zertifikate des Betriebssystems (über ureq, T21), damit ein Firmenproxy mit eigener Zertifizierungsstelle funktioniert. Neue Pakete der Kette: `rustls-webpki` 0.103.15 (ISC), `rustls-native-certs` 0.8.5 (Apache-2.0 OR ISC OR MIT), `openssl-probe` 0.2.1, `untrusted` 0.9.0 (ISC), `subtle` 2.6.1 (BSD-3-Clause), `webpki-root-certs` 1.0.9 (CDLA-Permissive-2.0, Ausnahme in `deny.toml` seit T17) |
| keyring | 4.2.0 | MIT OR Apache-2.0 | Schlüsselbund des Betriebssystems für die Garmin-Token in `garmin` (T21), Funktion `v1`: Windows-Anmeldeinformationsverwaltung, macOS-Schlüsselbund, Secret Service unter Linux in reinem Rust über `zbus` (keine Systembibliothek). Neue Pakete: `keyring-core` 1.0.1, `zbus-secret-service-keyring-store` 1.0.1, `secret-service` 5.2.0, `apple-native-keyring-store` 1.0.2, `windows-native-keyring-store` 1.1.0, `zbus` 5.19.0 (MIT) mit `zvariant`, `zbus_names`, `endi`, `winnow` (MIT) und der async-Kette (`async-io`, `async-lock`, `event-listener` und weitere, MIT OR Apache-2.0), dazu `aes`, `cbc`, `hkdf`, `hmac`, `zeroize` (MIT OR Apache-2.0) |
| zip | 8.6.0 | MIT | Entpacken der FIT-Originale von Garmin in `cli` (T22), nur lesen, ohne Standardfeatures und mit der einen Funktion `deflate-flate2-zlib-rs` (Deflate mit dem reinen Rust-Backend; die Funktion `deflate` hätte zusätzlich den Komprimierer `zopfli` mitgebracht, Verschlüsselung, bzip2, lzma, zstd und ppmd bleiben aus). Neue Pakete der Kette: `flate2` 1.1.10 (MIT OR Apache-2.0), `zlib-rs` 0.6.8 (Zlib), `crc32fast` 1.5.2 (MIT OR Apache-2.0), `typed-path` 0.12.3 (MIT OR Apache-2.0). Version 8.6.0 statt der Reihe 9.0 gewählt, weil 9.0.0 erst seit dem 9. Oktober 2026 erscheint (Stand 15. Oktober); `cargo deny check` ist ohne Änderung an `deny.toml` grün |
| sysinfo | 0.39.6 | MIT | Gesamter Arbeitsspeicher für die Empfehlung eines Modells in `app/src-tauri` (T29, 9.2). Ohne Standardfeatures, nur die Funktion `system` (keine Platten, kein Netz, keine Benutzer, keine Sensoren). Neue Pakete der Kette: `ntapi` 0.4.3 und `objc2-io-kit` 0.3.2, die übrigen (`libc`, `memchr`, `windows`, `objc2-core-foundation`) waren schon im Baum. `cargo deny` ist ohne Änderung an `deny.toml` grün |
| base64 | 0.23.1 | MIT OR Apache-2.0 | JWT-Nutzlast lesen, Basic-Authentifizierung bilden (`garmin`, T21). schon im Baum durch ureq |
| chrono | 0.4.45 | MIT OR Apache-2.0 | Datum und Zeit |
| serde_json | 1.0.154 | MIT OR Apache-2.0 | JSON |
| proptest | 1.12.0 | MIT OR Apache-2.0 | Property-Tests (dev) |
| insta | 1.49.0 | Apache-2.0 | Snapshot-Tests (dev) |
| uuid | 1.28.0 | Apache-2.0 OR MIT | Plan-ID (UUID v4) in `cli`, Feature `v4`. Der Zufall gehört nicht in `core` (T08) |
| rmcp | 3.5.1 | Apache-2.0 | MCP-Server in `lastwerk-mcp` (offizielles Rust-SDK der MCP-Organisation, Q24), Features `server`, `macros`, `transport-io`, kein HTTP (T10) |
| tokio | 1.53.3 | MIT | Laufzeit, die rmcp verlangt, in `lastwerk-mcp` mit den Features `rt` und `macros` (ein Thread, T10) |
| schemars | 1.2.3 | MIT | JSON-Schema der Werkzeugargumente und -ausgaben, dieselbe Hauptversion wie in rmcp, Feature `chrono04` (T10) |

Das FIT-Profil in `fitparser` stammt laut Crate-Dokumentation aus dem offiziellen SDK von Garmin (aus `Profile.xlsx` erzeugte Dateien, im Repository eingecheckt). Die Lizenzlage dieses Profils bei Weitergabe in einem Open-Source-Projekt ist nicht abschließend geklärt. Deshalb liest `lastwerk-fit` mit einem eigenen minimalen Decoder und ohne Garmin-Profildateien (Entscheidung O2, Abschnitt 14).

### 5.4 Schnittstellen der Crates

Alle Signaturen sind Entwurf. Typnamen und Fehlerarten sind verbindlich, Feinheiten dürfen sich bei der Umsetzung ändern, solange die Tests aus Abschnitt 11 gelten.

#### lastwerk-core

Keine Abhängigkeit von Dateisystem, Netzwerk, Systemzeit oder Zufall. Erlaubte Abhängigkeiten: `serde`, `chrono` (nur Datumstypen), `thiserror`.

```rust
pub enum Sport { Cycling, Running, Swimming, Strength, Other }

pub struct Sample {
    pub t_s: u32,                 // Sekunden seit Start, 1 Hz nach Aufbereitung
    pub power_w: Option<f32>,
    pub hr_bpm: Option<f32>,
    pub speed_mps: Option<f32>,
}

pub struct ActivityInput {
    pub sport: Sport,
    pub start_utc: DateTime<Utc>,
    pub local_offset_s: i32,      // aus FIT, bestimmt das Tagesdatum
    pub samples: Vec<Sample>,
    pub rpe: Option<u8>,          // 1 bis 10, nur Kraft
    pub pool_length_m: Option<f32>,
}

pub struct Thresholds {
    pub valid_from: NaiveDate,
    pub ftp_w: Option<f32>,
    pub lthr_bpm: Option<f32>,
    pub hr_max_bpm: Option<f32>,
    pub hr_rest_bpm: Option<f32>,
    pub run_threshold_speed_mps: Option<f32>,
    pub css_mps: Option<f32>,
    pub sex: Option<Sex>,         // nur für TRIMP-Konstanten
}

pub enum LoadMethod { Tss, HrTss, RTss, STss, SessionRpe }

pub struct Load {
    pub value: f32,
    pub method: LoadMethod,
    pub intensity_factor: Option<f32>,
    pub estimated: bool,           // rTSS, sTSS, Session-RPE (Faustregel, 8.1)
    pub duration_s: u32,           // Dauer ohne Pausen (6.5)
    pub plausibility: Plausibility // Zählung verworfener Samples und Lücken (6.5)
}

pub fn normalized_power(p: &[f32]) -> Option<f32>;
pub fn compute_load(a: &ActivityInput, th: &Thresholds, cfg: &LoadConfig) -> Result<Load, LoadError>;
pub fn zone_seconds(a: &ActivityInput, scheme: &ZoneScheme, th: &Thresholds, cfg: &LoadConfig)
    -> Result<Vec<u32>, ZoneError>;

pub struct DailyLoad { pub date: NaiveDate, pub load: f32 }
pub struct PmcPoint { pub date: NaiveDate, pub ctl: f32, pub atl: f32, pub tsb: f32 }
pub struct PmcParams { pub tau_ctl_d: f32, pub tau_atl_d: f32, pub ctl0: f32, pub atl0: f32 }
pub fn pmc(daily: &[DailyLoad], p: &PmcParams, from: NaiveDate, to: NaiveDate) -> Vec<PmcPoint>;

pub struct PlanContext { /* Ziel, Verfügbarkeit, CTL/ATL, Wochenhistorie, Katalog */ }
pub struct Rules { /* Parameter aus rules.default.json */ }
pub fn generate_plan(ctx: &PlanContext, rules: &Rules) -> Result<Plan, PlanError>;
pub fn validate_plan(plan: &Plan, ctx: &PlanContext, rules: &Rules) -> Vec<Violation>;
```

Die Fehlerart `LoadError` unterscheidet fehlende Schwellenwerte, fehlende Messkanäle und unplausible Daten. Fehlende Daten erzeugen nie eine Null als Belastung, sondern einen Fehler, den die UI als "nicht berechenbar" ausweist.

Umsetzung in T05 (Stand 13. Oktober 2026), mit den Abweichungen von der Skizze:

- `pmc(daily, p, from, to)` liefert je Tag von `from` bis `to` einen Punkt. `daily` enthält nur Tage mit Last (so liefert sie die Speicherschicht), Tage ohne Eintrag zählen als Last 0, mehrere Einträge eines Tages werden addiert, Einträge außerhalb des Zeitraums bleiben unberücksichtigt. Der TSB des ersten Tages ist `ctl0 - atl0`. Gerechnet wird in `f64`, ausgegeben in `f32`. Eine Zeitkonstante von null oder weniger (oder nicht endlich) gibt das Gewicht 1, nie `NaN`. `DailyLoad` steht seit T05 in `core` (`pmc.rs`), der Speicher gibt den Typ weiter.
- `zone_seconds(a, scheme, th, cfg)` hat zusätzlich den Bezugswert `th` und die Plausibilitätsgrenzen `cfg` (verworfene Werte nach 6.5 zählen nicht) und liefert `Result<Vec<u32>, ZoneError>`. `ZoneError` kennt `missing_threshold`, `missing_channel` (kein Sample mit dem Kanal des Schemas), `implausible` und `invalid_scheme`. Index 0 ist Zone 1. Jedes Sample steht für eine Sekunde und zählt in genau eine Zone, Samples ohne Kanal und Zeitlücken zählen nicht, die Summe ist also die Zahl der Samples mit Kanal. Zonengrenzen sind obere Grenzen einschließlich (Konvention): Genau auf einer Grenze liegende Werte gehören zur unteren Zone.
- `ZoneScheme` hat zusätzlich `sports` (Sportarten, für die der Speicher das Schema füllt) und `note`. `ZoneCatalog { format_version, schemes }` prüft sich selbst (`validate`: Grenzen endlich, positiv, streng steigend, Kennungen eindeutig, Quelle und Status angegeben). `core` liest keine Datei, den Katalog liefert der Speicher (siehe unten).
- `best_power(a, cfg, durations)` liefert die beste mittlere Leistung je Dauer (`BEST_POWER_DURATIONS_S` = 5, 60, 300, 1200, 3600 s als Eckdauern; gespeichert wird seit T33 das Raster `POWER_CURVE_DURATIONS_S` mit 22 Dauern von 1 s bis 2 h, Konvention). Ein Fenster liegt vollständig in einem Abschnitt ohne Pause (6.5). Innerhalb eines Abschnitts zählt eine Sekunde ohne Leistungswert (kurze Lücke, fehlender oder verworfener Wert) als 0 W (Konvention, überschätzt nie, anders als die NP, die Werte überspringt). Dauern, für die kein Abschnitt lang genug ist, fehlen in der Antwort. Ohne jeden Leistungswert gibt es keine Bestwerte.
- `estimate`: `suggest_ftp` (95 Prozent der besten 20-Minuten-Leistung, Faustregel), `suggest_lthr_from_test` (Mittel der letzten 1200 s einer 1800-s-Reihe, Faustregel), `suggest_css` (Steigung Strecke gegen Zeit aus zwei Läufen, Q15) und `resolve_hr_max` (gemessener Wert, sonst `208 - 0,7 * Alter` nach Q7 mit `estimated = true`). Alle Faktoren sind benannte Konstanten.

Umsetzung in T07 (Stand 13. Oktober 2026), mit den Abweichungen von der Skizze:

- Modul `plan` in `core` (`types`, `catalog`, `rules`, `context`, `generate`, `validate`). `core` liest keine Datei: `TemplateCatalog` und `Rules` kommen deserialisiert vom Aufrufer, ebenso `plan_id` (UUID) und `created_at` im `PlanContext`. Die Kataloge lädt nur der Test (`include_str!`).
- `PlanContext` trägt `plan_id`, `created_at`, `locale`, `week_start` (Montag, sonst `PlanError::WeekStartNotMonday`), `goal` (`GoalSpec::Event { event_date, sport, priority }` oder `Continuous`), `horizon_weeks` (1 bis 26), `ctl0` und `atl0` (Werte am Ende des Tages vor `week_start`), `history` (je Woche und Sportart `load` und `minutes`), `availability` (sieben Tage Montag bis Sonntag mit `max_minutes`, `allow_hard`, `fixed_rest`), `sports` (Ausdauersportarten mit Gewicht), `strength_per_week`, `ctl_rise_per_week` (eigene Grenze für R1, höchstens `R01.max_setting`), `pmc_params` (nur die Zeitkonstanten zählen, `ctl0` und `atl0` darin nicht) und `catalog`.
- `validate_plan(plan, ctx, rules) -> Vec<Violation>` mit `Violation { code, hardness, date, message_key, detail }`. `code` ist `R01` bis `R11`, `message_key` ist `rule.R**.explanation`, `detail` trägt die Parameter für den Text. `Plan::is_acceptable(&violations)` ist wahr, wenn keine Verletzung hart ist. Fehler von `generate_plan` sind `PlanError` mit stabilem `code()`.
- `PlanSport` im Plan (cycling, running, swimming, strength, rest) ist nicht `Sport` der Belastungsberechnung (dort `Other`, kein `rest`). Die Historie nutzt `Sport`.
- Gleiche Eingaben ergeben gleiches JSON: kein Zufall, keine Uhr, Gleichstände nach Katalog- und Wochentagsreihenfolge. Zahlen im Plan sind auf eine Nachkommastelle gerundet.

Umsetzung in T14 (Stand 13. Oktober 2026), Regel R12:

- `RecoveryDay { date, hrv_overnight_ms, resting_hr_bpm, sleep_duration_s, sleep_score }` (alle Werte `Option`) steht in `plan` (`plan/recovery.rs`). `PlanContext::recovery: Vec<RecoveryDay>` ist die Eingabe, in beliebiger Reihenfolge, mit `#[serde(default)]` und ohne Ausgabe, wenn leer. `recovery_signal(days, week_start, &rules.r12) -> Option<RecoverySignal>` wertet nur Tage vor `week_start` aus; `RecoverySignal` nennt den Tag und um welchen Anteil HRV und Schlafdauer unter der Basislinie liegen.
- `Rules` hat den Abschnitt `R12` (Quelle `Q6`, weich), und `reserved` in `rules.default.json` ist leer. Die Version der Regeln bleibt `1.0.0`: ohne Erholungsdaten ändert sich kein Plan (Test: Pläne mit leerer Liste und mit unauffälligen Daten sind Byte für Byte die Pläne ohne die Liste, und die Golden-Dateien blieben unverändert), eine neue Version hätte jeden Plan über `rules_version` verändert.
- Neuer Begründungsschlüssel `reason.session.recovery_swap` in `plan::reason_keys`. Texte `rule.R12.name` und `.explanation` in beiden Sprachdateien.

#### lastwerk-fit

```rust
pub struct RawActivity {
    pub sport: Sport,
    pub sub_sport: Option<String>,
    pub start_utc: DateTime<Utc>,
    pub local_offset_s: i32,
    pub device_serial: Option<u32>,
    pub device_product: Option<String>,
    pub samples: Vec<Sample>,        // aus record-Nachrichten, auf 1 Hz gebracht
    pub laps: Vec<Lap>,
    pub session: SessionSummary,
}

pub fn parse_activity(bytes: &[u8]) -> Result<RawActivity, FitError>;
pub fn scan_dir(root: &Path) -> impl Iterator<Item = Result<FitEntry, FitError>>;
```

`FitError` unterscheidet Prüfsummenfehler, unbekannte Version, keine Aktivität (z. B. Monitoring-Datei) und leere Datei. Eine fehlerhafte Datei bricht den Import nicht ab, sie erscheint in der Fehlerliste.

Umsetzung in T03 (Stand 13. Oktober 2026), mit den Abweichungen von der Skizze oben:

- Fehlercodes (`FitError::code`): `empty_file`, `too_large`, `invalid_header`, `crc_mismatch`, `truncated`, `unsupported_version`, `no_activity`, `malformed`, `io`.
- `scan_dir` liefert `Result<FitEntry, ScanError>`. `ScanError` trägt zusätzlich `rel_path` (relativ zur Wurzel, Trenner `/`) und `error: FitError`. Grund: Ohne Pfad ließe sich ein Fehler im Import keiner Datei zuordnen. Die Reihenfolge ist nach relativem Pfad sortiert, Symlinks werden nicht verfolgt, Endung `.fit` ohne Beachtung der Groß- und Kleinschreibung.
- `SessionSummary` enthält Start, Dauer mit und ohne Pausen, Distanz und Beckenlänge (`pool_length_m`), alles `Option`. `RawActivity` hat zusätzlich `plausibility` (Zählung nach 6.5, berechnet mit `lastwerk_core::load::check_plausibility` und `LoadConfig::default()`). `samples` enthält die Werte unverändert, Verwerfen und Lücken behandelt weiterhin `check_plausibility` beim Berechnen der Belastung.
- Aufbereitung auf 1 Hz (Konvention): Ursprung von `t_s` ist der erste `record`, `start_utc` ist dessen Zeitpunkt. Der Start der Session steht in `session.start_time`. Mehrere `record` in derselben Sekunde: der letzte in Dateireihenfolge gewinnt. Lücken werden nicht aufgefüllt.
- Sportart: `session.sport`, sonst die `sport`-Nachricht, sonst `Other`. `training` mit Unterart `strength_training` ist `Strength`. Bei mehreren Sessions zählt die erste.
- Lokaler Zeitversatz: `activity.local_timestamp` minus `activity.timestamp`, 0 wenn nicht vorhanden. Beträge über 24 Stunden gelten als beschädigt (`malformed`).
- Grenzen (Konvention, SPEC 10.2): höchstens 64 MiB je Datei (`too_large`), 2 000 000 `record` und 100 000 `lap` je Datei (`malformed`). Zeitstempel unter `0x10000000` sind Systemzeit und kein Datum (`malformed`). Eine Datei ohne `file_id` ist `malformed`, mit anderem Dateityp als `activity` oder ohne `record` und `session` `no_activity`. Nach dem ersten Dateiende angehängte Bytes werden ignoriert.
- Zusatz aus T04: `list_fit_files(root)` listet die Dateien ohne Dekodieren (`Vec<Result<FitFile, ScanError>>`, gleiche Reihenfolge und Regeln wie `scan_dir`) und `read_fit_file(path)` liest eine Datei mit dem Größenlimit. Der Import braucht beides, um unveränderte Dateien über die Prüfsumme zu überspringen, ohne sie zu dekodieren. `scan_dir` bleibt unverändert.

#### lastwerk-store

```rust
pub struct Store { /* rusqlite::Connection */ }

impl Store {
    pub fn open(path: &Path) -> Result<Store, StoreError>;      // führt Migrationen aus
    pub fn upsert_source_file(&mut self, f: &SourceFileRow) -> Result<i64, StoreError>;
    pub fn upsert_activity(&mut self, a: &ActivityRow) -> Result<i64, StoreError>;
    pub fn thresholds_for(&self, sport: Sport, on: NaiveDate) -> Result<Option<Thresholds>, StoreError>;
    pub fn daily_loads(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<DailyLoad>, StoreError>;
    pub fn save_plan(&mut self, p: &Plan) -> Result<(), StoreError>;
    pub fn export_user_data(&self) -> Result<UserDataExport, StoreError>;
    pub fn import_user_data(&mut self, e: &UserDataExport) -> Result<(), StoreError>;
    pub fn rebuild_cache(&mut self, dir: &Path, progress: &mut dyn FnMut(Progress)) -> Result<(), StoreError>;
}
```

Umsetzung in T04 (Stand 13. Oktober 2026), mit den Abweichungen von der Skizze:

- `rebuild_cache(dir, now, opts, progress)` und `import_dir(dir, now, opts, progress)` liefern einen `ImportReport`. `now` ist der Zeitpunkt für `imported_at` und `computed_at` (Tests bleiben deterministisch, das Crate liest nie die Systemuhr). `opts` trägt `LoadConfig` und `DuplicateRule`. Der ganze Lauf ist eine Transaktion.
- `ImportReport`: `files_found`, `new`, `updated`, `unchanged`, `not_activity`, `errors` (Liste aus Pfad und Fehlercode), `duplicates`, `primary`, `loads_ok`, `loads_missing_threshold`, `loads_missing_channel`, `loads_implausible`. Jede Datei zählt in genau einem von `new`, `updated`, `unchanged`, `not_activity`, `errors`. Die Belastungszahlen zählen nur den laufenden Lauf, `duplicates` und `primary` den Stand der Datenbank danach.
- Überspringen: gleicher `rel_path` und gleiche SHA-256 heißt `unchanged`, auch bei Dateien, die beim letzten Mal Fehler oder keine Aktivität waren. Geänderte Dateien (gleicher Pfad, andere Prüfsumme) ersetzen ihre Aktivität. Dateien, die nur noch in der Datenbank stehen, bleiben als Cache stehen, bis ein `rebuild_cache` sie entfernt. Dateien, die sich nicht lesen lassen (Dateisystemfehler, über 64 MiB), stehen im Bericht, haben aber keine Zeile in `source_file`, weil keine Prüfsumme vorliegt.
- Monitoring (`FitError::NoActivity`) ist `source_file.status = 'not_activity'`, alle anderen Parserfehler sind `'error'` mit `error_code = FitError::code()`.
- Belastung: ohne Schwellenwert (`thresholds_for` liefert `None`) rechnet `compute_load` mit leeren Schwellen und meldet `missing_threshold`. Ein `LoadError` wird als Zeile mit `status = code()`, `value = NULL` und `method = 'none'` gespeichert, nie als Null. Bei Erfolg nennt `threshold_id` die Zeile der Schwelle, die zur Methode gehört (Tss: `ftp_w`, HrTss: `lthr_bpm`, rTSS: `run_speed_mps`, sTSS: `css_mps`, Session-RPE: keine). Die Belastung entsteht beim Import. Ändert der Nutzer Schwellen, gilt der neue Wert nach `rebuild_cache` oder nach `recompute_loads` (T05, unten). Eine Neuberechnung ohne Dateizugriff ist nicht möglich, weil der Cache keine Messpunkte speichert.
- `thresholds_for(sport, on)`: je `kind` der jüngste Eintrag mit `valid_from <= on` unter den Einträgen der Sportart und denen mit `sport = 'all'` (sportartübergreifend, zum Beispiel Ruhepuls). Bei gleichem Datum gewinnt der Eintrag der Sportart, danach der später angelegte. `Thresholds::valid_from` ist das jüngste Datum der verwendeten Werte. `sex` kommt aus `setting` mit Schlüssel `athlete.sex` (`male` oder `female`), siehe 6.2.
- `activity`: `avg_hr`, `max_hr`, `avg_power_w` und `np_w` zählen nur Werte, die die Regeln aus 6.5 behalten. `np_w` ist die NP über alle Abschnitte mit demselben Gewicht (Zahl der vollen Fenster) wie in `compute_load`. `ascent_m` bleibt `NULL`, der Parser liest keine Höhenmeter. `has_power` und `has_hr` gelten nach den Plausibilitätsregeln. `moving_s` ist die Timerzeit der Session.
- Zusätzliche Methoden: `open_in_memory`, `insert_threshold` (die CLI in T05 nutzt sie), `get_setting` und `set_setting`, `apply_duplicate_rule`, lesende Zugriffe `source_files`, `activities`, `activity_loads` und `plausibility_summary` (Kennzahlen für `lastwerk plausibility`). `DailyLoad` führt seit T05 `lastwerk-core`, der Speicher gibt den Typ weiter. `daily_loads` lässt Tage ohne zählende Belastung aus, statt Null zu liefern.
- `export_user_data` und `import_user_data` verarbeiten (ab T14 auch `recovery_day`, siehe unten) `threshold`, `goal`, `availability`, `plan`, `manual_session` und `setting` als JSON (`format_version` 1, Zeilen mit denselben IDs). Das Einspielen ersetzt den Inhalt dieser Tabellen vollständig in einer Transaktion; Verweise von `activity_load.threshold_id` auf Schwellen, die der Export nicht enthält, werden gelöst. Der Cache bleibt unberührt.

Umsetzung in T08 (Stand 13. Oktober 2026), zusätzlich zu T04 und T05:

- `save_plan(&Plan, PlanStatus)` (`PlanStatus` ist `Draft`, `Active` oder `Archived`) und `save_plan_for_goal(&Plan, PlanStatus, Option<i64>)` schreiben die Zeile in `plan` (`id` = `plan_id`, Spalten als Kopie aus dem Plan, `json` als Text). Eine gleiche `plan_id` ersetzt die Zeile. Konvention: Es gibt höchstens einen aktiven Plan. Speichern mit `Active` und `set_plan_status(id, Active)` archivieren den bisherigen aktiven Plan in derselben Transaktion. Weitere Methoden: `list_plans` (neueste zuerst, ohne JSON), `load_plan(id)`, `active_plan`. Der Speicher prüft nur, dass sich der Text als `Plan` lesen lässt, die Schemaprüfung liegt bei der CLI.
- `set_goal(NewGoal)` legt ein Ziel an, macht es zum aktiven und deaktiviert alle anderen (Konvention: höchstens ein aktives Ziel, frühere Ziele bleiben für die Verweise der Pläne). Ein Termin braucht ein Datum, ein fortlaufendes Ziel hat keins. `list_goals`, `active_goal`. `set_availability(weekday, max_minutes, allow_hard, fixed_rest)` mit `weekday` 0 bis 6 (Montag zuerst) und höchstens 1440 Minuten, `list_availability`.
- `week_history(before, weeks)` liefert `WeekHistory` (Typ aus `core`) für die `weeks` ISO-Wochen vor der Woche von `before`: Last und Minuten je Woche und Sportart aus den primären Aktivitäten. Die Minuten (aus `duration_s`) zählen alle primären Aktivitäten, auch ohne gültige Last, die Last nur Belastungen im Status `ok` (wie `daily_loads`), ohne solche ist sie 0. Stand 14. Oktober 2026 (Integrationsprüfung, Fund B): vorher zählten auch die Minuten nur Aktivitäten mit Last `ok`. Kraft aus FIT ist immer `missing_channel` (RPE nur von Hand), deshalb fehlte sie in der Vorwochenzeit, während der Plan sie mit 45 Minuten je Einheit führt, und R2 verglich Planminuten mit Kraft gegen eine Vorwoche ohne Kraft. `pmc_state_at(date, &PmcParams)` rechnet CTL und ATL am Ende des Tages `date` aus `daily_loads` ab dem ersten Tag mit Last, davor und ohne Last gelten die Startwerte aus den Parametern. Die Zeitkonstanten und Startwerte als Einstellung kommen mit der Oberfläche (T06), die CLI nimmt bis dahin die Standardwerte.
- `UserDataExport` heißt das Versionsfeld jetzt `backup_version` (Wert 1, beim Lesen gilt `format_version` aus T04 weiter als Alias) und trägt `exported_at` (RFC 3339 in UTC, `null` ohne Angabe, gesetzt über `with_exported_at`, weil der Speicher keine Uhr liest). Der Import lehnt jede andere `backup_version` ab (`UnsupportedExportVersion`, Exit 2 in der CLI) und lässt den Bestand unverändert.

Umsetzung in T05 (Stand 13. Oktober 2026), zusätzlich zu T04:

- Import und `rebuild_cache` füllen `activity_zone_time` (alle Schemata des Zonenkatalogs, die für die Sportart gelten, mit allen Zonen, auch mit 0 Sekunden) und `activity_best_power`. Ein Schema, dessen Bezugswert oder Kanal fehlt, erzeugt keine Zeilen: "nicht berechenbar" ist keine Null. Die Zonen rechnen mit den Schwellen, die am Tag der Aktivität gelten. Fehlt `hr_max_bpm`, nimmt der Speicher `208 - 0,7 * Alter` (Q7), wenn die Einstellung `athlete.birth_year` vorliegt (Konvention: Alter ist Kalenderjahr der Aktivität minus Geburtsjahr), und setzt `activity_zone_time.estimated`. Fehlt auch das Alter, sind die Zonen nach Herzfrequenzreserve nicht berechenbar, der Import läuft normal weiter.
- Zonenkatalog: `ImportOptions::zones` ist standardmäßig `embedded_zone_catalog()`, die Datei `catalog/zones.json` ist per `include_str!` in `lastwerk-store` eingebettet. Grund: Das fertige Programm läuft ohne Nebendateien, und `core` bleibt frei von Dateizugriff. `parse_zone_catalog(json)` lädt und prüft einen anderen Katalog.
- `recompute_loads(dir, sport, from, to, now, opts)`: Der Cache speichert keine Messpunkte, deshalb liest die Neuberechnung die FIT-Dateien aus `dir`. Sie rechnet Belastung und Zeit in Zonen der Aktivitäten mit `from <= local_date <= to` (`to = None` ohne Ende) und passender Sportart (`None` heißt alle) neu und verändert sonst nichts. Eine Datei, die fehlt oder deren SHA-256 nicht mehr zur gespeicherten passt, wird übersprungen und im `RecomputeReport` gezählt. Eine Schwelle mit späterem Datum ändert so frühere Lasten nicht (SPEC 3). `threshold_valid_until(id)` nennt den letzten Tag, an dem eine Schwelle gilt (Tag vor der nächsten Schwelle derselben Art für dieselbe Sportart oder `all`).
- Weitere Methoden: `list_thresholds`, `effective_hr_max(sport, on)` (Messwert oder Schätzung mit Kennzeichnung), `zone_times`, `best_powers`, `best_power_peak(sport, duration_s)` (beste Leistung unter den zählenden Aktivitäten, Grundlage der FTP-Schätzung) und `first_load_date` (erster Tag mit zählender Last). Neue Einstellung `athlete.birth_year` (Jahreszahl).

Umsetzung in T10 (Stand 13. Oktober 2026), zusätzlich:

- `open_read_only(path)` öffnet eine vorhandene Datenbank mit `SQLITE_OPEN_READ_ONLY` (ohne `CREATE`) und setzt `PRAGMA query_only`. Anders als `open` legt es keine Datei an und migriert nicht: Ein älteres Schema ist ein Fehler (`InvalidData`, die Datenbank einmal mit `lastwerk` öffnen), ein neueres `SchemaTooNew`. Ein Schreibzugriff über diese Verbindung scheitert mit "readonly".
- Nachtrag 14. Oktober 2026 (Integrationsprüfung): `recovery_days_between` aus T10 ist entfernt. Ihr Zweig "leere Liste, solange die Tabelle fehlt" war unerreichbar, weil `open_read_only` genau die neueste Schemaversion verlangt und `recovery_day` damit immer existiert. Der MCP-Server liest mit `recovery_days` aus T14.

Umsetzung in T14 (Stand 13. Oktober 2026), Erholungstage:

- Migration `0003_recovery_day.sql` legt `recovery_day` nach 6.2 an (Version 3). Modul `recovery.rs`: `RecoveryDayRecord { date, hrv_overnight_ms, resting_hr_bpm, sleep_duration_s, sleep_score, source, imported_at }`, `upsert_recovery_day(&RecoveryDayRecord) -> UpsertOutcome` (`Inserted`, `Updated`, `Unchanged`), `upsert_recovery_days(&[...])` (eine Transaktion, alle Zeilen vorab geprüft) und `recovery_days(from, to)` (beide Grenzen einschließlich, nach Datum). `imported_at` setzt der Aufrufer (der Speicher liest keine Uhr). Eine Zeile mit gleichen Werten und gleicher Quelle bleibt samt `imported_at` stehen, damit ein zweiter Import nichts schreibt; ein späterer Abruf mit anderen Werten ersetzt die Zeile (Garmin finalisiert nachträglich). Der Speicher prüft die Wertebereiche des Schemas 7.3 (`InvalidInput`); `From<&RecoveryDayRecord> for lastwerk_core::plan::RecoveryDay` bildet die Eingabe des Planers.
- `recovery_day` ist nicht aus FIT ableitbar und damit Teil der Nutzerdaten: `UserDataExport` hat die Liste `recovery_days` (Datum und Zeitpunkt als Text wie in der Tabelle), `import_user_data` ersetzt die Tabelle wie die übrigen. Eine Sicherung ohne den Schlüssel (vor T14) gilt als leere Liste und leert die Tabelle; sie lässt sich mit `import-recovery` neu füllen. `backup_version` bleibt 1, weil das Feld nur hinzukommt. `rebuild_cache` lässt die Tabelle unberührt (Test). Nachtrag 14. Oktober 2026 (Integrationsprüfung): `import_user_data` prüft jede Zeile mit derselben Regel wie der Import (`RecoveryDayRecord::check`, Wertebereiche 7.3); eine Sicherung mit Werten außerhalb wird ganz abgelehnt (`InvalidInput`), der alte Stand bleibt. Beim Lesen sind Werte außerhalb der Ganzzahltypen ein Lesefehler statt still umgebrochen.
- Hinweis für T10: `Store::recovery_days(from, to)` ist die Lesemethode für `get_recovery_days`. Die Freigabe (O19) prüft der MCP-Server, die Tabelle selbst kennt keine.

Umsetzung in T22 (Stand 15. Oktober 2026), Body Battery und Einstellungen des Abgleichs:

- Migration `0004_recovery_garmin.sql` (Version 4): `recovery_day.body_battery_high` und `.body_battery_low`. `RecoveryDayRecord` hat dafür `body_battery_high` und `body_battery_low` (`Option<u8>`, 0 bis `BODY_BATTERY_MAX` = 100, `InvalidInput` darüber). `RecoveryDayExport` führt die zwei Felder mit `#[serde(default)]`: Eine Sicherung aus der Zeit davor lässt sich weiter einspielen, die Felder sind dann leer (Test mit einer Sicherung im Format von T14). `backup_version` bleibt 1, `schema_version` des Erholungstags ebenfalls (7.3). Die Plausibilitätsgrenzen stehen als öffentliche Konstanten bereit (`HRV_MAX_MS`, `RESTING_HR_MIN_BPM`, `RESTING_HR_MAX_BPM`, `SLEEP_MAX_S`, `SLEEP_SCORE_MAX`, `BODY_BATTERY_MAX`, `SOURCE_MAX_CHARS`), damit der Abgleich Werte einzeln verwerfen kann, statt dass eine Zeile abgelehnt wird.
- `Store::garmin_settings() -> GarminSettings { last_sync: Option<NaiveDate>, sync_on_start: bool, backfill_days: u32 }`, `set_garmin_last_sync`, `set_garmin_sync_on_start`, `set_garmin_backfill_days` (1 bis 1825, sonst `InvalidInput`). Schlüssel `garmin.last_sync`, `garmin.sync_on_start`, `garmin.backfill_days`, Standardwerte nein, an, 365 (Faustregel).
- Die MCP-Sicht (`get_recovery_days`) und die Planer-Eingabe `lastwerk_core::plan::RecoveryDay` zeigen Body Battery noch nicht; sie gehört nicht zu R12.

#### lastwerk-coach

```rust
pub trait LlmClient {
    fn complete(&self, req: &LlmRequest) -> Result<LlmResponse, LlmError>;
}

pub enum Provider {
    Ollama { base_url: Url, model: String },
    OpenAiCompatible { base_url: Url, model: String },   // lokale Server
}

pub struct RefineConfig { pub max_corrections: u8 /* höchstens 2 */, pub timeout_s: u32 }

pub enum RefineOutcome {
    Applied { plan: Plan, explanation: String, attempts: u8 },
    Fallback { plan: Plan, reason: FallbackReason },     // Regelplan bleibt
}

pub fn refine_plan(
    base: &Plan, ctx: &PlanContext, rules: &Rules,
    client: &dyn LlmClient, cfg: &RefineConfig,
) -> RefineOutcome;
```

`base_url` ist beim Anlegen auf Loopback oder einen vom Nutzer bestätigten Host im lokalen Netz beschränkt. Es gibt keinen Standardwert für einen öffentlichen Host.

Umsetzung in T15 (Stand 13. Oktober 2026). Abweichungen und Festlegungen gegenüber dem Entwurf oben:

- **Adresse.** Statt `Url` gibt es den Typ `BaseUrl`, der nur über `BaseUrl::parse(text, allow_lan)` entsteht (Felder privat, keine Deserialisierung). Erlaubt sind Schema `http`, Host `localhost`, jede Loopback-Adresse (`127.0.0.0/8`, `::1`, auch als IPv4-abgebildete IPv6-Adresse) und mit `allow_lan = true` (CLI: `--allow-lan`) private Adressen: `10/8`, `172.16/12`, `192.168/16`, Link-Local (`169.254/16`, `fe80::/10`) und `fc00::/7`. Alles andere ist `ProviderError::HostNotAllowed { host, lan_confirmation_missing }`, auch ein öffentlicher Host mit `allow_lan` (getestet mit `api.openai.com`, `api.anthropic.com`, `8.8.8.8`, `0.0.0.0`, Multicast). Hostnamen außer `localhost` sind abgelehnt, weil sich ihr Ziel ohne Namensauflösung nicht prüfen lässt; im lokalen Netz zählt die IP-Adresse. Kurzformen wie `2130706433` oder `127.1` gelten nicht als Adresse und fallen durch. Zugangsdaten (`user:pw@`) und Query sind `InvalidUrl`. `Provider::ollama(None, ...)` nimmt `http://127.0.0.1:11434`, `Provider::openai_compatible` hat keinen Standardwert.
- **Nur `http`.** `https` ist in der ersten Fassung `ProviderError::TlsNotSupported` (nach der Hostprüfung). Grund: `ureq` läuft ohne TLS-Feature; `rustls` mit `ring` brächte `ring` und `webpki-roots` in die Kette (nicht in `deny.toml` erlaubt) für einen Fall, den lokale Server selten brauchen. Wer TLS im lokalen Netz braucht, ergänzt das Feature mit eigener Lizenzprüfung.
- **HTTP-Client.** `ureq` blockierend, je Aufruf ein eigener Agent mit globalem Zeitlimit (`RefineConfig::timeout_s`, Standard 120 s als Faustregel für Rechner ohne eigene Grafikkarte, O13), `proxy(None)` (ein Proxy aus der Umgebung würde lokalen Verkehr aus dem Rechner leiten), `max_redirects(0)` (eine Weiterleitung darf nie zu einem anderen Host führen), Statuscodes außerhalb von 2xx als Fehler, Antwort höchstens 2 MiB. Ollama: `POST /api/chat` mit `stream: false`, `format` (Schema), `options.temperature = 0`. OpenAI-kompatibel: `POST /v1/chat/completions` mit `stream: false`, `temperature = 0` und `response_format` vom Typ `json_schema`; endet die Basisadresse auf `/v1`, wird das Präfix nicht verdoppelt. Es gibt keine Kopfzeile für Zugangsdaten. Dem Server geht das Schema ohne `$schema` und `$id` mit.
- **`LlmClient`** nimmt `LlmRequest { messages, schema, timeout_s }` und liefert `LlmResponse { content }`. Fehler: `LlmError::{Timeout, Transport, Status, BadResponse}`. `FakeClient` (Skript von Antworten, merkt sich alle Aufrufe) ist öffentlich, damit die Tests der CLI ihn nutzen.
- **`RefineOutcome::Fallback`** trägt zusätzlich `attempts` (Zahl der Aufrufe an das Modell). `FallbackReason`: `BaseInvalid` (der Basisplan verletzt selbst eine harte Regel, kein Aufruf), `Timeout`, `Transport(LlmError)` (Netz, Status, unbekannte Form), `InvalidOutput(Vec<Issue>)` (nach den erlaubten Korrekturen noch Fehler, die Codes der letzten Antwort). `RefineConfig::max_corrections` wird auf 2 begrenzt; `attempts` ist höchstens 3. Der Zähler der Korrekturen ist `attempts - 1`.
- **Neuer Plan.** `Applied` trägt `source = llm`, die Erklärung (auf 1200 Zeichen gekürzt, siehe 7.2) und behält `plan_id` und `created_at` des Basisplans; die CLI vergibt neue. `planned_load` je geänderter Einheit rechnet `core` mit `LoadModel::session_load` (kein neuer Code in `core`), `hard` und `rpe` kommen aus der Vorlage, `planned_load` je Woche ist die Summe, `target_load` bleibt. Die `warnings` des neuen Plans sind die weichen Verletzungen von `validate_plan` plus die Hinweise des Basisplans, die keine Regelprüfung ableitet (zum Beispiel `R12` als Info). Eine ersetzte Einheit verliert ihre Begründungsschlüssel; trägt sie `reason.session.recovery_swap` und die neue Einheit ist nicht hart, bleibt der Schlüssel, damit R12 nicht die nächste harte Einheit meldet.

Ergänzung in T26 Teil A (Stand 15. Oktober 2026): Module `chat` (Gespräch, `chat_turn`) und `discover` (`ollama_models`, `pick_model`), beschrieben in 9.7. Der OpenAI-kompatible Client nennt das Schema in `response_format` jetzt nach dessen `$id` (`plan_edit` oder `plan_chat`) statt fest `plan_edit`. Die Prüfschritte von `refine_plan` nach dem Parsen (`check_edits`) und der Aufbau des HTTP-Agenten sind herausgelöst und werden von `chat` und `discover` mitbenutzt.

#### lastwerk-garmin

```rust
pub fn ticket_from_redirect(url: &str) -> Option<Ticket>;
pub fn needs_refresh(tokens: &TokenSet, now_s: i64) -> bool;

pub trait TokenStore: Send + Sync {
    fn load(&self) -> Result<Option<TokenSet>, StoreError>;
    fn save(&self, tokens: &TokenSet) -> Result<(), StoreError>;
    fn delete(&self) -> Result<(), StoreError>;
}                                                  // KeyringStore, MemoryStore (Tests)

impl GarminClient {
    pub fn new(endpoints: Endpoints, store: Arc<dyn TokenStore>, lockout_file: impl Into<PathBuf>) -> Self;
    pub fn builder(...) -> ClientBuilder;          // Uhr, Zufall, Drosselwerte, Zeitlimits einsetzen
    pub fn connect(&self, ticket: &Ticket) -> Result<()>;
    pub fn disconnect(&self) -> Result<()>;
    pub fn is_connected(&self) -> Result<bool>;
    pub fn active_lockout(&self) -> Option<DateTime<Utc>>;
    pub fn social_profile(&self) -> Result<Profile>;
    pub fn activities(&self, start: u32, limit: u32) -> Result<Vec<ActivitySummary>>;
    pub fn activity_fit(&self, activity_id: u64) -> Result<Vec<u8>>;   // ZIP-Rohbytes
    pub fn hrv_range(&self, start: NaiveDate, end: NaiveDate) -> Result<serde_json::Value>;
    pub fn hrv_day(&self, date: NaiveDate) -> Result<serde_json::Value>;
    pub fn sleep_day(&self, display_name: &str, date: NaiveDate) -> Result<serde_json::Value>;
    pub fn rhr_range(&self, display_name: &str, start: NaiveDate, end: NaiveDate) -> Result<serde_json::Value>;
    pub fn body_battery(&self, start: NaiveDate, end: NaiveDate) -> Result<serde_json::Value>;
    pub fn stress_day(&self, date: NaiveDate) -> Result<serde_json::Value>;
}
```

Das Crate ist die einzige Stelle mit Netzwerk zu Garmin (5.6). Es kennt weder Datenbank noch FIT-Import noch Oberfläche.

Umsetzung in T21 (Stand 15. Oktober 2026). Festlegungen und Abweichungen gegenüber dem Entwurf oben und der Aufgabe:

- **Adressen.** `Endpoints` hat private Felder und als Standard nur die echten `https`-Adressen (`sso.garmin.com`, `diauth.garmin.com`, `connectapi.garmin.com`). `Endpoints::for_tests("http://127.0.0.1:<port>")` gibt es nur mit der Funktion `test-endpoints` (die Entwicklungsabhängigkeit des Crates schaltet sie für `cargo test` ein) und verweigert jede andere Adresse. Der HTTP-Teil erzwingt `https`, solange die Adressen nicht aus `for_tests` stammen. `Endpoints::sign_in_url(sprache)` baut die Adresse der Anmeldeseite für T23.
- **Ticket.** `ticket_from_redirect` nimmt nur `https://connect.garmin.com/app` (mit oder ohne `/`) und danach genau `?ticket=ST-…` mit Zeichen `[A-Za-z0-9-]`, höchstens 256 Zeichen insgesamt (Faustregel, echte Tickets sind kürzer). Fremde Hosts, Unterdomänen, `http`, Port, Zugangsdaten, Fragment, weitere oder doppelte Parameter, Prozentkodierung und Leerzeichen ergeben `None` (über 50 Gegenbeispiele im Test). `Ticket` und `TokenSet` zeigen in `Debug` nur `Ticket { .. }` und `TokenSet { .. }`. `TokenSet` hat weder `Display` noch `Serialize` noch einen öffentlichen Zugriff auf die Zeichenketten (Doctests mit `compile_fail`); die Ablage nutzt einen privaten Typ.
- **Tausch und Erneuern sind Methoden von `GarminClient`** (`connect`, intern beim Datenabruf), nicht freie Funktionen `exchange_ticket` und `refresh`. Grund: Jede Anfrage an Garmin, auch die zum Tausch, muss durch dieselbe Drossel und Sperre laufen; eine freie Funktion könnte sie umgehen. Der Tausch probiert die vier Client-Kennungen der Bibliothek der Reihe nach, sammelt je Kennung nur die Statuszahl (`AuthFailed { attempts }`), bricht bei 429 sofort ab und bei einem Netzfehler ebenfalls. Erneuert wird bei weniger als 900 s bis `exp` (aus der Nutzlast, ohne Signaturprüfung: der Client besitzt den Schlüssel nicht, Garmin entscheidet bei der Anfrage; `alg: none` gilt als unlesbar). Ist `exp` nicht lesbar, gilt das Token als gültig und der 401-Weg fängt es ab (wie die Bibliothek). Bei 401 wird genau einmal erneuert und wiederholt, ein zweites 401 ist `auth_failed`.
- **Ablage.** `KeyringStore` nutzt Dienst `lastwerk-garmin`, Konto `default`, als Geheimnis JSON-Bytes (`set_secret`, nicht `set_password`, weil Windows Zeichenketten als UTF-16 ablegt und damit das Limit von 2560 Bytes je Eintrag halbiert). Passt der Eintrag nicht unter 2400 Bytes, wird er ohne Zugangstoken abgelegt; der Client erneuert dann vor der ersten Anfrage (ein Aufruf mehr je Sitzung). Passt auch das Erneuerungstoken nicht, ist der Fehler `store_failed`. Ein unlesbarer Eintrag zählt als `not_connected`. Fehler des Schlüsselbund-Crates werden auf `StoreError` ohne Meldung und ohne Bytes abgebildet (`BadEncoding` enthält die gelesenen Bytes). Linux: Secret Service über `zbus` in reinem Rust, also ohne neue Systembibliothek in der CI; ohne laufenden Secret Service (GNOME-Schlüsselbund, KWallet) schlagen `load`, `save` und `delete` mit `Unavailable` fehl, einen Ersatz im Klartext gibt es nicht. Der Kernel-Schlüsselbund (`linux-native`) bleibt aus, weil er einen Neustart nicht überlebt. Ein Test gegen den echten Schlüsselbund (`keyring_round_trip_live`) ist `#[ignore]` und läuft nur von Hand.
- **Allowlist.** Es gibt keine freie Pfadmethode. Jede Methode baut den Pfad aus getypten Eingaben (Datum als `NaiveDate`, Kennung als Zahl, `displayName` prozentkodiert und ohne `.`, `..`, leer, über 128 Bytes), danach prüft `ensure_allowed` ihn gegen acht feste Präfixe und eine Zeichenliste. Gesendet wird nur GET. Zusätzlich zu den Endpunkten der Aufgabe gibt es `stress_day` (`/wellness-service/wellness/dailyStress/{datum}`), weil 5.6 Stress in der Allowlist nennt. `activities` begrenzt `limit` auf 1 bis 1000 (Q27: `MAX_ACTIVITY_LIMIT`).
- **Antworten.** 204 gibt `Value::Null` (JSON), `Vec::new()` (Liste, FIT) oder eine leere Liste; 404 `not_available`; 429 und 403 `throttled`; andere 4xx und 5xx `Http(status)`; 3xx und 1xx `invalid_response`. Antwortgrößen: JSON höchstens 8 MiB, FIT höchstens 256 MiB, Rümpfe von Fehlerantworten werden nie gelesen. Keine Weiterleitung wird befolgt. Zeitlimit 15 s je Datenabruf, 30 s für Tausch und Erneuern (`ClientConfig`). Ein Umgebungsproxy (`HTTPS_PROXY`) bleibt erlaubt, nur die Tests schalten ihn ab. Gegenüber `coach` gilt hier TLS: rustls mit den Zertifikaten des Betriebssystems.
- **Fehlercodes.** Die sieben Codes der Aufgabe (`not_connected`, `auth_failed`, `throttled`, `locked`, `not_available`, `network`, `invalid_response`) plus zwei Zusätze: `store_failed` (Schlüsselbund nicht erreichbar oder voll) und `invalid_input` (Datum, Kennung oder Name vom Aufrufer abgelehnt, bevor etwas gesendet wird). `Http(status)` hat den Code `network`. Kein Fehler trägt URL, Query, Token, Ticket oder Antwortinhalt; der Markertest (`tests/secrecy.rs`) lässt den Server absichtlich alles zurückspielen und prüft jede Meldung.
- **Drossel.** Werte aus `bridge/src/lastwerk_bridge/throttle.py` (Faustregel, O24): 1,5 s plus bis 0,5 s Zufall zwischen zwei Anfragen, nach einer Antwort 429 oder 403 zusätzlich 30 s Pause, dritte 429 oder 403 in Folge: Sperre für 6 Stunden. Jede andere Antwort setzt den Zähler zurück, ein Netzfehler weder zählt noch setzt zurück. Die Sperre steht in einer Datei, deren Pfad der Aufrufer bestimmt (`{"since_epoch_s","until_epoch_s","http_status","consecutive"}`, atomar geschrieben, ohne Token), und wird bei jedem Aufruf gelesen, gilt also über einen Neustart hinweg; lässt sich die Datei nicht schreiben, bleibt sie im Speicher. Eine unlesbare Datei gilt als keine Sperre. Die Wartezeit ist auf 30 s begrenzt, damit ein Sprung der Wanduhr nach hinten die App nicht Stunden schlafen lässt. Uhr, Schlafen und Zufall sind über `Clock` und `Jitter` austauschbar; ein `Mutex` hält Anfragen hintereinander. Der Zufall kommt ohne Zusatzabhängigkeit aus `RandomState` der Standardbibliothek (kein Sicherheitszweck).
- **Offen für T23.** Ob `diauth.garmin.com` Anfragen von rustls statt eines Browsers annimmt (O30), zeigt erst der Livetest. Die Kopfzeilen sind die der mobilen App aus der Bibliothek (`GCM-Android-5.23`), nicht die eines Browsers.

Umsetzung in T39 (Stand 16. Oktober 2026): fünfzehn weitere lesende Methoden nach 5.6 (`user_settings`, `cycling_ftp`, `lactate_threshold`, `heart_rate_zones`, `daily_steps`, `heart_rate_day`, `respiration_day`, `spo2_day`, `intensity_minutes_day`, `training_readiness`, `fitness_age`, `endurance_score`, `hill_score`, `training_load_activities`, `hydration_day`), alle mit der Rohantwort als `serde_json::Value`. Die Allowlist hat zwei Listen: `EXACT_PATHS` für feste Pfade (nur genau dieser Pfad, kein Anhängsel; dazu gehören seit T39 auch `socialProfile` und `bodyBattery/reports/daily`, die vorher Präfixe waren) und `ALLOWED_PREFIXES` mit einem engen Präfix je Endpunkt mit Datum oder Namen im Pfad. Spannen werden vor dem Senden geprüft: Schritte höchstens 28 Tage (`MAX_STEPS_SPAN_DAYS`, Docstring der Bibliothek), Ausdauer- und Bergwert höchstens 392 (`MAX_SCORE_SPAN_DAYS`, Sonde), Garmins Last höchstens 366 (`MAX_LOAD_SPAN_DAYS`, Faustregel). S3 (`hydration_day`) verlangt den Marker `SensitiveConsent`, den der Aufrufer nur nach gelesener Freigabe bildet. Tests: jeder Pfad gleich dem der Bibliothek, `never_fetched_paths_are_rejected` (alle sechs lesenden Pfade von `periodichealth-service`, kein Eintrag enthält `periodichealth`, `menstrual` oder `pregnan`), `unreleased_sensitive_paths_are_rejected` (Gewicht, Blutdruck, Ernährung, Lifestyle und nicht belegte Lesepfade), dazu ein Test gegen die Attrappe, der Pfade und GET-only prüft.

#### lastwerk-cli

```
lastwerk import      --dir <ordner> --db <datei>
lastwerk import-recovery --dir <ordner> --db <datei>      # exchange/out/recovery, ab M4d
lastwerk rebuild     --dir <ordner> --db <datei>
lastwerk thresholds  --db <datei> set --sport cycling --ftp 250 --from 2026-01-01
lastwerk thresholds  --db <datei> list
lastwerk thresholds  --db <datei> suggest [--sport cycling|running|swimming]
lastwerk athlete     --db <datei> set --sex male|female --birth-year <jahr>
lastwerk pmc         --db <datei> --from <datum> --to <datum> [--format csv|json]
lastwerk plan        --db <datei> --week-start <datum> [--goal-date <datum>] --out <plan.json>
lastwerk plans       --db <datei>                                   # Nachtrag 14.10.2026
lastwerk plan-status --db <datei> --id <plan_id> --status draft|active|archived
lastwerk validate    <plan.json> --db <datei>
lastwerk export-ics  <plan.json> --out <plan.ics>
lastwerk backup      --db <datei> --out <datei.json>
lastwerk plausibility --dir <ordner>         # nur Kennzahlen auf der Konsole
lastwerk refine      <plan.json> --provider ollama|openai-compatible --model <name> --db <datei>   # M5
                     [--base-url <url>] [--allow-lan] [--out <datei>] [--yes] [--timeout <s>]
```

Exit-Codes: 0 Erfolg, 1 Validierungsfehler, 2 Eingabefehler, 3 interner Fehler.

Ergänzungen in T08 (Stand 13. Oktober 2026), nicht in der Skizze oben: `lastwerk restore --db <datei> --in <datei.json>` (spielt eine Sicherung ein und ersetzt die Nutzertabellen, danach `rebuild`), `lastwerk goal set|list`, `lastwerk availability set|list` (die Eingaben des Planers, ohne die der Befehl `plan` nichts aus der Datenbank läse) und bei `plan` zusätzlich `--goal-sport`, `--weeks`, `--locale de|en`, `--sports` und `--strength`.

Umsetzung in T04 (Stand 13. Oktober 2026): `import`, `rebuild` und `plausibility` sind umgesetzt (`clap`, Ausgabe auf Deutsch). Fehlerhafte FIT-Dateien stehen im Bericht und ändern den Exit-Code nicht (0), Exit-Code 1 bleibt Validierungsfehlern wie einem Plan gegen das Schema (T08) vorbehalten. Argumentfehler, ein fehlender Ordner, ein Datenbankpfad, der ein Ordner ist, und eine Datei, die keine Datenbank ist, enden mit 2. `--db` ist optional, der Standardpfad ist die Umsetzung von 6.1 ohne zusätzliches Crate: unter Windows `%LOCALAPPDATA%\Lastwerk\lastwerk.db`, unter macOS `~/Library/Application Support/Lastwerk/lastwerk.db`, sonst `$XDG_DATA_HOME/Lastwerk/lastwerk.db` oder `~/.local/share/Lastwerk/lastwerk.db` (seit dem 14. Oktober 2026 mit der Plattform als Parameter von `paths::default_db_path_from`, damit die CLI dieselbe Datei wie Tauri `local_data_dir` nimmt; zuvor nahm sie unter macOS den XDG-Pfad und wich von der App ab). `LASTWERK_DB` (nicht leer) ersetzt den Standardpfad wie in der App, `--db` geht beiden vor. `plausibility` hat zusätzlich das optionale `--db`: daraus werden nur die Nutzerdaten (Schwellenwerte) in eine Arbeitsdatenbank im Speicher kopiert, damit Belastungen berechenbar sind. Ohne `--db` sind alle Belastungen "nicht berechenbar". Die Ausgabe enthält nur Zahlen, keine Dateinamen und Pfade, der Zielordner und die Platte bleiben unberührt.

Umsetzung in T05 (Stand 13. Oktober 2026):

- `thresholds set --sport <cycling|running|swimming|all> (--kind <ftp_w|lthr_bpm|hr_max_bpm|hr_rest_bpm|run_speed_mps|css_mps> --value <zahl> | --ftp | --lthr | --hr-max | --hr-rest | --run-speed | --css <zahl>) --from <datum> [--note <text>] [--from-suggestion] [--dir <ordner>]`. Die Kurzformen erlauben das Beispiel aus der Skizze (`--ftp 250`) und dürfen zusammen vorkommen. Werte müssen positiv sein, Leistung höchstens 2500 W und Herzfrequenzen zwischen 30 und 230 bpm (Grenzen aus 6.5), FTP, Laufschwelle und CSS gelten nur für ihre Sportart oder `all`. Die Herkunft ist `manual`, mit `--from-suggestion` `suggested_accepted` (ausdrückliche Übernahme eines Vorschlags, die CLI übernimmt nie selbst).
- Neuberechnung nach `thresholds set` (Entscheidung T05): Mit `--dir` rechnet der Befehl die betroffenen Aktivitäten (Sportart, ab `--from` bis zur nächsten Schwelle gleicher Art) aus den FIT-Dateien neu (`recompute_loads`, 5.4 store). Ohne `--dir` gibt er den Hinweis aus, dass Belastungen erst nach `rebuild` oder mit `--dir` gelten, und endet mit 0. `athlete set` verhält sich ebenso (`--dir` rechnet alle Aktivitäten neu).
- `thresholds list` zeigt alle Werte mit Sportart, Gültigkeitsdatum, Herkunft und Notiz. Gibt es keine `hr_max_bpm`, steht die Schätzung aus dem Alter dabei, gekennzeichnet als geschätzt (`estimated`).
- `thresholds suggest` zeigt FTP als 95 Prozent der besten 20-Minuten-Leistung aus `activity_best_power` (mit Datum, Warnhinweis und dem Befehl zur Übernahme), CSS aus `--css-t400` und `--css-t200` (Sekunden für 400 m und 200 m) und den Hinweis, dass die LTHR aus dem Cache nicht ableitbar ist (sie braucht die Herzfrequenz eines 30-Minuten-Tests). Gespeichert wird nichts.
- `athlete set [--sex male|female] [--birth-year <jahr>] [--dir <ordner>]` legt `athlete.sex` (nur TRIMP-Konstanten) und `athlete.birth_year` (nur Schätzung der maximalen Herzfrequenz) als Einstellungen ab. Das Geburtsjahr muss zwischen 1900 und dem laufenden Jahr liegen.
- `pmc --db --from --to [--format csv|json] [--tau-ctl <tage>] [--tau-atl <tage>] [--ctl0 <wert>] [--atl0 <wert>]`: Der Verlauf beginnt am ersten Tag mit zählender Last (primäre Aktivität, Status `ok`), falls der vor `--from` liegt, damit CTL und ATL bei `--from` eingeschwungen sind. Ausgegeben werden nur Tage ab `--from`, Tage ohne Last mit Last 0. CSV-Spalten `date,load,ctl,atl,tsb` mit Dezimalpunkt und drei Nachkommastellen, JSON als Liste mit denselben Feldern. Hinweise (Einschwingzeit, keine Belastungen im Zeitraum) stehen auf der Fehlerausgabe, damit die Standardausgabe rein bleibt. Startwerte und Zeitkonstanten sind Optionen. Seit dem 14. Oktober 2026 gelten ohne Option die Einstellungen `pmc.*` der Datenbank (`Store::pmc_params`, dasselbe wie in App, MCP-Server und Planer, Standard 42 und 7 Tage, Start 0), und eine Option überschreibt nur ihren Wert. Zuvor nahm `pmc` feste Standardwerte, sodass es von App und MCP-Server abwich.
- Lesende Befehle (`pmc`, `thresholds list`, `thresholds suggest`) legen keine Datenbank an; fehlt die Datei, enden sie mit 2.

Umsetzung in T14 (Stand 13. Oktober 2026):

- `import-recovery --dir <ordner> --db <datei>` liest `YYYY-MM-DD.json` aus dem Austauschordner. Dateien dort sind nicht vertrauenswürdig (5.4, Austausch): je Datei zählt nur ein regulärer Eintrag direkt im Ordner (Symlinks und Sonderdateien sind ungültig, Unterordner und Dateien ohne Endung `.json` werden übersprungen), der Name muss genau `YYYY-MM-DD.json` mit einem echten Datum sein, die Datei höchstens 64 KiB (Faustregel; gelesen wird nie mehr als das Limit plus ein Byte), der Inhalt gültig gegen `schema/recovery-day.schema.json` (per `include_str!` eingebettet, Formatprüfung für `date` und `date-time`), und `date` muss zum Namen passen. Pfade aus dem Inhalt gibt es nicht. Ungültige Dateien werden abgelehnt, gezählt und mit Grund (`bad_name`, `not_regular_file`, `too_large`, `unreadable`, `not_json`, `schema`, `date_mismatch`) gemeldet, der Import läuft zu Ende, der Exit-Code bleibt 0 (wie bei fehlerhaften FIT-Dateien). Höchstens 20 000 Einträge je Ordner werden angesehen. Die Meldungen nennen weder Messwerte noch den Ordnerpfad, Steuerzeichen in Dateinamen werden ersetzt, bei Schemaverletzungen stehen nur JSON-Pfad und Schlüsselwort (`/hrv_overnight_ms: maximum`). Die gültigen Tage gehen in einer Transaktion in `recovery_day`, ein zweiter Lauf mit denselben Dateien ändert nichts (Bericht: gültig mit neu, aktualisiert, unverändert; ungültig; übersprungen). Ein fehlender Ordner endet mit 2 und legt keine Datenbank an.
- `plan` und `validate` lesen die Erholungstage der `baseline_days + max_age_days` Tage vor dem Planbeginn aus der Datenbank und übergeben sie als `PlanContext::recovery` (R12). Ohne importierte Tage ist die Liste leer und nichts ändert sich. `backup` und `restore` führen die Tage mit.

Umsetzung in T15 (Stand 13. Oktober 2026): `refine <plan.json> --provider ollama|openai-compatible --model <name> [--base-url <url>] [--allow-lan] [--db <datei>] [--out <datei>] [--yes] [--timeout <s>]` (Modul `refine.rs`). Ablauf: Anbieter anlegen (nicht erlaubter Host, fehlende `--base-url` bei `openai-compatible`, `https`, Zugangsdaten in der Adresse: Exit 2, noch vor dem Lesen von Datenbank und Plan), ohne `--yes` und ohne Terminal Exit 2, Plan lesen (Schema, Exit 1) und gegen die harten Regeln prüfen (Verletzung: Exit 1, nichts geht zum Modell), dann den Eingang anzeigen: die feste Systemanweisung und der Eingang als JSON, genau der Text, der zum Modell geht (ein Test vergleicht beide). Ohne `--yes` folgt die Frage `Senden? [j/N]`, nur `j`, `ja`, `y`, `yes` senden, alles andere bricht mit Exit 0 ab. Bei `Applied` schreibt der Befehl den neuen Plan (neue `plan_id`, `created_at` = jetzt) nach `--out`, ohne Angabe neben die Eingabe als `<name>.refined.json`, speichert ihn als `draft` (mit `goal_id` des aktiven Ziels, wenn es zum Ziel des Plans passt) und gibt die Erklärung als Text aus. Die Meldung nennt den Pfad der Ausgabedatei nicht, nur dass eine Datei geschrieben wurde (Stand 14. Oktober 2026, wie bei den übrigen Befehlen). Der Regelplan in der Datenbank und die Eingabedatei bleiben unberührt. Bei `Fallback` steht der Grund (`timeout`, `transport`, `invalid_output`) mit Zahl der Aufrufe und Korrekturen auf der Ausgabe, es wird nichts geschrieben, Exit 0. `--timeout` gilt je Aufruf (Standard 120 s, 1 bis 3600). Die Ausgabe ersetzt Steuerzeichen außer Zeilenumbruch und Tabulator in der Erklärung durch `?`, damit ein Modell keine Terminal-Steuerfolgen ausgeben kann; der Plan behält den Text unverändert. Die Meldungen stehen wie bei den übrigen Befehlen fest auf Deutsch, `catalog/locales` bekommt keine neuen Schlüssel. `lastwerk-cli` hängt damit von `lastwerk-coach` ab, und die Desktop-App (die `lastwerk-cli` einbindet) trägt `ureq` in ihrer Kette; `app/src-tauri/Cargo.lock` hat dafür vier neue Pakete (`lastwerk-coach`, `ureq`, `ureq-proto`, `utf8-zero`).

Ergänzung in T26 Teil A (Stand 15. Oktober 2026): `lastwerk chat --message <text> [--db <datei>] [--plan <plan.json>] [--provider ollama|openai-compatible] [--model <name>] [--base-url <url>] [--allow-lan] [--show-input] [--timeout <s>]` (Modul `chat.rs`), ein Gesprächsschritt zum Ausprobieren mit einem echten Modell (O13). Ohne `--plan` gilt der aktive Plan der Datenbank (fehlt er: Exit 2). Ohne `--model` fragt der Befehl bei Ollama die vorhandenen Modelle ab und wählt eines mit `pick_model` (9.7); bei `openai-compatible` ist `--model` Pflicht. Adressprüfung wie bei `refine`, vor jedem Netzzugriff (Exit 2). Leere Nachricht oder mehr als 1000 Zeichen: Exit 2, nichts gesendet. `--show-input` zeigt vor dem Senden jede Nachricht genau so, wie sie zum Modell geht; eine Rückfrage gibt es nicht, weil die Nachricht selbst der ausdrückliche Wunsch ist und nichts übernommen wird. Ausgabe: die Antwort als Text (Steuerzeichen als `?` wie bei `refine`), ein geprüfter Vorschlag mit seinen Änderungen und Vorher/Nachher je Woche (Minuten, harte Einheiten, Last) oder den neuen Eingaben des Planers, verworfene Vorschläge mit ihren Codes. Gespeichert wird nichts, es gibt keinen Verlauf über den Aufruf hinaus. Antwortet das Modell nicht oder unbrauchbar, steht der Grund mit Zahl der Aufrufe auf der Ausgabe, Exit 0 (wie `refine`). Kennzahlen des Dashboards gibt es vor T25 nicht, die Liste `metrics` ist leer.

Nachtrag vom 14. Oktober 2026 (Planstatus in der Kommandozeile): `lastwerk plans --db <datei>` listet die gespeicherten Pläne, die neuesten zuerst, mit ID, Status (`draft`, `active`, `archived`), Erstellungsdatum ohne Uhrzeit und Zahl der Wochen (`-`, wenn sich der Plantext nicht lesen lässt). `lastwerk plan-status --db <datei> --id <plan_id> --status draft|active|archived` setzt den Status. Beide sind eigene Befehle der obersten Ebene, weil `plan` schon den Plan erzeugt und seine Optionen (`--week-start`, `--out`) nicht zu einem Unterbefehl passen; `goal` und `availability` bleiben die Befehle mit `set` und `list`. Beide lesen eine vorhandene Datenbank und legen keine an (fehlt sie: Exit 2). Die Statuslogik ist `Store::set_plan_status`, dieselbe Methode wie `set_plan_status` der App, es gibt keine zweite: Aktiv setzen archiviert in derselben Transaktion den bisherigen aktiven Plan (höchstens einer ist aktiv), andere Statuswechsel berühren keinen anderen Plan. Die Meldung nennt die archivierte ID. Eine unbekannte ID ist Exit 2 (`PlanNotFound`), ein unbekannter Status scheitert schon in `clap` mit Exit 2. Damit führt auch die reine Kommandozeile zu einem aktiven Plan für `get_current_plan` und `get_plan_week` (lastwerk-mcp).

Umsetzung in T09 (Stand 13. Oktober 2026): `lastwerk-cli` ist Bibliothek und Binary (`src/lib.rs` exportiert `commands`, `planning`, `schema`, `ics`, `texts`, `userdata`, `paths` und die übrigen Module als `pub mod`, `src/main.rs` parst nur die Argumente). Grund: Die Desktop-App soll Pläne mit derselben Logik erzeugen, prüfen und exportieren wie die Kommandozeile (Kontextbildung aus der Datenbank, Schemaprüfung, ICS, Sicherung), ohne sie zu kopieren. Entscheidung gegen ein neues Crate `crates/planning`: weniger Verschiebung, die Module bleiben an ihrem Ort, die Kosten sind `clap` als Abhängigkeit der App, das die Bibliothek nicht nutzt (nur das Binary). Geändert hat sich nur die Sichtbarkeit (`planning::validation_context` ist öffentlich) und der Präfix der insta-Dateien (`lastwerk_cli__` statt `lastwerk__`, weil insta sie nach dem Crate benennt).

Umsetzung in T22 (Stand 15. Oktober 2026), Abgleich mit Garmin (Modul `garmin`, Unterordner `garmin/` mit `fit_zip.rs` und `parse.rs`):

```
lastwerk garmin status   [--db <datei>]
lastwerk garmin sync     [--db <datei>] [--days <1..1825>]
lastwerk garmin logout
```

```rust
pub trait GarminSource { /* social_profile, activities, activity_fit, hrv_range, rhr_range, sleep_day, body_battery */ }
impl GarminSource for GarminClient { .. }
pub struct SyncOptions { pub today: NaiveDate, pub now: DateTime<Utc>, pub days: Option<u32> }
pub fn sync(store: &mut Store, source: &dyn GarminSource, fit_dir: &Path, opts: &SyncOptions,
            progress: &mut dyn FnMut(SyncProgress)) -> Result<SyncSummary, SyncError>;
pub fn sync_with_client(store: &mut Store, client: &GarminClient, fit_dir: &Path,
            today: NaiveDate, now: DateTime<Utc>, progress: &mut dyn FnMut(SyncProgress))
            -> Result<SyncSummary, SyncError>;     // die Funktion der App
```

- **Quelle als Trait.** `sync` arbeitet gegen `GarminSource`, nicht gegen den `GarminClient`; Tests setzen eine Attrappe ein, ohne Netz und ohne Warten. Abweichung von der Aufgabe: `sync` nimmt zusätzlich `now` (für `imported_at`, der Speicher liest keine Uhr) und optional `days` (für `--days`), gebündelt in `SyncOptions`; `sync_with_client` hat `now` als eigenen Parameter. `today` ist das lokale Datum des Nutzers.
- **Fenster.** Ohne `garmin.last_sync`: `today - backfill_days` (Standard 365, Faustregel). Sonst `last_sync - 7 Tage` (Garmin ergänzt Tageswerte nachträglich, wie `--refresh-days 7`, 5.5). `--days N` ersetzt beides (`today - N`). Ein `last_sync` in der Zukunft macht das Fenster nicht leer (Beginn höchstens `today`).
- **Aktivitäten.** Liste in Seiten zu 100 (Faustregel), neueste zuerst, bis zur ersten Aktivität vor dem Fenster, höchstens 200 Seiten (Faustregel). Für jede Kennung ohne Datei `<fit_dir>/<Kennung>.fit` kommt das ZIP. Aus dem ZIP wird die erste Datei mit Endung `.fit` entpackt (`fit_zip::extract_fit`): der Name aus dem Archiv entscheidet nur, ob es eine FIT-Datei ist, und wird nie zu einem Pfad (Zip-Slip ist durch Bauart ausgeschlossen, Test mit sieben bösen Namen); höchstens 256 MiB entpackt (die angegebene Größe wird vorab geprüft, gezählt wird trotzdem beim Lesen, weil die Angabe lügen kann); geschrieben wird in `<Kennung>.fit.part`, mit `sync_all` und dann umbenannt; höchstens 64 Einträge werden angesehen (Faustregel); nur Stored und Deflate. Fehler je Aktivität (`zip_no_fit`, `zip_invalid`, `fit_too_large`, `empty_download`, dazu die Codes von `lastwerk-garmin`) werden gezählt, der Lauf geht weiter; ein Schreibfehler des Dateisystems beendet ihn als `SyncError::Io`. Danach der gewöhnliche Import des Ordners (`import_dir`, Duplikatregel 6.4, Belastung).
- **Erholung.** `hrv_range` in Spannen von höchstens 336 Tagen, `body_battery` höchstens 31 (beide gemessen in L1, `bridge/registry.yaml`), `rhr_range` höchstens 365 (Faustregel nach dem Docstring der Bibliothek; die Sonde fand bis 1792 Tage kein Limit), `sleep_day` je Tag nur für die letzten 90 Tage des Fensters (Faustregel: 7-Tage-Basis und 90-Tage-Trend von R12 brauchen nicht mehr, und jeder Tag ist ein Aufruf). Ruhepuls und Schlaf brauchen den Anzeigenamen des Kontos: ein Aufruf `social_profile` je Lauf, der Name bleibt im Speicher des Laufs und wird nirgends abgelegt oder ausgegeben (deshalb zeigt `garmin status` ihn nicht an). Werte außerhalb der Grenzen aus 7.3 werden einzeln verworfen und gezählt (`dropped_values`); eine Schlafdauer von 0 s ist keine Messung und bleibt leer. Je Tag wird mit der vorhandenen Zeile gemischt: ein neuer Wert ersetzt den alten, ein fehlender lässt ihn stehen; ein Tag ohne einen einzigen Wert legt keine Zeile an. Antworten mit Tagen außerhalb der angefragten Spanne werden an dieser Stelle verworfen. Quelle `garmin-app`.
- **Feldnamen.** Nach `docs/garmin-feldabdeckung.md`: `hrvSummaries[].calendarDate` und `.lastNightAvg`; `bodyBatteryValuesArray[][1]` und `[].date` (Höchst- und Tiefstwert, `null` übergangen); `dailySleepDTO.sleepTimeSeconds` und `dailySleepDTO.sleepScores.overall.value`. **Annahmen (O9):** Für `hrv_range` nennt der Bericht nur `hrvSummaries: array(7) / object(8 keys)`, die zwei Namen sind vom Tagesendpunkt übertragen. Für `rhr_range` zeigt der Bericht die Liste `[].calendarDate/.value`, die die Python-Bibliothek aus der Rohantwort baut; `GarminClient::rhr_range` liefert die **Rohantwort** (`allMetrics.metricsMap.WELLNESS_RESTING_HEART_RATE[]` mit `calendarDate` und `value`, aus dem Quelltext von garminconnect 0.3.17), deshalb liest `parse::rhr_days` beide Formen.
- **Fehlerbehandlung.** Ein Fehler von Garmin ist kein Fehler der Funktion: `Ok(SyncSummary)` mit `aborted: Some(SyncAbort { code, until })`, wenn der Lauf wegen `not_connected`, `auth_failed`, `throttled`, `locked`, `network` oder `store_failed` endet, und mit allem, was bis dahin da war (geladene Dateien werden importiert, gesammelte Erholungstage geschrieben). Ebenso nach 5 Fehlern in Folge (Faustregel; "nicht verfügbar", 404, zählt nicht und ist beim Schlaf normal). Ein einzelner anderer Fehler (`Http`, `invalid_response`, 404) trifft nur seinen Aufruf und steht in `errors` je Code. `SyncError` (Speicher, Dateisystem) ist nur für lokale Fehler und trägt nie einen Pfad. `garmin.last_sync` wird nur bei einem Lauf ohne Abbruch auf `today` gesetzt; eine Aktivität, deren Laden einzeln scheiterte, wird beim nächsten Lauf nachgeholt, solange sie im Fenster (7 Tage Überlappung) liegt. Die Kommandozeile gibt den Bericht aus und endet bei einem Abbruch mit Exit-Code 2 und einer festen deutschen Meldung (`abort_message`) ohne Pfad, Token und Namen.
- **Fortschritt.** `SyncProgress { phase: Activities | Recovery | Import, done, total: Option<u32> }`; beim Lesen der Liste zählt `done` die gefundenen Aktivitäten und `total` fehlt, danach ist `total` die Zahl der zu ladenden Dateien; Erholung zählt die geplanten Aufrufe, Import die Dateien.
- **Befehle.** `status` ohne Netzzugriff: verbunden ja oder nein (Token im Schlüsselbund), letzter Abgleich, Einstellungen, Ende einer aktiven Sperre. `logout` löscht die Token (der Zugriff bei Garmin bleibt, bis der Nutzer ihn dort widerruft). Eine Anmeldung gibt es in der Kommandozeile nicht (sie braucht das Fenster der App, 5.6); die Meldung bei `not_connected` verweist darauf.
- **Pfade (Stand T22, ersetzt in T23 und T28).** FIT-Ablage war `<Datenordner>/Lastwerk/garmin/fit`; seit T23 siehe "Ort der Ablage" unten. Sperrdatei `<Datenordner>/Lastwerk/garmin/lockout.json` (unverändert, nicht im FIT-Ordner, damit der Import sie nie sieht). `LASTWERK_GARMIN_FIT` entfällt seit T28, an seine Stelle tritt `LASTWERK_FIT` (fester Stamm, siehe "Feste Ablage").
- **Grenze der Ablage (T22, gelöst in T23).** `import_dir` führt `source_file` über `rel_path` relativ zum eingelesenen Ordner. Mit dem eigenen Ordner `garmin/fit` neben dem FIT-Ordner des Nutzers gab es zwei Wurzeln in einer Tabelle: gleiche Dateinamen hätten sich ersetzt, und `rebuild_cache` mit nur einem Ordner hätte die Aktivitäten des anderen verloren.
- **Ort der Ablage (Entscheidung T23, Stand 15. Oktober 2026; seit T28 abgelöst durch "Feste Ablage" unten, die Regeln zu `app.fit_dir`, `assigned` und `FitDirMissing` gelten nicht mehr).** Die Dateien von Garmin liegen im Unterordner `garmin/` des FIT-Ordners des Nutzers (`app.fit_dir`), eingelesen wird der ganze FIT-Ordner (rekursiv, wie bisher). `garmin::fit_location(store, default_root) -> FitLocation { root, download, assigned }` bestimmt beides, `garmin::sync_into(store, source, download_dir, import_root, opts, progress)` trennt Ablage und Importordner (`sync` ruft es mit demselben Ordner für beides auf). Ist kein FIT-Ordner gewählt (nur Garmin), legt der erste Abgleich `<Datenordner>/Lastwerk/fit` (`paths::default_fit_root`, neben der Datenbank) an und merkt ihn als `app.fit_dir`. Ein gewählter, aber fehlender Ordner (Laufwerk nicht verbunden) wird nicht neu angelegt, der Abgleich endet mit `SyncError::FitDirMissing`. Damit gibt es eine Wurzel: Garmin-Dateien haben `rel_path` `garmin/<Kennung>.fit`, `rebuild_cache` und `import` mit dem FIT-Ordner verlieren nichts, die Messreihe im Detail findet die Datei, und 6.1 gilt weiter (der FIT-Ordner ist die Quelle). Legt der Nutzer dieselbe Aktivität zusätzlich selbst in den Ordner (USB-Kopie), greift die Duplikatregel 6.4 (gleiche Sportart, Start und Dauer). `lastwerk garmin sync` folgt derselben Regel und meldet, wenn es den Standardordner angelegt hat (ohne Pfad). Test: `one_root_keeps_own_and_garmin_files_through_a_rebuild` in `crates/cli/tests/garmin_sync.rs`.

Umsetzung in T39 (Stand 16. Oktober 2026), Vollabzug (Module `garmin/full.rs`, `garmin/parse_full.rs`):

- **Phasen.** Nach Aktivitäten und Erholung folgen `account` (Profil, FTP, Laktatschwelle, Herzfrequenzzonen; vier Aufrufe) und `wellness` (Schritte in Spannen zu 28 Tagen, Bergwert und Garmins Last je Aktivität über das ganze Fenster, Ausdauerwert einmal über höchstens 392 Tage, Fitnessalter für heute; Tagesherzfrequenz, Atmung, SpO2, Intensitätsminuten, Stress, Trainingsbereitschaft und mit Freigabe Hydration je Tag für die letzten `DETAIL_DAYS` = 28 Tage, Faustregel). `SyncPhase` kennt dafür `account` und `wellness`. Der Anzeigename aus der Erholungsphase bleibt im Speicher des Laufs für die Tagesherzfrequenz.
- **Quelle.** `GarminSource` hat die neuen Methoden mit Vorgaben, die "keine Daten" liefern (`Value::Null`), damit Attrappen nur nachbilden, was ihr Test braucht; `GarminClient` setzt alle um.
- **Schreiben.** Nach allen Phasen und vor dem Import: Schwellen nach 8.6 (`Store::upsert_garmin_threshold`), Geschlecht und Geburtsjahr nach Herkunft (ein Wert ohne Herkunft aus der Zeit vor T39 gilt als eigener Wert), dann die Tabellen aus 6.2. Tagesantworten mit anderem `calendarDate` als dem angefragten Tag und Tage außerhalb des Fensters werden verworfen, Werte außerhalb der Grenzen gezählt (`dropped_values`). Danach Import und, wenn sich eine Schwelle oder ein Athletenwert geändert hat, `recompute_loads` ab dem frühesten geänderten Tag (bei Athletenwerten alles).
- **Bericht.** `SyncSummary` hat zusätzlich `thresholds_changed`, `recomputed`, `wellness_days`, `training_days`, `sensitive_days`, `garmin_loads`, `sensitive_released`; `lastwerk garmin sync` gibt sie aus.
- **Athlet.** `athlete set` und `set_athlete` der App setzen `athlete.sex_origin` und `athlete.birth_year_origin` auf `manual`.
- **Tests** (`crates/cli/tests/garmin_full.rs`): Aktivität vor heute bekommt eine Last aus einer früheren Garmin-FTP ohne Eingabe (Handrechnung TSS 32 bei 250 W); eine spätere Garmin-FTP rechnet die bekannte Fahrt neu (TSS 50); manuell vor Garmin bei gleichem Datum; Werte ohne Datum ab Fensterbeginn, Änderung ab heute; Geschlecht und Geburtsjahr nur ohne eigenen Wert, Gewicht nie gespeichert; Tageswerte mit Grenzen und Datumsprüfung; Hydration nur mit Freigabe genau `true`, nie ein Aufruf zu Menstruation oder Schwangerschaft; ein zweiter Lauf mit denselben Antworten schreibt nichts.

Umsetzung in T40 (Stand 16. Oktober 2026):

- **Reihenfolge.** `account` läuft zuerst und holt zusätzlich den Verlauf von FTP (Rad), LTHR und Schwellentempo (Lauf) in Stücken zu höchstens 366 Tagen über das ganze Fenster; dann `activities`; dann werden Schwellen und Athletenwerte geschrieben (`write_account`), der FIT-Stamm eingelesen und Belastungen neu gerechnet; erst danach `recovery` und `wellness`, am Ende die Tageszeilen (`write_days`). Grund: Beim ersten Abgleich brauchen Erholung und Tageswerte mehrere Minuten, Last und Form stehen so vorher. Bricht Garmin in der ersten Hälfte ab, laufen Erholung und Tageswerte nicht mehr.
- **Tageswerte.** Zusätzlich VO2max-Verlauf über das ganze Fenster, Belastungsfokus für heute, Lauftoleranz und Rennprognosen (Verlauf und jüngster Stand) für die Detailtage, je Detailtag Trainingsstatus und Etagen, mit Freigabe je Bereich Gewicht und Blutdruck über das ganze Fenster und Ernährung je Detailtag. Ein 404 (zum Beispiel Rennprognosen ohne Verlauf) ist kein Fehler.
- **Tests** (`crates/cli/tests/garmin_full.rs`): Ein Lauf bekommt mit Garmins Schwellentempo 0,30 eine Last (rTSS 100 bei 3,0 m/s); der FTP-Verlauf gibt einer Fahrt vor dem jüngsten Wert den damals gültigen (TSS 50 bei 200 W statt 32 bei 250 W), eine Lauf-FTP im Verlauf gilt nicht fürs Rad; Konto vor Aktivitäten, Aktivitäten vor Erholung; neue Tageswerte geschrieben; S3 nur für freigegebene Bereiche.

#### lastwerk-app

Tauri-Hülle in `app/src-tauri`, eigenes Cargo-Projekt (kein Mitglied des Wurzel-Workspace, damit `cargo test --workspace` ohne die Systembibliotheken von Tauri läuft, eigenes `Cargo.lock`, Pfadabhängigkeiten auf `crates/core`, `fit` und `store`). Die Befehle sind eine dünne Schicht über Speicher und Kern. Die Oberfläche (`app/ui`) spricht über das Interface `Backend` mit ihnen, in Tauri über `invoke`, im Browser, in Vitest und in Playwright über eine Attrappe mit synthetischen Daten.

Umsetzung in T06 (Stand 13. Oktober 2026):

- Befehle (alle `snake_case`, Fehler als `{ code, message_key }` ohne Pfade): `choose_folder` (Ordnerdialog im Rust-Teil, merkt den Ordner; **entfällt seit T28**), `get_settings`, `set_settings` (Sprache, Thema, Parameter des Verlaufs, Freigabe der Erholungswerte für den MCP-Server, nie ein Pfad), `import_folder` (Fortschritt als Ereignis `import-progress`), `get_week_overview(week_start, pmc_weeks)`, `list_activities(filter)`, `get_activity(id)`, `list_thresholds`, `set_threshold(input)` (mit Neuberechnung wie `lastwerk thresholds set --dir`), `set_athlete(input)`. Einstellungen liegen als `setting`: `app.fit_dir` (seit T28 nicht mehr geschrieben, nur noch für die Übernahme gelesen), `app.locale`, `app.theme`, `pmc.tau_ctl_d`, `pmc.tau_atl_d`, `pmc.ctl0`, `pmc.atl0` (die letzten vier schließen die offene Frage zur Ablage aus 8.2: sie stehen im Backup) und die Athletenwerte aus 6.2.
- Last mit Status: `ok` mit Wert und Methode, sonst `missing_threshold`, `missing_channel`, `implausible` oder `unknown` ohne Wert. rTSS, sTSS und Session-RPE tragen `estimated`.
- Die Messreihe im Detail kommt aus der FIT-Datei (der Cache hat keine Messpunkte). Der Befehl liest sie über den gespeicherten relativen Pfad erneut, prüft die SHA-256 und mittelt auf höchstens 800 Punkte.
- Der Speicher bekam dafür gezielte Lesemethoden (`activity`, `activities_between`, `loads_between`, `activity_loads_of`, `zone_times_of`, `best_powers_of`, `source_file`).
- Datenbankpfad: `<lokaler Datenordner>/Lastwerk/lastwerk.db`, derselbe wie in der Kommandozeile (Tauri `local_data_dir`, nicht `app_local_data_dir`, das den Bezeichner der App anhängt). `LASTWERK_DB` überschreibt ihn. Liegt der Pfad in einem bekannten Sync-Ordner (OneDrive, Dropbox, iCloud, Google Drive, Nextcloud, ownCloud, pCloud), warnt der Einstellungsdialog (6.1).
Umsetzung in T08 (Stand 13. Oktober 2026):

- Eingebettete Dateien (Konvention wie beim Zonenkatalog in T05, das Programm läuft ohne Nebendateien): `schema/plan.schema.json`, `catalog/rules.default.json`, `catalog/templates.json` und `catalog/locales/de.json` und `en.json` stehen per `include_str!` in `lastwerk-cli`. Neue Schlüssel `sport.cycling`, `sport.running`, `sport.swimming`, `sport.strength`, `sport.rest` in beiden Sprachdateien (Sportartnamen für den ICS-Titel).
- `plan --db <datei> --week-start <datum> [--goal-date <datum>] [--goal-sport cycling|running|swimming] [--weeks 1..26] [--locale de|en] [--sports a,b] [--strength 0..2] --out <plan.json>`. `--week-start` muss ein Montag sein, sonst Exit 2. Die Datenbank muss existieren (Exit 2). Der Kontext des Planers entsteht so: Verfügbarkeit aus `availability`, nicht gesetzte Tage gelten als 90 Minuten mit erlaubten harten Einheiten (Konvention, Hinweis auf der Fehlerausgabe). Historie aus `week_history(week_start, 12)`. CTL und ATL aus `pmc_state_at` am Tag vor dem Planbeginn mit den Parametern des Verlaufs (Einstellungen `pmc.*`, Standard 42 und 7 Tage, Start 0; so seit dem 14. Oktober 2026, siehe den Vermerk zu den Parametern des Verlaufs unter lastwerk-app). Sportarten: `--sports` mit gleichem Gewicht, sonst die Ausdauersportarten der letzten zwölf Wochen mit ihrer Trainingszeit als Gewicht, ohne Historie nur Rad (mit Hinweis). Kraft: `--strength`, Standard 1 je Woche (Konvention). Ziel: `--goal-date` (Priorität A, Sportart aus `--goal-sport` oder die mit dem größten Gewicht), sonst das aktive Ziel der Datenbank (liegt sein Termin vor dem Planbeginn, wird mit Hinweis fortlaufend geplant), sonst fortlaufend. Fehlt die Sportart des Ziels unter den Sportarten, wird sie mit dem höchsten Gewicht ergänzt. Horizont: `--weeks`, sonst bis zur Woche des Termins (höchstens 26), ohne Termin 8 Wochen. `plan_id` ist eine UUID v4 und `created_at` die Systemuhr, beides nur in der CLI.
- Der erzeugte Plan wird mit `validate_plan` und mit dem Schema geprüft, bevor er geschrieben wird (eine harte Verletzung ist Exit 1, ein Schemafehler ein interner Fehler, beides sollte nie auftreten). Die Datei ist eingerücktes JSON in der Feldreihenfolge des Schemas mit Zeilenumbruch am Ende. Zusätzlich speichert `plan` den Plan als `draft` in `plan` (mit `goal_id`, falls er auf dem Ziel der Datenbank beruht). Auf der Standardausgabe steht eine Zeile mit ID, Wochen, Einheiten und Warnungen, nie der Dateipfad.
- `validate <plan.json> --db <datei>`: erst das Schema mit der Bibliothek `jsonschema` (Draft 2020-12, Formatprüfung für `uuid`, `date` und `date-time` eingeschaltet, keine Auflösung von Verweisen über Netz oder Dateisystem), jede Schemaverletzung ist Exit 1 mit Liste aus Pfad und Meldung. Dann `validate_plan` gegen einen Kontext aus der Datenbank (Beginn der ersten Woche, Verfügbarkeit, Historie, CTL und ATL am Vortag). Konvention für Kraft: Enthält der Plan Krafteinheiten, gilt `strength_per_week` 1, sonst 0. Harte Verletzungen sind Exit 1, weiche stehen als Liste "Warnungen" auf der Standardausgabe und ändern den Exit-Code nicht. Eine nicht lesbare oder nicht als JSON lesbare Datei ist Exit 2, eine erste Woche, die kein Montag ist, Exit 1.
- `export-ics <plan.json> --out <plan.ics>` prüft den Plan zuerst gegen das Schema (Exit 1) und schreibt eine Veranstaltung je Einheit, siehe 10.2.
- `backup --db <datei> --out <datei.json>` schreibt `export_user_data` mit `exported_at`. `restore --db <datei> --in <datei.json>` legt die Datenbank bei Bedarf an und ersetzt die Nutzertabellen vollständig. Datei fehlt, kein JSON oder unbekannte `backup_version`: Exit 2, der Bestand bleibt unverändert. Danach `rebuild`, um den Cache aufzubauen. Die Sicherung kann persönliche Angaben enthalten (Zielbezeichnung, Pläne) und gehört nicht ins Repository.
- `goal set [--date <datum>] [--sport ...] [--priority A|B|C] [--label <text>]` (mit Datum ein Termin, ohne ein fortlaufendes Ziel, das neue Ziel ist das aktive), `goal list`, `availability set --weekday mon|tue|wed|thu|fri|sat|sun (auch mo bis so) --max-minutes <0..1440> [--allow-hard true|false] [--fixed-rest]`, `availability list`.
- Meldungen nennen nie den Pfad einer Datei, nur ihre Art (wie in T04).

Umsetzung in T09 (Stand 13. Oktober 2026):

- Die App hängt zusätzlich von `crates/cli` ab (Pfadabhängigkeit auf `lastwerk-cli` als Bibliothek, dazu `uuid` mit `v4` für die Plan-ID und `serde_json`). Begründung bei `lastwerk-cli` oben.
- Neue Befehle (21 insgesamt, an allen vier Stellen eingetragen): `list_plans`, `get_plan(id)`, `generate_plan(input)`, `set_plan_status(id, status)`, `get_planning_inputs`, `set_goal(input)`, `set_availability(days)`, `export_plan_ics(id)`, `export_plan_json(id)`, `export_backup`, `restore_backup(confirmed)`. `generate_plan` ruft `planning::generate` (wie `lastwerk plan`), prüft den Plan mit `planning::plan_json` gegen das Schema und speichert ihn als `draft`. `get_plan` liefert den Plan nach `plan.schema.json` (nur Schlüssel, keine Texte), die Vorausberechnung (`pmc` ab CTL und ATL am Vortag des Planbeginns mit den geplanten Tageslasten, wie `validate`) und die Prüfung (Schemaverletzungen, Leitplankenverletzungen mit Härte). Die Oberfläche schlägt Text und Quelle der Regeln unter `rule.R**.name` und `.explanation` nach.
- Dateidialoge: ICS, Plan-JSON und Backup öffnen den Speichern-Dialog, die Wiederherstellung den Öffnen-Dialog, jeweils im Rust-Befehl (`tauri-plugin-dialog`). Die Oberfläche bekommt nie einen Pfad, nur `saved` oder `restored` (`false` bei Abbruch) und Zahlen. Es gibt keine neue Berechtigung außer `allow-<befehl>` für die elf Befehle, kein Dialog- und kein Dateisystem-Plugin im Frontend. `restore_backup` verlangt `confirmed = true` (Fehler `unconfirmed`), liest höchstens 64 MiB und ersetzt die Nutzertabellen wie `lastwerk restore`. Weil die Einstellungen im Backup stehen, ersetzt es auch die Sprache (und einen noch nicht übernommenen alten FIT-Ordner, `app.fit_dir`). Danach weist die Oberfläche auf den neuen Import (oder `lastwerk rebuild`) hin.
- Schalter für Erholungswerte (Stand 14. Oktober 2026, O19): `get_settings` liefert `mcp_recovery_enabled` (`true` nur, wenn die Einstellung `mcp.recovery_enabled` genau `true` ist, sonst `false`, Standard aus), `set_settings` nimmt `mcp_recovery_enabled` als Wahrheitswert und schreibt genau `true` oder `false`. `SettingsPatch` ist ein getippter Datensatz, keine freie Liste von Schlüsseln: Zahlen, Texte und Listen statt eines Wahrheitswerts scheitern schon beim Einlesen (Test in `app/src-tauri/tests/service.rs`), ein Patch ohne das Feld lässt die Freigabe unverändert. Der Schlüssel steht als `SETTING_MCP_RECOVERY_ENABLED` in `lastwerk-store` und wird vom MCP-Server und von der App gelesen, es gibt keine zweite Schreibweise. Die Einstellungen zeigen einen Abschnitt mit Schalter und einem Hinweis, dass freigegebene Werte (HRV, Ruhepuls, Schlafdauer) Teil des Gesprächs mit dem Client werden und nicht zurückholbar sind. Wie 9.6 Punkt 1 verlangt, fragt das Einschalten nach (Freigeben oder Abbrechen) und speichert erst mit der Bestätigung, das Ausschalten sperrt sofort. Weil die Einstellung im Backup steht, stellt `restore_backup` auch die Freigabe wieder her. Die Freigabe gilt für das Werkzeug `get_recovery_days` insgesamt, nicht je Sitzung (offen bleibt O19, ob eine feinere Freigabe nötig ist).
- Parameter des Verlaufs (Stand 14. Oktober 2026, ersetzt die Abweichung aus T09): `planning::generate` und `planning::validation_context` rechnen mit den Einstellungen `pmc.tau_ctl_d`, `pmc.tau_atl_d`, `pmc.ctl0` und `pmc.atl0`. Gelesen werden sie von `Store::pmc_params()` in `lastwerk-store`, der einzigen Stelle mit dieser Auslese: ein fehlender, nicht lesbarer oder nicht endlicher Wert ergibt je Wert den Standard (42 und 7 Tage, Start 0), weitere Grenzen prüft das Lesen nicht (die Oberfläche lässt beim Schreiben nur Zeitkonstanten über 0 und Startwerte ab 0 zu, der Planer lehnt anderes als Eingabefehler ab). Der MCP-Server und die Oberfläche lesen dieselbe Methode, ihre eigenen Kopien des Parsers sind entfallen. CTL und ATL am Vortag des Planbeginns (`pmc_state_at`), die Zeitkonstanten in `PlanContext::pmc_params` (R1, Vorausberechnung) und die Vorausberechnung der Planansicht folgen damit den Einstellungen, in `lastwerk plan` wie in `lastwerk validate`. Ohne Einstellungen ändert sich nichts (die Golden-Snapshots bleiben bytegleich). Der Hinweis `pmc_settings_differ` ist samt Text entfallen. `lastwerk pmc` liest dieselben Einstellungen (Optionen überschreiben sie, siehe T05 oben), `get_fitness_status` rechnet damit wie `lastwerk pmc`.
- Ziele und Verfügbarkeit: `set_goal` legt wie `lastwerk goal set` ein neues aktives Ziel an (Sportart nur Rad, Lauf oder Schwimmen, Priorität nur bei einem Termin, Bezeichnung höchstens 200 Zeichen, bleibt lokal). `set_availability` prüft alle Tage vor dem Schreiben (Wochentag 0 bis 6 ohne Wiederholung, höchstens 1440 Minuten). Nicht gesetzte Tage zeigt die Oberfläche mit dem Standard des Planers (90 Minuten, harte Einheiten erlaubt).
- Fehlercodes neu: `not_monday`, `plan_failed`, `plan_invalid`, `export_failed`, `restore_invalid`, `unconfirmed` (mit Texten in beiden Sprachen). `PlanNotFound` wird `not_found`, `InvalidInput` wird `invalid_value`. Die Fehlerzuordnung kannte beide seit T08 neuen Varianten des Speichers nicht, das Projekt übersetzte deshalb nicht.
- Die Attrappe der Oberfläche erzeugt einen kleinen eigenen Plan (Standard 3 Wochen) ohne die Regeln des echten Planers und protokolliert die Aufrufe mit Dateidialog (`calls`, im Browser `window.__lastwerkMockCalls`).
- Release: `bundle.active` bleibt in `tauri.conf.json` aus (der Debug-Build der CI baut nur die Binärdatei, `tauri build --debug` ändert sich nicht). `tauri.release.conf.json` schaltet die Bündelung ein. Siehe 11.6.

Umsetzung in T19 (Stand 14. Oktober 2026): Neuer Befehl `get_bridge_status` (24 Befehle, an allen Stellen eingetragen). Er liest nur Dateinamen unter `<exchange>/out/recovery` (`YYYY-MM-DD.json`) und `<exchange>/out/activities` (`*.fit`) und gibt Anzahlen und den jüngsten Tag zurück, nie Pfade oder Inhalt, höchstens 50 000 Einträge je Ordner (Konvention). Der Ordner ist `local/exchange` oder `LASTWERK_EXCHANGE`. Abweichung: Eine installierte App hat kein Projektverzeichnis, deshalb wirkt der Standardpfad nur beim Start aus dem Repository, sonst setzt der Nutzer `LASTWERK_EXCHANGE`. Neue Einstellung `app.onboarded` (`true` oder `false`, Feld `onboarded` in `Settings` und `SettingsPatch`) für den Erststart-Assistenten, Feld `version` in `Settings` aus den Paketdaten ohne Netzwerk. Die App nimmt keine Garmin-Zugangsdaten an, der Assistent erklärt nur den Startbefehl von `bridge/` (SPEC 10.2). **Abgelöst durch T20 und T23:** Der Assistent verbindet über das Garmin-Fenster (5.6). Weiterhin gilt: Lastwerk zeigt kein eigenes Feld für Passwort oder MFA-Code, die Zugangsdaten tippt der Nutzer nur in die Seite von Garmin.

Umsetzung in T23 (Stand 15. Oktober 2026), Garmin in der App (5.6):

- Neue Befehle (28 insgesamt, an allen Stellen eingetragen): `garmin_status` → `{ connected, last_sync, sync_on_start, locked_until }` (ohne Netz; nie Token, nie Anzeigename), `garmin_login` (öffnet das Anmeldefenster und kehrt sofort zurück, Ergebnis als Ereignis `garmin-login` `{ ok, code }`), `garmin_sync(trigger)` (`manual` oder `start`, Fortschritt als Ereignis `garmin-sync-progress` `{ phase, done, total }`, Ergebnis `GarminSyncSummary` ohne Pfade oder `null`), `garmin_logout` (löscht den Eintrag im Schlüsselbund). Neue Einstellung in `Settings` und `SettingsPatch`: `garmin_sync_on_start` (Schlüssel `garmin.sync_on_start`, Standard an, nur Wahrheitswert).
- Ein `GarminClient` für die ganze App (eine Drossel, eine Sperre, kein Parallelabruf), echte Adressen, `KeyringStore`, Sperrdatei `<lokaler Datenordner>/Lastwerk/garmin/lockout.json` (dieselbe wie in der Kommandozeile). Der Abgleich läuft auf einem Arbeitsthread über `lastwerk_cli::garmin::{fit_location, sync_into}` (Ort der Ablage siehe `lastwerk-cli`, Entscheidung T23); danach lädt die Oberfläche ihre Ansichten neu. Eine Sperre (`AtomicBool`) verhindert zwei Abgleiche zugleich, der zweite Aufruf endet sofort mit `busy`. `manual` ohne Token endet mit `garmin_not_connected`.
- Abgleich beim Start: Die Oberfläche ruft nach dem Laden `garmin_sync("start")`, der Rust-Teil lässt das je Programmstart höchstens einmal zu und nur, wenn `garmin.sync_on_start` an ist und Token vorliegen, sonst `null`. Ein nicht erreichbarer Schlüsselbund gilt beim Start als "nicht verbunden" (keine Meldung). Abweichung von der Aufgabe: Der Anstoß kommt aus der Oberfläche statt aus `setup`, damit sie das Fortschrittsereignis sicher empfängt (ein Ereignis vor dem `listen` ginge verloren).
- Fehlercodes neu: `busy` und `garmin_<code>` für die Codes von `lastwerk-garmin` (`not_connected`, `auth_failed`, `throttled`, `locked`, `not_available`, `network`, `invalid_response`, `store_failed`, `invalid_input`) sowie `garmin_window_failed`, `garmin_fit_dir_missing` (entfällt seit T28), `garmin_io`. Ein Abbruch durch Garmin ist kein Fehler des Befehls, sondern `aborted { code, until }` in der Zusammenfassung (wie in T22). Die Ereignisse tragen die kurzen Codes (`connected`, `cancelled`, `unexpected_redirect` oder ein Code von `lastwerk-garmin`), die Oberfläche bildet sie auf `ui.garmin.code.*` und `ui.error.garmin_*` ab.
- `get_bridge_status` bleibt als Befehl. Die Oberfläche zeigt den Status von `bridge/` nur noch in den Einstellungen unter "Erweitert" im Abschnitt Garmin und liest ihn erst beim Aufklappen; der Erststart zeigt ihn nicht mehr.
- Tests: `tests/garmin.rs` (Zustand, Sperre mit Ende, Trennen, Einstellung, Abgleich mit Attrappe der Quelle über eine Wurzel, Standardordner, fehlender Ordner, Abbruch, Markertest gegen Token, Anzeigenamen und Pfad in allen DTOs und Ereignissen), `garmin_login::tests` (Navigationsfilter mit feindlichen Adressen), `service::garmin::tests` (Sperre gegen zwei Abgleiche, auch nach Panik; Ausdünnen des Fortschritts), `lib.rs` (Befehlsliste, nur `main` hat eine Capability).

Umsetzung in T26 Teil B (Stand 15. Oktober 2026), Planer mit drei Fragen und Gespräch mit dem Coach:

- Acht neue Befehle, an allen vier Stellen eingetragen (Funktion mit `COMMAND_NAMES` und `generate_handler!`, `build.rs`, `capabilities/default.json`, Methode im `Backend` mit `tauri.ts` und `mock.ts`): `quick_plan(input)`, `coach_status`, `coach_set_model(model)`, `coach_preview(message)`, `coach_chat(message)`, `coach_apply(proposal_id)`, `coach_reset`, `coach_cancel`. Das Frontend bekommt weiterhin weder HTTP noch einen Pfad; alle Aufrufe an das Modell laufen im Rust-Teil (`service/coach.rs`, `service/quick.rs`).
- `quick_plan({ goal: { kind: "event" | "continuous", date?, sport? }, hours_per_week, days, week_start, locale })` prüft die Antworten (Stunden 1 bis 30, mindestens ein Tag, Tage 0 bis 6 ohne Wiederholung, Termin nicht vor `week_start`, Sportart Ausdauer, `week_start` ein Montag), setzt das Ziel (ein gleiches aktives Ziel bleibt mit Bezeichnung und Priorität stehen, ein neues bekommt Priorität A und keine Bezeichnung), schreibt die Verfügbarkeit nach O31, erzeugt den Plan mit `plans::generate` (Sportarten aus der Historie, Kraft 1 je Woche) und aktiviert ihn. Scheitert die Erzeugung, bleiben Ziel und Verfügbarkeit gesetzt.
- `coach_status` liefert `{ available, provider, base_url, model, models[], reason }`. Bei Ollama fragt er `/api/tags` mit 2 Sekunden Zeitlimit (Konvention), bei einem OpenAI-kompatiblen Server (Einstellung `coach.provider = openai_compatible`, mit `coach.base_url` und `coach.model`) nur die Einstellungen. `reason` ist `no_server`, `no_model`, `bad_url` (die Adresse ist nicht Loopback, es wird nie angefragt) oder `not_configured`. Einstellungen `coach.provider`, `coach.base_url` (Standard `http://127.0.0.1:11434`), `coach.model` (leer: das Programm wählt mit `pick_model`; ein gespeichertes Modell gilt nur, wenn der Server es hat). Es gibt kein Feld für einen Zugangsschlüssel, die Schlüssel stehen im Backup und enthalten nichts Geheimes. `coach_set_model` prüft den Namen (nicht leer oder leer für automatisch, höchstens 200 Zeichen, keine Steuerzeichen).
- Fehlercodes neu (mit Texten in beiden Sprachen): `too_little_time`, `busy`, `no_plan`, `empty_message`, `message_too_long`, `coach_unavailable`, `coach_timeout`, `coach_unreachable`, `coach_invalid_output`, `coach_cancelled`, `proposal_stale`.
- Neue Abhängigkeit im Cargo-Projekt der App: `lastwerk-coach` als Pfadabhängigkeit (ein Crate des Workspace, keine neue externe Abhängigkeit). `lastwerk-cli` bekam `planning::generate_with(store, req, availability_override, ...)`; `generate` ruft es mit `None`. Damit rechnet die App einen `planning_change` vor, ohne in die Datenbank zu schreiben.
- Die Attrappe der Oberfläche antwortet aus einer festen Reihenfolge (Text, `plan_edit`, `planning_change`, verworfen) und kennt `?coach=off` für einen Rechner ohne Modell.

Umsetzung in T29 Teil B (Stand 15. Oktober 2026), Einrichtung des lokalen Coachs:

- Vier neue Befehle, an allen vier Stellen eingetragen: `coach_setup_status`, `coach_install_ollama`, `coach_pull_model(name)`, `coach_pull_cancel`. Code in `app/src-tauri/src/service/coach_setup.rs`, Registrierung in `lib.rs`, `build.rs`, `capabilities/default.json`, Methoden im `Backend` (`coachSetupStatus`, `coachInstallOllama`, `coachPullModel`, `coachPullCancel`) mit `tauri.ts` und `mock.ts`. Ereignisse `coach-setup-progress` (`{ phase, value, percent }`, eine erkannte Zeile des Skripts) und `coach-pull-progress` (`{ name, phase, completed, total }`, höchstens vier je Sekunde).
- `coach_setup_status` liefert `{ ollama: running | installed | missing, can_install, models[], model, hardware: { ram_gb, vram_gb | null }, catalog[], recommended, state, busy }`. `catalog` trägt je Modell `name`, `size_gb`, `label_key`, `starter`, `fit` (`fits`, `slow`, `too_big`), `recommended`, `installed`. `state` ist der geprüfte Inhalt von `%LOCALAPPDATA%\Lastwerk\ollama-setup.json` (neben der Datenbank, Vertrag in Task T29), nur bekannte Felder, kein Pfad. `busy` ist ein laufender Auftrag mit Phase und Fortschritt, damit die Oberfläche ihn nach einem Wechsel der Ansicht wiederfindet. `can_install` ist `true` nur unter Windows und mit vorhandenem Skript.
- `coach_install_ollama`: nur Windows (sonst `coach_setup_unsupported`, fehlt das Skript `coach_setup_missing_script`). Das Skript wird über `app.path().resolve("resources/ollama-setup.ps1", BaseDirectory::Resource)` gefunden, ein Präfix `\\?\` wird entfernt. Aufruf `powershell -NoProfile -ExecutionPolicy Bypass -File <skript> -Mode app -StatePath <datei>` ohne Fenster (`CREATE_NO_WINDOW`), ohne `-Model` (das Modell lädt `coach_pull_model` mit Fortschritt). Gelesen werden nur Zeilen `LASTWERK:<phase>:<wert>` mit festen Werten (reiner Parser mit Test), alles andere wird ignoriert; ein unbekannter Fehlercode wird `unknown`. Zeitlimit 30 Minuten (Faustregel), dann wird der Prozess beendet (`coach_setup_timeout`). Exit-Code 0 ist Erfolg, sonst gilt der Code der letzten Fehlerzeile (`coach_setup_<code>`), Exit 2 ohne Zeile `coach_setup_bad_args`, sonst `coach_setup_failed`.
- `coach_pull_model(name)`: erlaubt sind nur Namen aus dem Katalog oder schon installierte, jeweils im Muster `^[a-z0-9][a-z0-9._-]*(:[a-z0-9._-]+)?$` (Positivliste, Test). Danach `POST /api/pull` (`lastwerk-coach::pull_model`) und bei Erfolg `coach.model` (nur bei Anbieter Ollama). Ein zweiter Auftrag, Installation oder Laden, endet mit `coach_setup_busy`. `coach_pull_cancel` wirkt nur auf ein laufendes Laden und beendet es sofort.
- Hardware: Arbeitsspeicher über `sysinfo` (5.3), Dezimal-GB. Grafikspeicher über `nvidia-smi --query-gpu=memory.total --format=csv,noheader,nounits` mit 3 Sekunden Zeitlimit (Faustregel), größte Karte, sonst `null`; andere Hersteller werden nicht erkannt (Faustregel: Ollama beschleunigt unter Windows vor allem mit NVIDIA). Gemessen wird einmal und gemerkt.
- Fehlercodes neu (mit Texten in beiden Sprachen): `coach_setup_busy`, `coach_setup_unsupported`, `coach_setup_missing_script`, `coach_setup_failed`, `coach_setup_timeout`, `coach_setup_no_windows`, `coach_setup_winget_failed`, `coach_setup_download_failed`, `coach_setup_signature_invalid`, `coach_setup_install_failed`, `coach_setup_start_timeout`, `coach_setup_pull_failed`, `coach_setup_bad_args`, `coach_pull_invalid_model`, `coach_pull_cancelled`, `coach_pull_timeout`, `coach_pull_unreachable`, `coach_pull_bad_response`, `coach_pull_model_not_found`, `coach_pull_disk_full`, `coach_pull_network`, `coach_pull_failed`. Der Text einer Fehlermeldung von Ollama, PowerShell oder winget erreicht die Oberfläche nie.
- `lastwerk-coach` bekam `catalog` (`catalog/models.json` eingebettet, `CatalogModel`, `is_valid_model_name`), `recommend` und `pull` (9.2).

Umsetzung in T28 (Stand 15. Oktober 2026), feste Ablage der FIT-Dateien (F1, 5.6, 6.1):

- **Entscheidung vom 15. Oktober 2026:** Der Ordner wird nicht gewählt, die Konfiguration soll so klein wie möglich sein. Mit dem Abgleich in der App (T23) kommen die Dateien ohnehin von Lastwerk selbst. Der Nutzer wählt deshalb keinen FIT-Ordner mehr, nirgends.
- **Ort.** `<lokaler Datenordner>/Lastwerk/fit` (Windows `%LOCALAPPDATA%\Lastwerk\fit`), derselbe Stamm wie die Datenbank. `paths::default_fit_root` (`crates/cli`) gilt für App und Kommandozeile, `LASTWERK_FIT` (nicht leer, ganzer Ordnerpfad) überschreibt ihn wie `LASTWERK_DB` die Datenbank; `LASTWERK_GARMIN_FIT` entfällt. **Warum der Datenordner der App und nicht der Installationsordner:** ein Update ersetzt den Programmordner, die Deinstallation löscht ihn, und unter `Program Files` darf die App nicht schreiben. Unterordner: `garmin/` (Abgleich mit Garmin, `<Kennung>.fit`), `manual/` (von Hand hinzugefügt, `<SHA-256-Präfix>.fit`), alles Weitere (Dateien aus der Übernahme) mit dem relativen Pfad des alten Ordners. Eingelesen wird immer der ganze Stamm, `source_file.rel_path` ist relativ dazu, `rebuild_cache` verliert nichts (6.1).
- **Befehle (29, einer ersetzt).** `choose_folder` entfällt in `build.rs`, `COMMAND_NAMES`, `generate_handler!`, Capability, `Backend`, `tauri.ts`, `mock.ts`. `import_folder` liest immer den festen Stamm (legt ihn an, wenn er fehlt: ein frischer Stamm ist ein Import von null Dateien, kein Fehler). Neu `add_fit_files`: Dateidialog im Rust-Teil (`blocking_pick_files`, Filter `.fit`, Mehrfachauswahl), kopiert nach `fit/manual/<SHA-256-Präfix>.fit` (32 Hexzeichen, der Name entsteht allein aus dem Inhalt, nie aus der Eingabe; Schreiben über `.part` und Umbenennen; gleicher Inhalt zählt als schon vorhanden), Größenlimit und Lesen wie der Import (`lastwerk_fit::read_fit_file`, 64 MiB), danach Import mit demselben Fortschrittsereignis. Antwort `{ cancelled, selected, copied, already_present, rejected, import }`, nie ein Pfad oder Dateiname. Abgelehnt werden Dateien ohne Endung `.fit`, unlesbare, leere und zu große; ob eine Datei eine gültige Aktivität ist, meldet der Import je Datei mit Code.
- **Weggefallen.** Fehlercodes `no_folder` und `garmin_fit_dir_missing`, Messreihenstatus `no_folder`, `GarminSyncSummary.fit_dir_assigned`, `SyncError::FitDirMissing`, `FitLocation.assigned`, `Settings.fit_dir`. Neu: Fehlercode `fit_root_io` (Stamm nicht beschreibbar). `garmin::fit_location(root)` legt den Stamm an und liefert `{ root, download = root/garmin }`.
- **Übernahme beim Start (einmalig, ohne Rückfrage).** `lastwerk_cli::fit_migration::migrate_legacy_fit_dir(store, root)` liest `app.fit_dir`. Ist es gesetzt, kopiert die Funktion alle `*.fit` (rekursiv, Groß- und Kleinschreibung der Endung egal, Verknüpfungen werden nicht verfolgt) mit gleichem relativen Pfad in den Stamm, damit `rel_path` gültig bleibt. Nichts wird gelöscht oder überschrieben: eine gleiche Datei am selben Pfad wird übersprungen (`identical`), eine andere bleibt unberührt (`conflicts`); kopiert wird über `.part`. Danach wird `app.fit_dir` entfernt (`Store::delete_setting`, neu). Ausnahmen: Schlug ein Kopieren fehl (`failed > 0`), bleibt die Einstellung und der nächste Start versucht es erneut; ein fehlender alter Ordner (Laufwerk nicht eingesteckt) wird übersprungen und gemeldet (`missing`), die Einstellung entfällt dann (kein Dauerzustand, der bei jedem Start meldet; die Dateien bleiben dort, der Nutzer reicht sie über "FIT-Dateien hinzufügen" nach). Zeigt die Einstellung auf den Stamm selbst, wird nur sie entfernt. Die App führt das im `setup` aus; `lastwerk garmin sync` führt es ebenfalls aus und meldet die Zahlen im Klartext. Test mit synthetischen Dateien in `crates/cli/tests/fit_migration.rs`.
- **Meldung an die Oberfläche.** Die Zahlen stehen in `Settings.fit_migration` (`copied`, `identical`, `conflicts`, `failed`, `missing`, sonst `null`), ebenso `Settings.fit_root` (der Pfad nur zur Anzeige, die Oberfläche kann ihn nicht ändern). Gewählt wegen Einfachheit und Robustheit: kein Ereignis, das vor dem `listen` verloren gehen kann, und kein Zustand in der Datenbank. Der Wert bleibt im Prozess stehen, bis die App endet; die Oberfläche zeigt beim Laden eine ausblendbare Notiz (`ui.migration.*`). Wer die App vor dem Lesen schließt, verpasst nur die Meldung, nicht die Dateien.
- **Oberfläche.** Der Erststart hat vier statt fünf Schritte (kein Schritt "Ordner"). Die Einstellungen zeigen den Abschnitt "Daten" (Baustein `Section`): Text, wo Lastwerk ablegt, den Pfad als Anzeige, die Knöpfe "Neu einlesen" und "FIT-Dateien hinzufügen". Texte `ui.settings.*` und `ui.migration.*`, entfallene Schlüssel sind entfernt.
- **Abweichung von der Aufgabe.** Die Aufgabe nannte eine Meldung "einmal"; die Notiz bleibt bis zum Ausblenden oder Programmende stehen (siehe oben). Neu im Speicher: `Store::delete_setting` (die Aufgabe nannte `crates/store` nicht, ohne Löschen ließe sich `app.fit_dir` nicht entfernen).
- Tests: `crates/cli/tests/fit_migration.rs` (sieben Fälle: nichts zu tun, Kopieren mit relativen Pfaden, Überspringen und nie Überschreiben, fehlender Ordner, Stamm im oder gleich dem alten Ordner, Verknüpfungen, Import des Stamms danach gleicht dem alten), `app/src-tauri/tests/fit_files.rs` (Name aus Hash, Ablehnung, feindliche Namen, blockierter Stamm, nur Zahlen im Ergebnis), `paths.rs` (`LASTWERK_FIT`), Vitest `DataSection.test.ts`, Playwright (Neu einlesen, Dateien hinzufügen, Übernahme).

Umsetzung in T39 (Stand 16. Oktober 2026): zwei neue Befehle, `get_health { today, days }` (Seite "Gesundheit": Tages- und Trainingswerte je Tag für 30, 90 oder 365 Tage, Hydration nur bei Freigabe) und `delete_sensitive_data` (löscht `sensitive_day`), an allen Stellen eingetragen (46 Befehle). `Settings` hat `garmin_share_hydration` (Patch ebenso) und `athlete.sex_origin`, `athlete.birth_year_origin`; `LoadInfo.missing` nennt den fehlenden Schwellenwert; `ActivityDetail.garmin_load` zeigt Garmins Last und Trainingseffekt für Dateien `garmin/<Kennung>.fit`, nur zur Anzeige (E2); `GarminSyncSummary` meldet geänderte Schwellen und Neuberechnungen. Der Erststart hat nur noch die Schritte Sprache und Garmin (Abweichung von T19 und T29, Entscheidung des Nutzers vom 16. Oktober 2026: nach der Anmeldung keine Fragen nach Schwellen; Ziel und Coach bleiben in Planer und Einstellungen). **Abgelöst durch T41** (siehe unten).

Umsetzung in T41 (Stand 16. Oktober 2026, Entscheidung E6 des Nutzers: "Einrichtung neu denken, effizient zum fertig eingerichteten Tool"): Die Einrichtung hat drei Schritte und endet mit einem aktiven Plan.

1. **Garmin.** Anmeldung im Fenster von Garmin wie bisher (kein Feld für Zugangsdaten). Die Sprache übernimmt der Assistent beim ersten Start aus dem System (`navigator.language`, `de` oder `en`, sonst `en`) und speichert sie; oben steht eine Auswahl. Ein eigener Sprachschritt entfällt. Ohne Verbindung heißt der Knopf "Ohne Garmin weiter" und nennt, was dann fehlt.
2. **Während Garmin lädt.** Fortschritt des ersten Abgleichs, mit Verbindung die Freigaben der S3-Bereiche (alle aus, Baustein `SensitiveShares` wie in den Einstellungen, ohne Löschen) und der lokale Coach zum Aufklappen (Baustein `CoachSetup`, eingerichtet wird nur auf Klick, 9.2).
3. **Plan.** Die drei Fragen (`QuickPlanner`), vorbelegt aus der Historie: `get_planning_inputs` liefert seit T41 `history` (Hauptsportart nach Zeit, Median der Wochenstunden über Wochen mit Training, Wochentage mit Training in mindestens der Hälfte dieser Wochen, aus den zwölf vollen Wochen vor dieser; nur ab vier Wochen mit Training; Faustregeln in `service::plans::history_defaults`). Eine gespeicherte Verfügbarkeit und die Sportart eines aktiven Ziels gehen vor. Kommt die Historie erst mit dem Abgleich, baut sich das Formular einmal neu auf. "Plan erstellen" erzeugt und aktiviert den Plan und schließt die Einrichtung ab, "Später planen" schließt ohne Plan ab.

Danach zeigt Heute für einen Plan, der erst am Montag beginnt, den Starttag statt "deckt heute nicht ab", und die leeren Abschnitte Form und Bereitschaft sagen während eines Abgleichs, dass die Werte unterwegs sind. Keine neue Abhängigkeit, kein neuer Befehl. Nicht geändert: Die Freigabe des Gesprächs mit dem Coach gilt weiter je Sitzung (9.7, Punkt 4).

#### lastwerk-mcp

MCP-Server auf Basis von `rmcp`. Transport nur stdio (kein HTTP in Version 1). Die Datenbank wird schreibgeschützt geöffnet (`SQLITE_OPEN_READ_ONLY`). Der Server ist in den Einstellungen standardmäßig ausgeschaltet.

Ausgaben sind eigene Typen mit fester Feldliste. Datenbankzeilen werden nie direkt serialisiert.

```rust
pub struct ActivityView {
    pub id: i64,                    // interne Nummer, nicht die Garmin-Kennung
    pub local_date: NaiveDate,      // nur Datum, keine Uhrzeit (offen O20)
    pub sport: Sport,
    pub duration_min: u32,
    pub load: Option<f32>,
    pub load_method: Option<LoadMethod>,
    pub intensity_factor: Option<f32>,
    pub zone_minutes: Vec<ZoneMinutes>,
    pub best_power_w: Vec<BestPower>,   // nur in get_activity
}

pub struct FitnessView { pub date: NaiveDate, pub ctl: f32, pub atl: f32, pub tsb: f32 }
pub struct WeeklyLoadView { pub week_start: NaiveDate, pub sport: Sport, pub load: f32, pub duration_min: u32 }
```

Werkzeuge (alle mit `readOnlyHint = true`, `openWorldHint = false`):

| Werkzeug | Parameter | Ausgabe | Begrenzung |
|---|---|---|---|
| get_fitness_status | from, to | Liste `FitnessView` | höchstens 366 Tage |
| get_weekly_load | weeks, sport | Liste `WeeklyLoadView` | höchstens 52 Wochen |
| list_activities | from, to, sport | Liste `ActivityView` ohne `best_power_w` | höchstens 200 Zeilen |
| get_activity | id | `ActivityView` | eine Aktivität |
| get_current_plan | keine | Plan nach `plan.schema.json` | aktiver Plan |
| get_plan_week | week_start | eine Planwoche | eine Woche |
| get_thresholds | on_date | Schwellenwerte mit Gültigkeitsdatum | ein Datum |
| explain_rule | code | Regelparameter, Quelle oder "Faustregel" | eine Regel |
| get_recovery_days | from, to | HRV, Ruhepuls, Schlafdauer je Tag | nur wenn freigegeben (Standard aus), höchstens 90 Tage |

Nicht enthalten: Aktivitätsnamen, Notizen, Koordinaten, Streckenpunkte, Gerätekennungen, Dateipfade, Plan-Freitexte (Zielbezeichnung), Strava-Daten. Es gibt kein Werkzeug, das schreibt, löscht oder Dateien liest.

Jeder Aufruf wird lokal protokolliert (Werkzeug, Parameter, Zeilenzahl, ohne Nutzdaten). Das Protokoll ist in der Oberfläche einsehbar.

Umsetzung in T10 (Stand 13. Oktober 2026), mit den Abweichungen von der Skizze:

- Aufbau: `crates/mcp` ist Bibliothek (`lib.rs`) und Programm (`main.rs`), damit Tests die Werkzeuge direkt aufrufen. `views.rs` (Ausgabetypen), `args.rs` (Argumente), `tools.rs` (die neun Werkzeuge als Funktionen über einer lesenden `Store`-Verbindung), `server.rs` (Anbindung an `rmcp`), `calllog.rs`, `rules.rs` (`explain_rule`), `startup.rs`. Nur stdio, kein HTTP. Der Server meldet nur die Fähigkeit `tools` (keine Ressourcen, keine Eingabeaufforderungen).
- Aufruf: `lastwerk-mcp --db <datei> [--log <datei>] [--list-tools]`. `--db` ist Pflicht (Exit 2 ohne), es gibt bewusst keinen Standardpfad und kein `LASTWERK_DB`: Ein Client soll nie unbemerkt eine andere Datenbank lesen als die, die der Nutzer in die Konfiguration geschrieben hat. Der übliche Pfad steht im README. Exit-Codes: 0, 2 (Argumente, fehlende oder unbrauchbare Datenbank, nicht schreibbares Protokoll), 3 (Fehler des Transports). `--list-tools` gibt Name, Beschreibung und Hinweise je Werkzeug als JSON aus und endet mit 0, ohne die Datenbank zu öffnen. Meldungen nennen keinen Pfad.
- Hinweise: jedes Werkzeug trägt `readOnlyHint = true`, `destructiveHint = false`, `idempotentHint = true` und `openWorldHint = false`. Ein Test hält die Werkzeugliste gegen die Tabelle oben fest.
- Antworten sind Objekte, weil der strukturierte Inhalt einer MCP-Antwort ein Objekt sein muss: `{ "days": [FitnessView] }`, `{ "weeks": [WeeklyLoadView] }`, `{ "activities": [ActivityView], "truncated": bool }` (mehr als 200 Treffer: die ältesten 200 und `truncated = true`), `{ "on_date", "sports": [{ "sport", "values": [{ "kind", "value", "unit", "valid_from" }] }] }`, `{ "days": [RecoveryDayView] }`. `get_activity` liefert eine `ActivityView`, `get_current_plan` und `get_plan_week` den Plan beziehungsweise die Woche als JSON nach `plan.schema.json`.
- Zahlen sind `f64` und auf eine Nachkommastelle gerundet (Intensitätsfaktor auf zwei), statt `f32`: Ein `f32` erschiene nach `serde_json::Value` als `66.69999694824219`. `best_power_w` ist `Option` und fehlt in `list_activities`. `ZoneMinutes` trägt `scheme`, `zone`, `minutes`, `estimated`, `BestPower` trägt `duration_s` und `watts`. Zonenschemas außerhalb des Zonenkatalogs und Bestwerte mit anderen Dauern als 5, 60, 300, 1200 und 3600 Sekunden fallen weg: Kein Text aus der Datenbank wird ungeprüft weitergegeben.
- `sport` ist bei `get_weekly_load` und `list_activities` optional (ohne Angabe alle Sportarten). `get_weekly_load` zählt die letzten `weeks` Kalenderwochen bis einschließlich der laufenden (lokales Datum von heute), nur primäre Aktivitäten, Wochen ohne Eintrag fehlen. `duration_min` ist die Zeit aller primären Aktivitäten, `load` die Summe der berechneten Lasten (wie im Verlauf); eine Sportart ohne berechnete Last in der Woche (Kraft aus FIT) steht mit `load` 0 und ihrer Zeit (Stand 14. Oktober 2026, vorher fehlten solche Aktivitäten ganz). `list_activities` führt nur primäre Aktivitäten (Duplikate nicht, 6.4). Eine Last, die nicht berechenbar ist, steht als `null`, nie als 0.
- `get_fitness_status` rechnet wie `lastwerk pmc`: Der Verlauf beginnt am ersten Tag mit zählender Last, Zeitkonstanten und Startwerte kommen aus den Einstellungen `pmc.*` (Standard 42 und 7 Tage, Start 0). Ausgegeben werden nur Tage von `from` an, die nicht vor dem ersten Tag mit Last und nicht nach heute liegen (kein Wert ohne Grundlage, keine Vorhersage). Die Spanne ist höchstens 366 Tage, beide Enden eingeschlossen.
- `get_thresholds(on_date)` nennt je Ausdauersportart (Rad, Laufen, Schwimmen) die Werte, die `threshold_lookup` am Tag wählt, mit Art, Wert, Einheit und dem Tag, ab dem der Wert gilt. Ohne Notiz, Herkunft und Geschlecht.
- `explain_rule(code)` nimmt `R01` bis `R12` (Muster `^R[0-9]{2}$`) und liefert `title` und `explanation` aus `catalog/locales` (Sprache der Einstellung `app.locale`, `de` oder sonst `en`), `hardness`, `source`, `references` und die Zahlenparameter aus `catalog/rules.default.json`. Das Feld heißt `title` und nicht `name`, damit die Prüfung auf ein Feld `name` (Aktivitätsname) eindeutig bleibt. R12 kam mit T14 dazu (Nachtrag 14. Oktober 2026).
- `get_current_plan` und `get_plan_week` lesen den Plan mit Status `active`. Der Plan wird vor der Ausgabe bereinigt: `explanation` entfällt, `created_at` verliert die Uhrzeit (O20, es bleibt der Tag mit 00:00:00 UTC), und das Ergebnis wird gegen `plan.schema.json` geprüft (Formatprüfung an). Alle Zeichenketten des Schemas sind durch Aufzählung, Muster oder Format begrenzt, bis auf `explanation`. Ein gespeicherter Plan, der das Schema verletzt (zum Beispiel Text in `reason_keys`), wird nicht gezeigt. `get_plan_week` wählt die Woche, in der das Datum liegt (jeder Tag der Woche genügt). Ohne aktiven Plan, oder mit einer Woche außerhalb des Plans, ist die Antwort ein Fehler. Aktiv setzen lässt sich ein Plan in der Oberfläche und mit `lastwerk plan-status` (Nachtrag vom 14. Oktober 2026 bei `lastwerk-cli`).
- `get_recovery_days` ist gesperrt, solange die Einstellung `mcp.recovery_enabled` nicht genau `true` ist (O19, Standard aus, andere Werte wie `TRUE` oder `1` geben nicht frei). Ohne Freigabe ist die Antwort ein Fehler. Mit Freigabe liefert es je Tag `date`, `hrv_overnight_ms`, `resting_hr_bpm` und `sleep_duration_min` (ohne Schlafwert), höchstens 90 Tage, und eine leere Liste, solange die Tabelle fehlt. Die Oberfläche hat seit dem 14. Oktober 2026 einen Schalter dafür (Abschnitt "Claude als Client (MCP-Server)" der Einstellungen, siehe lastwerk-app).
- Argumente: nur Daten (`YYYY-MM-DD`), ganze Zahlen, die Sportart als Aufzählung und der Regelcode. Unbekannte Felder werden abgelehnt (`additionalProperties: false`). Bereichsgrenzen stehen im Schema (`minimum`, `maximum`, `pattern`) und werden unabhängig davon in `tools.rs` geprüft. Fehler (`invalid parameters`, `not available`, `not found`) kommen als Werkzeugfehler (`isError`) mit festen Sätzen, nie mit dem Text eines Datenbankfehlers.
- Aufrufprotokoll: `mcp-calls.log` im Ordner der Datenbank, anderer Ort mit `--log`. Eine Zeile JSON je Aufruf: `ts` (UTC, Sekunden), `tool`, `params`, `rows`, `duration_ms`, `status` (`ok`, `invalid_params`, `disabled`, `not_found`, `internal`). Aufrufe, deren Argumente schon beim Einlesen scheitern (Typ, unbekanntes Feld), beantwortet `rmcp` vor der Werkzeugmethode und sie stehen nicht im Protokoll. Der Server startet nicht, wenn sich das Protokoll nicht zum Anhängen öffnen lässt. Die Anzeige des Protokolls in der Oberfläche ist noch nicht umgesetzt.
- "In den Einstellungen standardmäßig ausgeschaltet" gilt vorläufig so: Der Server läuft nur, wenn ein Client ihn startet, und der Nutzer trägt ihn selbst in dessen Konfiguration ein (README). Ein Schalter für den Server selbst und die Bestätigung aus 9.6 Punkt 1 für den Server fehlen noch; für die Erholungswerte gibt es seit dem 14. Oktober 2026 einen Schalter mit Bestätigung.
- Die Datenbank hat keine Koordinaten: `activity` speichert weder Streckenpunkte noch Orte. Der Markertest legt Koordinaten deshalb als Text in Namen, Notizen und Zielbezeichnung ab und prüft, dass sie nirgends erscheinen.

#### Austausch mit dem Zusatzprogramm

Seit T20 eingefroren: Der Austausch gilt nur noch für Nutzer, die `bridge/` weiter selbst betreiben. Die App holt Garmin-Daten über `lastwerk-garmin` (5.6).

Dateibasiert über einen Austauschordner `exchange/`, Standardort `local/exchange/`. Lastwerk ruft das Zusatzprogramm nicht auf und lädt keinen Code daraus. Der Quellordner des Zusatzprogramms heißt `bridge/` und ist davon getrennt.

| Richtung | Pfad | Inhalt |
|---|---|---|
| Zusatzprogramm nach Lastwerk | `exchange/out/activities/*.fit` | Heruntergeladene Aktivitäten |
| Zusatzprogramm nach Lastwerk | `exchange/out/recovery/YYYY-MM-DD.json` | Schema `recovery-day`, Abschnitt 7.3 |
| Zusatzprogramm nach Lastwerk | `exchange/out/push-report.json` | Ergebnis einer Workout-Übertragung (`push --yes`), je Einheit Zustand und Workout-Kennung, keine Nutzlast, keine Zugangsdaten |
| Zusatzprogramm nach Lastwerk | `exchange/out/push-dry-run.json` | Ausgabe des Trockenlaufs von `push` (nur mit `--out`), enthält die Nutzlasten |
| Lastwerk nach Zusatzprogramm | `exchange/in/plan.json` | Plan nach `plan.schema.json` |

Lastwerk behandelt alles unter `exchange/out/` als nicht vertrauenswürdige Eingabe (Größenlimits, Schema-Prüfung, keine Pfade aus dem Inhalt).

Umsetzung in T16 (`push`): Das Zusatzprogramm behandelt `exchange/in/plan.json` ebenfalls als nicht vertrauenswürdig und prüft es vor jeder Nutzung gegen `schema/plan.schema.json` (Größenlimit 1 MiB, `schema_version` 1, ungültig ist Exit-Code 2). Die Vorlagen und Texte liest `push` als Dateien aus `catalog/` (Option `--templates`, Standard `<Repository>/catalog/templates.json` relativ zur Paketdatei; Texte aus `locales/<locale>.json` neben dem Katalog, Option `--locales`). Das ist ein Dateizugriff auf Daten, die weder zu `crates/` noch zu `app/` gehören, und verletzt die Isolation nicht (`scripts/check-isolation.sh`). Die Schema-Prüfung ist die eigene kleine Umsetzung aus `manifest.py`, erweitert um die Schlüsselwörter des Plan-Schemas, ohne neue Abhängigkeit (Begründung in T16).

### 5.5 Archiv des Zusatzprogramms (Vollabzug)

Das Zusatzprogramm `lastwerk-bridge` (Ordner `bridge/`) holt alle lesbaren Garmin-Daten in ein Archiv mit unveränderten Antworten. `exchange/out` und die Lastwerk-Datenbank werden daraus abgeleitet und sind neu aufbaubar. Das Archiv liegt unter `local/archive/garmin/` im Projektordner, ist nicht versioniert und nicht in einem Cloud-Sync-Ordner. Lastwerk liest es nie direkt. Der Garmin-Datenexport (ZIP aus dem Konto) ist keine Quelle, das Werkzeug funktioniert ohne ihn.

#### Schnittstelle der Bibliothek (garminconnect 0.3.17, Q27)

| Eigenschaft | Stand |
|---|---|
| Methoden | README nennt 154+ in 14 Kategorien, rund 100 davon lesend |
| Freie Methoden | `connectapi(path)` (GET), `connectwebproxy(path)`, `download(path)`, `query_garmin_graphql(query)` für Endpunkte ohne Wrapper. Pfade mit `..`, `?`, `#`, `\` werden abgelehnt |
| Anmeldung | Mobiler SSO-Login wie die Android-App, MFA über Rückruf, DI-OAuth-Tokens in `garmin_tokens.json`, Erneuerung 900 s vor Ablauf |
| Drosselung | Keine. 429 und 403 führen nur zur nächsten Anmeldestrategie oder zum nächsten TLS-Profil, kein exponentielles Backoff |
| Zeitlimits | API 15 s, Login 30 s |
| Abhängigkeiten | `curl_cffi>=0.15.0`, `requests>=2.33.0`, `ua-generator>=1.0`, Python 3.12 oder neuer |
| Bruch in 0.3.17 | `get_goals` zählt ab 1, `start=0` löst `ValueError` aus |

Drosselung, Fortsetzung, Zeitlimit je Aufruf und Abbruchregeln sind Teil des Zusatzprogramms, nicht der Bibliothek.

#### Datenarten und Sensibilitätsklassen

| Klasse | Inhalt | Umgang |
|---|---|---|
| S1 Training | Aktivitätsliste, je Aktivität Zusammenfassung, Details, Splits, typisierte Splits, Split-Zusammenfassungen, Wetter, Zeit in Herzfrequenz- und Leistungszonen, Kraftsätze, Ausrüstung, FIT-Original. Zonen, Schwellenwerte, FTP, Laktatschwelle, Ziele, Pläne, Workouts, Ausrüstung, Geräte | Standardordner |
| S2 Gesundheit | Tageswerte (Schritte, Etagen, Herzfrequenz, Ruhepuls, Stress, Body Battery, Atmung, SpO2, Intensitätsminuten, Schlaf, HRV, Trainingsbereitschaft, Trainingsstatus, Fitnessalter), Bereichs- und Verlaufswerte (Ausdauerwert, Bergwert, Lauftoleranz, Rennprognosen, VO2max) | Standardordner, Erholungswerte für MCP gesperrt |
| S3 gesperrt abgelegt | Gewicht und Körperzusammensetzung, Blutdruck, Hydration, Ernährung, Lifestyle-Logging | Eigener Ordner `sensitive/`. Von `normalize` und `lastwerk-mcp` nicht gelesen, bis die Kategorie in den Einstellungen freigegeben ist. In der App (T39): ohne Freigabe kein Abruf, nach Freigabe eigene Tabelle `sensitive_day` (6.2) |
| Nie abgerufen | Menstruations- und Schwangerschaftsdaten | Kein Wrapper wird aufgerufen, die Allowlist enthält keinen Eintrag |

#### Archivlayout

```
<archive_root>/garmin/
  manifest.jsonl                 wird nur angehängt
  state.json                     Zeiger je Quelle (letztes Datum, Seitenstand)
  coverage.json                  Abdeckungsbericht von `coverage` (abgeleitet)
  registry.yaml                  Endpunkt-Register (Kopie des Standes beim Lauf)
  account/                       profile.json, settings.json, devices.json, zones.json, ftp.json
  activities/list/               page-0000.json, page-0001.json, ...
  activities/<yyyy>/<id>/        summary.json, details.json, splits.json, typed_splits.json,
                                 split_summaries.json, weather.json, hr_zones.json,
                                 power_zones.json, exercise_sets.json, gear.json,
                                 original.zip (unverändert), original.fit (daraus entpackt)
  daily/<yyyy>/<yyyy-mm-dd>/     sleep.json, hrv.json, stress.json, body_battery.json, ...
  ranges/<endpoint>/             <start>_<end>.json
  sensitive/<kategorie>/         Gewicht, Blutdruck, Hydration, Ernährung, Lifestyle
  device_usb/<geraet>/           Kopien der Geräteordner (optional, Ordner Activity, Monitor, Sleep, siehe M4c)
```

`state.json`, `coverage.json` und `registry.yaml` sind abgeleitet und werden bei jedem Lauf neu geschrieben, sie haben keinen Manifest-Eintrag (ebenso `device-import-report.json`, `reconcile-report.json` und `normalize-report.json`). `state.json` ist eine Momentaufnahme der Zeiger je Quelle (Listenseiten, ältestes und jüngstes Datum der Tageswerte, Enden je Bereichsendpunkt, letzter Lauf). Maßgeblich für die Fortsetzung bleibt das Manifest: der Plan eines Laufs wird aus Register und Manifest berechnet.

`download_activity` mit `ORIGINAL` liefert laut Bibliothek ein ZIP (Festlegung T12). Die Bytes bleiben unverändert als `original.zip` (Endpunkt `original`). Daraus wird die erste `.fit`-Datei als `original.fit` entpackt (Endpunkt `original_fit`, eigener Manifest-Eintrag mit SHA-256). Pfade im ZIP werden ignoriert, die entpackte Größe ist auf 256 MiB begrenzt (Faustregel). Schlägt das Entpacken fehl, steht für `original_fit` der Status `error` mit `error_class` (`BadZipFile`, `NoFitInZip`, `FitTooLarge`, `EmptyFit`); das ZIP bleibt erhalten, ein erneuter Versuch folgt erst bei einem neueren ZIP. Eine Aktivität gilt als „mit `original.fit`“, wenn `original_fit` den Status `ok` hat.

Dateien werden atomar geschrieben (temporäre Datei, dann Umbenennen). Eine geschriebene Datei wird nie überschrieben. Eine Neuabfrage erzeugt eine neue Datei mit Zeitstempel im Namen und einen neuen Manifest-Eintrag. Der jüngste Eintrag je Schlüssel gilt.

#### Manifest (`schema/archive-manifest.schema.json`)

Ein JSON-Objekt je Zeile in `manifest.jsonl`:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "archive-manifest.schema.json",
  "type": "object",
  "additionalProperties": false,
  "required": ["kind", "endpoint", "status", "fetched_at", "lib_version"],
  "properties": {
    "kind": { "enum": ["account", "activity", "daily", "range", "device_usb"] },
    "endpoint": { "type": "string", "maxLength": 128 },
    "params": { "type": "object" },
    "status": { "enum": ["ok", "empty", "not_available", "error"] },
    "http_status": { "type": "integer" },
    "fetched_at": { "type": "string", "format": "date-time" },
    "lib_version": { "type": "string", "maxLength": 32 },
    "path": { "type": "string", "maxLength": 512 },
    "bytes": { "type": "integer", "minimum": 0 },
    "sha256": { "type": "string", "pattern": "^[0-9a-f]{64}$" },
    "error_class": { "type": "string", "maxLength": 64 }
  }
}
```

Leere und nicht verfügbare Antworten werden vermerkt und nicht erneut abgefragt. Ausnahme: Die letzten N Tage (Einstellung, Standard 7, Faustregel) holt `sync` immer neu, weil Garmin Werte nachträglich finalisiert.

#### Endpunkt-Register (`registry.yaml`)

Je Eintrag: `name`, `method`, `granularity` (`once`, `day`, `range`, `page`, `per_activity`), `class` (`S1`, `S2`, `S3`), `params`, `max_span_days` (in der Sonde gemessen), `consumer` (Lastwerk ja oder nein). `coverage` zeigt je Eintrag, ob das Konto Daten geliefert hat, und die älteste und jüngste Datumsangabe.

#### Abrufregeln

Festlegungen aus M4b (T12), soweit sie die Regeln unten konkretisieren:

- **Plan:** `backfill` und `sync` berechnen den Plan bei jedem Lauf aus Register und Manifest. Ein Schlüssel mit `ok`, `empty` oder `not_available` kommt nicht mehr vor (Fortsetzung), ein Schlüssel ohne Eintrag oder mit `error` schon. `--dry-run` berechnet denselben Plan, gibt die Anzahl je Endpunkt aus und sendet nichts (kein Client, keine Anmeldung, keine Datei).
- **`backfill`:** Stufen in der Reihenfolge der Regel 1. Die Aktivitätsliste wird seitenweise (100 je Seite, Konvention) bis zur kurzen Seite geholt, je Aktivität neueste zuerst der Detailsatz samt `original`. Tageswerte laufen von gestern bis zum frühesten Datum des Kontos (das ältere von Profildatum und ältester Aktivität, durch `--until` nach oben begrenzt), alle Tagesendpunkte je Tag, bevor der nächste ältere Tag folgt. Bereichsendpunkte laufen danach in Spannen von neu nach alt. Das Auffrischen der letzten Tage gehört nicht zu `backfill`.
- **Bereichsspannen:** `max_span_days` je Registereintrag. Solange die Sonde nichts gemessen hat (Wert `null`), gilt die Konvention 28 Tage. Spannen liegen auf einem festen Raster (ab 2000-01-01), damit jeder Lauf dieselben Schlüssel bildet. Die letzte Spanne am neuen Ende wird bei gestern gekürzt. Ändert sich `max_span_days`, ändert sich das Raster, bereits geholte Dateien bleiben bestehen und die Spannen werden neu geholt.
- **`sync`:** Aktivitätsliste ab Seite 0, immer neu, bis eine Seite eine bereits bekannte Aktivität enthält. Je neue Aktivität (und je Aktivität der letzten N Tage, falls ein Schlüssel fehlt oder fehlerhaft ist) der Detailsatz. Tageswerte der letzten N Tage und heute werden immer neu geholt. Zusätzlich gilt ein Tageswert als vorläufig, wenn er höchstens N Tage nach dem Tag selbst geholt wurde, und wird beim nächsten `sync` einmal erneuert. Bereichsendpunkte werden ab N Tage vor ihrem bisher jüngsten Ende bis heute neu geholt. Kontodaten (Zonen, FTP, Geräte) werden alle N Tage neu geholt, weil sie sich ändern. Aktivitäten selbst werden nicht aufgefrischt.
- **Aufrufbudget:** `--max-calls N` zählt alle Aufrufe an die Bibliothek einschließlich Wiederholungen. Der Lauf endet nach höchstens N Aufrufen mit Zusammenfassung (Aufrufe, neue Dateien, leere, nicht verfügbare, Fehler, noch offen) und Exit-Code 0. Fehler einzelner Abrufe ergeben Exit-Code 1, drei Antworten 429 oder 403 in Folge Exit-Code 3.
- **Sperrliste:** Menstruations- und Schwangerschaftsmethoden stehen nicht im Register und damit nie im Plan. Die Allowlist im Aufrufer lehnt sie zusätzlich ab.

1. **Reihenfolge:** Konto und Zonen, Aktivitätsliste, je Aktivität Detailsatz und `original.fit`, Tageswerte von neu nach alt, Bereichsabfragen, danach optional die USB-Kopie.
2. **Bereichsendpunkte** bevorzugen, wo sie dieselben Werte liefern wie Tagesaufrufe. Das senkt die Aufrufzahl. Ohne Bereiche sind für fünf Jahre rund 27.000 bis 37.000 Aufrufe zu erwarten (Schätzung: 15 bis 20 Tagesendpunkte mal 365 Tage mal 5 Jahre).
3. **FIT-Original statt Details** für Streams. `get_activity_details` ist auf `maxchart=2000` und `maxpoly=4000` begrenzt.
4. **Drosselung:** Mindestabstand mit Zufallsanteil, Startwert aus der Sonde (offen O24). Kein Parallelabruf. Bei drei aufeinanderfolgenden 429 oder 403 stoppt der Lauf, schreibt den Zustand und endet mit klarer Meldung. Kein Neuversuch der Anmeldung in einer Schleife.
5. **Keine Umgehung:** kein IP-Wechsel, keine Umgehung von Challenges. Bei Sperre bleibt der Rückfall auf die USB-Kopie der Geräteordner (und der FIT-Ordner, den der Kern ohnehin liest) und ein späterer Versuch.
6. **Anmeldung** einmalig und interaktiv mit MFA, erst in M4a. Token liegen im Benutzerprofil (`~/.garminconnect`, also außerhalb des Projektordners) mit eingeschränkten Rechten, nie im Repository. Zugangsdaten stehen in keiner Datei.
7. **Nur lesen:** Eine ausdrückliche Allowlist von Methodennamen (kein Muster). Ein Test vergleicht sie mit allen öffentlichen Methoden der installierten Bibliothek. Jede Methode muss in der Allowlist oder in einer Sperrliste stehen. Neue Methoden nach einer Aktualisierung lassen den Test fehlschlagen, bis sie eingestuft sind. Einzige Ausnahme ist der Schreibpfad von `push --yes` (M6): eine eigene, ebenfalls ausdrückliche Freigabe `WORKOUT_WRITE_ALLOWLIST` mit genau `upload_workout` und `schedule_workout`. Beide bleiben in der Sperrliste und fehlen in der Allowlist, der lesende Aufrufer lehnt sie weiter ab.
8. **Zeitlimit je Aufruf** und Gesamtlimit je Lauf (`--max-calls`). Jeder Lauf endet mit einer Zusammenfassung (Aufrufe, neue Dateien, leere, Fehler).
9. **Lesender curl_cffi-Sitzungsmodus:** Schalter, der die API-Sitzung der Bibliothek für lesende Aufrufe durch eine `curl_cffi`-Sitzung ersetzt (Workaround aus Issue #444). Standard aus, da er in Bibliotheksinterna eingreift. Ein Test prüft, dass der Schalter beim Bibliotheksupdate noch greift.

#### Befehle des Zusatzprogramms

```
lastwerk-bridge auth                          einmalig, interaktiv, MFA, legt Token an
lastwerk-bridge auth --check                  prüft das Token mit genau einem Aufruf
lastwerk-bridge probe                         Sonde: je Datenart ein Aufruf, Bericht ohne Werte
lastwerk-bridge backfill --root R [--until D] [--max-calls N] [--dry-run]
lastwerk-bridge sync --root R [--refresh-days 7]
lastwerk-bridge coverage --root R
lastwerk-bridge import-device ORDNER --root R   optional, USB-Kopie
lastwerk-bridge reconcile --root R            Abgleich API gegen USB-Kopie
lastwerk-bridge normalize --root R --out exchange/out [--allow-sensitive KATEGORIE]
lastwerk-bridge push --plan P [--dry-run]     M6, Standard: Nutzlast je Einheit als JSON, sendet nichts
lastwerk-bridge push --plan P --yes --out exchange/out --max-workouts N   M6, echter Versand (O18 vorher klären)
```

Festlegungen aus M6 (T16), soweit sie die Regeln oben konkretisieren:

- **Trockenlauf ist der Standard.** `push --plan P` (mit oder ohne `--dry-run`) prüft den Plan gegen `schema/plan.schema.json` (ungültig: Exit-Code 2), berechnet je Einheit ohne Ruhetage die Nutzlast eines Garmin-Connect-Workouts aus der Vorlage und gibt ein JSON-Dokument auf der Standardausgabe aus. Es entsteht kein Client, es braucht keine Anmeldung, es gibt keine Netzwerkanfrage (Test: gesperrter Socket, Bibliothek nicht importierbar). Mit `--out` entsteht zusätzlich `push-dry-run.json`. Auswahl mit `--from`, `--to`, `--max-workouts`; Termine vor heute sind im Trockenlauf mit `in_past` gekennzeichnet.
- **Nutzlast.** Titel aus `template.<id>.name` der Plan-Sprache (`catalog/locales`), `sportType` (cycling 2, running 1, swimming 4, strength_training 5), ein Segment mit `workoutSteps`: `ExecutableStepDTO` (`warmup`, `interval`, `recovery`, `cooldown`) mit Endbedingung Zeit in Sekunden, Ziel `power.zone` (Rad), `pace.zone` (Lauf, Schwimmen) oder `heart.rate.zone` (Herzfrequenzschemata) mit `zoneNumber`, Wiederholungen als `RepeatGroupDTO` mit `numberOfIterations`. Der Schritt `steady` der Vorlage wird ein `interval`-Schritt. Kraft ist ein Schritt mit Zeitvorgabe ohne Ziel, der RPE (Einheit, sonst Vorlage) steht im Titel. Dauer wie im Planer (7.4): Intervallblöcke bleiben, die übrigen Schritte werden auf ganze Minuten gedehnt, die Summe ist genau `duration_min`. Eine Wiederholung ist `RepeatGroupDTO` über Arbeit und Pause für Wiederholungen minus eins, danach die letzte Arbeit ohne Pause (der Planer zählt nur die Pausen zwischen den Wiederholungen); bei zwei Wiederholungen stehen die Schritte ohne Gruppe.
- **Die Struktur ist bis zum ersten echten Versand eine Annahme** (analog O4). Quelle: Quelltext von `garminconnect/workout.py`, `upload_workout` und `schedule_workout` in 0.3.17 (Q21, Q27, gelesen am 14. Oktober 2026), keine aufgezeichneten Antworten. Ungeprüft sind insbesondere: ob Garmin bei `pace.zone` eine `zoneNumber` annimmt (die Bibliothek kennt dort Grenzwerte in m/s), und dass `zoneNumber` auf die im Garmin-Profil hinterlegten Zonen zeigt, nicht auf die Grenzen aus `catalog/zones.json`.
- **Echter Versand (`--yes`).** Pflicht sind `--out` (Ordner `exchange/out`, dort `push-report.json`) und `--max-workouts N` (die ersten N gewählten Einheiten in Planreihenfolge); `--yes` und `--dry-run` schließen sich aus. Standard für `--from` ist heute: vergangene Termine werden nicht geplant. Ohne Token-Verzeichnis, mit unbrauchbarem Protokoll oder bei aktiver Sperre endet der Befehl vor jeder Anfrage (Exit-Code 2 oder 3). Je Einheit zwei Aufrufe (`upload_workout`, dann `schedule_workout` mit der `workoutId` der Antwort), über `WorkoutWriteCaller`: gedrosselt (Mindestabstand 3 s statt 1,5 s, Faustregel), ohne automatische Wiederholung (ein erneuter POST nach einem Zeitlimit würde Workouts doppelt anlegen), Aufrufbudget `2 * N`, Abbruch beim ersten Fehler. Exit-Codes: 1 bei Fehler oder unklarem Ergebnis, 2 bei ungültiger Anmeldung, 3 bei Sperre.
- **Protokoll `push-report.json`** (kumulativ, atomar nach jedem Aufruf geschrieben): je Einheit `key` (`plan_id:datum:position`), Datum, Platz, Vorlage, Titel, `status`, `workout_id`, `error_class`, `http_status`, Zeitpunkt. Zustände: `ok` (angelegt und geplant), `uploaded` (angelegt, nicht geplant: ein erneuter Lauf plant nur noch), `error` (Anfrage sicher nicht angekommen: ein erneuter Lauf versucht es noch einmal), `unknown` (Zeitlimit, Netzfehler, 5xx oder Antwort ohne Kennung: kann angekommen sein, wird nie automatisch wiederholt, in Garmin Connect prüfen, dann den Eintrag aus der Datei löschen). Schon gesendete Einheiten (`ok`) werden übersprungen. Das Protokoll enthält weder Nutzlast noch Zugangsdaten.
- **Einordnung von Fehlern nach dem Senden (Änderung vom 14. Oktober 2026, Integrationsprüfung).** `error` gilt nur, wenn der Fehler sicher vor dem Senden entstand (eigene Prüfung `PreSendError` von Nutzlast, Kennung und Datum, Freigabeliste, Sperre, Aufrufbudget) oder die Antwort eine Ablehnung war (401, 429, 403 und übrige 4xx). Alles andere ist `unknown`, ausdrücklich auch jedes `ValueError` und `TypeError` aus dem Aufruf: die Bibliothek liest nach einem erfolgreichen POST die Antwort mit `resp.json()`, und eine 2xx-Antwort ohne JSON wirft `json.JSONDecodeError` oder die gleichnamigen Klassen von `requests` und `curl_cffi` (beide erben von `ValueError`), obwohl das Workout schon angelegt ist. Zusätzlich liest `http_status_of` nur noch Zahlen, die als Status formuliert sind (`API Error 503`, `HTTP 403`, `client error (403)`), nicht mehr jede dreistellige Zahl: `port 443` in der Meldung eines Netzfehlers galt bisher als Status 443 und damit als sichere Ablehnung. Im Zweifel `unknown`.
- **Heute ist das lokale Datum (Änderung vom 14. Oktober 2026, Integrationsprüfung).** Plandaten und Garmin-Tageswerte sind Kalendertage des Nutzers (6.1). Wo das Zusatzprogramm "heute" braucht (`push --from` und `in_past`, Standard von `probe --today`, Fenster von `sync` und `backfill`), gilt der lokale Kalendertag der Systemuhr, nicht der UTC-Tag. Bei UTC-5 nach 19 Uhr ließ `push --yes` sonst die heutige Einheit weg, bei UTC+2 kurz nach Mitternacht plante es die gestrige noch ein. Zeitstempel in Manifest, Protokollen und Berichten bleiben UTC.
- **Nicht umgesetzt:** Löschen, Ändern oder Abbestellen von Workouts (`delete_workout`, `update_workout`, `unschedule_workout`), das Senden auf ein Gerät (`push_workout_to_device`) und FIT-Workout-Dateien (F32, O4). Ein geänderter Plan (neue `plan_id`) legt neue Workouts an; die alten sind in Garmin Connect von Hand zu entfernen.

### 5.5 Gestaltung der Oberfläche (seit T18)

Stand 14. Oktober 2026, Entwurf "Lastwerk Designsprache" bestätigt. Diese Vorgabe löst die Beschreibung "dicht, datenorientiert" aus T06 ab.

**Haltung.** Lastwerk ist ein Trainer, kein Tabellenblatt. Jede Hauptansicht beginnt mit einer Aussage in einem Satz und belegt sie danach mit Zahlen. Entscheidungen des Planers erscheinen mit Grund, nie still.

**Navigation seit T42 (16. Oktober 2026, Entscheidung E8 des Nutzers: radikal minimal).** Die Seitenleiste hat genau drei Hauptpunkte, Heute, Plan und Verlauf, und unten die Einstellungen; darüber steht nur noch die Zeile zum Abgleich mit Garmin. Alles Weitere ist Tiefe innerhalb dieser Seiten, als Reiter (`lib/components/Tabs.svelte`) oder als Umschalter (`Segmented`). Jede Wahl gibt es an genau einer Stelle: Sprache und Thema stehen nur in den Einstellungen (Reiter App), die Einrichtung des Coachs nur im Reiter Coach.

- **Verlauf** (`views/HistoryView.svelte`): Titel "Verlauf", darunter die Reiter Kalender (Umschalter Monat oder Woche; die Woche zeigt Last je Tag und Sportart, aber kein zweites PMC-Diagramm, das steht auf Heute), Aktivitäten (Liste, oder das Detail einer Aktivität an derselben Stelle mit "Zur Liste"; ein Klick auf den aktiven Reiter führt ebenfalls zurück), Fortschritt, Leistung und Körper (Umschalter Erholung oder Gesundheit). Gezeichnet wird nur der aktive Reiter. Die eingebetteten Seiten haben keinen eigenen Titel mehr, ihr Leitsatz (`data-testid="lead"`) steht direkt unter den Reitern, weil er dort aus den Daten entsteht.
- **Einstellungen** in vier Reitern: Daten und Garmin (Daten, Garmin, MCP), Coach (Einrichtung des lokalen Modells), Profil und Schwellen (Athlet, Schwellenwerte, PMC), App (Darstellung, Update, Über Lastwerk). Jeder Reiter hat einen Leitsatz (`ui.lead.settings_<reiter>`).
- **Reiter** folgen dem Muster "Tabs" der WAI-ARIA Authoring Practices: `role="tablist"`, nur der aktive Reiter ist per Tab-Taste erreichbar (wandernder Tabindex), Pfeiltasten, Pos1 und Ende wechseln und aktivieren sofort, der Inhalt steht in einem `tabpanel`. Testkennungen `tab-<gruppe>-<wert>`. Reiter bewegen sich nicht: Sie werden oft gewechselt, eine Bewegung würde nur aufhalten.
- **Zustand** in `lib/state.ts`: `view` (`today`, `plan`, `history`, `settings`), `historyTab`, `calendarMode`, `bodyTab`, `settingsTab`, `selectedActivityId`. Verweise zwischen Seiten laufen nur über `openHistory(tab)`, `openSettings(tab)` und `openActivity(id)`; so führt etwa "Zu den Einstellungen" bei fehlender Schwelle in den Reiter Profil und Schwellen, ohne Garmin-Werte in den Reiter Daten und Garmin, und der Hinweis ohne Modell im Plan in den Reiter Coach. Den früheren Punkt "Detail" gibt es nicht mehr.

**Startseite "Heute".** Zeigt die Einheit des heutigen Tages aus dem aktiven Plan (Vorlagenname, Dauer, Begründung aus den `reason_keys`), die Form als Zahl mit Zustand und den Verlauf der letzten Wochen. Ohne aktiven Plan oder ohne Einheit heute sagt sie das und verweist auf die Planansicht. Sie erfindet nichts: Was die Daten nicht hergeben, bleibt weg.

Aufbau seit T24: Kopf mit Datum, Titel und Ein-Satz-Aussage (`ui.lead.today`), die Empfehlung als einzige Karte, dann die Abschnitte "Form" (Aussage aus dem Formzustand, TSB groß mit Zustandspille, Fitness und Ermüdung als Kennzahlenreihe, Verlauf über die ganze Breite) und "Diese Woche" (Tagesbalken). Das Dashboard (T25) setzt weitere Abschnitte mit denselben Bausteinen ein.

Aufbau seit T25 Teil B (Abweichungen vom Entwurf in Klammern): Kopf mit Ein-Satz-Aussage, die Form, Erholung und den Plan von heute verbindet (`leadSentence`, jeder Teil entfällt, wenn die Daten ihn nicht hergeben), die Empfehlung als Karte, dann Bereitschaft (HRV, Ruhepuls, Schlaf gegen den eigenen Schnitt mit Mini-Verlauf, Hinweis "Lastwerk hat angepasst" nur bei `readiness.low` und einer vom Planer wegen R12 ersetzten Einheit der letzten sechs Tage, Body Battery erst mit T22), Form-Verlauf (120 Tage mit den vier Formzuständen als dezente Bänder, gestrichelte Prognose), und im zweispaltigen Raster (eine Spalte, wenn die Seite schmaler als 760 px ist) Rampe, Verteilung, Diese Woche, Planerfüllung, Effizienz, Leistungskurve, zuletzt der Belastungskalender (365 Tage, eigenes SVG). Jeder Abschnitt trägt eine Aussage aus dem Zustandscode (`ui.dash.statement.<code>`, keine Schwellen in der Oberfläche) und sagt ohne Daten, was fehlt.

- Die Prognose rechnet mit den tatsächlichen Lasten bis heute und den geplanten danach (nicht ab Planbeginn wie die Planansicht), damit die Kurve mitten im Plan ohne Sprung anschließt.
- Der Formzustand liegt als `metrics::form` in `core` (Grenze -30 aus `R10.tsb_warn_below`), die Oberfläche hält die Bänder mit denselben Grenzen in `lib/today.ts`.
- Planerfüllung rechnet bis gestern: Die Einheit von heute zählt am Morgen nicht als verpasst.
- Die aerobe Entkopplung erscheint nicht im Dashboard (sie braucht die Messreihe, die nur die Detailansicht liest).
- Die Legende der Bibliothek (uPlot) entfällt in allen Verläufen, stattdessen steht eine eigene mit Punkt, Name und Wert (Wert folgt dem Zeiger).
- Die Attrappe erzeugt die Dashboard-Daten eigenständig und deterministisch. Varianten für Tests und Sichtprüfung: `?dashboard=rich|low|sparse|empty|extreme|gaps` im Browser.

**Planansicht seit T26 Teil B.** Kopf mit Titel und Ein-Satz-Aussage, darunter in zwei Spalten (ab 880 px Seitenbreite, sonst untereinander): links die Karte "Dein Plan in drei Fragen" (Ziel als Wahl "Wettkampf" mit Datum und Sportart oder "Fit bleiben", Stunden je Woche als Schieber von 2 bis 20 in halben Stunden mit großem Livewert, sieben Tageschips, eine Primäraktion "Plan erstellen", vorbelegt aus aktivem Ziel und Verfügbarkeit) und der aktive Plan mit `PlanDisplay`, rechts das Gespräch mit dem Coach (mitscrollend, nie höher als das Fenster). Ganz unten, eingeklappt, "Feineinstellungen und Export" mit den bisherigen Panels (Ziel, Verfügbarkeit, Plan erzeugen, Planliste, Export und Sicherung). Der Vorschlag des Coachs ist die einzige `.card` im Gespräch; "Übernehmen" ist dort die Primäraktion, "Senden" ein gewöhnlicher Knopf. Bewegung: nur die Vorschlagskarte blendet 180 ms ein (`opacity`, `transform`), die Punkte "denkt nach" pulsieren in `opacity`, alles aus bei `prefers-reduced-motion`.

**Auswertungsseiten seit T31 bis T37** (seit T42 Reiter des Verlaufs, ohne eigenen Seitenkopf). Gleiche Bausteine wie "Heute" (Seitenkopf mit Ein-Satz-Aussage, Abschnitte ohne Kasten, Kennzahlen auf der Fläche). Umschalter mit wenigen festen Möglichkeiten (Größe, Zeitraum) als `Segmented`. "Fortschritt" (T32) zeigt Umfang und Last über 52 Wochen, 24 Monate und dieses Jahr gegen das Vorjahr bis zum gleichen Kalendertag; der Befehl liefert Tagessummen, die Oberfläche summiert nur. "Kalender" (T35) zeigt einen Monat als Raster mit geplanten Einheiten des aktiven Plans und Aktivitäten je Tag; der Zustand (erledigt, teilweise, verpasst, zusätzlich) ist reine Zählung und steht als Text, heute und Zukunft sind nie verpasst; die Wochenspalte zeigt Last und Stunden Ist gegen Soll. Er nutzt die vorhandenen Befehle `list_activities` und `get_plan`. "Erholung" (T34) zeigt HRV, Ruhepuls, Schlaf und Body Battery über 30, 90 oder 365 Tage mit eigener Basislinie und der Untergrenze aus R12 je Tag (`core::metrics::readiness::baseline_series`, dieselbe Rechnung wie R12 und das Dashboard), Tage unter der Untergrenze hervorgehoben, darunter die Tageslast auf derselben Zeitachse; alle Verläufe teilen den Zeiger. "Leistung" (T33) zeigt die Leistungskurve der Radeinheiten von 1 s bis 2 h auf logarithmischer Zeitachse für 6 Wochen, 90 Tage, ein Jahr oder alles, gegen den gleich langen Zeitraum davor, mit Tag und Verweis auf die Aktivität jedes Bestwerts; das Detail zeigt weiter nur die Eckdauern. Darunter die Bestzeiten im Laufen über 1 km, 5 km, 10 km, Halbmarathon und Marathon (T37, Tabelle `activity_best_effort`, Cache): kürzestes Fenster aus ganzen Sekunden innerhalb eines Laufs, nie über eine Pause, fehlende Geschwindigkeit und Werte über 12 m/s zählen als Stillstand (Faustregel). Zuletzt die Schwellen (T36): FTP als Stufenlinie mit dem FTP-Vorschlag je Kalendermonat (8.6) als Punkte, alle gesetzten FTP, LTHR, Laufschwellen und CSS als Tabelle; liegt der jüngste Vorschlag über dem gültigen FTP, sagt die Seite das und führt in die Einstellungen, übernimmt aber nichts.

**Formzustand.** Aus dem TSB des letzten Tages. Faustregel, keine begutachtete Quelle, wie R10 (8.4):

| Zustand | TSB | Schlüssel |
|---|---|---|
| Frisch | ab +5 | `ui.form.fresh` |
| Ausgewogen | -10 bis unter +5 | `ui.form.balanced` |
| Belastet | -30 bis unter -10 | `ui.form.loaded` |
| Überlastet | unter -30 | `ui.form.overloaded` |

Die Grenze -30 ist die von R10, die anderen sind Faustregel. Der Zustand steht immer auch als Text, nie nur als Farbe.

**Farben (dunkel zuerst).** Eine Signalfarbe Limette (`--accent`) für das, was jetzt zählt, und für die Primäraktion, höchstens eine Primäraktion je Karte. Flächen in Graphit (`--bg`, `--panel`, `--panel-2`), Text in warmem Weiß, gedämpfter Text in Grau. Zustandsfarben unterscheiden sich in Helligkeit und Beschriftung, nicht allein im Farbton. Das helle Thema bleibt erhalten, mit abgedunkeltem Akzent (Kontrast mindestens 4,5 zu 1 für Text).

**Schrift.** Seit T24 (Entscheidung vom 15. Oktober 2026) Cairo (variable Schrift, Gewicht 200 bis 1000, SIL Open Font License 1.1, npm-Paket `@fontsource-variable/cairo` 5.3.0) überall, als Datei im Build gebündelt und nie aus dem Netz (10.2, 10.4: `font-src 'self'`). Eingebunden in `app/ui/src/fonts.css` mit eigenem `@font-face` nur für die Untermengen latin und latin-ext, damit die arabische Datei des Pakets nicht ins Bundle gelangt (im Build zwei woff2-Dateien, zusammen rund 50 KB). Zeichen außerhalb (zum Beispiel ▲ ▼) fallen auf die Systemschrift zurück. Die Familie steht als `--font-sans`, Diagramme lesen sie daraus. Ersatz: `system-ui`.

Ziffern: Cairo hat kein OpenType-Merkmal `tnum`, aber alle Ziffern, das Minuszeichen U+2212 und das Prozentzeichen sind in jedem Gewicht gleich breit (gemessen an der Datei: 560, 559, 558, 555 Einheiten bei 1000 pro em für Gewicht 400, 600, 650, 800, jeweils für alle zehn Ziffern). Zahlen springen deshalb nicht, es braucht keine feste Breite. `font-variant-numeric: tabular-nums` bleibt gesetzt und schützt nur die Ersatzschrift. Negative Kennzahlen zeigt `Metric` mit U+2212 statt Bindestrich.

Skala für Cairo (Tokens in `app.css`): Cairo ist breit, hat eine kleine x-Höhe (0,5 em), hohe Oberlängen (Ascent 1,30 em, Descent 0,57 em, `normal` ergäbe Zeilenhöhe 1,87). Zeilenhöhen stehen deshalb immer ausdrücklich. Gegenüber der Systemschrift ist der Fließtext einen Punkt größer.

| Token | Wert | Verwendung |
|---|---|---|
| `--fs-metric` | 60 px, Zeilenhöhe 1,05, Abstand -0,03 em | größte Kennzahl einer Seite (SPEC: 56 bis 64) |
| `--fs-display` | 48 px, Zeilenhöhe 1,05, Abstand -0,02 em | Empfehlung (SPEC: 40 bis 52) |
| `--fs-title` | 32 px, Zeilenhöhe 1,15, Abstand -0,02 em | Seitentitel |
| `--fs-metric-sm` | 34 px | Kennzahl in einer Reihe |
| `--fs-heading` | 20 px, Zeilenhöhe 1,3 | Aussage unter einer Abschnittsmarke |
| `--fs-lead` | 17 px, Zeilenhöhe 1,35 | Ein-Satz-Aussage unter dem Titel |
| `--fs-body` | 15 px, Zeilenhöhe 1,55 | Fließtext |
| `--fs-dense` | 14 px | Tabellen, Formularfelder |
| `--fs-small` | 13 px | Erläuterungen, Legenden, Achsen |
| `--fs-label` | 12 px, Versalien, Abstand 0,08 em, Gewicht 700 | Marken (Abweichung von "13 in Versalien": 12 reicht in Cairo und lässt längere Marken in einer Zeile) |

**Seit T24: kein Kachelraster.** Kennzahlen stehen groß direkt auf der Fläche, Abschnitte trennt Weißraum und eine feine Linie (`--line`) statt einer Karte je Wert. Bausteine (Klassen in `app.css`, Svelte-Hüllen in `lib/components/`):

- `.section` / `Section` (`label`, optional `statement`, `testid`, `aside`): Marke in Versalien, optional eine Aussage in einem Satz, Linie oben, kein Kasten.
- `.metric` / `Metric` (`value`, `unit`, `label`, `delta` mit Text, Richtung und Bewertung, `size` md, lg oder xl, `swatch`, `extra`): großer Wert, Einheit klein daneben, Beschriftung darunter, Veränderung mit Pfeil und Text. Der Wert kommt fertig formatiert.
- `.stat-row` / `StatRow`: mehrere Kennzahlen nebeneinander, getrennt durch dünne senkrechte Linien, unter 640 px untereinander.
- `.pill` (mit Zustandsklassen): Zustand als Punkt und Text.
- `.card` bleibt für Dinge, die man anfasst (Empfehlung, Vorschlag des Coachs, Formulare): leicht erhoben, feiner Innenrand, Radius 20. `.panel` bleibt als ruhige Arbeitsfläche in Plan, Aktivitäten und Detail.
- Diagramme sind randlos über die Breite des Abschnitts, Achsen in Cairo (13 px), kein Rahmen, keine Teilstriche, Gitter nur waagerecht und hell.
- Der Inhaltsbereich trägt oben einen sehr dezenten Verlauf, nur aus Tokens (`--glow`, `--glow-2`).

Limette bleibt die einzige Signalfarbe, höchstens eine Primäraktion je Bereich.

**Form.** Raster 8, Radien 12 (Knöpfe), 16 bis 20 (Karten), 999 (Pillen). Primäraktionen mindestens 44 hoch, übrige Knöpfe mindestens 36. Navigation als Seitenleiste.

**Bewegung.** Nur CSS, nur `transform` und `opacity`, 120 bis 200 ms, `ease-out`; nichts bewegt sich beim Laden einer Ansicht, und `prefers-reduced-motion` schaltet alles ab. Keine neue Abhängigkeit für Bewegung. Skills dazu: `.claude/skills`.

**Stand T19 (14. Oktober 2026).** Woche, Plan, Aktivitäten, Detail und Einstellungen tragen die Ein-Satz-Aussage unter dem Titel (`ui.lead.*`; seit T42 im Verlauf und in den Einstellungen unter den Reitern), der Ansichtswechsel bewegt sich über `opacity` und `transform` in 160 ms. Der Erststart-Assistent und der Abschnitt "Über Lastwerk" nutzen dieselben Bausteine. Die Änderungen je Version stehen in `catalog/changelog.json` und speisen App und Release-Notizen (`scripts/release-notes.mjs`). Die Dichte der Tabellen in Aktivitäten und Detail bleibt bewusst hoch, sie sind Arbeitsflächen.

**Noch nicht gebaut (Vorschlag).** Die Karte "Lastwerk hat angepasst" aus dem Entwurf setzt voraus, dass der Planer Änderungen an einem laufenden Plan mit Grund speichert. Dafür gibt es noch kein Datenmodell. Sie fehlt bis dahin in der App, damit keine erfundenen Einträge erscheinen.

### 5.6 Garmin-Zugang in der App (seit T20)

Entscheidung vom 15. Oktober 2026, bewusst gegen die frühere Festlegung (2.1, 2.3, 5.4): Lastwerk meldet sich selbst bei Garmin an und holt Aktivitäten und Erholungswerte, ohne Python und ohne Kommandozeile. `bridge/` bleibt eingefroren im Repository. Grundlage der Protokolldetails ist der Quelltext von python-garminconnect 0.3.17 (Q27, MIT), gelesen in `bridge/.venv`, übernommen wird Wissen, kein Code.

**Aufteilung.**

| Teil | Ort | Aufgabe |
|---|---|---|
| `lastwerk-garmin` | `crates/garmin` | Tausch Ticket gegen Token, Erneuern, Token-Ablage, lesender Client mit Allowlist, Drosselung, Sperre. Netzwerk erlaubt, kein Zugriff auf die Datenbank |
| Abgleich | `crates/cli` (Bibliothek, Befehl `lastwerk garmin`) | Aktivitätsliste, FIT-Download, Erholungswerte, Schreiben in Speicher und FIT-Ablage |
| Anmeldefenster und Befehle | `app/src-tauri` | Fenster mit der Garmin-Seite, Ticket abfangen, `garmin_*`-Befehle |
| `lastwerk-core` | `crates/core` | unverändert rein, kennt Garmin nicht |

**Anmeldung (T23).** Die App öffnet ein eigenes Fenster mit der Anmeldeseite von Garmin (`https://sso.garmin.com/portal/sso/<sprache>/sign-in?clientId=GarminConnect&service=https://connect.garmin.com/app`). Passwort, MFA und Captcha laufen vollständig auf der Seite von Garmin, Lastwerk sieht sie nie. Nach Erfolg leitet Garmin auf `https://connect.garmin.com/app?ticket=ST-…` weiter. Der Navigationsfilter des Fensters erkennt genau diese Adresse, bricht die Navigation ab, schließt das Fenster und gibt das Ticket an `lastwerk-garmin`. Das Fenster hat keine Capability, also keinen Zugriff auf Befehle der App (10.4). Der Weg erfüllt Abrufregel 5 besser als der frühere: Ein echter Browser löst Captcha und MFA, nichts wird umgangen oder nachgeahmt.

Umsetzung (T23): Fenster `garmin-login`, etwa 480 × 720, Titel aus `ui.garmin.window_title`, Sprache der Anmeldeseite `de-DE` oder `en-US` nach `app.locale`, inkognito (keine Sitzungsdaten von Garmin im Datenordner der Oberfläche), Entwicklerwerkzeuge nur im Debug-Build, neue Fenster und Downloads abgelehnt. Der Navigationsfilter ist die reine Funktion `decide_navigation(url) -> NavDecision` (`app/src-tauri/src/garmin_login.rs`): Liefert `ticket_from_redirect` ein Ticket, wird die Navigation abgebrochen, das Fenster geschlossen und `GarminClient::connect` in `spawn_blocking` ausgeführt, danach Ereignis `garmin-login` `{ ok: true, code: "connected" }` oder `{ ok: false, code: <Code> }`. Erlaubt sind sonst nur `https`-Adressen ohne Port und ohne Zugangsdaten, deren Host genau `garmin.com` ist oder auf `.garmin.com` endet (IP-Adressen, `garmin.com.evil…`, `evilgarmin.com`, `http`, `data:`, `blob:`, `about:`, `javascript:` werden abgelehnt). `https://connect.garmin.com/app…` ohne lesbares Ticket (etwa mit einem weiteren Parameter) bricht ab und meldet `unexpected_redirect`; ins Protokoll geht nur der Pfad, nie die Query. Schließt der Nutzer das Fenster, meldet es `cancelled`. Fremde Hosts für Captcha (falls Garmin welche einbindet) würden als Navigation abgelehnt; ob das die Anmeldung stört, zeigt der Livetest.

**Ereignisse an das Hauptfenster (T23).** `garmin-login` `{ ok, code }` und `garmin-sync-progress` `{ phase: account | activities | import | recovery | wellness, done, total }` (seit T40 in dieser Reihenfolge: Konto und Aktivitäten zuerst, dann Schwellen schreiben und einlesen, damit Last und Form früh stehen, danach Erholung und Tageswerte) (beim Import höchstens etwa 200 Ereignisse, wie `import-progress`).

**Token (T21).** `POST https://diauth.garmin.com/di-oauth2-service/oauth/token` mit `grant_type=https://connectapi.garmin.com/di-oauth2-service/oauth/grant/service_ticket`, `service_ticket`, `service_url` wie oben und Basic-Auth `<client_id>:`; die Client-Kennungen werden der Reihe nach versucht wie in der Bibliothek. Erneuern mit `grant_type=refresh_token`, wenn das Token in weniger als 900 s abläuft oder eine Anfrage 401 liefert. Ablage im Schlüsselbund des Betriebssystems (Windows-Anmeldeinformationsverwaltung, macOS-Schlüsselbund, Secret Service unter Linux) über das Crate `keyring`, Dienst `lastwerk-garmin`. Token stehen nie in Datenbank, Backup, Logs, Fehlermeldungen oder Antworten an die Oberfläche. Umsetzung, Grenzen des Schlüsselbunds (Windows 2560 Bytes, Linux nur mit Secret Service) und Abweichungen stehen in 5.4 unter `lastwerk-garmin`. "Trennen" löscht den Eintrag; bei Verdacht widerruft der Nutzer den Zugriff zusätzlich in den Garmin-Kontoeinstellungen.

**Abruf (T22).** Nur lesend, Allowlist der Pfade im Code: Profil (`socialProfile`), Aktivitätsliste, FIT-Original (`download-service/files/activity/{id}`, ZIP, nur `*.fit` darin, höchstens 256 MiB), HRV, Schlaf, Ruhepuls, Body Battery, Stress. Drosselung wie Abrufregel 4 und O24: 1,5 s Mindestabstand plus bis zu 0,5 s Zufall, kein Parallelabruf, drei 429 oder 403 in Folge sperren für 6 Stunden (Zustand im Datenordner der App). Erster Abgleich 365 Tage zurück (Faustregel, einstellbar), danach ab dem letzten Abgleich minus 7 Tage, weil Garmin Tageswerte nachträglich ergänzt (wie `sync --refresh-days 7`). Die FIT-Dateien landen im Unterordner `garmin/` des FIT-Ordners des Nutzers (`<FIT-Ordner>/garmin/<Kennung>.fit`), danach wird der ganze FIT-Ordner eingelesen (Duplikatregel 6.4). Der FIT-Stamm ist seit T28 fest `<Datenordner>/Lastwerk/fit`, der Nutzer wählt keinen (Entscheidung T28, ersetzt die Regel aus T23 mit gewähltem Ordner und `<Datenordner>/Lastwerk/garmin/fit/` aus T22, Begründung in 5.4 unter `lastwerk-cli`, "Feste Ablage"). Damit bleibt 6.1 gültig: FIT-Dateien sind die Quelle, die Datenbank ein Cache, und es gibt nur einen Ordner zum Einlesen und Wiederaufbauen. Erholungswerte sind Nutzerdaten wie bisher (`recovery_day`, im Backup). Umsetzung, Fenster, Fehlerbehandlung und Abweichungen stehen in 5.4 unter `lastwerk-cli` (Umsetzung in T22).

**Vollabzug in der App (T39, Entscheidungen E1 bis E4 vom 16. Oktober 2026).** Der Abgleich holt zusätzlich zu Aktivitäten und Erholung alles, was Garmin über belegte Endpunkte liefert. Belegt heißt: Pfad, Parameter und gelesene Felder stehen im Quelltext von python-garminconnect 0.3.17 (Q27) und im Bericht der Sonde (`docs/garmin-feldabdeckung.md`); die Liste mit Zeilenangaben steht in `docs/tasks/T39-garmin-vollabzug.md`. Was damals nicht belegt war, kam mit T40 dazu (Entscheidung des Nutzers vom 16. Oktober 2026, "alles bauen"): Verlauf von FTP, Laktatschwellen-Herzfrequenz und Schwellentempo, das Schwellentempo selbst, VO2max-Verlauf, Trainingsstatus mit akuter und chronischer Last, Belastungsfokus, Rennprognosen, Lauftoleranz, Etagen und mit Freigabe je Bereich Gewicht und Körperwerte, Blutdruck und Ernährung. Die Felder belegen Aufzeichnungen und Lesecode Dritter (Q28 bis Q33, nur Sekundärquelle); Pfade und Parameter stehen weiter in Q27. Liste mit Belegen: `docs/tasks/T40-garmin-vollabzug-2.md`. Der Livetest (O32) bestätigt die Formate; bricht eine Annahme, verwirft der Parser den Wert, es entsteht kein falscher. Nicht abgerufen werden weiter die Tageszusammenfassung (`usersummary/daily`), Mahlzeiten-Einstellungen und das Lebensstil-Protokoll.

| Gruppe | Daten | Ablage |
|---|---|---|
| Konto | Geschlecht und Geburtsjahr, VO2max (Lauf, Rad) aus dem Profil; FTP und Laktatschwellen-Herzfrequenz mit Datum; maximale Herzfrequenz und Ruhepuls aus den Herzfrequenzzonen | `setting` (`athlete.*`), `threshold` mit Herkunft `garmin`, `training_day` |
| Tageswerte (S2) | Schritte und Strecke, minimale und maximale Tagesherzfrequenz, Atmung wach und im Schlaf, SpO2 Mittel und Tiefstwert, Intensitätsminuten, Stress Mittel und Höchstwert | `wellness_day` |
| Training (S2) | Trainingsbereitschaft (Wert, Stufe, Garmins akute Last), Fitnessalter, Ausdauerwert, Bergwert | `training_day` |
| Training (S1) | Garmins Trainingslast und Trainingseffekt je Aktivität, nur zur Anzeige (E2) | `garmin_activity_load` |
| Tageswerte (S2, T40) | Etagen auf und ab | `wellness_day` |
| Training (S2, T40) | VO2max-Verlauf, Trainingsstatus (Schlüssel aus Garmins Phrase), chronische Last und Verhältnis akut zu chronisch, Belastungsfokus mit Zielspannen, Rennprognosen, Lauftoleranz | `training_day` |
| Konto (T40) | Verlauf von FTP (Rad), LTHR und Schwellentempo (Lauf) im ganzen Fenster, Schwellentempo aus der Laktatschwelle | `threshold` mit Herkunft `garmin` |
| S3 | Hydration (`garmin.share.hydration`), seit T40 je Bereich Gewicht und Körperwerte (`garmin.share.body`), Blutdruck (`garmin.share.blood_pressure`), Ernährung (`garmin.share.nutrition`), nur nach Freigabe des Bereichs (Entscheidung E7) | `sensitive_day`, `body_day`, `bp_day`, `nutrition_day` |
| Weiterhin | Aktivitäten samt FIT, HRV, Schlaf, Ruhepuls, Body Battery | wie T22 |

Nie abgerufen werden Menstruations- und Schwangerschaftsdaten: Die Allowlist hat keinen Eintrag, ein Test lehnt alle Pfade von `periodichealth-service` ab (E3, 5.5). Gewicht und Größe aus dem Profil liest der Abgleich nicht, obwohl die Antwort sie enthält; Gewicht kommt nur über den freigegebenen Bereich "Körper". S3-Abrufe gehen nur über den Marker `SensitiveConsent`, den der Abgleich nur nach der Freigabe in den Einstellungen anlegt; ohne Freigabe kein Aufruf. Garmins eigene Last und Trainingseffekte ersetzen nie die Last nach 8.1 (E2). Das Frontend greift nie aufs Netz zu, alle Abrufe laufen im Rust-Teil (E4, 10.1). Schwellen aus Garmin: Regeln in 8.6.

**Wann.** Auf Klick ("Jetzt abgleichen") und einmal beim Start der App, wenn verbunden (Einstellung, Standard an). Kein Hintergrunddienst, kein Zeitplan. Jede Verbindung ist in den Einstellungen sichtbar (10.1).

**Offen.** O18 (Nutzungsbedingungen) bleibt offen; das Anmeldefenster nennt das Risiko vor dem ersten Verbinden. Ob `diauth.garmin.com` Anfragen ohne Browser-TLS (reqwest mit rustls) annimmt, zeigt erst der Livetest beim Nutzer (O30).

---

## 6. Datenmodell

### 6.1 Grundsatz

Quelle der Wahrheit sind die FIT-Dateien im festen Ablageordner `<lokaler Datenordner>/Lastwerk/fit` (seit T28, Entscheidung vom 15. Oktober 2026: keine Ordnerwahl, so wenig Konfiguration wie möglich). Der Ordner liegt im Datenordner der App und nicht im Programmordner: Den ersetzt jedes Update, die Deinstallation löscht ihn, und unter `Program Files` darf die App nicht schreiben. `LASTWERK_FIT` überschreibt den Ort (Tests, Vorführungen), wie `LASTWERK_DB` die Datenbank. Die Datenbank ist für alles, was aus FIT ableitbar ist, ein Cache und darf jederzeit gelöscht und neu aufgebaut werden. Nutzereingaben (Schwellenwerte, Ziele, Verfügbarkeit, Pläne, Einstellungen, importierte Erholungsdaten) sind nicht ableitbar und werden per JSON-Export gesichert.

Speicherort: `%LOCALAPPDATA%\Lastwerk\lastwerk.db` unter Windows, `~/Library/Application Support/Lastwerk/lastwerk.db` unter macOS, `$XDG_DATA_HOME` oder `~/.local/share/Lastwerk/lastwerk.db` unter Linux (Tauri: `local_data_dir`, CLI und App nehmen dieselbe Datei; `LASTWERK_DB` ersetzt den Pfad in beiden). Lokal statt Roaming, weil die Datei neu aufbaubar ist und nicht in Profilsynchronisation gehört. Die Datenbank liegt nicht in einem Cloud-Sync-Ordner (Beschädigungsrisiko bei SQLite im WAL-Modus). Die App weist beim Öffnen darauf hin, wenn der Pfad in bekannten Sync-Ordnern liegt.

Zeit: Zeitstempel in UTC als ISO 8601 Text. Tagesdatum einer Aktivität ist das lokale Datum aus `start_utc + local_offset_s`. Konvention, damit eine Fahrt um 23:30 Ortszeit dem richtigen Tag zugerechnet wird.

Archiv: Das Garmin-Archiv des Zusatzprogramms (5.5) liegt unter `local/archive/garmin/` im Projektordner, getrennt von der Lastwerk-Datenbank, und ist nicht Teil des JSON-Exports der Nutzerdaten. Es ist nicht aus FIT-Dateien ableitbar, weil es JSON-Antworten enthält, die nur Garmin liefert. Geht es verloren, muss es neu geholt werden, soweit Garmin die Daten noch ausliefert.

Sicherung des Archivs (O28, Ort entschieden, Sicherung offen): Der Ort im Projektordner hat ein Risiko. `git clean -x` und `git clean -X` löschen ignorierte Dateien und damit das Archiv. Regel: `git clean` nie mit `-x` oder `-X` ausführen. Sicherung durch regelmäßige Kopie auf einen anderen Datenträger. Verschlüsselung (zum Beispiel BitLocker) ist offen. Der Projektordner selbst darf nicht in einen Cloud-Sync-Ordner verschoben werden.

### 6.2 Tabellen

Cache (aus FIT ableitbar):

```sql
CREATE TABLE source_file (
  id            INTEGER PRIMARY KEY,
  rel_path      TEXT NOT NULL,
  sha256        TEXT NOT NULL UNIQUE,
  size_bytes    INTEGER NOT NULL,
  mtime_utc     TEXT NOT NULL,
  imported_at   TEXT NOT NULL,
  status        TEXT NOT NULL CHECK (status IN ('ok','not_activity','error')),
  error_code    TEXT
);

CREATE TABLE activity (
  id              INTEGER PRIMARY KEY,
  source_file_id  INTEGER NOT NULL REFERENCES source_file(id),
  sport           TEXT NOT NULL,
  sub_sport       TEXT,
  start_utc       TEXT NOT NULL,
  local_offset_s  INTEGER NOT NULL,
  local_date      TEXT NOT NULL,
  duration_s      INTEGER NOT NULL,
  moving_s        INTEGER,
  distance_m      REAL,
  ascent_m        REAL,
  avg_hr          REAL,
  max_hr          REAL,
  avg_power_w     REAL,
  np_w            REAL,
  device_serial   INTEGER,
  device_product  TEXT,
  has_power       INTEGER NOT NULL DEFAULT 0,
  has_hr          INTEGER NOT NULL DEFAULT 0,
  is_primary      INTEGER NOT NULL DEFAULT 1,
  duplicate_of    INTEGER REFERENCES activity(id)
);
CREATE INDEX activity_local_date ON activity(local_date);

CREATE TABLE activity_load (
  activity_id     INTEGER NOT NULL REFERENCES activity(id),
  method          TEXT NOT NULL,
  value           REAL,
  intensity_factor REAL,
  threshold_id    INTEGER REFERENCES threshold(id),
  status          TEXT NOT NULL CHECK (status IN ('ok','missing_threshold','missing_channel','implausible')),
  computed_at     TEXT NOT NULL,
  PRIMARY KEY (activity_id, method)
);

CREATE TABLE activity_zone_time (
  activity_id INTEGER NOT NULL REFERENCES activity(id),
  scheme      TEXT NOT NULL,        -- z. B. 'power7', 'hr_lthr5'
  zone        INTEGER NOT NULL,
  seconds     INTEGER NOT NULL,
  PRIMARY KEY (activity_id, scheme, zone)
);

CREATE TABLE activity_best_power (
  activity_id INTEGER NOT NULL REFERENCES activity(id),
  duration_s  INTEGER NOT NULL,     -- seit T33 Raster 1 s bis 7200 s (POWER_CURVE_DURATIONS_S)
  watts       REAL NOT NULL,
  PRIMARY KEY (activity_id, duration_s)
);

CREATE TABLE activity_best_effort (   -- seit T37, nur Läufe
  activity_id INTEGER NOT NULL REFERENCES activity(id),
  distance_m  INTEGER NOT NULL,     -- 1000, 5000, 10000, 21097, 42195
  seconds     INTEGER NOT NULL,
  PRIMARY KEY (activity_id, distance_m)
);
```

Nutzerdaten (nicht ableitbar, im Backup enthalten):

```sql
CREATE TABLE threshold (
  id          INTEGER PRIMARY KEY,
  sport       TEXT NOT NULL,
  kind        TEXT NOT NULL CHECK (kind IN ('ftp_w','lthr_bpm','hr_max_bpm','hr_rest_bpm','run_speed_mps','css_mps')),
  value       REAL NOT NULL,
  valid_from  TEXT NOT NULL,        -- Datum
  origin      TEXT NOT NULL CHECK (origin IN ('manual','suggested_accepted','garmin')),  -- 'garmin' seit T39
  note        TEXT
);

CREATE TABLE goal (
  id        INTEGER PRIMARY KEY,
  kind      TEXT NOT NULL CHECK (kind IN ('event','continuous')),
  sport     TEXT,
  event_date TEXT,
  label     TEXT,                   -- Freitext, nie an ein LLM
  priority  TEXT CHECK (priority IN ('A','B','C')),
  active    INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE availability (
  weekday      INTEGER PRIMARY KEY CHECK (weekday BETWEEN 0 AND 6),  -- 0 = Montag
  max_minutes  INTEGER NOT NULL,
  allow_hard   INTEGER NOT NULL DEFAULT 1,
  fixed_rest   INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE plan (
  id            TEXT PRIMARY KEY,   -- UUID
  created_at    TEXT NOT NULL,
  goal_id       INTEGER REFERENCES goal(id),
  source        TEXT NOT NULL CHECK (source IN ('rules','llm')),
  rules_version TEXT NOT NULL,
  locale        TEXT NOT NULL,
  json          TEXT NOT NULL,      -- gegen plan.schema.json validiert
  status        TEXT NOT NULL CHECK (status IN ('draft','active','archived'))
);

CREATE TABLE manual_session (         -- Kraft ohne FIT, Nachträge
  id          INTEGER PRIMARY KEY,
  local_date  TEXT NOT NULL,
  sport       TEXT NOT NULL,
  duration_s  INTEGER NOT NULL,
  rpe         INTEGER CHECK (rpe BETWEEN 1 AND 10)
);

CREATE TABLE recovery_day (           -- M4
  local_date           TEXT PRIMARY KEY,
  hrv_overnight_ms     REAL,
  resting_hr_bpm       REAL,
  sleep_duration_s     INTEGER,
  sleep_score          INTEGER,
  body_battery_high    INTEGER,         -- seit T22 (Migration 0004), 0 bis 100
  body_battery_low     INTEGER,         -- seit T22 (Migration 0004), 0 bis 100
  source               TEXT NOT NULL,
  imported_at          TEXT NOT NULL
);

CREATE TABLE setting (key TEXT PRIMARY KEY, value TEXT NOT NULL);
```

Migration: `PRAGMA user_version`. Jede Migration ist ein nummeriertes SQL-Skript im Repository und hat einen Test, der die Datenbank vom Stand N-1 auf N hebt. Eine Migration löscht niemals Nutzerdaten. Ist ein Cache-Schema inkompatibel, wird der Cache verworfen und aus FIT neu aufgebaut.

Umsetzung in T04 (Migration `crates/store/migrations/0001_initial.sql`), mit den Abweichungen vom Schema oben:

- `source_file.sha256` ist nicht `UNIQUE`, stattdessen ist `rel_path` `UNIQUE` und `sha256` indiziert. Der Import überspringt Dateien mit gleichem Pfad und gleicher Prüfsumme. Zwei Dateien mit gleichem Inhalt in verschiedenen Ordnern würden bei `UNIQUE(sha256)` eine Fehlerzeile erzeugen. So werden sie zu zwei Aktivitäten, die die Duplikatregel (6.4) zusammenführt.
- `activity.source_file_id` ist `UNIQUE`: eine Aktivität je Datei (Mehrkampf ist nicht Teil von Version 1).
- `threshold.sport` kennt neben den Sportarten den Wert `all` für sportartübergreifende Schwellen (Ruhepuls, maximale Herzfrequenz).
- `activity_load.method` kennt für Fehlerzeilen den Wert `none`, weil bei einem Fehler keine Methode feststeht.
- Das Geschlecht für die TRIMP-Konstanten (`Thresholds::sex`) hat kein Feld in der Tabelle `threshold`. Es liegt als `setting` mit Schlüssel `athlete.sex` und den Werten `male` oder `female`.
- `recovery_day` fehlt in Migration 0001 und kommt mit M4 (Task T14, Migration 0003, siehe unten). Eine neue Migration braucht eine Datei `crates/store/migrations/NNNN_name.sql` und einen Eintrag in `MIGRATIONS` (`crates/store/src/migrate.rs`). Jede Migration läuft in einer Transaktion; der Test hebt eine Datenbank vom Stand N-1 auf N und prüft, dass Nutzerdaten erhalten bleiben.

Umsetzung in T05 (Migration `crates/store/migrations/0002_zone_time_estimated.sql`):

- `activity_zone_time.estimated INTEGER NOT NULL DEFAULT 0` hält fest, dass die Zonen nach Herzfrequenzreserve auf einer aus dem Alter geschätzten maximalen Herzfrequenz beruhen (8.5). `zone` zählt ab 1.
- `activity_zone_time` enthält je Aktivität und Schema alle Zonen (auch mit 0 Sekunden) oder gar keine Zeile, wenn das Schema nicht berechenbar ist. `activity_best_power` enthält nur Dauern, die der längste zusammenhängende Abschnitt hergibt, und keine Zeilen ohne Leistung.
- Das Geburtsjahr für die Schätzung der maximalen Herzfrequenz liegt als `setting` mit Schlüssel `athlete.birth_year` (Jahreszahl), wie das Geschlecht in `athlete.sex`.

Umsetzung in T14 (Migration `crates/store/migrations/0003_recovery_day.sql`):

- `recovery_day` genau nach der Tabelle oben, ohne weitere Einschränkungen in SQL. Die Wertebereiche aus 7.3 prüft der Speicher in Rust (`InvalidInput`) und die CLI über das Schema. Die Tabelle gehört zu den Nutzerdaten (nicht aus FIT ableitbar), steht im Backup und bleibt bei `rebuild_cache` unberührt (6.3). Das Datum ist das Datum der Datei `YYYY-MM-DD.json` und des Feldes `date`. `fetched_at` aus der Datei hat in der Tabelle keine Spalte.
- Die Feldnamen gelten vorläufig, bis `docs/garmin-feldabdeckung.md` aus L1 vorliegt (O9). Ändern sie sich, ändert sich das Schema und gegebenenfalls eine weitere Migration.

Umsetzung in T22 (Migration `crates/store/migrations/0004_recovery_garmin.sql`): `recovery_day` bekommt `body_battery_high` und `body_battery_low` (beide `INTEGER`, leer erlaubt, Wertebereich 0 bis 100 prüft der Speicher in Rust wie bei den übrigen Werten). Vorhandene Zeilen behalten alle Werte, die neuen Spalten sind dort leer (Test: Stand 3 auf 4 mit einer Erholungszeile). Neue Einstellungen (Tabelle `setting`, im Backup) seit T22: `garmin.last_sync` (`JJJJ-MM-TT`), `garmin.sync_on_start` (`true`/`false`, Standard `true`) und `garmin.backfill_days` (Standard 365, Faustregel, erlaubt 1 bis 1825). Lesen und Schreiben nur über `Store::garmin_settings` und die Setter, ein Wert außerhalb gilt beim Lesen als nicht gesetzt.

Umsetzung in T39 (Migration `crates/store/migrations/0007_garmin_full.sql`, Stand 16. Oktober 2026):

- `threshold` wird neu angelegt, weil SQLite eine `CHECK`-Bedingung nicht ändern kann: neue Tabelle mit `origin IN ('manual','suggested_accepted','garmin')`, Zeilen mit denselben IDs kopiert, alte Tabelle gelöscht, neue umbenannt, Index neu. Die Fremdschlüssel aus `activity_load.threshold_id` werden dabei erst beim Abschluss geprüft (`defer_foreign_keys`). Nutzerdaten bleiben erhalten (Test Stand 6 auf 7).
- Cache: `activity_load.missing TEXT` nennt bei `missing_threshold` die fehlende Art (`ftp_w`, `lthr_bpm`, `hr_max_bpm`, `hr_rest_bpm`, `run_speed_mps`, `css_mps`, `sex`), sonst leer. `DERIVED_CACHE_VERSION` steigt, damit der nächste Start die Spalte für alte Dateien füllt (6.3).
- Nutzerdaten (nicht aus FIT ableitbar, im Backup, `rebuild_cache` lässt sie stehen), je Tag eine Zeile, jeder Wert leer erlaubt, Wertebereiche prüft der Speicher in Rust (Faustregeln im Code), `source` und `imported_at` wie bei `recovery_day`:

```sql
CREATE TABLE wellness_day (           -- S2, T39
  local_date TEXT PRIMARY KEY,
  steps INTEGER, step_distance_m REAL,
  hr_min_bpm REAL, hr_max_bpm REAL,
  respiration_waking_brpm REAL, respiration_sleep_brpm REAL,
  spo2_avg_pct REAL, spo2_low_pct REAL,
  intensity_moderate_min INTEGER, intensity_vigorous_min INTEGER,
  stress_avg INTEGER, stress_max INTEGER,
  source TEXT NOT NULL, imported_at TEXT NOT NULL
);
CREATE TABLE training_day (           -- S2, T39
  local_date TEXT PRIMARY KEY,
  readiness_score INTEGER, readiness_level TEXT, garmin_acute_load REAL,
  fitness_age REAL, vo2max_running REAL, vo2max_cycling REAL,
  endurance_score INTEGER, hill_score INTEGER,
  source TEXT NOT NULL, imported_at TEXT NOT NULL
);
CREATE TABLE garmin_activity_load (   -- S1, T39, nur Anzeige (E2)
  garmin_activity_id INTEGER PRIMARY KEY,
  local_date TEXT NOT NULL,
  training_load REAL, training_effect TEXT,
  source TEXT NOT NULL, imported_at TEXT NOT NULL
);
CREATE TABLE sensitive_day (          -- S3, T39, nur nach Freigabe
  local_date TEXT PRIMARY KEY,
  hydration_ml REAL,
  source TEXT NOT NULL, imported_at TEXT NOT NULL
);
```

- Neue Einstellungen: `garmin.share.hydration` (`true` gibt den Abruf frei, Standard aus), `athlete.sex_origin` und `athlete.birth_year_origin` (`garmin` oder `manual`). Ein Abgleich mischt je Tag wie bei `recovery_day`: ein neuer Wert ersetzt den alten, ein fehlender lässt ihn stehen, ein Tag ohne Wert legt keine Zeile an. Ein zweiter Abgleich mit denselben Antworten ändert nichts.
- `garmin_activity_load` hängt an der Kennung der Aktivität bei Garmin. Die Anzeige im Aktivitätsdetail findet sie über den Dateinamen `garmin/<Kennung>.fit` (5.4, `lastwerk-cli`).
- S3 liegt in einer eigenen Tabelle, die der MCP-Server nicht liest. Nimmt der Nutzer die Freigabe zurück, holt der Abgleich nichts mehr; vorhandene Zeilen löscht der Nutzer mit "Hydration löschen" in den Einstellungen (Befehl `delete_sensitive_data`).

Umsetzung in T40 (Migration `crates/store/migrations/0008_garmin_more.sql`, Stand 16. Oktober 2026): nur neue Spalten und Tabellen, bestehende Zeilen bleiben (Test Stand 7 auf 8).

- `wellness_day`: `floors_up`, `floors_down` (Summe der Viertelstunden eines Tages).
- `training_day`: `training_status` (Schlüssel aus Garmins Phrase, zum Beispiel `productive`), `garmin_chronic_load`, `garmin_acwr`, Belastungsfokus `load_aerobic_low`, `load_aerobic_high`, `load_anaerobic` je mit `_min` und `_max` (Garmins Zielspanne) und `load_balance` (Hinweis als Schlüssel), Rennprognosen `race_5k_s`, `race_10k_s`, `race_half_s`, `race_marathon_s`, Lauftoleranz `run_acute_distance_m`, `run_impact_load`, `run_tolerance_m`, `run_tolerance_level`.
- S3 je Bereich in eigenen Tabellen (Entscheidung E7), damit Freigabe und Löschen je Bereich gehen: `body_day` (`weight_kg`, `bmi`, `body_fat_pct`, `body_water_pct`, `bone_kg`, `muscle_kg`), `bp_day` (`systolic_mmhg`, `diastolic_mmhg`, `pulse_bpm`), `nutrition_day` (`calories_kcal`, `carbs_g`, `fat_g`, `protein_g`, `calories_goal_kcal`). Hydration bleibt in `sensitive_day`. Freigaben `garmin.share.body`, `garmin.share.blood_pressure`, `garmin.share.nutrition` wie `garmin.share.hydration`. `delete_sensitive_data { category }` löscht einen Bereich, ohne Angabe alle. Alle drei Tabellen stehen in der Sicherung (`body_days`, `bp_days`, `nutrition_days`, fehlend gilt als leer) und werden vom MCP-Server nicht gelesen.

### 6.3 Wiederaufbau

`rebuild_cache` löscht `source_file`, `activity`, `activity_load`, `activity_zone_time`, `activity_best_power`, `activity_best_effort` (seit T37) (nicht `recovery_day` und die Nutzertabellen aus T39, siehe 6.2), liest den Ordner und rechnet neu. Seit T33 hält die Cache-Tabelle `cache_meta` (Migration 0005) den Stand der abgeleiteten Werte (`derived_version`, Konstante `DERIVED_CACHE_VERSION`). Ist er älter als der des Programms, liest der nächste Import jede bekannte Datei neu ein, auch unverändert, und schreibt danach den neuen Stand; die IDs der Aktivitäten bleiben erhalten. Die App liest beim Start einmal ein, wenn `Settings.cache_outdated` gilt, und erst danach gleicht sie mit Garmin ab. Eine leere Datenbank gilt nie als veraltet. Nutzertabellen bleiben unberührt. Rekonstruktion nach Datenverlust: Backup-JSON einspielen, danach Ordner einlesen.

Umsetzung in T08: `lastwerk restore` spielt die Sicherung ein, `lastwerk rebuild --dir <ordner>` baut danach den Cache aus den FIT-Dateien. Der Test `backup_restore_and_rebuild_reproduce_user_data_and_figures` (`crates/cli/tests/plan.rs`) sichert eine Datenbank mit Fixtures, Schwellen, Ziel, Verfügbarkeit, Plan und Einstellung, spielt sie in eine neue leere Datenbank ein, baut den Cache neu auf und vergleicht Nutzerdaten, Aktivitäten, Belastungen (ohne Berechnungszeitpunkt) und den Verlauf von CTL und ATL.

Neuberechnung nach geänderten Schwellen (T05): Weil der Cache keine Messpunkte hält, kann eine Schwelle nicht allein aus der Datenbank neu angewandt werden. `recompute_loads` liest die betroffenen FIT-Dateien aus dem Ordner, prüft ihre SHA-256 gegen `source_file` und ersetzt Belastung und Zeit in Zonen der Aktivitäten im Zeitraum. Die Bestleistungen hängen nicht von Schwellen ab und bleiben stehen. Das Ergebnis gleicht dem eines Imports mit der Schwelle von Anfang an (Test).

### 6.4 Duplikate (Edge und Uhr)

Zwei Aktivitäten gelten als dieselbe Fahrt, wenn alle Bedingungen zutreffen:

1. gleiche Sportart,
2. Startzeiten unterscheiden sich um höchstens 120 Sekunden (Konvention, einstellbar),
3. Dauer unterscheidet sich um höchstens 10 Prozent (Konvention, einstellbar).

Primär ist die Aktivität mit Leistungsdaten, bei Gleichstand die mit der längeren Dauer, bei weiterem Gleichstand die mit der früheren `source_file.id`. Die andere erhält `duplicate_of` und `is_primary = 0` und zählt nicht in die Tageslast. Die UI zeigt Duplikate an und erlaubt die manuelle Umkehr.

Die Schwellen sind Konvention und werden an synthetischen Paaren sowie in einer Plausibilitätsprüfung auf `local/` kontrolliert (Abschnitt 11).

Umsetzung in T04 (`crates/store/src/duplicates.rs`):

- "Dauer unterscheidet sich um höchstens 10 Prozent" bedeutet: Differenz höchstens 10 Prozent der längeren Dauer. Beispiel Testkit-Paar: 3600 s gegen 3570 s, Differenz 30 s, erlaubt sind 360 s. Beide Schwellen sind Felder von `DuplicateRule` (120 s, 0,10).
- Die Regel läuft nach jedem Import über alle Aktivitäten und ist deterministisch (unabhängig von der Importreihenfolge). Die Aktivitäten werden nach Rang geordnet (mit Leistung, längere Dauer, kleinere `source_file.id`, kleinere `id`). Jede wird primär, wenn sie keiner bereits primären Aktivität gleicht, sonst bekommt sie `duplicate_of` auf die bestplatzierte passende. Damit verschmelzen Ketten (A passt zu B, B zu C, A nicht zu C) keine verschiedenen Fahrten.
- Dateien ohne `session` und ohne `sport`-Nachricht haben die Sportart `other` und passen deshalb zu keiner Aktivität einer bestimmten Sportart.
- Die manuelle Umkehr der Primärwahl (UI) ist nicht umgesetzt; der nächste Import setzt `is_primary` und `duplicate_of` neu.

### 6.5 Plausibilitätsregeln für Messdaten

| Regel | Behandlung | Status |
|---|---|---|
| Leistung größer als konfigurierbare Obergrenze (Standard 2500 W) | Sample verwerfen, Aktivität markieren | Faustregel |
| Herzfrequenz kleiner als 30 oder größer als 230 bpm | Sample verwerfen | Faustregel |
| Zeitlücken im 1-Hz-Raster bis 10 s | Null-Leistung bei Pausen nicht interpolieren, Werte halten nur bei Herzfrequenz | Konvention |
| Zeitlücken über 10 s | Als Pause behandeln, Zeitfenster für NP nicht überbrücken | Konvention |

Verworfene Samples und Lücken werden je Aktivität gezählt und im Detail angezeigt.

---

## 7. Schemas

### 7.1 Plan (`schema/plan.schema.json`)

JSON Schema 2020-12. Texte stehen nie im Plan, nur Schlüssel und Parameter. Die Oberfläche setzt daraus Deutsch oder Englisch.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "plan.schema.json",
  "title": "Lastwerk Plan",
  "type": "object",
  "additionalProperties": false,
  "required": ["schema_version", "plan_id", "created_at", "locale", "source", "rules_version", "goal", "weeks"],
  "properties": {
    "schema_version": { "const": 1 },
    "plan_id": { "type": "string", "format": "uuid" },
    "created_at": { "type": "string", "format": "date-time" },
    "locale": { "enum": ["de", "en"] },
    "source": { "enum": ["rules", "llm"] },
    "rules_version": { "type": "string", "pattern": "^[0-9]+\\.[0-9]+\\.[0-9]+$" },
    "goal": { "$ref": "#/$defs/goal" },
    "weeks": {
      "type": "array",
      "minItems": 1,
      "maxItems": 26,
      "items": { "$ref": "#/$defs/week" }
    },
    "warnings": {
      "type": "array",
      "items": { "$ref": "#/$defs/warning" }
    },
    "explanation": { "type": "string", "maxLength": 1200 }
  },
  "$defs": {
    "sport": { "enum": ["cycling", "running", "swimming", "strength", "rest"] },
    "goal": {
      "type": "object",
      "additionalProperties": false,
      "required": ["kind"],
      "properties": {
        "kind": { "enum": ["event", "continuous"] },
        "event_date": { "type": "string", "format": "date" },
        "sport": { "$ref": "#/$defs/sport" },
        "priority": { "enum": ["A", "B", "C"] }
      },
      "if": { "properties": { "kind": { "const": "event" } } },
      "then": { "required": ["event_date"] }
    },
    "week": {
      "type": "object",
      "additionalProperties": false,
      "required": ["week_start", "phase", "target_load", "days"],
      "properties": {
        "week_start": { "type": "string", "format": "date" },
        "phase": { "enum": ["base", "build", "peak", "taper", "recovery", "race"] },
        "target_load": { "type": "number", "minimum": 0 },
        "planned_load": { "type": "number", "minimum": 0 },
        "reason_keys": { "type": "array", "items": { "type": "string", "pattern": "^[a-z0-9_.]+$" } },
        "days": { "type": "array", "minItems": 7, "maxItems": 7, "items": { "$ref": "#/$defs/day" } }
      }
    },
    "day": {
      "type": "object",
      "additionalProperties": false,
      "required": ["date", "sessions"],
      "properties": {
        "date": { "type": "string", "format": "date" },
        "sessions": { "type": "array", "maxItems": 2, "items": { "$ref": "#/$defs/session" } }
      }
    },
    "session": {
      "type": "object",
      "additionalProperties": false,
      "required": ["slot", "sport", "template_id", "duration_min", "planned_load", "hard"],
      "properties": {
        "slot": { "enum": ["am", "pm"] },
        "sport": { "$ref": "#/$defs/sport" },
        "template_id": { "type": "string", "pattern": "^[a-z]+(\\.[a-z0-9_]+)+$" },
        "duration_min": { "type": "integer", "minimum": 10, "maximum": 360 },
        "planned_load": { "type": "number", "minimum": 0 },
        "hard": { "type": "boolean" },
        "rpe": { "type": "integer", "minimum": 1, "maximum": 10 },
        "reason_keys": { "type": "array", "items": { "type": "string", "pattern": "^[a-z0-9_.]+$" } }
      }
    },
    "warning": {
      "type": "object",
      "additionalProperties": false,
      "required": ["code", "severity"],
      "properties": {
        "code": { "type": "string", "pattern": "^[A-Z]{1,3}[0-9]{2}$" },
        "severity": { "enum": ["info", "warn"] },
        "date": { "type": "string", "format": "date" }
      }
    }
  }
}
```

Der Planungshorizont von höchstens 26 Wochen ist Konvention. `planned_load` und `target_load` berechnet immer `core`, nie das LLM.

Umsetzung in T07: `warnings` steht immer im Plan (auch leer), `planned_load` je Woche setzt der Planer immer. `target_load` ist das tatsächlich verfolgte Ziel: Ist die Woche durch Verfügbarkeit, R1 oder eine Neuwahl begrenzt, steht der begrenzte Wert dort, mit Begründungsschlüssel `reason.week.capped_ctl`, `reason.week.capped_availability` oder `reason.week.target_reduced`. Begründungsschlüssel haben die Form `reason.week.*` und `reason.session.*`, ihre Texte stehen in `catalog/locales`, die Liste steht in `plan::reason_keys`. Der Termin liegt in der Rennwoche (`phase = race`). Der Tag vor dem Termin, der Termin und die Tage danach tragen keine Einheit, weil der Wettkampf keine Vorlage ist. Der Plan endet mit der Woche des Termins. Liegt der Termin hinter dem Horizont, endet der Plan früher, die letzte Woche trägt `reason.week.event_after_horizon`.

Umsetzung in T08: `schema/plan.schema.json` ist der Entwurf oben unverändert als Datei. Die CLI prüft Pläne mit der Bibliothek `jsonschema` mit eingeschalteter Formatprüfung (`uuid`, `date`, `date-time`). Negativtests entfernen jedes Pflichtfeld einzeln (Kopf, Ziel, Woche, Tag, alle sechs Pflichtfelder der Einheit) und prüfen unbekannte Felder auf allen Ebenen, ein Ziel `event` ohne `event_date`, 8 und 6 Tage, 3 Einheiten, `template_id` ohne Punkt und weitere Wertebereiche. Jeder erzeugte Plan (die beiden festen Kontexte und 300 zufällige) besteht das Schema (`crates/cli/src/schema_tests.rs`).

### 7.2 Änderungsvorschlag des LLM (`schema/plan-edit.schema.json`)

Das Modell gibt keinen vollständigen Plan zurück, sondern höchstens 14 Änderungen an einem vorhandenen Regelplan und einen kurzen Erklärungstext. Die App wendet die Änderungen an, rechnet die Last neu und validiert den Gesamtplan.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "plan-edit.schema.json",
  "title": "Lastwerk Plan-Änderung",
  "type": "object",
  "additionalProperties": false,
  "required": ["schema_version", "edits", "explanation"],
  "properties": {
    "schema_version": { "const": 1 },
    "edits": {
      "type": "array",
      "maxItems": 14,
      "items": {
        "oneOf": [
          {
            "type": "object",
            "additionalProperties": false,
            "required": ["op", "date", "slot", "template_id", "duration_min"],
            "properties": {
              "op": { "const": "replace" },
              "date": { "type": "string", "format": "date" },
              "slot": { "enum": ["am", "pm"] },
              "template_id": { "type": "string", "pattern": "^[a-z]+(\\.[a-z0-9_]+)+$" },
              "duration_min": { "type": "integer", "minimum": 10, "maximum": 360 }
            }
          },
          {
            "type": "object",
            "additionalProperties": false,
            "required": ["op", "from_date", "from_slot", "to_date", "to_slot"],
            "properties": {
              "op": { "const": "move" },
              "from_date": { "type": "string", "format": "date" },
              "from_slot": { "enum": ["am", "pm"] },
              "to_date": { "type": "string", "format": "date" },
              "to_slot": { "enum": ["am", "pm"] }
            }
          },
          {
            "type": "object",
            "additionalProperties": false,
            "required": ["op", "date", "slot"],
            "properties": {
              "op": { "const": "remove" },
              "date": { "type": "string", "format": "date" },
              "slot": { "enum": ["am", "pm"] }
            }
          }
        ]
      }
    },
    "explanation": { "type": "string", "maxLength": 1200 }
  }
}
```

Zusätzlich zur Schema-Prüfung gilt: `template_id` muss im Katalog stehen und zur Sportart des Slots passen, `duration_min` muss im Bereich der Vorlage liegen, und die Verfügbarkeit des Tages darf nicht überschritten werden.

Umsetzung in T15 (Stand 13. Oktober 2026): `schema/plan-edit.schema.json` ist der Entwurf oben unverändert als Datei, eingebettet per `include_str!` in `lastwerk-coach` (Prüfung mit `jsonschema`, Formatprüfung für `date`). Bedeutung der Operationen: `replace` braucht eine vorhandene Einheit im Slot (die Sportart der Einheit ist die Sportart des Slots und ändert sich nicht) und setzt Vorlage und Dauer, `move` setzt eine vorhandene Einheit in einen freien Slot (auch an einem anderen Tag, `from == to` ist ohne Wirkung), `remove` streicht die Einheit. Es gibt keine Operation, die einen leeren Slot neu belegt. Änderungen gelten der Reihe nach. Jeder Fehler wird zu einem Code ohne Freitext: `NOT_JSON`, `SCHEMA` (mit JSON-Pfad und Schlüsselwort), `UNKNOWN_TEMPLATE`, `SPORT_MISMATCH`, `DURATION_OUT_OF_WINDOW`, `UNKNOWN_DATE`, `NO_SESSION`, `SLOT_OCCUPIED` und die Regelcodes `R01` bis `R12` (mit Datum und den Parametern aus `core`). Die Verfügbarkeit prüft R11 in `validate_plan`. Eine Erklärung über 1200 Zeichen wird vor der Schemaprüfung auf 1200 Zeichen gekürzt und ist kein Fehler (Konvention: die Länge betrifft nur die Anzeige, ein Korrekturaufruf dafür wäre verschwendet); alles andere am Schema ist ein Fehler.

### 7.3 Erholungstag (`schema/recovery-day.schema.json`)

Feldnamen vorläufig, endgültig in M4a nach der Sonde mit der tatsächlichen Ausgabe der Bibliothek.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "recovery-day.schema.json",
  "type": "object",
  "additionalProperties": false,
  "required": ["schema_version", "date", "source", "fetched_at"],
  "properties": {
    "schema_version": { "const": 1 },
    "date": { "type": "string", "format": "date" },
    "hrv_overnight_ms": { "type": "number", "minimum": 0, "maximum": 400 },
    "resting_hr_bpm": { "type": "number", "minimum": 20, "maximum": 120 },
    "sleep_duration_s": { "type": "integer", "minimum": 0, "maximum": 86400 },
    "sleep_score": { "type": "integer", "minimum": 0, "maximum": 100 },
    "body_battery_high": { "type": "integer", "minimum": 0, "maximum": 100 },
    "body_battery_low": { "type": "integer", "minimum": 0, "maximum": 100 },
    "source": { "type": "string", "maxLength": 64 },
    "fetched_at": { "type": "string", "format": "date-time" }
  }
}
```

Die Wertebereiche sind Plausibilitätsgrenzen (Faustregel), keine physiologischen Aussagen.

Änderung in T22 (Stand 15. Oktober 2026): `body_battery_high` und `body_battery_low` sind neue, **optionale** Felder (Höchst- und Tiefstwert des Tages). `schema_version` bleibt 1, weil jede bisher gültige Datei weiter gültig ist und das Zusatzprogramm die Felder nicht schreibt. Die App schreibt `recovery_day` direkt und nicht über Dateien dieses Schemas (5.6), `import-recovery` nimmt die Felder aber an.

Umsetzung in T14 (Stand 13. Oktober 2026): `schema/recovery-day.schema.json` ist der Entwurf oben unverändert als Datei (mit `title` und `description`). `docs/garmin-feldabdeckung.md` aus L1 liegt nicht vor, deshalb gilt der Entwurf und ist **vorläufig** (O9). Die Datei eines Tages heißt `YYYY-MM-DD.json` und trägt dasselbe Datum in `date`. Die Werte außer `schema_version`, `date`, `source` und `fetched_at` sind optional: ein Tag ohne HRV hat die übrigen Werte, ein Tag ohne jeden Wert bekommt keine Datei. `source` ist `garmin-bridge <Version>`, `fetched_at` der jüngste Abrufzeitpunkt der benutzten Quellen laut Manifest (UTC mit `Z`). Welche Antwortfelder der Bibliothek die Werte liefern, steht in `FIELD_MAP` (`bridge/src/lastwerk_bridge/normalize.py`) und ist bis L1 eine Annahme.

Änderung vom 14. Oktober 2026 (Integrationsprüfung): `normalize` bricht mit Exit-Code 2 und der Meldung "Schemadatei fehlt" ab, wenn `schema/recovery-day.schema.json` nicht vorhanden oder nicht lesbar ist, und zwar vor dem ersten Schreiben. Bisher wurden die Erholungsdateien in diesem Fall ohne Schemaprüfung geschrieben, obwohl die Prüfung die Grenze zu Lastwerk ist (10.2).

### 7.4 Vorlagenkatalog (`catalog/templates.json`)

Jede Vorlage ist eine feste, von Hand geschriebene Struktur. Das LLM wählt nur die ID und die Dauer.

```json
{
  "id": "bike.threshold.2x20",
  "sport": "cycling",
  "intensity_class": "hard",
  "duration_min": { "min": 60, "max": 120 },
  "steps": [
    { "kind": "warmup",   "minutes": 15, "zone_scheme": "power7", "zone": 2 },
    { "kind": "interval", "reps": 2, "minutes": 20, "zone_scheme": "power7", "zone": 4, "rest_minutes": 5 },
    { "kind": "cooldown", "minutes": 10, "zone_scheme": "power7", "zone": 1 }
  ],
  "text_key": "template.bike.threshold.2x20"
}
```

Startkatalog (Konvention, ohne Behauptung über Wirksamkeit einzelner Einheiten):

| ID | Sport | Klasse | Inhalt |
|---|---|---|---|
| bike.endurance.steady | Rad | easy | Gleichmäßig in Zone 2 |
| bike.recovery.spin | Rad | easy | Locker in Zone 1 |
| bike.tempo.blocks | Rad | moderate | Blöcke in Zone 3 |
| bike.threshold.2x20 | Rad | hard | 2 mal 20 Minuten in Zone 4 |
| bike.vo2.5x4 | Rad | hard | 5 mal 4 Minuten in Zone 5 |
| run.easy.steady | Lauf | easy | Gleichmäßig locker |
| run.long.steady | Lauf | moderate | Langer Lauf, gleichmäßig |
| run.tempo.continuous | Lauf | hard | Dauerlauf an der Schwelle |
| run.intervals.6x3 | Lauf | hard | 6 mal 3 Minuten schnell |
| swim.technique.easy | Schwimmen | easy | Technik und lockeres Schwimmen |
| swim.threshold.sets | Schwimmen | hard | Serien an der kritischen Geschwindigkeit |
| strength.general.a | Kraft | moderate | Allgemeines Programm A, Zeit und RPE |
| strength.general.b | Kraft | moderate | Allgemeines Programm B, Zeit und RPE |
| rest.day | Ruhe | easy | Ruhetag |

Die genauen Strukturen und Dauern entstehen in M3. Die Klasse `hard` ist Voraussetzung für die Leitplanken R3 und R4.

Umsetzung in T07: Der Katalog hat das Format `{ format_version, templates }`. Schritte sind `warmup`, `steady`, `interval` (`reps`, `minutes` je Wiederholung, `rest_minutes` zwischen den Wiederholungen, die Pause zählt als Zone 1) und `cooldown`. Ausdauervorlagen tragen je Schritt `zone_scheme` (`power7` Rad, `run_speed5` Lauf, `swim_css5` Schwimmen) und `zone`. Kraftvorlagen haben keine Zonen, dafür `rpe` je Schritt und `rpe` der Einheit (6, Konvention). Die Schritte beschreiben die Struktur bei der Nenndauer. Bei anderer Dauer bleiben die Intervallblöcke gleich, die übrigen Schritte werden im Verhältnis gedehnt oder gestaucht (Konvention). Deshalb ist die Mindestdauer nie kürzer als die Intervallblöcke (Prüfung in `TemplateCatalog::validate`). Dauern und Strukturen sind Konvention, nicht belegt.

`rest.day` steht im Katalog für die Anzeige, hat Dauer 0 bis 0 und keine Schritte. Das Schema verlangt für jede Einheit mindestens 10 Minuten, deshalb plant der Planer nie eine Einheit mit `rest.day`. Ein Ruhetag ist ein Tag mit `sessions: []`.

Last einer Einheit (Faustregel, Parameter `load_model` in `rules.default.json`): `planned_load = Stunden * IF^2 * 100` mit IF je Klasse easy 0,65, moderate 0,8, hard 0,95. Kraft rechnet wie in 8.1 mit `Stunden * (RPE/10)^2 * 100`. Die Klassenwerte sind eine grobe Abbildung und ersetzen weder die Zonenzeit noch die Methoden aus 8.1.

### 7.5 Texte und Sprachen

Alle Anzeigetexte stehen in `catalog/locales/de.json` und `en.json`, Schlüssel gleichlautend. Ein Test stellt sicher, dass beide Dateien dieselben Schlüssel haben und keine Schlüssel im Code fehlen. Die Erklärung eines LLM wird in der gewählten Sprache erzeugt, als "KI-Text" gekennzeichnet und nie als Rechengrundlage verwendet.

---

## 8. Berechnungen und Leitplanken

### 8.1 Belastung je Aktivität

| Sportart | Methode | Formel | Quelle |
|---|---|---|---|
| Rad mit Leistung | TSS | `NP` = vierte Wurzel des Mittelwerts der vierten Potenzen der 30-Sekunden-Gleitmittelwerte. `IF = NP / FTP`. `TSS = t_s * NP * IF / (FTP * 3600) * 100` | Q16, aus Sekundärquellen, Buch nicht eingesehen |
| Rad ohne Leistung, Lauf mit Herzfrequenz | hrTSS | `hrTSS = TRIMP(Aktivität) / TRIMP(1 h bei LTHR) * 100` | TRIMP: Q17 (Sekundärquelle). Normierung auf 100 pro Stunde bei LTHR: Konvention |
| Lauf mit Geschwindigkeit | rTSS | `IF = v / v_Schwelle`, `rTSS = t_h * IF^2 * 100` | Faustregel, ohne Steigungskorrektur |
| Schwimmen | sTSS | `IF = v / CSS`, `sTSS = t_h * IF^3 * 100` | Faustregel. CSS: Q15 |
| Kraft | Session-RPE | `AU = RPE * t_min`. Für den Verlauf: `L = t_h * (RPE/10)^2 * 100` | AU: Q4. Umrechnung auf die Lastskala: Faustregel, einstellbar |
| Sonstige mit Herzfrequenz | hrTSS | wie oben | Konvention (T01): einzige sportartunabhängige Methode. Ohne Herzfrequenz nicht berechenbar |

Methodenwahl (Konvention, T01): Die Methode folgt den vorhandenen Messkanälen, bei mehreren Kanälen gilt Leistung vor Herzfrequenz (Rad) und Geschwindigkeit vor Herzfrequenz (Lauf). Fehlt die Schwelle der so bestimmten Methode, meldet die Berechnung `missing_threshold` und wechselt nicht still auf eine andere Methode. NP zählt nur volle 30-Sekunden-Fenster, es gibt kein verkürztes Anfangsfenster. Die Dauer umfasst kurze Lücken (bis 10 s) und schließt Pausen aus (6.5).

Garmins Last (seit T39, Entscheidung E2 vom 16. Oktober 2026): Trainingslast und Trainingseffekt, die Garmin je Aktivität meldet, und Garmins akute Last aus der Trainingsbereitschaft zeigt die App nur zusätzlich und gekennzeichnet an. Sie gehen in keine Rechnung ein, weder in die Tageslast noch in CTL, ATL, TSB oder den Planer.

TRIMP nach Banister, Parameter nach Sekundärliteratur (Original vor Implementierung prüfen):

```
x     = (HR - HR_rest) / (HR_max - HR_rest)
TRIMP = Summe über Minuten:  dt_min * x * a * exp(b * x)
        Männer:  a = 0,64   b = 1,92
        Frauen:  a = 0,86   b = 1,67
```

Die Konstanten sind in `LoadConfig` abgelegt. Fehlt die Angabe zum Geschlecht, meldet die Berechnung `missing_threshold`. Es wird kein Mittelwert der beiden Parametersätze angenommen.

Hinweis zu rTSS und sTSS: Die Formeln sind in der Trainingspraxis verbreitet, aber nicht aus einer begutachteten Studie belegt. Sie sind deshalb als Faustregel geführt und in der UI an der Last als "geschätzt" gekennzeichnet.

Hinweis zu Kraft: Für Krafttraining liegt in FIT-Dateien meist nur Dauer und Herzfrequenz vor. Die Herzfrequenz bildet Krafteinheiten schlecht ab. Daher ist RPE (Nutzereingabe) der Standard, die Umrechnung auf die Lastskala ist Faustregel.

### 8.2 Fitness, Ermüdung, Form

Modellgrundlage: Impuls-Antwort-Modell von Banister und Kollegen (Q1, Q2, Q3), in der Praxis als exponentiell gewichtete Mittelwerte der Tageslast angewandt.

```
CTL_t = CTL_{t-1} + (L_t - CTL_{t-1}) * (1 - exp(-1 / tau_ctl))      tau_ctl = 42 Tage
ATL_t = ATL_{t-1} + (L_t - ATL_{t-1}) * (1 - exp(-1 / tau_atl))      tau_atl = 7 Tage
TSB_t = CTL_{t-1} - ATL_{t-1}
```

- Zeitkonstanten 42 und 7 Tage: Konvention der Praxis (Sekundärquellen). Das Originalmodell berichtet 50 und 15 Tage (Q2, laut Suchtreffer, Seitenangabe nicht geprüft). Beide Parametersätze sind einstellbar, der Standard ist 42 und 7.
- `TSB` mit den Werten des Vortags ist Konvention. Es entspricht der gängigen Anzeige "Form am Morgen".
- Startwerte: CTL0 und ATL0 sind einstellbar und stehen im Backup. Standard: 0 am ersten Tag mit Daten, mit Hinweis, dass die ersten Wochen nicht belastbar sind.

Handgerechnete Referenz (konstante Tageslast L = 100, Start bei 0, Taschenrechner):

| Tag | CTL | ATL | TSB |
|---|---|---|---|
| 1 | 2,3528 | 13,3122 | 0,0000 |
| 2 | 4,6503 | 24,8523 | -10,9594 |

Nebenrechnung: `1 - exp(-1/42) = 0,023528`, `1 - exp(-1/7) = 0,133122`.
Tag 1: `CTL = 0 + 100 * 0,023528 = 2,3528`, `ATL = 0 + 100 * 0,133122 = 13,3122`, `TSB = 0 - 0 = 0`.
Tag 2: `CTL = 2,3528 + (100 - 2,3528) * 0,023528 = 4,6503`, `ATL = 13,3122 + (100 - 13,3122) * 0,133122 = 24,8523`, `TSB = 2,3528 - 13,3122 = -10,9594`.
Toleranz im Test: 1e-3 (Rundung der Handrechnung).

Umsetzung in T05: Tage ohne Last zählen als 0, der Verlauf beginnt in der CLI am ersten Tag mit zählender Last (Startwerte `ctl0` und `atl0`, Standard 0). Die ersten Wochen sind nicht belastbar, die CLI weist darauf hin, wenn der Ausgabezeitraum innerhalb von `tau_ctl` Tagen nach diesem Tag beginnt. Referenzwerte: T-PMC-1 (Tag 1 und 2 aus der Tabelle) und T-PMC-2 (`50 * e^-1 = 18,3940`, Toleranz 1e-3) als Tests in `core`. Dazu die Eigenschaft, dass konstante Last `L` CTL und ATL monoton gegen `L` laufen lässt.

### 8.3 Planer

Eingaben: aktuelle CTL und ATL, Wochenhistorie je Sportart, Ziel (Termin oder fortlaufend), Verfügbarkeit je Wochentag, Katalog, Regelparameter.

Ablauf (deterministisch, ohne Zufall; Gleichstände nach Katalogreihenfolge):

1. **Phasen.** Mit Termin: rückwärts vom Termin Taper, Spitze, Aufbau, Basis. Ohne Termin: Blöcke aus 3 Belastungswochen und 1 Erholungswoche (Faustregel). Phasenlängen stehen in `rules.default.json`.
2. **Wochenziel.** Zielbelastung je Woche aus der Rampenvorgabe (Regel R1) und der Erholungswoche (R6), begrenzt durch die Verfügbarkeit.
3. **Wochenstruktur.** Ruhetag (R5), harte Tage (R3, R4), Langeinheit auf den verfügbaren Tag mit der größten Zeit, Krafteinheiten (R7).
4. **Vorlagenwahl.** Je Slot aus dem Katalog nach Sportart, Klasse und Dauerfenster. Dauer wird so gewählt, dass die Wochenlast dem Ziel nahekommt.
5. **Vorausberechnung.** CTL, ATL, TSB je Tag der Planung mit `pmc`.
6. **Validierung** (Abschnitt 8.4). Verletzte harte Regeln führen zu Neuwahl in Schritt 3 bis 4. Gelingt das nicht, erhält der Plan eine Warnung statt einer Verletzung.
7. **Begründungsschlüssel** je Woche und je Einheit.

Das Ergebnis ist ein Plan nach `plan.schema.json` mit `source = "rules"`.

Umsetzung in T07 (Konventionen, alle Werte in `rules.default.json`):

- **Phasen.** Mit Termin: Rennwoche (eine Woche), `taper_weeks` (1), `peak_weeks` (2), `build_weeks` (4), davor Basis. Der Taper reicht vom Montag der ersten Taperwoche bis zum Termin und ist damit 7 bis 13 Tage lang (R9 erlaubt 7 bis 14). `Rules::validate` lehnt Werte ab, die R9 verletzen würden. Fortlaufend: Belastungswochen mit Phase `base`, nach `R06.load_weeks` Belastungswochen eine Woche `recovery`. Vor Taper oder Rennwoche entfällt die Erholungswoche, der Taper wirkt als solche.
- **Wochenziel.** Startwert ist die Last der Woche vor `week_start` aus der Historie, ohne Historie die CTL-äquivalente Wochenlast `7 * ctl0`, mindestens `structure.min_week_load` (120, Konvention). Belastungswochen wachsen um `weekly_growth` (base und build 7 Prozent, peak 3 Prozent, Faustregel, bewusst unter der R2-Warnschwelle). Erholungswoche 65 Prozent, Taperwoche 70 Prozent, Rennwoche 50 Prozent der letzten Belastungswoche (Faustregel, Zahlen zum Taper nur aus Sekundärquellen, O8). Obergrenze aus R1: Bei gleichmäßiger Tageslast `L` steigt die CTL in sieben Tagen um `(L - CTL) * (1 - exp(-7 / tau_ctl))`, daraus folgt die höchste Wochenlast `7 * (CTL + Grenze * planning_margin / (1 - exp(-7 / tau_ctl)))` (`planning_margin` 0,9, Konvention).
- **Wochenstruktur.** Reihenfolge: nutzbare Tage (kein fester Ruhetag, genug Zeit), Ruhetag nach R5 (fehlt einer, wird der nutzbare Tag mit der wenigsten Zeit frei), Langeinheit am nutzbaren Tag mit der meisten Zeit (Gleichstand: der spätere Tag) in der Hauptsportart, harte Tage (nur an `allow_hard`-Tagen, nie an Folgetagen, auch nicht über die Wochengrenze, bevorzugt nicht neben der Langeinheit, mit größtem Abstand), mittlere Einheiten, Kraft (nicht am Tag vor einer harten Einheit, auch nicht am Sonntag vor einem harten Montag), übrige Tage leicht. Harte und mittlere Einheiten je Phase stehen unter `phases.<phase>`. In der Rennwoche liegen harte Einheiten mindestens `race_opener_min_days_before` (3) Tage vor dem Termin. Sportarten verteilt das D'Hondt-Verfahren nach `sports[].weight`, Gleichstand nach Reihenfolge in `sports`.
- **Vorlagenwahl und Dauer.** Aus Sportart und Klasse die Vorlage, deren Dauerfenster die Zieldauer enthält: bei der Langeinheit (Klassen leicht und mittel) das größte Fenster, bei harten Einheiten reihum je Sportart, sonst das engste Fenster. Enthält keines die Zieldauer, gewinnt das nächstliegende Fenster, Gleichstand nach Katalogreihenfolge. Die Zieldauer verteilt die Wochenlast (ohne Kraft) im Verhältnis der Gewichte, die Langeinheit zählt `structure.long_weight` (2) mal. Dauern liegen auf Vielfachen von 5 Minuten, begrenzt durch Fenster und Tagesbudget, und werden in 5-Minuten-Schritten dem Ziel angenähert. Kraft hat die Nenndauer der Vorlage. Reicht das Wochenziel nicht für alle Einheiten, entfallen zuerst leichte, dann mittlere Einheiten, Kraft und harte Einheiten, die Langeinheit bleibt.
- **Neuwahl.** Verletzt die Woche eine harte Regel oder eine vermeidbare weiche (R2, R7, R8 außer in Taper und Rennwoche, R10), probiert der Planer bis zu `generator.max_retries` Fassungen: Wochenziel in Schritten von 5 Prozent verkleinern (nur bei R2 und R10 hilfreich, bis zu 8 Schritte), dann die mittlere Einheit streichen, dann harte Einheiten durch leichte ersetzen. Gelingt keine Fassung ohne harte Verletzung, bleibt die Woche leer (`reason.week.fallback_rest`) und der Plan trägt eine Warnung mit dem Code der verletzten Regel. Verbleiben nur weiche Verletzungen, gilt die Fassung mit den wenigsten, sie erscheinen als Warnungen.
- **Abweichung vom Wortlaut (14. Oktober 2026).** Die Abwurfreihenfolge "zuerst leichte, dann mittlere Einheiten, Kraft und harte Einheiten" führt wörtlich angewandt zu einer leeren Woche, wenn eine harte Einheit schon allein mehr Last braucht, als das Wochenziel hergibt: Erst fallen alle leichten Einheiten, dann die harte, und die leere Woche hat keine Verletzung und wird angenommen. Das traf regelmäßig die Rennwoche (Ziel 50 Prozent der letzten Belastungswoche, eine harte Radeinheit braucht mindestens 60 min bei IF 0,95, also 90,25 Last). Deshalb gilt vor der Reihenfolge: Jede Einheit außer der Langeinheit, deren Mindestlast zusammen mit der Mindestlast der Langeinheit das Wochenziel übersteigt, entfällt zuerst, weil sie ohnehin nicht bleiben kann. Danach gilt die Reihenfolge wie beschrieben. Bleiben weniger harte Einheiten als die Fassung vorsieht, trägt die Woche `reason.week.hard_reduced`. So endet eine Woche mit positivem Ziel nur leer, wenn schon die kürzeste Einheit über dem Ziel liegt; sie trägt dann `reason.week.fallback_rest`, aber keine Warnung, weil keine Regel verletzt ist (Warnungen tragen den Code einer Regel). "Locker nach einer harten Einheit" (`reason.session.recovery_after_hard`) steht nur hinter einem Tag, an dem die harte Einheit auch geplant bleibt. Die Neuwahl verkleinert das Wochenziel nur, solange R2 oder R10 verletzt ist; bei anderen vermeidbaren weichen Verletzungen (R7, R8) folgt gleich die nächste Fassung. Vorher lief die Verkleinerung in jedem Fall durch, eine R8-Warnung wurde so mit einem um bis zu 40 Prozent kleineren Ziel "behoben", das alle Folgewochen mitzog, und mit dem Vorziehen der harten Einheiten hätten verkleinerte Fassungen ohne harte Einheit gewonnen.

### 8.4 Leitplanken

Härte: **hart** bedeutet, ein Plan, der die Regel verletzt, wird abgelehnt. **weich** bedeutet Warnung. Alle Parameter liegen in `catalog/rules.default.json`.

| ID | Regel | Standardwert | Härte | Quelle |
|---|---|---|---|---|
| R1 | Anstieg der CTL je Woche | höchstens 5 Punkte (Einstellung bis 8) | hart | Faustregel, in der Praxis (TrainingPeaks-Umfeld) verbreitet, nicht begutachtet |
| R2 | Anstieg des Wochenvolumens gegenüber der Vorwoche | Warnung ab 10 Prozent, Ablehnung ab 30 Prozent | weich ab 10, hart ab 30 | "10-Prozent-Regel": Faustregel. Studien zeigen keinen belastbaren Schutzeffekt (Q12, Q13). Schwelle 30 Prozent: Hinweis aus Q12 (nicht signifikant, p = 0,07), deshalb als Vorsichtswert |
| R3 | Harte Einheiten je Woche | höchstens 2 | hart | Faustregel. Beschreibende Beobachtung: etwa 20 Prozent der Einheiten hochintensiv bei Ausdauerathleten (Q5) |
| R4 | Harte Einheiten an aufeinanderfolgenden Tagen | nicht erlaubt | hart | Faustregel |
| R5 | Ruhetage je Woche | mindestens 1 | hart | Faustregel |
| R6 | Erholungswoche nach Belastungswochen | nach 3 Belastungswochen eine Woche mit 60 bis 70 Prozent des Zielvolumens | weich | Faustregel. Verhältnis 3 zu 1 verbreitet, nicht belegt |
| R7 | Krafteinheiten je Woche | 1 bis 2, nicht am Tag vor einer harten Einheit, in der Taper-Phase höchstens 1 | weich | Faustregel |
| R8 | Anteil leichter Einheiten | mindestens 70 Prozent der Trainingszeit in Zone 1 und 2 | weich | Q5 (beschreibende Beobachtung etwa 80 Prozent), Grenzwert 70 Prozent: Faustregel |
| R9 | Taper vor Wettkampf | 7 bis 14 Tage, Volumen sinkt, Intensität bleibt | hart (Länge) | Q10 (Übersicht, Prinzip). Dauer und Volumenreduktion nur aus Sekundärbericht zu Q11. Dauer 7 bis 14 Tage: Faustregel |
| R10 | Form (TSB) vorausberechnet | Warnung unter -30 | weich | Faustregel, keine begutachtete Quelle |
| R11 | Verfügbarkeit | Tageszeit je Wochentag nie überschreiten | hart | Nutzereingabe |
| R12 | Erholungsdaten | Bei HRV deutlich unter individueller Basislinie oder Schlaf kürzer als Basislinie: nächste harte Einheit durch lockere ersetzen | weich | Q6 (7-Tage-Mittelwert als Vorschlag nur aus Sekundärbericht). Schwellen: Faustregel. Ab M4d |

Bewusst keine Leitplanke:

- **ACWR (Verhältnis akuter zu chronischer Last).** Gabbett (Q8) führte das Verhältnis als Verletzungsindikator ein, Impellizzeri und Kollegen (Q9) kritisieren fehlende kausale Belegbarkeit. Lastwerk zeigt CTL, ATL und TSB, aber verwendet kein ACWR für Ablehnung von Plänen.
- **Verletzungs- oder Überlastungsprognose.** Es gibt keine belastbare Methode, die dieses Werkzeug umsetzen könnte.

Jeder Plan listet verletzte weiche Regeln als `warnings` mit Code `R**`. Der Code ist die Regelnummer, die UI schlägt Text und Quelle nach.

Umsetzung in T07 (Messgrößen und Konventionen). Härte, Quelle, Verweise und Parameter jeder Regel stehen in `rules.default.json` (`hardness`, `source`, `references`, `note`), `validate_plan` liest die Härte von dort. R2 hat zwei Stufen (`warn_at`, `reject_at`), R9 ist hart für die Länge und weich für das Volumen.

- R1: CTL am letzten Tag der Woche minus CTL am Ende der Vorwoche (erste Woche: `ctl0`), aus den geplanten Lasten mit `pmc` vorausberechnet, Toleranz 1e-3.
- R2: "Volumen" ist die Trainingszeit in Minuten aller Einheiten, im Plan wie in der Historie (`week_history` zählt die Zeit aller primären Aktivitäten, auch ohne gültige Last). Bezug ist die Vorwoche (erste Woche: die Woche vor `week_start` in der Historie), nach einer Erholungswoche die letzte Woche davor, sonst wäre jede Rückkehr zum normalen Umfang eine Verletzung. Unter `min_reference_minutes` (60) Vorwochenzeit gilt R2 nicht, weil dann schon eine kurze Einheit die Grenze überschreitet. Die Grenzen gelten "ab": genau 10 Prozent warnt, genau 30 Prozent lehnt ab.
- R3 zählt Einheiten mit `hard = true`. R4 prüft Tage mit mindestens einer harten Einheit auf Folgetagen, auch über Wochengrenzen. R5: Ein Ruhetag ist ein Tag ohne Einheit.
- R6: Eine Belastungswoche (base, build, peak) über `load_weeks` hinaus warnt, außer Taper oder Rennwoche folgt (am Planende ungeprüft). Die Erholungswoche liegt relativ zur geplanten Last der Belastungswoche davor bei 60 bis 70 Prozent, mit Toleranz von 5 Prozentpunkten wegen der Rundung auf ganze Einheiten.
- R7: Die Untergrenze 1 gilt nur bei `strength_per_week > 0` und nicht in der Rennwoche. Die Obergrenze ist 2, in Taper und Rennwoche 1.
- R8: Anteil der Zeit in Zone 1 und 2 an der Trainingszeit mit Zonen (Kraft zählt nicht), aus den Schritten der Vorlage (`Template::easy_zone_minutes`). Pausen zwischen Intervallen zählen als Zone 1.
- R9: Der Taper reicht vom Montag der ersten Taperwoche bis zum Termin. Beginnt der Plan in der Rennwoche, ist der Taper vor dem Plan angelaufen und wird nicht geprüft. Weich: Die geplante Last der Taperwochen und der Rennwoche liegt unter der Woche vor dem Taper. "Intensität bleibt" prüft die Regel nicht, das Phasenprofil des Planers sichert sie.
- R10: Eine Meldung je Woche, am Tag mit dem tiefsten vorausberechneten TSB unter -30.
- R11: Tageszeit, feste Ruhetage, harte Einheiten nur an `allow_hard`-Tagen und höchstens 2 Einheiten je Tag.

Umsetzung in T14 (R12, Erholungsdaten). Die Regel ist weich, ihre Quelle ist Q6 (HRV-Überwachung; die Basislinie als 7-Tage-Mittel nur aus einem Sekundärbericht, O8), alle Schwellen sind Faustregeln und Parameter in `rules.default.json`:

- Eingang ist `PlanContext::recovery`. Betrachtet wird der jüngste Erholungstag `D` vor `week_start`, höchstens `max_age_days` (3) Tage davor, sonst ist er veraltet und die Regel schweigt. Basislinie je Größe ist das Mittel der Werte der `baseline_days` (7) Tage vor `D` (ohne `D`), sie braucht mindestens `min_baseline_days` (4) Tage mit Wert, sonst gibt es kein Signal. Ruhepuls und Schlafwert fließen nicht ein.
- Signal, wenn die HRV (`hrv_overnight_ms`) um mindestens `hrv_drop_fraction` (0,15) oder die Schlafdauer (`sleep_duration_s`, 0 zählt als fehlend) um mindestens `sleep_deficit_fraction` (0,20) unter ihrer Basislinie liegt; die Grenzen gelten "ab" wie bei R2. Handrechnung der Tests: Basislinie 60 ms, Wert 51 ms, `(60 - 51) / 60 = 0,15` löst aus, 52 ms (0,133) nicht; Basislinie 25 200 s, Wert 20 160 s, `5 040 / 25 200 = 0,2` löst aus, 20 400 s (0,19) nicht.
- Wirkung im Planer: Die erste harte Einheit in Planreihenfolge wird durch eine lockere Vorlage derselben Sportart ersetzt, deren Dauerfenster die Dauer der Einheit enthält (das engste Fenster gewinnt, Gleichstand nach Katalogreihenfolge). Die Dauer bleibt gleich, damit Verfügbarkeit (R11) und Wochenvolumen (R2) unberührt bleiben; `hard` wird `false`, `planned_load` der Einheit und der Woche folgen der neuen Vorlage, `reason_keys` der Einheit ist `["reason.session.recovery_swap"]`. Der Plan trägt die Warnung `R12` mit Stufe `info` und dem Datum der ersetzten Einheit. Gibt es keine harte Einheit oder keine passende lockere Vorlage, bleibt der Plan unverändert. Der Austausch geschieht nach dem Aufbau aller Wochen und senkt nur Lasten, er kann keine harte Regel verletzen (Eigenschaftstest über 200 Fälle).
- `validate_plan` meldet `R12` als weiche Verletzung (Datum der Einheit, `detail`: `recovery_date`, `hrv_drop`, `sleep_deficit`), wenn bei Signal die erste Einheit, die hart ist oder von R12 ersetzt wurde, noch hart ist. Eine ersetzte Einheit erkennt die Prüfung am Begründungsschlüssel.
- Ohne Erholungsdaten ändert R12 nichts. Die Quelle der Werte ist die Datenbank (`import-recovery`), `plan` und `validate` der CLI füllen die Liste daraus.

### 8.5 Zonen

Zonentabellen sind daten- und konfigurationsgetrieben (`catalog/zones.json`), nicht fest verdrahtet.

| Schema | Bezug | Zonen | Quelle |
|---|---|---|---|
| power7 | FTP | 7 Zonen nach Coggan (Zone 1 aktive Erholung bis Zone 7 neuromuskuläre Leistung), Grenzen als Prozent der FTP | Q16, Grenzwerte aus Sekundärliteratur und vor Übernahme am Original zu prüfen |
| hr_lthr5 | LTHR | Fünf bis sieben Zonen als Prozent der Schwellenherzfrequenz | Faustregel der Trainerliteratur, nicht geprüft |
| hr_karvonen | HR_max, HR_rest | Anteile der Herzfrequenzreserve | Q14 (Konzept der Reserve), Anteile: Faustregel |
| run_speed5 | Schwellengeschwindigkeit | Prozent der Schwellengeschwindigkeit | Faustregel |
| swim_css5 | CSS | Prozent der CSS | Q15 (CSS), Prozentgrenzen Faustregel |

Fehlende `HR_max`: Rückfall auf `208 - 0,7 * Alter` (Q7). Die Abweichung im Einzelfall ist groß (Sekundärquelle nennt etwa 10 Schläge pro Minute Standardfehler). Die UI kennzeichnet den Wert als Schätzung.

Umsetzung in T05: `catalog/zones.json` hat `format_version` und eine Liste `schemes`. Jedes Schema trägt `id`, `basis` (`ftp`, `lthr`, `hr_reserve`, `run_threshold_speed`, `css`), `sports`, `bounds` (obere Grenzen als Faktor des Bezugswerts, die letzte Zone ist offen), `source` (Quellennummer aus Anhang A oder `Faustregel`), `status` (`Sekundärquelle` oder `Faustregel`) und `note`. Zonen zählen ab 1, obere Grenze einschließlich. Die Zonennamen stehen in `catalog/locales/de.json` und `en.json` unter `zones.<id>.name` und `zones.<id>.z<n>`.

| Schema | Grenzen (obere, Anteil des Bezugswerts) | Status |
|---|---|---|
| power7 | 0,55, 0,75, 0,90, 1,05, 1,20, 1,50 (Coggan, Q16) | Sekundärquelle |
| hr_lthr5 | 0,81, 0,89, 0,93, 0,99 (angelehnt an Friel, Q22) | Faustregel |
| hr_karvonen | 0,60, 0,70, 0,80, 0,90 der Herzfrequenzreserve (Q14 nur für das Konzept) | Faustregel |
| run_speed5 | 0,75, 0,85, 0,95, 1,05, vom Projekt gewählt | Faustregel |
| swim_css5 | 0,85, 0,95, 1,05, 1,15 (Q15 nur für den Begriff CSS) | Faustregel |

Alle Grenzen sind vor einer Freigabe am Original zu prüfen (O8). `power7` gilt für Rad, `run_speed5` für Lauf, `swim_css5` für Schwimmen, die beiden Herzfrequenzschemata für alle Sportarten mit Herzfrequenz. Die Zeit in Zonen wird beim Import mit den Schwellen vom Tag der Aktivität gerechnet.

### 8.6 Schätzungen von Schwellenwerten

**Schwellen aus Garmin (seit T39, Entscheidung E1 vom 16. Oktober 2026).** Für Werte, die Garmin liefert, übernimmt Lastwerk sie beim Abgleich ohne Rückfrage in `threshold` mit Herkunft `garmin`. Das ersetzt für diese Werte die Regel "nur als Vorschlag" unten.

- Datum: Das Datum aus Garmin (`calendarDate` bei FTP und Laktatschwelle). Werte ohne Datum bei Garmin (maximale Herzfrequenz und Ruhepuls aus den Herzfrequenzzonen) bekommen beim ersten Auftreten den ersten Tag des Abgleichfensters, bei einer späteren Änderung den Tag des Abgleichs (Konvention, damit frühere Aktivitäten im Fenster eine Last bekommen; vom Nutzer am 16. Oktober 2026 bestätigt, Entscheidung E5 in T40).
- Zuordnung: FTP gilt für Rad. Die Laktatschwellen-Herzfrequenz misst Garmin im Laufen (Docstring der Bibliothek, Q27) und gilt deshalb nur für Lauf; `heartRateCycling` gilt für Rad. Maximale Herzfrequenz und Ruhepuls gelten für alle Sportarten, wenn alle Zonenprofile denselben Wert nennen, sonst je Profil mit dem Namen einer Sportart (`RUNNING`, `CYCLING`, `SWIMMING`) und das Profil `DEFAULT` für alle (Annahme zu den Namen, Livetest O32). Das Schwellentempo (seit T40) ist Garmins Laktatschwellen-Geschwindigkeit im Laufen: Garmin liefert Dekameter je Sekunde, Lastwerk rechnet mal 10 in m/s (Q30, Sekundärquelle; Gegenprobe an Q28: 0,336 bei LTHR 166 und VO2max 50 ergibt 4:57 min/km, der Kehrwert ergäbe unplausible 5:36 min/km). Werte außerhalb von 1,5 bis 7 m/s verwirft der Abgleich (Faustregel). Der Verlauf (`/biometric-service/stats/.../range`, je Zeile `from` und `value`) gilt je Zeile ab `from`; FTP für Rad mit `sport=CYCLING`, eine Zeile mit anderer `series` wird übergangen.
- Vorrang: Je Art gilt der jüngste Wert mit `valid_from <= Tag`; bei gleichem Datum gewinnt ein Wert, der nicht von Garmin stammt (manuell oder übernommener Vorschlag), danach der der Sportart, danach der später angelegte. Ein manueller Wert mit späterem Datum überschreibt also alle älteren Garmin-Werte, ein späterer Garmin-Wert gilt wieder ab seinem Datum.
- Abgleich: Ein Garmin-Wert mit gleicher Sportart, Art und gleichem Datum wird ersetzt, nicht verdoppelt; ein unveränderter Wert schreibt nichts. Ändert sich eine Schwelle, rechnet der Abgleich die Aktivitäten ab dem frühesten geänderten Datum neu (`recompute_loads`). Geschlecht und Geburtsjahr setzt Garmin nur, solange der Nutzer sie nicht selbst gesetzt hat (Herkunft in `athlete.sex_origin` und `athlete.birth_year_origin`); dann wird alles neu berechnet.
- Fehlt ein Wert: Die Belastung bleibt `missing_threshold`, `activity_load.missing` nennt die fehlende Art (zum Beispiel `ftp_w`), und die Oberfläche sagt, welcher Wert fehlt, dass Garmin ihn nicht geliefert hat und dass er unter Einstellungen, Schwellenwerte eingetragen werden kann.

Für alle übrigen Werte schlägt die App vor und übernimmt nie selbst:

- FTP: 95 Prozent der besten 20-Minuten-Leistung (Faustregel, Q16 nur sekundär). Warnhinweis, dass das nur bei maximalem 20-Minuten-Test sinnvoll ist.
- LTHR: Mittelwert der Herzfrequenz in den letzten 20 Minuten eines 30-Minuten-Tests (Faustregel der Trainerliteratur).
- CSS: aus zwei Zeitläufen (Q15), zum Beispiel 400 m und 200 m.

Umsetzung in T05: `core::estimate` rechnet die drei Vorschläge als reine Funktionen. Die CLI (`thresholds suggest`) nutzt nur den FTP-Vorschlag aus der Datenbank (beste 20-Minuten-Leistung aller zählenden Radfahrten, mit Datum) und den CSS-Vorschlag aus zwei eingegebenen Zeiten (400 m und 200 m). Die LTHR ist aus dem Cache nicht ableitbar, weil er keine Herzfrequenzreihe speichert und ein 30-Minuten-Test aus einer Datei nicht erkennbar ist. Die CLI weist darauf hin. Ein Vorschlag wird nie gespeichert, `suggested_accepted` entsteht nur über `--from-suggestion`.

---

## 9. LLM-Vertrag

Das Sprachmodell ist optional, lokal und beratend. Der Regelplan ist stets vollständig gültig und bleibt das Ergebnis, wenn irgendetwas fehlschlägt.

### 9.1 Rolle

- Das Modell wählt aus erlaubten Vorlagen und Dauerfenstern und erklärt in Worten.
- Das Modell rechnet nicht. Lasten, CTL, ATL und TSB berechnet `core`.
- Das Modell hat keinen Zugriff auf Werkzeuge, Dateien oder Netzwerk.

### 9.2 Anbieter

| Anbieter | Standard-URL | Hinweis |
|---|---|---|
| Ollama | `http://127.0.0.1:11434` | Strukturierte Ausgabe über Parameter `format` mit JSON-Schema, Verfügbarkeit je Version prüfen |
| OpenAI-kompatibler lokaler Server | vom Nutzer gesetzt, Loopback oder bestätigter LAN-Host | LM Studio, llama.cpp-Server und ähnliche. Unterstützung für `response_format` schwankt, deshalb prüft Lastwerk jede Ausgabe selbst |

Lastwerk ruft keinen Cloud-Anbieter auf. Es gibt kein Feld für API-Schlüssel. Cloud-Modelle kommen nur über den MCP-Weg nach 9.6 ins Spiel.

Einrichtung (seit T29, Entscheidung vom 15. Oktober 2026): Ollama wird nur auf Wunsch eingerichtet, nie von selbst. Der NSIS-Installer fragt einmal ("Lokalen Coach (Ollama) einrichten?"), merkt die Antwort und fragt bei Updates und im stillen oder passiven Modus nicht. Die App bietet dasselbe nach Klick an (T29 Teil B). Beide rufen das gebündelte Skript `resources/ollama-setup.ps1` auf: ein vorhandenes Ollama wird erkannt und nicht angetastet, sonst Installation über winget, ersatzweise über den offiziellen Download mit Prüfung der Signatur (10.2). Das Startmodell `qwen3:4b` ist eine Faustregel, bis O13 gemessen ist. Das Modell lädt der Dienst von Ollama selbst über seine Loopback-Schnittstelle (`POST /api/pull`), Lastwerk lädt keine Modelldateien. Vertrag zwischen Skript, Hook und App: Task T29.

Umsetzung in T29 Teil B (Stand 15. Oktober 2026), Einrichtung des lokalen Coachs:

- **Katalog.** `catalog/models.json` (eingebettet in `lastwerk-coach::catalog`): `qwen3:1.7b`, `qwen3:4b` (`starter: true`), `qwen3:8b`, `qwen3:14b` mit `size_gb`, `min_ram_gb`, `comfortable_vram_gb` und Textschlüssel. Alle Werte sind Faustregel. Die Größen (1,4, 2,5, 5,2 und 9,3 GB) sind **ungeprüft**: `ollama.com` war am 15. Oktober 2026 aus der Cloud-Umgebung nicht erreichbar, die Zahlen stammen aus dem Gedächtnis und sind vor dem Livetest gegen `ollama.com/library/qwen3` zu prüfen. Das Startmodell `qwen3:4b` ist ein Vorschlag bis O13 gemessen ist und muss mit dem Hook des Installers (T29 Teil A) übereinstimmen.
- **Einstufung** (`recommend`, reine Funktion, Faustregel mit Begründung im Code): `too_big`, wenn Arbeitsspeicher plus 0,5 GB Toleranz (Systeme melden weniger, als verbaut ist) unter `min_ram_gb` liegt, wobei `min_ram_gb` grob die doppelte Dateigröße ist (Modell, Kontext, Windows und Programme des Nutzers). Sonst `fits`, wenn der Grafikspeicher `comfortable_vram_gb` erreicht (Modell samt Kontext komplett auf der Karte) oder die Datei höchstens 3 GB groß ist (auf einer CPU begrenzt der Speicherdurchsatz die Wörter je Sekunde auf grob 40 bis 60 GB/s geteilt durch die Größe; bis 3 GB bleibt eine Antwort mit einigen hundert Wörtern im Zeitlimit von 120 s), sonst `slow`. Empfohlen wird das größte Modell mit `fits`, sonst das Startmodell. Die Oberfläche nennt ein Modell nur "Empfohlen für deinen PC", wenn es auch `fits` ist; "langsam auf diesem PC" und "zu groß für diesen PC" bleiben wählbar. Handrechnungen für 8, 16 und 32 GB mit und ohne Grafikkarte stehen als Tests in `recommend.rs`.
- **Laden** (`pull_model`): `POST /api/pull` mit `{"model": name, "stream": true}` an die geprüfte Basisadresse (Loopback, keine Weiterleitung, kein Proxy aus der Umgebung, Zeitlimit 6 Stunden für die ganze Verbindung, Faustregel). Der Strom wird auf einem Hilfsthread gelesen, damit ein Abbruch sofort wirkt (Prüfung alle 100 ms, auch vor der ersten Antwort); 5 Minuten ohne Zeile sind ein Stillstand (`timeout`, Faustregel). Zeilen über 64 KiB und Zeilen ohne JSON sind Fehler. Der Fortschritt ist die Summe über alle Dateien (`digest`) des Modells, `status` wird zu einer Phase (`manifest`, `download`, `verify`, `write`, `done`, `other`), Fremdtext geht nicht weiter. Eine Zeile `{"error": ...}` wird nach Stichwörtern (Faustregel, die Texte von Ollama sind nicht stabil) zu `model_not_found`, `disk_full`, `network` oder `pull_failed`, der Text selbst wird verworfen. Ohne Zeile `success` ist der Strom unvollständig (`bad_response`).
- **Ablauf in der App** (5.4): Ollama erkennen, einrichten (Windows), Modell laden, danach `coach.model` setzen. Die Verbindung zu `ollama.com` und zur winget-Quelle stellt nur das Skript her (10.1, 10.2), und nur nach dem Klick auf "Jetzt einrichten"; das Laden eines Modells geht über Ollama selbst nach dem Klick auf "Laden". Die Oberfläche zeigt im Erststart (Schritt "Coach", überspringbar) und in den Einstellungen (Abschnitt "Lokaler Coach") denselben Baustein; ein Download läuft im Hintergrund weiter. Bis T42 zeigte ihn zusätzlich eine Zeile in der Seitenleiste; seit T42 steht er nur noch im Reiter Coach der Einstellungen (eine Stelle je Sache, 5.5).
- **Abweichung:** Ollama wird nur auf Wunsch eingerichtet, nie automatisch beim Start der App.

### 9.3 Eingang an das Modell

Nur aggregierte, nicht identifizierende Werte:

- Ziel (Art, Abstand zum Termin in Wochen), Phase der Woche
- CTL, ATL, TSB (gerundet)
- Wochenlast der letzten vier Wochen je Sportart
- Verfügbarkeit je Wochentag
- Basisplan als Liste von Slots mit `template_id` und `duration_min`
- erlaubte `template_id` je Sportart mit Klasse und Dauerfenster
- aktive Leitplanken als Codes mit Parametern

Nie im Eingang: Freitext aus FIT-Dateien (Aktivitätsnamen, Notizen, Workout-Namen), Zielbezeichnungen, Dateipfade, GPS, Zeitstempel einzelner Aktivitäten, Strava-Daten, Gerätekennungen. Vor jedem Aufruf zeigt die UI den exakten Eingang an und fordert eine Bestätigung (F24).

### 9.4 Ausgang und Prüfung

1. Antwort muss gültiges JSON nach `plan-edit.schema.json` sein.
2. Jede `template_id` muss im Katalog stehen und zur Sportart passen.
3. Änderungen werden auf den Basisplan angewandt. `core` rechnet Lasten neu.
4. Der Gesamtplan wird mit `validate_plan` gegen alle harten Leitplanken geprüft.
5. Weiche Verletzungen werden als Warnungen angehängt, aber nicht unterdrückt.

Bei einem Fehler in 1 bis 4 geht die Liste der verletzten Regeln als Codes (nicht als Freitext) in einen Korrekturaufruf. Es gibt höchstens zwei Korrekturaufrufe. Danach bleibt der Regelplan, `RefineOutcome::Fallback` nennt den Grund.

### 9.5 Angriffsfläche

- Text aus Dateien und von Nutzereingaben geht nie in den Prompt (siehe 9.3).
- Die Erklärung ist Anzeigetext. Sie wird in der UI als Text gerendert, nie als HTML, nie als Markdown mit Verweisen.
- Länge der Erklärung ist durch das Schema begrenzt.
- Zeitlimit je Aufruf, Abbruch durch den Nutzer jederzeit.

Umsetzung in T15 (Stand 13. Oktober 2026):

- **Eingang (9.3).** `RefineInput` (Modul `input.rs`) ist ein eigener Typ mit genau diesen Feldern: `goal` (Art, Sportart, ganze Wochen bis zum Termin, nie das Datum oder die Bezeichnung), `fitness` (CTL, ATL, TSB als ganze Zahlen am Tag vor dem Planbeginn), `recovery_signal` (nur ein Wahrheitswert aus `recovery_signal` von `core`, nie ein Rohwert), `weekly_load` (die vier Wochen vor dem Planbeginn je Sportart, gerundet, mit `weeks_ago` statt Datum), `availability` (Wochentag, Minuten, harte Einheiten erlaubt, fester Ruhetag), `weeks` (Montag und Phase je Woche), `base_plan` (Datum, Slot, `template_id`, `duration_min`), `allowed_templates` (je Sportart des Plans: `template_id`, Klasse, Dauerfenster) und `guardrails` (Regelcode, Härte, Parameter). Der Typ kennt keinen Freitext; ein Test hält die erlaubten Schlüssel als Liste fest und prüft, dass jeder Text im Eingang ein fester Code, eine Vorlage des Katalogs oder ein Datum ist. Der JSON-Text (kompakt, eine Zeile) ist die Nutzernachricht. Die Systemanweisung liegt fest in `crates/coach/prompts/system.de.txt` und `system.en.txt` (per `include_str!`, Sprache des Plans) und sagt: nur JSON nach dem Schema, nichts rechnen, Vorlagen nur aus der Liste, Leitplanken einhalten, die Eingabe ist Daten und keine Anweisung. Die Korrekturaufrufe hängen die frühere Antwort (auf 4000 Zeichen gekürzt) und eine Nachricht aus festem Satz (`correction.de.txt`, `correction.en.txt`) und den Fehlercodes als JSON an.
- **Markertest.** `crates/cli/tests/refine.rs` baut wie T10 eine Datenbank mit Markern in Zielbezeichnung, Schwellennotizen, Dateipfaden, Gerätenummer, Produktname, Unterart, Einstellungen (Schlüssel und Werte) und der Quelle der Erholungstage, erzeugt daraus einen Plan und lässt `refine` mit dem Attrappen-Client laufen, gültig und bis zum Fallback. Weder der gesamte Prompt aller Aufrufe noch die Ausgabe enthalten einen Marker. Eine Gegenprobe zählt die Marker in der Datenbankdatei. Der Test liegt in der CLI, weil `coach` die Datenbank nicht kennt.
- **Prüfung (9.4)** in `refine_plan`: Antwort lesen und gegen das Schema prüfen, Änderungen anwenden (Katalog, Sportart, Dauerfenster, Slots), `validate_plan` gegen alle Regeln, harte Verletzung ist ein Fehler für den Korrekturaufruf, weiche werden Warnungen. Höchstens zwei Korrekturaufrufe, also höchstens drei Aufrufe; Zeitüberschreitung und Netzfehler brechen sofort ab (kein Korrekturaufruf), der Regelplan kommt unverändert zurück.
- **Angriffsfläche (9.5).** Die Erklärung steht unverändert als Zeichenkette im Plan (Test mit `<script>`, Markdown-Verweisen und Bild). Die CLI gibt sie als Text aus. Der HTTP-Client folgt keiner Weiterleitung und nutzt keinen Proxy aus der Umgebung (Test mit zwei Attrappen-Servern auf Loopback für die Weiterleitung). Die Oberfläche (T09) zeigt die Erklärung bisher nicht an; wenn sie es tut, gilt dort dieselbe Regel (Text, nie HTML).

### 9.6 Claude als MCP-Client

Der Nutzer kann Claude Desktop oder Claude Code mit `lastwerk-mcp` verbinden, um Fragen zu Training und Plan im Gespräch zu klären. Das ist eine bewusste Ausnahme von der Regel, dass kein Cloud-Modell beteiligt ist. Sie gilt nur für diesen Weg.

Rahmen:

1. **Folge für die Daten.** Jede Werkzeugantwort wird Teil des Gesprächs und geht an den Anbieter des Clients. Sie ist danach nicht zurückholbar. Die Einstellungsseite sagt das vor der ersten Aktivierung in einem Satz und verlangt eine Bestätigung.
2. **Erlaubter Umfang.** Aggregierte Kennzahlen (CTL, ATL, TSB, Wochenlast), einzelne Aktivitäten ohne GPS (Datum, Sportart, Dauer, Last, Intensitätsfaktor, Zonenzeiten, Bestwerte), Plan und Schwellenwerte.
3. **Ausgeschlossen.** Koordinaten und Streckenpunkte, Aktivitätsnamen und Notizen, Gerätekennungen, Dateipfade, Strava-Daten, Zielbezeichnungen. Erholungswerte (HRV, Schlaf, Ruhepuls) sind standardmäßig gesperrt und nur durch eine eigene Einstellung freigebbar (O19).
4. **Nur lesen.** Claude kann keinen Plan ändern, keine Schwellenwerte setzen und keine Dateien anlegen. Plananpassungen laufen weiter über den lokalen Coach nach 9.4 oder durch den Nutzer.
5. **Getrennte Server.** Der Garmin-nahe Server des Zusatzprogramms liefert Rohdaten und wird nie mit einem Cloud-Client verbunden.
6. **Nachvollziehbarkeit.** Das lokale Aufrufprotokoll (5.4) zeigt, was abgefragt wurde.

Prompt-Injection: Werkzeugargumente sind Datumswerte, Zahlen und feste Codes und werden gegen Typen geprüft. Ausgaben enthalten keinen Freitext aus Dateien oder Nutzereingaben. Damit kann ein Eintrag in einer FIT-Datei den Client nicht anweisen.

### 9.7 Gespräch mit dem Coach (seit T26)

Entscheidung vom 15. Oktober 2026: Der Planer fragt nur noch drei Dinge (Ziel, Stunden je Woche, Trainingstage), alles Weitere klärt ein Gespräch mit dem lokalen Modell. Das ändert 9.3 und 9.5 an genau einer Stelle: **Der Text, den der Nutzer selbst in das Gesprächsfeld tippt, geht an das lokale Modell.** Begründung: Es ist seine eigene Nachricht an ein Modell auf seinem Rechner (9.2, nur Loopback oder bestätigtes LAN), und das Modell kann nichts tun außer einen Vorschlag zu machen, den `core` prüft und der Nutzer bestätigt. Alles andere aus 9.3 gilt weiter: kein Freitext aus FIT-Dateien, keine Zielbezeichnung, keine Pfade, keine Rohwerte der Erholung.

1. **Eingang.** Kontext wie `RefineInput` (9.3) plus die Kennzahlen des Dashboards als gerundete Zahlen mit Code (zum Beispiel `ramp_ctl_7d`, `share_z12_4w`), der Verlauf des Gesprächs (höchstens 12 Nachrichten, je höchstens 1000 Zeichen) und die neue Nachricht.
2. **Ausgang.** JSON nach `schema/plan-chat.schema.json`: `reply` (Text, höchstens 1200 Zeichen) und optional `proposal`, entweder `plan_edit` (wie 7.2) oder `planning_change` (Minuten je Wochentag, harte Einheiten je Tag, Sportarten, Kraft je Woche, Wochen). Das Modell rechnet nicht und nennt nur Zahlen, die im Eingang stehen.
3. **Prüfung.** Wie 9.4: Schema, Katalog, Anwenden, `validate_plan`, höchstens zwei Korrekturaufrufe. Ein `planning_change` erzeugt über den Regelplaner einen neuen Entwurf, der ebenfalls geprüft wird. Scheitert die Prüfung, zeigt die App die Antwort ohne Vorschlag und nennt die Regelcodes.
4. **Bestätigung.** Ein Vorschlag ändert nichts, bis der Nutzer "Übernehmen" wählt. Die Oberfläche zeigt Vorher und Nachher. Der Eingang des Modells ist vor jedem Aufruf einsehbar (F24), statt einer Bestätigung je Nachricht genügt eine einmalige Freigabe des Gesprächs je Sitzung.
5. **Anzeige.** `reply` ist Text, nie HTML oder Markdown (9.5).
6. **Ohne Modell.** Läuft kein lokales Modell, bleibt der Planer mit den drei Fragen voll bedienbar, das Gesprächsfeld nennt die Einrichtung (Ollama).

Umsetzung in T26 Teil A (Stand 15. Oktober 2026), Modul `chat` in `lastwerk-coach`:

- **Schema.** `schema/plan-chat.schema.json` (Draft 2020-12, per `include_str!` eingebettet, `additionalProperties: false` in jedem Objekt, ein Test prüft das): `schema_version` 1, `reply` (1 bis 1200 Zeichen), optional `proposal` als `oneOf` aus `{kind: "plan_edit", edit}` und `{kind: "planning_change", availability?, sports?, strength_per_week?, weeks?}`. `edit` ist `plan-edit.schema.json` ohne Kopf, **eingebettet statt per `$ref`**, weil lokale Server Verweise im Parameter `format` unterschiedlich gut auflösen; ein Test hält beide Fassungen gleich. `availability` genau 7 Einträge (Montag bis Sonntag) mit `max_minutes` 0 bis 1440 (dieselbe Grenze wie `store`), `allow_hard`, `fixed_rest`; `sports` 1 bis 3 verschiedene aus `cycling`, `running`, `swimming`; `strength_per_week` 0 bis 2; `weeks` 1 bis 26; mindestens ein Feld neben `kind` (`minProperties`).
- **Eingang.** `ChatInput::build(base, ctx, rules, metrics)` ist `RefineInput` (9.3, unverändert) plus `metrics`: Liste von `MetricValue`, nur über `MetricValue::new(code, value)` zu bekommen, Code `[a-z0-9_]{1,40}`, Wert endlich und auf eine Nachkommastelle gerundet, höchstens 40 Kennzahlen (Konvention), kein Code doppelt. Ein Test vergleicht den JSON-Text mit dem von `RefineInput` und lässt nur `metrics` mehr zu. `ChatInput` hält Basisplan, Kontext und Regeln für die Prüfung, sie gehen nicht in den Text. **Abweichung zu `refine`:** Der Eingang steht als JSON hinter der festen Anweisung in der Systemnachricht (`prompts/chat.de.txt`, `chat.en.txt`, endet mit "Eingang:"), nicht als erste Nutzernachricht; die Rolle `user` gehört im Gespräch allein den Worten des Nutzers. Dann folgen der Verlauf und die neue Nachricht. `chat_messages(...)` liefert genau diese Nachrichten für die Vorschau (F24).
- **Verlauf.** `ChatHistory` hält höchstens 12 Nachrichten (`ChatRole::User` oder `Assistant`, je höchstens 1000 Zeichen, gekürzt, Steuerzeichen außer Zeilenumbruch und Tabulator werden Leerzeichen, leere entfallen), die ältesten fallen weg. Nur Text des Nutzers und `reply` des Modells, nie ein Vorschlag. Antworten des Modells gehen im Antwortformat `{"schema_version":1,"reply":...}` zurück, damit das Modell beim Format bleibt. `record(user_text, &outcome)` hängt eine Runde an. Der Verlauf lebt beim Aufrufer, `coach` speichert nichts.
- **Neue Nachricht.** `prepare_user_text`: Steuerzeichen wie oben, Rand getrimmt, leer ist `ChatError::EmptyMessage`, mehr als 1000 Zeichen ist `MessageTooLong` (**nicht gekürzt**, damit nicht stillschweigend etwas fehlt). Nichts wird gesendet.
- **Ablauf.** `chat_turn(client, &input, &history, user_text, locale, &RefineConfig) -> Result<ChatOutcome, ChatError>`. `ChatOutcome { reply, proposal: Option<CheckedProposal>, rejected_codes, calls }`. Zu lange Texte (`reply`, `edit.explanation`) werden vor der Prüfung auf 1200 Zeichen gekürzt (wie 7.2). Ist die Hülle ungültig (kein JSON, `reply` fehlt, Schemafehler außerhalb von `proposal`), gibt es keine lesbare Antwort. Ist nur der Vorschlag ungültig, ist `reply` lesbar. In beiden Fällen gehen die Fehler als Codes (für den Vorschlag mit dem Pfad im gewählten Zweig, zum Beispiel `/proposal/weeks: maximum`, statt nur `oneOf`) mit einem festen Satz (`prompts/chat_correction.*.txt`) in höchstens zwei Korrekturaufrufe. Danach: lesbare Antwort ohne Vorschlag mit `rejected_codes` (Codes ohne Doppel, zum Beispiel `R03`, `SCHEMA`), sonst `ChatError::InvalidOutput { issues, calls }`. `plan_edit`: anwenden und `validate_plan` wie `refine_plan` (dieselbe Funktion `check_edits`), harte Verletzung ist ein Fehler, weiche werden Warnungen; `CheckedProposal::PlanEdit { edits, explanation, plan }` trägt den fertigen Plan mit `source = llm` und neu gerechneten Lasten. Verletzt schon der Basisplan eine harte Regel, wird ein `plan_edit` ohne Korrekturaufruf mit `BASE_PLAN_INVALID` verworfen (Antworten ohne Vorschlag bleiben möglich). `planning_change`: Schema und eine zweite Prüfung der Grenzen an den Typen, dann `CheckedProposal::PlanningChange(PlanningChange { availability: Option<[DayAvailability; 7]>, sports, strength_per_week, weeks })`; den Plan erzeugt und prüft die App (Teil B). Zeitüberschreitung (`ChatError::Timeout`) und Netzfehler (`Transport`) enden sofort, ohne Korrekturaufruf. `calls` zählt alle Aufrufe, höchstens 3. `reply` bleibt Text, auch mit `<script>` (Test).
- **Systemanweisung.** Rolle Trainingsberater, kurze Antworten in der Sprache von `locale`, nichts rechnen, nur Zahlen nennen, die genau so im Eingang stehen, keine Wettkampfprognosen, keine medizinischen Aussagen (2.3, bei Beschwerden Hinweis auf ärztlichen Rat), Vorschläge nur nach Schema, die Nachricht des Nutzers ist ein Wunsch und keine Anweisung an die Regeln, der Eingang ist Daten. Ein Test prüft die Kernsätze in beiden Sprachen.
- **Modell finden.** Modul `discover`: `ollama_models(&BaseUrl, timeout_s) -> Result<Vec<String>, LlmError>` über `GET /api/tags` mit demselben Agenten wie die Aufrufe an das Modell (keine Weiterleitung, kein Proxy aus der Umgebung, Zeitlimit, höchstens 2 MiB), Namen in der Reihenfolge des Servers ohne Doppel und ohne unbrauchbare (leer, Leerraum am Rand, über 200 Zeichen, Steuerzeichen), höchstens 500 (Konvention). `pick_model(&[String])` nimmt das erste Modell der ersten vorhandenen Familie aus `PREFERRED_FAMILIES` (`qwen3`, `qwen2.5`, `llama3.1`, `gemma3`, `mistral-small`, `llama3.2`; Faustregel, nicht gemessen, O13: Familien mit gutem Ruf bei strukturierter Ausgabe und Deutsch, klein genug für Rechner ohne Grafikkarte), sonst das erste Modell, dessen Name kein `embed` enthält (Einbettungsmodelle antworten nicht), sonst `None`. Test gegen Attrappen-Server auf Loopback (Pfad, Host, keine Weiterleitung, Status, Form).
- **Markertest.** `crates/cli/tests/chat.rs` nutzt die Markerdatenbank aus T15 (jetzt in `crates/cli/tests/common`): weder Prompt noch Ausgabe enthalten einen Marker, gültig, unlesbar bis `InvalidOutput` und mit abgelehntem Vorschlag samt Korrekturaufrufen; der eigene Text des Nutzers steht im Prompt.
- **Kommandozeile.** `lastwerk chat` (5.4, lastwerk-cli) für Versuche mit einem echten Modell, ohne Speichern.
- Offen für Teil B: Befehle der App, Oberfläche, Erzeugung des Plans aus `planning_change`, O31. Abhängigkeiten kamen keine hinzu.

Umsetzung in T26 Teil B (Stand 15. Oktober 2026), Befehle in `app/src-tauri/src/service/coach.rs` (Eintragung und Fehlercodes: 5.4, lastwerk-app), Oberfläche in `app/ui/src/views/plan/`:

- **Ablauf von `coach_chat`.** Nachricht prüfen (`prepare_user_text`), Basisplan wählen (der aktive Plan, sonst der neueste Entwurf, sonst `no_plan`), Kontext mit `planning::validation_context` bilden, Kennzahlen aus dem Dashboard holen, `ChatInput::build`, `chat_turn` mit dem Zeitlimit von 120 Sekunden je Aufruf (O13, Faustregel, bis zu drei Aufrufe). Der Aufruf läuft auf einem Arbeitsthread. Ein zweites Gespräch gleichzeitig endet mit `busy`. Die Sprache der Anweisung ist die der Oberfläche (Einstellung `app.locale`), nicht die des Plans.
- **Kennzahlen im Eingang** (aus `get_dashboard`, je eine Nachkommastelle, fehlende Werte fehlen in der Liste, nie 0): `ctl_now`, `atl_now`, `tsb_now`, `ramp_ctl_7d`, `ramp_limit_ctl_7d`, `easy_share_28d`, `easy_share_min`, `adherence_last_week`, `ef_28d`, `ef_change_28d_pct`. Keine Rohwerte der Erholung: Sie stecken nur im Zeichen `recovery_signal` des Kontexts (9.3). Ein Fehler im Dashboard lässt das Gespräch ohne Kennzahlen laufen.
- **Verlauf und Vorschläge** leben im Speicher der App (`CoachState`, nicht in der Datenbank, weg beim Neustart). Höchstens vier offene Vorschläge, ältere fallen weg. Der Verlauf wächst nur, wenn das Modell geantwortet hat; bei einem Fehler kommt die Nachricht ins Eingabefeld zurück. `coach_cancel` (SPEC 9.5: Abbruch jederzeit) stellt weitere Aufrufe ein und verwirft die Antwort eines laufenden, der Verlauf bleibt; ein Aufruf an das Modell selbst endet erst mit seinem Zeitlimit, bis dahin bleibt die Sperre `busy`. `coach_reset` bricht ab und leert Verlauf und Vorschläge.
- **Vorher und Nachher.** Je Woche Minuten, harte Einheiten und geplante Last (von `core` gerechnet), nach dem Beginn der Woche gepaart (eine Woche kann auf einer Seite fehlen). Bei `plan_edit` zusätzlich die Änderungen mit Datum, Slot und Vorlage, bei `planning_change` die neuen Zeiten je Wochentag, Sportarten, Kraft, Wochen und der Beginn des neuen Plans.
- **`planning_change`.** Fehlende Felder bleiben wie im Basisplan (Sportarten und Kraft je Woche aus seinen Einheiten, Wochen bis zu seinem Ende). Der neue Plan ersetzt den laufenden und beginnt am Montag der heutigen Woche (ein Plan, der erst in der Zukunft beginnt, behält seinen Beginn). Vergangene Tage dieser Woche tragen damit geplante Einheiten, die in der Planerfüllung als offen zählen. Die Vorschau erzeugt den Plan mit `generate_with` ohne Schreiben. **Abweichung zu Punkt 3:** Scheitert der Regelplaner, wird der Vorschlag mit dem Code `PLAN_FAILED` verworfen; die Fehler gehen nicht in einen Korrekturaufruf, weil `chat_turn` seine Prüfung vor dem Erzeugen abschließt.
- **Übernehmen (`coach_apply`).** Ein `plan_edit` speichert den geprüften Plan mit neuer ID und `created_at` jetzt (`source = llm`), ein `planning_change` schreibt die Verfügbarkeit, erzeugt mit dem Regelplaner einen neuen Plan, prüft ihn gegen Schema und harte Regeln und speichert ihn; beide aktivieren ihn in einem Schritt, der bisherige aktive Plan wird archiviert. Hat sich der Basisplan seit dem Vorschlag geändert, ist der Vorschlag veraltet (`proposal_stale`). Nach dem Übernehmen verfallen alle offenen Vorschläge.
- **Freigabe (Punkt 4).** Vor der ersten Nachricht einer Sitzung zeigt die Oberfläche einmal, was an das Modell geht (Modell, lokaler Rechner, Hinweis auf die Vorschau) und verlangt "Gespräch starten". Die Freigabe liegt nur im Speicher der Oberfläche (`coachReleased`), ein Neustart fragt neu. "Was sieht der Coach?" zeigt auf Wunsch jederzeit den exakten Text der nächsten Nachricht (`coach_preview`, dieselbe Funktion `chat_messages`, folgt dem Eingabefeld); ohne Nachricht fehlt nur diese.
- **Anbieter.** Standard ist Ollama auf Loopback. Ein Host im lokalen Netz (9.2) ist in der App nicht einstellbar, weil die Oberfläche die ausdrückliche Bestätigung dafür noch nicht bietet; die Einstellung `coach.base_url` verlangt Loopback (sonst `bad_url`).
- **Ohne Modell** zeigt das Gesprächsfeld die Einrichtung (Ollama installieren, `ollama pull qwen3:4b` als Beispiel, erneut prüfen) und den Grund, der Planer bleibt voll nutzbar. Das genannte Modell folgt der Vorzugsliste aus `discover` (Faustregel, nicht gemessen, O13).
- **Anzeige.** `reply`, `explanation` und alle Texte aus dem Modell stehen als Textknoten (Svelte maskiert sie), nie als HTML oder Markdown. `no-html.test.ts` verbietet Markdown- und Bereinigungsbibliotheken und prüft, dass die Komponenten die Texte so setzen; Komponententests geben `<script>`, `<img>`, Markdown-Verweise und Fettdruck ein und prüfen, dass kein Element entsteht.
- **Markertest** auf dem Weg der App (`app/src-tauri/tests/coach.rs`, nutzt die Markerdatenbank aus T15): Zielbezeichnung, Notizen, Pfade, Namen und Gerätenummern erreichen das Modell nie, weder in einem Aufruf noch in der Vorschau, in vier Fällen (gültig, unlesbar, abgelehnt, `planning_change`); der Text des Nutzers schon.

---

## 10. Datenschutz und Sicherheit

### 10.1 Grundsätze

- Keine Telemetrie, keine Absturzberichte, kein Update-Check im Hintergrund.
- Abweichung T17 (Entscheidung vom 14. Oktober 2026): Der Einstellungsdialog bietet "Nach Update suchen". Erst auf diesen Klick fragt der Rust-Teil die Releases des Repositories (`https://github.com/flor1xn/lastwerk/releases/latest/download/latest.json`) ab, nie das Frontend, nie von selbst. Installiert wird erst nach einer zweiten Bestätigung und nur, wenn die Signatur zum öffentlichen Schlüssel in `tauri.conf.json` passt (Update-Signatur von Tauri, kein Codesigning von Windows). Der Abruf überträgt keine Nutzerdaten, außer was jede HTTPS-Anfrage verrät (IP-Adresse, Version der Bibliothek).
- Abweichung T29 (Entscheidung vom 15. Oktober 2026): Zur Einrichtung von Ollama (9.2) entstehen Verbindungen nach außen nur im Installer nach "Ja" auf die Frage oder in der App nach Klick, nie von selbst und nie aus dem Frontend. Das Skript `ollama-setup.ps1` ruft winget mit der Quelle `winget` (Paket `Ollama.Ollama`) auf; nur wenn winget fehlt oder scheitert, lädt es `https://ollama.com/download/OllamaSetup.exe` (Weiterleitung zu den Releases von Ollama auf GitHub). Danach spricht es nur mit `http://127.0.0.1:11434`. Modelle lädt der Dienst von Ollama aus seiner eigenen Registry, Lastwerk lädt keine Modelldateien. Beim Update von Lastwerk (passiver Modus) prüft der Installer Ollama und das Startmodell nur, wenn die gemerkte Antwort "Ja" lautet. Übertragen werden keine Nutzerdaten, außer was jede HTTPS-Anfrage verrät.
- Daten bleiben lokal. Es gibt kein Konto und keinen Server. Ausnahme sind die Antworten von `lastwerk-mcp` an den Client, den der Nutzer selbst verbindet (9.6).
- Jede Verbindung nach außen braucht eine ausdrückliche Handlung und ist in den Einstellungen sichtbar.
- Vollabzug aus Garmin (T39): nur die belegten lesenden Endpunkte aus 5.6, keine Menstruations- und Schwangerschaftsdaten, S3 nur nach Freigabe in den Einstellungen und getrennt gespeichert (`sensitive_day`, vom MCP-Server nicht gelesen). Neue Tages- und Trainingswerte gehen nicht an den MCP-Server.
- Echte FIT-Dateien liegen nur in `local/` und werden nie committet (Abschnitt 1.4, Abschnitt 11).

### 10.2 Bedrohungen und Gegenmaßnahmen

| Bedrohung | Gegenmaßnahme |
|---|---|
| FIT-Datei mit manipuliertem Inhalt (Absturz, Speicher) | Parser in Rust, Größenlimit, Fehler pro Datei statt Abbruch, Fuzz-Tests ab M1 |
| Prompt-Injection über Aktivitätsnamen oder Notizen | Freitext nie im Prompt, Ausgabe nur über Schema und Katalog |
| LLM liefert unzulässigen Plan | Schema, Katalog, Leitplanken, Fallback auf Regelplan |
| Versehentliches Veröffentlichen privater Daten | `.gitignore`, CI-Prüfung auf `*.fit`, Pre-commit-Hook, Secret-Scan |
| Schadcode über Abhängigkeiten | Lockfiles, `cargo deny`, `cargo audit`, `pnpm audit`, Dependabot-Hinweise nur als Information |
| Token des Zusatzprogramms gelangt in den Kern | Token liegen im Benutzerprofil, nicht im Projektordner. `bridge/` ist vom Kern durch Importverbot und Pfadfilter in der CI getrennt. Lastwerk liest nie Token-Dateien des Zusatzprogramms |
| Garmin-Token der App wird entwendet oder geleakt (seit T20) | Token nur im Schlüsselbund des Betriebssystems, nie in Datenbank, Backup, Log, Fehlertext oder Antwort an die Oberfläche. Typ ohne `Debug`-Ausgabe des Inhalts, Test mit Marker. Trennen löscht den Eintrag |
| Garmin-Anmeldefenster missbraucht die App (seit T20) | Fenster ohne Capability und ohne IPC, lädt nur Garmin-Seiten, Navigationsfilter nimmt nur die eine Weiterleitungsadresse mit Ticket an |
| Gesprächsnachricht steuert das lokale Modell zu einem schädlichen Plan (seit T26) | Modell gibt nur Vorschläge nach Schema, `core` prüft gegen alle Leitplanken, der Nutzer bestätigt, Antworttext nur als Text (9.7) |
| Manipulierter Installer von Ollama oder untergeschobenes Modell (seit T29) | Installation zuerst über winget (Hash im Manifest der Quelle `winget`), ersatzweise Download nur über HTTPS vom festen Endpunkt in einen neuen Ordner unter `%TEMP%`, Ausführung nur bei gültiger Authenticode-Signatur mit Herausgeber Ollama (`O=`), sonst Datei löschen und `signature_invalid`. Kein Text aus dem Netz wird ausgeführt (kein `Invoke-Expression`), Modellname gegen ein festes Muster geprüft (keine Einschleusung von Befehlen), Modelle nur über die Loopback-Schnittstelle von Ollama, Zustandsdatei ohne Pfade und ohne fremden Text. Restrisiko: Bis zum Livetest prüft das Skript nur, dass die Organisation mit "Ollama" beginnt, der genaue Herausgeber wird dann fest eingetragen (T29) |
| Zusatzprogramm liefert manipulierte Dateien | Schema-Prüfung, Größenlimits, keine Pfade aus dem Inhalt |
| Web-Inhalte in der App | Strenge Content-Security-Policy, keine entfernten Skripte, Schriften und Bibliotheken gepackt |
| Standort und Strecken über Kartenkacheln | Standard aus, siehe 10.3 |
| ICS-Datei verrät Trainingsdaten beim Teilen | ICS enthält nur Titel, Zeit und Dauer, keine Orte, keine Schwellenwerte |
| MCP-Antwort enthält Rohdaten oder Freitext | Feste Ausgabetypen statt Datenbankzeilen, Feldlisten als Golden-Snapshot, Markertest auf Koordinaten, Namen und Seriennummern (11.4) |
| Garmin-naher Server wird mit einem Cloud-Client verbunden | Getrennte Programme und Verzeichnisse, keine gemeinsamen Importe, Hinweis in README und Einstellungen, `lastwerk-mcp` liest nur die Lastwerk-Datenbank, nie `exchange/`, nie das Archiv und nie das Garmin-Konto |
| Prompt-Injection über Werkzeugargumente eines MCP-Clients | Typisierte Argumente (Datum, Zahl, Code), Längen- und Bereichsgrenzen, keine Pfade, keine Freitextfelder |
| Missbrauch der Garmin-Zugangsdaten durch ein Modell | Zusatzprogramm bietet in Version 1 kein Schreibwerkzeug im MCP. Die App gibt dem lokalen Modell und `lastwerk-mcp` keinen Zugang zum Garmin-Client |
| Archiv enthält vollständige Gesundheits- und Standortdaten | Archiv unter `local/` (nicht versioniert, durch Hook und CI gegen Commit geschützt), nicht in Cloud-Sync-Ordnern, `sensitive/` getrennt, Festplattenverschlüsselung empfohlen (O28), Archiv wird von Lastwerk und `lastwerk-mcp` nie direkt gelesen. Risiko: `git clean -x` löscht es (6.1) |
| Token des Garmin-Kontos wird entwendet | Token im Benutzerprofil mit eingeschränkten Rechten, nie im Repository, Abmelden löscht nur die lokale Datei: bei Verdacht Zugriff in den Garmin-Kontoeinstellungen widerrufen (Q21) |
| Massenabruf führt zu Drosselung oder Sperre des Kontos | Drosselung, Abbruch nach drei 429 oder 403, kein Parallelabruf, Etappenläufe mit `--max-calls`, keine Umgehung (5.5), Rückfall auf USB-Kopie und FIT-Ordner. Restrisiko bleibt (O18, O24) |
| Versehentlicher oder doppelter Schreibzugriff auf das Garmin-Konto durch `push` | Trockenlauf ist der Standard, `--yes` verlangt `--out` und `--max-workouts`, eigene Freigabe mit genau zwei Methoden, keine automatische Wiederholung, Abbruch beim ersten Fehler, Protokoll verhindert doppeltes Anlegen, nur mit Attrappen getestet (5.5, M6) |
| Aktualisierung der Bibliothek ändert Verhalten oder führt neue schreibende Methoden ein | Gepinnte Version, Allowlist-Test gegen die Methodenliste, Offline-Tests vor jeder Anhebung |

Umsetzung des ICS-Exports (T08, Konventionen): Eine `VEVENT` je Einheit, Ruhetage haben keine. `SUMMARY` ist der Name der Vorlage aus `catalog/locales/<locale>.json` (`template.<id>.name`) in der Sprache des Plans, dazu die Sportart in Klammern (entfällt, wenn der Name sie schon nennt). `DESCRIPTION` enthält die Beschreibung der Vorlage und die Texte der Begründungsschlüssel der Einheit, beides feste Texte aus den Sprachdateien, nie Zahlen aus dem Plan oder aus der Datenbank. Der Plan kennt keine Uhrzeiten: Slot `am` beginnt um 07:00, Slot `pm` um 17:00, als lokale Zeit ohne Zeitzone (floating time, kein `TZID`), die Dauer ist `duration_min`. `UID` ist `<plan_id>-<Datum>-<slot>`, `DTSTAMP` die Erzeugungszeit des Plans (damit die Datei bei gleichem Plan gleich bleibt). Es gibt kein `LOCATION`, kein `GEO`, keinen Organisator und keine Teilnehmer; ein Test liest die Datei mit `icalendar` ein und prüft die erlaubten Eigenschaften.

### 10.3 Netzwerk und Kartenkacheln

Kartenkacheln sind als Option erlaubt. Dieses Dokument hält dagegen fest, dass ein Kachelserver bei jedem Kartenaufruf Standort und Strecke der Anfrage sieht. Das widerspricht dem Ziel "privat".

Umsetzung:

- Standard: aus. Aktivitätsdetail zeigt den Streckenverlauf ohne Hintergrundkarte.
- Einschaltbar in den Einstellungen, mit Hinweis, was der Kachelserver erfährt.
- Kachelserver-URL ist einstellbar, damit ein selbst betriebener Server möglich bleibt.
- Die Content-Security-Policy erlaubt Bildquellen nur für diesen einen Host. Ohne Aktivierung erlaubt sie keinen.
- Nutzungsbedingungen des gewählten Kachelanbieters sind einzuhalten (offen O3).

### 10.4 Tauri-Konfiguration (Vorgabe)

- Capabilities minimal: Dateidialoge nur im Rust-Teil (seit T28 Auswahl von FIT-Dateien statt Ordnerwahl), Lesen und Schreiben im App-Datenordner und am Exportziel.
- Keine allgemeine Shell- oder HTTP-Berechtigung im Frontend. Alle Netzwerkzugriffe laufen im Rust-Backend (`coach`, Kacheln über eine feste Allow-List).
- Content-Security-Policy ohne `unsafe-eval`, ohne Inline-Skripte.

Umsetzung in T06: Die Capability `default` gibt dem Hauptfenster genau die eigenen Befehle (`allow-<befehl>`, erzeugt aus der Liste in `build.rs`) und `core:event:allow-listen` und `allow-unlisten` frei, sonst nichts: kein Dialog-Plugin, kein Dateisystem-Plugin, keine Shell, kein HTTP im Frontend. Den Dateidialog öffnet der Rust-Befehl `add_fit_files` (bis T27 der Ordnerdialog `choose_folder`), Lesen und Schreiben der Dateien geschieht im Rust-Teil. `withGlobalTauri` ist aus. CSP: `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self' ipc: http://ipc.localhost; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`, also auch ohne `unsafe-inline` für Stile (Svelte bündelt die Stile, Diagramme setzen Stile über das DOM). Nur `devCsp` für `tauri dev` erlaubt Inline-Stile des Vite-Servers. Ein Rust-Test prüft Befehlsliste, Capability, CSP und Fehlertexte, der Playwright-Rauchtest sendet dieselbe CSP als Header und schlägt bei jedem Verstoß fehl.

Umsetzung in T23: Das Anmeldefenster `garmin-login` lädt die Seite von Garmin und hat keine Capability. `capabilities/default.json` ist die einzige Capability-Datei, gilt nur für `windows: ["main"]` und hat weder `remote` noch `webviews`; `tauri.conf.json` nennt nur das Fenster `main` und keine weitere Capability (Test `only_the_main_window_has_a_capability`). Damit kann die Seite von Garmin weder Befehle der App aufrufen noch Ereignisse hören. Neu in der Capability sind nur `allow-garmin-status`, `allow-garmin-login`, `allow-garmin-sync` und `allow-garmin-logout` für das Hauptfenster. Die CSP des Hauptfensters bleibt unverändert, das Frontend spricht nie mit Garmin.

Umsetzung in T09: Die Capability enthält zusätzlich nur `allow-<befehl>` für die elf Befehle zu Plänen, Zielen, Verfügbarkeit und Export. Die Dateidialoge dafür öffnet der Rust-Befehl (Speichern von ICS, Plan-JSON und Backup, Öffnen einer Sicherung). Die Oberfläche bekommt weiterhin kein Dialog-, kein Dateisystem-, kein Shell- und kein HTTP-Plugin und keinen Pfad. Das geschriebene Exportziel bestimmt allein der Nutzer im Systemdialog.

Umsetzung in T26 Teil B: Die Capability enthält zusätzlich `allow-<befehl>` für die acht Befehle zum Planer und zum Coach (`quick_plan`, `coach_status`, `coach_set_model`, `coach_preview`, `coach_chat`, `coach_apply`, `coach_reset`, `coach_cancel`), sonst nichts. Der Modellserver wird nur vom Rust-Teil aufgerufen (`ureq`, keine Weiterleitung, kein Proxy, nur Loopback); die CSP bleibt unverändert, `connect-src` erlaubt dem Frontend weiterhin keinen Host.

Umsetzung in T29 Teil B: Die Capability enthält zusätzlich `allow-<befehl>` für die vier Befehle der Einrichtung des Coachs (`coach_setup_status`, `coach_install_ollama`, `coach_pull_model`, `coach_pull_cancel`), sonst nichts: keine Shell, kein Prozess, kein Dateisystem im Frontend. Das Skript startet der Rust-Teil, nur das mitgelieferte aus den Ressourcen, mit festen Argumenten (nie ein Text der Oberfläche); die Oberfläche bekommt weder den Pfad des Skripts noch den der Zustandsdatei. Der Modellname für `coach_pull_model` wird gegen den Katalog und die installierten Modelle geprüft. Die CSP bleibt unverändert.

### 10.5 Strava

Nicht Teil von Version 1. Für eine spätere Anbindung gelten vorab:

- Strava-Daten werden nur dem Nutzer selbst angezeigt (Strava API Agreement, Abschnitt Zusammenfassung, Stand der Seite: wirksam ab 1. Juni 2026, Q20).
- Strava-Daten gehen nie an ein LLM, bis die Bedingungen geklärt sind. Die abrufbare Vertragsseite nennt keine KI-Klausel, mehrere Presseberichte behaupten ein Verbot der KI-Nutzung. Die Aussagen widersprechen sich (offen O1).
- Es ist ein eigener API-Zugang des Nutzers nötig (ein Token je Anwendung, Q20).

### 10.6 Markenhinweise

README und Oberfläche nennen Garmin und Strava nur beschreibend ("liest FIT-Dateien von Garmin-Geräten"), ohne Logos. README enthält den Hinweis, dass das Projekt nicht von Garmin oder Strava unterstützt oder autorisiert wird. Der Projektname enthält keinen fremden Markennamen.

---

## 11. Teststrategie

### 11.1 Grundsätze

1. Jede Formel hat mindestens einen handgerechneten Referenzwert, im Test als Konstante mit Rechenweg im Kommentar.
2. Tests nutzen synthetische Daten. Echte Dateien aus `local/` dienen nur der Plausibilität und kommen nie in Testdaten, Snapshots oder Commits.
3. `core` ist ohne Dateisystem testbar. Zufall und Zeit werden übergeben.
4. Ein Fehler in einer Quelle (Sekundärquelle, Faustregel) ist ein Parameter, kein Code. Er wird in `rules.default.json` oder `LoadConfig` korrigiert, ohne Logik zu ändern.

### 11.2 Referenzwerte (Handrechnung)

| ID | Eingang | Erwartung | Rechenweg |
|---|---|---|---|
| T-NP-1 | 3600 s konstant 200 W | NP = 200 W | Gleitmittel konstant, vierte Wurzel des Mittels von 200^4 ergibt 200 |
| T-TSS-1 | wie T-NP-1, FTP 250 W | IF = 0,8, TSS = 64,0 | `3600 * 200 * 0,8 / (250 * 3600) * 100 = 64` |
| T-TSS-2 | 1800 s konstant 250 W, FTP 250 W | IF = 1,0, TSS = 50,0 | `1800 * 250 * 1 / (250 * 3600) * 100 = 50` |
| T-NP-2 | Wechsel 30 s mit 100 W und 30 s mit 300 W, lange Dauer, FTP 250 W | NP etwa 221,8 W (stetiger Grenzfall), Toleranz 2 Prozent bei 1 Hz | Gleitmittel läuft linear zwischen 100 und 300 W. `(300^5 - 100^5) / (5 * 200) = 2,42e9`, vierte Wurzel ergibt 221,8 |
| T-NP-3 | beliebiges Signal | `Mittelwert <= NP <= Maximum` | Eigenschaft der Potenzmittelung. Mittelwert meint das Mittel der 30-Sekunden-Fenstermittel, weil Randsamples in weniger Fenstern liegen und der rohe Mittelwert aller Samples deshalb keine Schranke ist (T01) |
| T-RTSS-1 | 3600 s bei Schwellengeschwindigkeit | rTSS = 100 | `1 * 1^2 * 100` |
| T-STSS-1 | 1800 s mit IF = 0,9 | sTSS = 36,45 | `0,5 * 0,9^3 * 100 = 36,45` |
| T-SRPE-1 | 60 min, RPE 6 | AU = 360, Verlaufslast = 36 | `6 * 60`, `1 * 0,36 * 100` |
| T-PMC-1 | konstante Last 100, Start 0, Tag 1 | CTL = 2,3528, ATL = 13,3122, TSB = 0 | siehe 8.2 |
| T-PMC-2 | Last 0 ab CTL0 = ATL0 = 50 | ATL nach 7 Tagen = 50 * e^-1 = 18,39 | exponentielles Abklingen mit tau 7 |
| T-HRMAX-1 | Alter 40 | 180 bpm | `208 - 0,7 * 40` |

Wo ein Referenzwert aus einer Sekundärquelle stammt (TRIMP-Konstanten, Zonengrenzen), steht im Test der Vermerk. Wird das Original beschafft und weicht ab, ändert sich der Parameter, nicht der Rechenweg.

Umsetzung in T05: T-PMC-1 (Tag 1 und Tag 2) und T-PMC-2 stehen als Konstanten mit Rechenweg im Testkommentar in `crates/core/src/pmc.rs`, Toleranz 1e-3. Ergänzend die Handrechnungen für die Zeit in Zonen (200 W bei FTP 250 liegen mit Anteil 0,8 in Zone 3), die Bestleistung (konstant 200 W gibt überall 200 W, Wechsel 300/100 W im 30-s-Takt gibt 300 W über 5 s und 200 W über 60 s), CSS (`200 / 170 = 1,176 m/s`) und die FTP-Schätzung (`0,95 * 300 = 285 W`).

### 11.3 Synthetische FIT-Dateien

`lastwerk-testkit` enthält einen Encoder für FIT-Dateien (Header, Definitionsnachrichten, Datennachrichten, CRC) und Generatoren:

- konstante Leistung, Intervallprofile, Rampen
- Herzfrequenz mit Drift
- Lücken, Spitzen und Aussetzer
- zwei Dateien derselben Fahrt (Edge und Uhr) mit leichtem Zeitversatz
- Dateien ohne Leistung, ohne Herzfrequenz, ohne `session`
- beschädigte Dateien (Prüfsumme, abgeschnitten)

Die Fixtures entstehen deterministisch aus dem Generator, im Speicher (API des Crates) und als Dateien in `target/fixtures` (`gen-fixtures --out`). Sie werden nicht eingecheckt, damit die Regel "kein `*.fit` im Index" ohne Ausnahme gilt (Entscheidung in Task T02, Stand 13. Oktober 2026). Jedes Fixture außer dem Edge/Uhr-Paar startet an einem eigenen Tag (Änderung in Task T04), sonst erkennt die Duplikatregel (6.4) gleichzeitig startende Fahrten gleicher Sportart und Dauer untereinander als eine Fahrt.

### 11.4 Weitere Testarten

| Art | Gegenstand | Werkzeug |
|---|---|---|
| Einheit | Formeln, Zonen, Duplikatregel | `cargo test` |
| Property | Planer: Ausgabe besteht `validate_plan` für zufällige Eingaben, keine harten Tage an Folgetagen, CTL-Anstieg höchstens R1, Verfügbarkeit nie überschritten | proptest |
| Golden | Plan-JSON, ICS, PMC-CSV für feste Eingaben | insta |
| Schema | Jeder erzeugte Plan gegen `plan.schema.json`. Negative Beispiele je Pflichtfeld | jsonschema |
| Migration | Datenbank N-1 nach N, Wiederaufbau aus FIT liefert dieselben Kennzahlen | `cargo test` |
| LLM (Attrappe) | Fake-Client liefert: gültig, ungültiges JSON, unbekannte Vorlage, Regelverletzung, Injektion im Erklärtext. Erwartung: höchstens zwei Korrekturen, danach Fallback, Text nie als HTML | `cargo test` |
| LLM (live) | Manueller Lauf gegen Ollama, `#[ignore]` | `cargo test -- --ignored` |
| Fuzz | `parse_activity` mit mutierten Dateien | cargo-fuzz oder proptest, ab M1 |
| UI | Komponententests, Rauchtest der Hauptansichten mit Attrappen-Backend | Vitest, Playwright |
| i18n | Schlüsselgleichheit von `de.json` und `en.json` | eigenes Skript in CI |
| Datenschutz | Kein getracktes `*.fit`, keine Dateien aus `local/` im Index, keine Treffer des Secret-Scanners | CI-Schritt |
| MCP-Datenschutz | Server über stdio starten, jedes Werkzeug mit synthetischer Datenbank aufrufen, Ausgabe gegen Feldliste prüfen. Datenbank enthält Markerwerte in Namen, Notizen, Koordinaten und Seriennummern. Kein Marker darf in einer Ausgabe vorkommen. Werkzeugliste darf nur `readOnlyHint = true` enthalten | `cargo test -p lastwerk-mcp` |
| Bridge, Archivkern | Allowlist gegen die Methodenliste der Bibliothek, Drosselung und Abbruch nach drei 429, Zeitlimit je Aufruf, Fortsetzung ohne Doppelabruf, negative Zwischenspeicherung leerer Antworten, atomares Schreiben, Manifest gegen Dateien (SHA-256). Attrappen-Client, Netzwerk gesperrt | `uv run pytest` in `bridge/` |
| Bridge, Quellen | Offline-Import der USB-Kopie mit synthetischen Fixtures, Abgleich gegen API-Aktivitäten (fehlende IDs in beide Richtungen) | `uv run pytest` in `bridge/` |
| Bridge, Isolation | Weder `bridge/` importiert aus `crates/` oder `app/`, noch umgekehrt. Prüfung per Pfadsuche in CI | CI-Schritt |
| Bridge, Datenschutz | `sensitive/` wird von `normalize` nicht gelesen, solange die Kategorie nicht freigegeben ist. Menstruations- und Schwangerschaftsmethoden stehen weder in der Allowlist noch werden sie aufgerufen | `uv run pytest` in `bridge/` |

Umsetzung Golden (T05): `insta` (Apache-2.0, nur Entwicklung) prüft `pmc.csv` für feste synthetische Tageslasten (60 Tage mit Wochenmuster und Erholungswoche, `crates/cli/src/report.rs`). Die Snapshots liegen unter `crates/cli/src/snapshots/` im Repository. Ein Snapshot wird mit `cargo insta accept` oder, wo `cargo-insta` fehlt, mit `INSTA_UPDATE=always cargo test` neu geschrieben und im Diff geprüft. In CI schreibt `insta` nie selbst. Die ersten drei Zeilen des Snapshots sind zusätzlich von Hand gerechnet (Test `golden_input_matches_the_hand_calculation_for_the_first_days`).

Umsetzung Planer (T07): Golden-Tests mit `insta` (`assert_json_snapshot!`) für zwei feste Kontexte (Termin in 12 Wochen, fortlaufend 8 Wochen), Snapshots unter `crates/core/src/plan/snapshots/`. Property-Tests mit proptest und `ProptestConfig { cases: 1000 }` über zufällige gültige Kontexte (Horizont bis 12 Wochen, Termin vor und hinter dem Horizont, wechselnde Verfügbarkeit, Sportarten und Historie). Die Eigenschaften aus der Tabelle werden unabhängig von `validate_plan` nachgerechnet (CTL-Anstieg mit eigenem `pmc`-Aufruf). Der Lauf dauert im Debug-Profil rund 4 Sekunden. Zusätzlich ein Determinismustest über 200 Kontexte. Neue Entwicklungsabhängigkeiten in `core`: `serde_json` (lädt die Kataloge im Test) und `insta` mit Feature `json`.

Umsetzung Golden und Schema (T08): `crates/cli/src/golden_tests.rs` prüft mit `insta` `plan.json` und `plan.ics` (deutsch, dazu der Anfang der englischen Fassung) für einen festen Kontext (Datenbank im Speicher mit Ziel und Verfügbarkeit, feste `plan_id` und Erzeugungszeit, die `planning::generate` als Parameter bekommt). Snapshots unter `crates/cli/src/snapshots/`. `pmc.csv` bleibt wie in T05. Die Schematests (7.1) nutzen `proptest` mit 300 Fällen. Neue Entwicklungsabhängigkeiten in `cli`: `proptest` (wie in `core`) und `icalendar` mit Feature `parser`.

Umsetzung LLM (T15): Die Attrappen-Fälle laufen in `crates/coach/tests/refine.rs` mit `FakeClient` (gültig, leere Änderungsliste, Verschieben und Streichen, ungültiges JSON, unbekannte Vorlage, Vorlage der falschen Sportart, Dauer außerhalb des Fensters, leerer Slot, belegter Zielslot, zwei harte Tage in Folge (R04), Verfügbarkeit (R11), Injektion im Erklärtext, Zeitüberschreitung, Netzfehler in einem Korrekturaufruf, gekürzte Erklärung, Basisplan mit harter Verletzung, weiche Verletzung als Warnung). "Höchstens zwei Korrekturen": drei ungültige Antworten enden nach genau drei Aufrufen im Fallback, auch bei `max_corrections = 200`. `crates/coach/tests/http.rs` startet einen Attrappen-Server (`std::net::TcpListener` auf Port 0) und prüft für beide Clients Adresse, Pfad, `stream: false`, Schema im Aufruf, Antwortform, Statuscodes, Weiterleitung (nicht gefolgt), Zeitlimit und die Strecke `refine_plan` bis zum Ergebnis. `crates/coach/tests/input.rs` hält Schlüssel und Texte des Eingangs fest. `crates/coach/tests/live.rs` ist der Live-Test (`#[ignore]`, Ollama auf `127.0.0.1:11434`, Modell aus `LASTWERK_OLLAMA_MODEL`). Die Tests der Hostprüfung stehen in `crates/coach/src/provider.rs`, der Markertest und die Tests der CLI in `crates/cli/tests/refine.rs`.

Umsetzung MCP-Datenschutz (T10): `crates/mcp/tests/` baut eine synthetische Datenbank über `lastwerk-store` mit Markerwerten in Zielbezeichnung, Schwellennotizen, Dateipfaden (`source_file.rel_path`), Gerätenummer, Produktname, Unterart, Einstellungen (Schlüssel und Werte), Plan-Erklärung und Erholungsquelle, dazu Koordinaten als Text. Jedes Werkzeug wird mit gültigen Parametern über dieselben Methoden aufgerufen wie das Protokoll (`LastwerkMcp::get_fitness_status` und so weiter, `privacy.rs`), die Antwort und das Aufrufprotokoll werden nach den Markern durchsucht und die Schlüssel jeder Antwort gegen die feste Feldliste verglichen (beim Plan gegen die Eigenschaften von `plan.schema.json` ohne `explanation`). Eine Gegenprobe prüft, dass die Marker in der Datenbank stehen. `tool_list.rs` hält die Werkzeugliste gegen die Tabelle in 5.4 und die Hinweise fest, `stdio.rs` startet das Programm als Kindprozess und spricht JSON-RPC über stdin (`initialize`, `tools/list`, `tools/call`, Zeitlimit 30 Sekunden), `tools.rs` rechnet `get_fitness_status` von Hand nach und prüft die Grenzen. Der MCP-Inspector (Paketname `@modelcontextprotocol/inspector` über `npm view` bestätigt, Version 2.10.1) lief einmal von Hand im CLI-Modus (`--cli <Aufruf> --method tools/list` und `tools/call`) und listete die neun Werkzeuge und beantwortete `explain_rule`. Der Inspector verschluckt Optionen wie `--db` des Servers, deshalb braucht der Aufruf ein kleines Skript, das `lastwerk-mcp --db ...` startet. Er ist nicht Teil der CI, der Test über stdio ersetzt ihn (M7).

### 11.5 Plausibilitätsläufe auf eigenen Daten

`lastwerk plausibility --dir local/fit` liest echte Dateien und gibt nur Kennzahlen aus (Anzahl, Spannweiten, Anteil nicht berechenbarer Aktivitäten, Duplikatpaare, auffällige NP/Durchschnitt-Verhältnisse). Die Ausgabe wird nicht gespeichert und nicht committet. Geprüft wird:

- Jahreslast und Wochenlasten sind in einer plausiblen Größenordnung für die bekannte Trainingshistorie.
- Duplikatregel findet erwartete Paare und keine falschen.
- Kein Absturz bei Dateien beider Geräte.

### 11.6 CI

GitHub Actions, Matrix Windows, Linux (bis 15. Oktober 2026 auch macOS):

1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test --workspace`
4. `cargo deny check` und `cargo audit`
5. `pnpm install --frozen-lockfile`, `pnpm lint`, `pnpm check`, `pnpm test`
6. Prüfung auf getrackte FIT-Dateien: `git ls-files "*.fit" "*.FIT"` muss leer sein
7. Schlüsselgleichheit der Sprachdateien
8. Tauri-Build ohne Signierung (Artefakt, kein Release)

Release auf Tag: Windows-Installer und Archive für Linux (bis 15. Oktober 2026 auch macOS) als Artefakte, ohne Signierung, mit Hinweis "ohne Support".

Umsetzung in T09: `.github/workflows/release.yml` läuft bei einem Tag `v*` (Matrix Windows, macOS, Linux) und baut mit `tauri build --config tauri.release.conf.json --bundles <liste>` (msi und nsis, dmg, deb und AppImage). Die Version kommt aus dem Tag. Die Artefakte werden mit `actions/upload-artifact` abgelegt, ein letzter Job (einziger mit `contents: write`, dazu `actions: read`) lädt sie mit `gh run download` und legt mit `gh release create --draft` einen Entwurf mit der Beschreibung "ohne Signierung, ohne Support" an. Der Entwurf wird von Hand veröffentlicht. Es gibt keine fremde Release-Action. Die Syntax aller Workflows prüft der Job `guards` der CI mit `actionlint` (v1.7.7). Der macOS-Installer ist ein dmg für die Architektur des Runners (`macos-latest`, Apple Silicon), ein Intel-Build ist nicht vorgesehen.

Änderung vom 15. Oktober 2026 (Entscheidung): macOS fällt aus CI und Release. Es ist kein Mac im Einsatz, und im privaten Repository zählte eine macOS-Minute in GitHub Actions zehnfach, eine Windows-Minute doppelt; das Freikontingent war nach einem Tag mit vielen Pushes erschöpft (Jobs starteten nicht mehr). Diese Begründung galt bis zur Umstellung auf ein öffentliches Repository, dort sind die Standard-Runner kostenlos; macOS wird dadurch nicht wieder aufgenommen, es bleibt entfallen. Die macOS-Pfade im Code (Datenordner, Schlüsselbund) bleiben, sind aber nicht mehr geprüft. Sessions bündeln ihre Arbeit weiterhin in möglichst einen Push (CLAUDE.md, Schritt 6), jetzt wegen CI-Last und Release-Takt: Jeder Push mit Produktänderung löst CI und Release aus.

Änderung vom 15. Oktober 2026 nach der Analyse aller 63 Läufe (13 von 32 abgeschlossenen CI-Läufen auf main rot, 11 davon nach Pushes ohne vollständigen lokalen Lauf, 10 Läufe durch Folge-Pushes abgebrochen):

- `scripts/check-all.sh` führt alle Prüfungen der CI lokal aus (auch das Tauri-Projekt, das `cargo test --workspace` nicht erfasst, `actionlint`, `cargo deny`, `bridge/`). CLAUDE.md verlangt den Lauf vor jedem Push.
- Rust ist in `rust-toolchain.toml` fest auf 1.99.0 gesetzt, CI und Release installieren genau diese Version (`rustup toolchain install` liest die Datei). Vorher zog die CI jede neue Stable-Version, deren neue Clippy-Lints Pushes ohne Codeänderung rot machten. Anheben geschieht bewusst mit eigenem Commit.
- Punkt 4 ist geteilt: Die Push-CI prüft `cargo deny check bans licenses sources` (Wurzel und Tauri-Projekt, cargo-deny fest 0.20.2, ein Job statt bisher zwei mit derselben Prüfung). RUSTSEC-Hinweise (`cargo deny check advisories`, `cargo audit`) prüft `.github/workflows/advisories.yml` täglich, auf Knopfdruck und als Vorbedingung jedes Releases (`release.yml`, Job `advisories`). Ein neuer Hinweis blockiert also das Release, nicht jeden Push.
- Läufe auf main werden nicht mehr abgebrochen (`cancel-in-progress` nur außerhalb von main). Die CI läuft auch auf Arbeitszweigen `claude/**`.
- Release ohne Tag-Push (15. Oktober 2026): `release.yml` lässt sich auch manuell auf `main` starten, mit dem neuen Tag als Eingabe (Muster `vX.Y.Z`, Tag darf noch nicht existieren). Der Entwurf zeigt dann auf den Commit des Laufs; `publish-release.yml` veröffentlicht ihn, und GitHub legt den Tag dabei an. Grund: Cloud-Sessions können keine Tags pushen. Der Weg über einen gepushten Tag bleibt unverändert.
- Automatischer Release (T30, Entscheidung und Anforderung vom 15. Oktober 2026): Nach jeder grünen CI eines Pushes auf `main` prüft `.github/workflows/auto-release.yml`, ob sich seit dem letzten Tag etwas am Produkt geändert hat (`app/`, `crates/`, `catalog/`, `schema/`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`). Wenn ja und der Commit noch die Spitze von `main` ist, ruft er `release.yml` mit dem nächsten Tag und genau diesem Commit auf und veröffentlicht den Release ohne Entwurfsprüfung von Hand. Nächster Tag: Patch + 1, außer `catalog/changelog.json` nennt schon eine höhere Version. Fehlt der Changelog-Eintrag, stehen die Betreffzeilen der Commits seit dem letzten Tag in den Notizen und in `latest.json`. Damit findet "Nach Update suchen" jeden Stand von `main` mit Produktänderung, und der Installer am neuesten Release ist immer aktuell. Abweichung von "Der Entwurf wird von Hand veröffentlicht" oben; Tag-Push und manueller Start bleiben möglich. Gibt es noch keinen Tag (Neuanfang nach T38), gilt die höchste Version aus `catalog/changelog.json`.
- Signaturschlüssel des Updaters (T38): Neues Schlüsselpaar zum Neuanfang, weil der alte private Schlüssel nicht mehr vorlag. Er liegt nur in der GitHub-Umgebung `release`, deren Zweigregel nur `main` zulässt; der Job `build` in `release.yml` nennt diese Umgebung. Versionen bis 1.0.2 prüfen gegen den alten öffentlichen Schlüssel und lehnen das Update auf 1.0.3 ab; sie werden einmal von Hand neu installiert.
- Windows nur noch als NSIS-Installer (`-setup.exe`, T30): Das Update aus der App installiert die `setup.exe`, die Frage nach Ollama hat nur der NSIS-Installer (T29), und ein per MSI installiertes Lastwerk bekäme beim Update eine zweite Installation daneben. Abweichung von "msi und nsis" oben.
- Jeder Job hat ein Zeitlimit (`timeout-minutes`). Der Tauri-Job baut mit Zeilentabellen statt voller Debug-Information und gibt Platz und Speicher vor den Tests aus (Linker-Abbruch "Bus error" am 15. Oktober 2026).

---

## 12. Meilensteine

Aufwandsangaben sind Schätzungen bei mehr als 10 Stunden pro Woche und nicht belegt.

| Nr. | Inhalt | Umfang (Schätzung) |
|---|---|---|
| M0 | Umgebung, Repository-Gerüst, CI, Datenschutzprüfungen | 1 Woche |
| M1 | FIT-Import, Duplikate, Belastung je Aktivität, SQLite | 3 bis 4 Wochen |
| M2 | Schwellenwerte, Zonen, PMC, Oberfläche (Übersichten, Detail) | 3 bis 4 Wochen |
| M3 | Planer, Vorlagen, Leitplanken, Plan- und ICS-Export, Backup | 4 bis 5 Wochen |
| M4a | Zusatzprogramm: Sonde (Phase 0), Archivkern mit Anmeldung, Drosselung, Fortsetzung, Manifest, Allowlist | 2 bis 3 Wochen |
| M4b | Zusatzprogramm: `backfill` und `sync` für Aktivitäten und Tageswerte, Abdeckungsbericht | 2 bis 3 Wochen |
| M4c | Zusatzprogramm: USB-Kopie als optionaler Rückfall, Abgleich (kann entfallen) | 1 Woche |
| M4d | Normalisierung nach `exchange/out`, Import der Erholungsdaten in Lastwerk, Planregel R12 | 1 bis 2 Wochen |
| M5 | LLM-Anbindung (Ollama, lokale OpenAI-kompatible Server) | 3 bis 4 Wochen |
| M6 | Workout-Übertragung über Zusatzprogramm, Entscheidung PWA | 2 bis 3 Wochen |
| M7 | MCP-Server `lastwerk-mcp` für Claude und andere Clients (nur lesend, gefilterte Ausgabe) | 2 bis 3 Wochen |

Version 1 ist der Stand nach M3. M7 setzt M2 (Kennzahlen) und M3 (Plan) voraus und kann vor M4a bis M6 gezogen werden, weil es ohne Garmin-Zugang auskommt.

### M0: Umgebung und Gerüst

Inhalt: Toolchains installiert, Workspace mit leeren Crates, Svelte-Projekt, `.vscode`, `.gitignore`, CI, Lizenzdateien, README mit Datenschutzhinweis. Die Tauri-Hülle `app/src-tauri` entsteht abweichend erst in M2 (Task T06), weil sie Systembibliotheken braucht und nicht Teil der M0-Abnahme ist. Umgesetzt in Task T00.

Abnahmeprüfung (alle Befehle im Repository-Wurzelverzeichnis, PowerShell):

```
rustc --version; cargo --version; node --version; pnpm --version
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm --dir app/ui install --frozen-lockfile; pnpm --dir app/ui check
git check-ignore -v local/test.fit
if ((git ls-files "*.fit" "*.FIT").Length -gt 0) { exit 1 }
```

Erfolg: alle Befehle enden mit Exit-Code 0, `git check-ignore` nennt die Regel für `local/`.

### M1: Import und Belastung

Abnahmeprüfung:

```
cargo test -p lastwerk-core load::
cargo test -p lastwerk-fit
cargo run -p lastwerk-testkit --bin gen-fixtures -- --out target/fixtures
cargo run -p lastwerk-cli -- import --dir target/fixtures --db target/m1.db
cargo run -p lastwerk-cli -- rebuild --dir target/fixtures --db target/m1b.db
```

Erfolg:

- Referenzwerte T-NP-1, T-NP-3, T-TSS-1, T-TSS-2, T-RTSS-1, T-STSS-1, T-SRPE-1 grün.
- Importbericht der Fixtures enthält die erwartete Anzahl: je Fixture eine Aktivität, das Edge/Uhr-Paar ergibt genau eine primäre Aktivität, die beschädigten Dateien erscheinen als Fehler und der Import läuft zu Ende.
- `import` und `rebuild` erzeugen identische Kennzahlen (Test vergleicht beide Datenbanken).

### M2: Auswertung und Oberfläche

Abnahmeprüfung:

```
cargo test -p lastwerk-core pmc::
cargo run -p lastwerk-cli -- pmc --db target/m1.db --from 2026-01-01 --to 2026-03-31 --format csv > target/pmc.csv
pnpm --dir app/ui test
pnpm --dir app/ui exec playwright test
pnpm --dir app/ui exec tauri build --debug
```

Erfolg:

- T-PMC-1 und T-PMC-2 grün, `pmc.csv` entspricht dem Golden-Snapshot.
- Playwright-Rauchtest öffnet Wochenübersicht, Aktivitätsliste, Detail und Einstellungen im Deutschen und im Englischen ohne Konsolenfehler.
- Debug-Build der App entsteht und startet (Prozess läuft 5 Sekunden ohne Absturz).
- Kein Netzwerkverkehr im Test (Playwright blockiert externe Hosts, Test schlägt bei Anfrage fehl).

### M3: Planer, Export, Version 1

Abnahmeprüfung:

```
cargo test -p lastwerk-core plan::
cargo run -p lastwerk-cli -- plan --db target/m1.db --week-start 2027-01-04 --goal-date 2027-05-01 --out target/plan.json
cargo run -p lastwerk-cli -- validate target/plan.json --db target/m1.db
cargo run -p lastwerk-cli -- export-ics target/plan.json --out target/plan.ics
cargo run -p lastwerk-cli -- backup --db target/m1.db --out target/backup.json
```

Erfolg:

- `validate` endet mit Exit-Code 0 und meldet keine Verletzung harter Regeln.
- Property-Tests des Planers laufen mit mindestens 1000 Fällen ohne Gegenbeispiel.
- `plan.json` besteht die Prüfung gegen `schema/plan.schema.json`. Jede Pflichtfeld-Verletzung im Negativtest wird erkannt.
- `plan.ics` lässt sich mit der `icalendar`-Bibliothek einlesen und enthält eine Veranstaltung je Einheit, ohne Ortsangaben.
- Löschen der Datenbank, Rückspielen von `backup.json`, `rebuild` ergibt dieselben Schwellenwerte, Ziele und Pläne.
- Tag `v0.1.0` baut in CI auf allen drei Plattformen.

### M4: Garmin-Vollabzug, Erholung und Zusatzprogramm

M4 teilt sich in vier Abschnitte, die einzeln abnehmbar sind. Befehle des Zusatzprogramms laufen im Ordner `bridge/` (Python 3.12 über `uv`), `cargo`-Befehle im Wurzelverzeichnis des Repositories. Die Garmin-Anmeldung findet erst hier in M4a statt und wird vom Nutzer interaktiv ausgeführt.

#### M4a: Sonde und Archivkern

Teil 1, Sonde (Phase 0, vor dem Bau des Kerns):

1. Python 3.12 über `uv` festlegen (`uv python install 3.12`), `garminconnect` und `curl_cffi` in der gepinnten Version in einer Projektumgebung installieren.
2. `lastwerk-bridge auth` einmalig ausführen (interaktiv, MFA). Token liegen im Benutzerprofil, nicht im Repository.
3. `lastwerk-bridge probe`: je Datenart aus 5.5 genau ein Aufruf, möglichst für ein Datum der letzten Woche. Danach eine Messung der größten Bereichsspanne je Bereichsendpunkt und der Historientiefe (ältestes Datum mit Daten). Alle Aufrufe gedrosselt.
4. Echte Antworten bleiben in `local/`, nicht im Repository.
5. Ergebnis: Kurzbericht `docs/garmin-feldabdeckung.md` mit Feldnamen, Datentypen und Abdeckung ohne Werte, erstes `registry.yaml`, Startwert der Drosselung (O24). Der Bericht legt die endgültigen Felder von `recovery-day` fest (O9).

Abnahme Teil 1: Der Bericht existiert, enthält je Datenart die Angabe "verfügbar", "leer" oder "nicht verfügbar" mit Feldnamen, und `git ls-files local` ist leer.

Teil 2, Archivkern. Abnahmeprüfung (offline, Attrappen-Client):

```
uv run pytest -q tests/test_allowlist.py tests/test_manifest.py tests/test_throttle.py tests/test_resume.py
uv run python -m lastwerk_bridge auth --check          (manuell, genau ein Aufruf)
```

Erfolg:

- Der Allowlist-Test vergleicht alle öffentlichen Methoden der installierten Bibliothek mit Allowlist und Sperrliste. Jede Methode steht in genau einer Liste. Menstruations- und Schwangerschaftsmethoden stehen in der Sperrliste und werden nie aufgerufen.
- Simulierte Antworten 429 oder 403 stoppen den Lauf nach drei Fehlern in Folge, der Zustand ist gespeichert, der Prozess endet mit Exit-Code 3 und klarer Meldung.
- Ein hängender Aufruf wird durch das Zeitlimit beendet.
- Ein zweiter Lauf holt nichts doppelt. Leere Antworten werden nicht erneut abgefragt, die letzten N Tage schon.
- Jede Datei im Archiv hat genau einen passenden Manifest-Eintrag mit korrekter SHA-256.
- Ohne vorherigen `auth`-Aufruf entsteht keine Netzwerkanfrage.

#### M4b: Backfill und Sync

Abnahmeprüfung (zuerst offline, danach manuell am echten Konto durch den Nutzer):

```
uv run pytest -q tests/test_backfill.py tests/test_sync.py tests/test_coverage.py
uv run python -m lastwerk_bridge backfill --root <archiv> --max-calls 200 --dry-run
uv run python -m lastwerk_bridge backfill --root <archiv> --max-calls 200
uv run python -m lastwerk_bridge sync --root <archiv>
uv run python -m lastwerk_bridge coverage --root <archiv>
```

Erfolg:

- `--dry-run` listet die geplanten Aufrufe mit Anzahl je Endpunkt und sendet nichts.
- Ein Lauf mit `--max-calls 200` endet nach höchstens 200 Aufrufen mit Zusammenfassung und lässt sich fortsetzen.
- `backfill` arbeitet von neu nach alt und erreicht das früheste Datum des Kontos (aus Profil und ältester Aktivität).
- `coverage` listet jeden Registereintrag mit Status, ältester und jüngster Datumsangabe und Anzahl Dateien. Einträge ohne Daten stehen als "leer" oder "nicht verfügbar".
- Aktivitäten haben `original.fit`, soweit Garmin es ausliefert. Fehlende stehen im Bericht.

#### M4c: USB-Kopie (optional)

Rückfall für den Fall, dass der Garmin-Zugang gesperrt ist. Das Werkzeug ist ohne diesen Abschnitt vollständig nutzbar. Der Garmin-Datenexport ist keine Quelle.

Abnahmeprüfung:

```
uv run pytest -q tests/test_usb_import.py tests/test_reconcile.py
```

Abweichung (T13, 13. Oktober 2026): Der Ordner `tests/fixtures/device` entfällt, weil `*.fit` im Index nicht erlaubt sind (SPEC 1.5, 11.4). Die Geräteordner entstehen zur Laufzeit unter `tmp_path` (`tests/device_fixture.py`: minimaler FIT-Kopf mit Kennung `.FIT`, wenige Bytes, Datei-CRC). Statt der beiden Einzelbefehle gegen den Ordner führen die Tests `import-device` und `reconcile` als Unterprozess (`python -m lastwerk_bridge`) gegen ein temporäres Verzeichnis aus. Von Hand mit einem echten Gerät: `uv run python -m lastwerk_bridge import-device PFAD --root ../local/archive/garmin`, danach `reconcile --root ../local/archive/garmin`.

Erfolg:

- `import-device` übernimmt FIT-Dateien aus Activity, Monitor und Sleep unverändert, vermerkt sie im Manifest und liefert einen Prüfbericht mit Anzahl erkannter, unbekannter und übersprungener Dateien.
- `reconcile` meldet Aktivitäten, die nur in der API oder nur auf dem Gerät vorkommen, als Liste ohne Werte.
- Ohne vorhandenes Gerät oder Ordner bricht der Befehl mit klarer Meldung und Exit-Code 2 ab.

Festlegungen (T13, Konventionen bis zur Prüfung am echten Gerät, O4):

- **Erkennung.** Nur über Ordnername (`Activity`, `Monitor`, `Sleep`, Groß- und Kleinschreibung egal, direkt im Datenträger oder unter `GARMIN/`), Endung `.fit` (Groß- und Kleinschreibung egal) und Kopfkennung (Byte 0 gleich 12 oder 14, Byte 8 bis 11 gleich `.FIT`). Die Bridge parst kein FIT. Alles andere in diesen Ordnern, jede `.fit`-Datei in anderen Ordnern und jeder Symlink zählen als unbekannt und werden nicht kopiert.
- **Ablage.** `device_usb/<geraet>/<Ordner>/<dateiname>`, Gerätename aus `--device`, sonst Name des Ordners (auf `A-Z a-z 0-9 . _ -` beschränkt). Manifest-Eintrag `kind: device_usb`, `endpoint: device_usb/<Ordner>`, `params: {device, file}`, `lib_version: lastwerk-bridge <Version>`. Gleiche SHA-256 unter demselben Schlüssel: übersprungen. Gleicher Name, anderer Inhalt: neue Datei mit Zeitstempel im Namen (Archivregel), der jüngste Eintrag gilt, im Bericht als "abweichend" gezählt.
- **Berichte.** `device-import-report.json` und `reconcile-report.json` sind abgeleitet (kein Manifest-Eintrag) und enthalten nur Zählungen, Kennungen und Dateinamen.
- **Zuordnung in `reconcile`.** Der Dateiname trägt entweder die Aktivitätskennung (mindestens sechs Ziffern, optional `_ACTIVITY`) oder den Startzeitpunkt `JJJJ-MM-TT-hh-mm-ss`. Der Zeitstempel gilt als UTC (`--name-offset-minutes` rechnet Ortszeit um) und wird mit `startTimeGMT` der API-Aktivität verglichen, Toleranz 120 s (Konvention, `--tolerance`). Je Aktivität und Datei höchstens eine Zuordnung, kleinste Abweichung zuerst. Das Muster ist über `--name-pattern` änderbar. Dateien mit unpassendem Namen stehen unter `nicht_zuordenbar`, nicht unter `nur_geraet`. Die Startzeit stammt aus `summary.json` (`startTimeGMT`, auch unter `summaryDTO`), sonst aus der Listenseite; der Ort des Feldes ist bis zur Sonde in L1 eine Annahme.
- **`coverage`.** Aktivitäten ohne `original.fit`, denen eine Gerätedatei zugeordnet ist, erscheinen mit dem Zusatz "über Gerät vorhanden". Ohne Gerätekopie bleibt der Bericht unverändert.

#### M4d: Normalisierung, Import und Planregel

Abnahmeprüfung:

```
uv run python -m lastwerk_bridge normalize --root target/archive --out ../target/exchange/out
cargo run -p lastwerk-cli -- import --dir target/exchange/out/activities --db target/m4.db
cargo run -p lastwerk-cli -- import-recovery --dir target/exchange/out/recovery --db target/m4.db
cargo test -p lastwerk-core plan::recovery
```

Abweichung (T14, 13. Oktober 2026): Der Ordner `tests/fixtures/archive` entfällt, weil ein Archiv `*.fit` enthielte, die nicht im Index stehen dürfen (SPEC 1.5, 11.4). Das Archiv entsteht zur Laufzeit: `bridge/tests/test_normalize.py` führt `backfill` gegen die Attrappe des Kontos aus (`bridge/tests/synthetic.py`, erfundene Tageswerte, Markerwerte in `sensitive/`) und `normalize` als Unterprozess. Ein Test schreibt dabei nach `../target/exchange/out`, danach läuft der Rust-Befehl wörtlich:

```
cd bridge
uv run pytest -q tests/test_normalize.py
cd ..
cargo run -p lastwerk-cli -- import-recovery --dir target/exchange/out/recovery --db target/m4.db
cargo test -p lastwerk-core plan::recovery
cargo test -p lastwerk-cli --test recovery
```

`import --dir target/exchange/out/activities` liest dort Attrappen-Bytes und meldet sie als fehlerhaft (`invalid_header`), weil die Fixtures keine gültigen FIT-Aktivitäten sind; mit dem echten Archiv liest er die Originale. Von Hand mit dem echten Archiv: `normalize --root ../local/archive/garmin --out ../local/exchange/out`, danach `lastwerk import --dir local/exchange/out/activities` und `lastwerk import-recovery --dir local/exchange/out/recovery`.

Erfolg:

- Alle erzeugten `recovery-day`-Dateien validieren gegen das Schema, ungültige werden abgelehnt und gezählt.
- `normalize` liest `sensitive/` nicht, solange keine Kategorie freigegeben ist. Ein Test legt Markerwerte dorthin und findet sie nicht in der Ausgabe.
- `normalize` ist wiederholbar und liefert dieselben Dateien (Prüfsummenvergleich), ohne einen Garmin-Aufruf.
- Planregel R12 ersetzt in den Testfällen die nächste harte Einheit, sonst nicht.
- Das Zusatzprogramm läuft nur, wenn der Nutzer es startet. Lastwerk startet es nie.

### M5: LLM

Abnahmeprüfung:

```
cargo test -p lastwerk-coach
cargo test -p lastwerk-coach -- --ignored      (manuell, Ollama läuft, Modell gezogen)
cargo run -p lastwerk-cli -- refine target/plan.json --provider ollama --model <name> --db target/m1.db
```

Erfolg:

- Alle Attrappen-Fälle aus 11.4 grün, einschließlich "höchstens zwei Korrekturen".
- Live-Lauf: Von 20 Läufen mit festem Eingang liefert kein Lauf einen Plan aus, der `validate` verletzt. Fehlschläge enden im Fallback und werden mit Grund gezählt.
- Der Prompt-Eingang enthält in keinem Test Freitext aus Fixtures (Test sucht die Marker-Zeichenketten der Fixtures).
- Gemessene Antwortzeit auf dem Zielrechner steht im Bericht des Laufs. Ein Grenzwert ist offen.

Umsetzung (T15, Stand 13. Oktober 2026): Die Offline-Kriterien sind erfüllt (siehe 11.4, Umsetzung LLM). Offen und nur lokal zu erledigen: der Live-Lauf mit 20 Läufen gegen ein echtes Modell und die Messung der Antwortzeit (O13). Der Befehl aus der Abnahme lautet unverändert, `--yes` lässt die Rückfrage aus: `cargo run -p lastwerk-cli -- refine target/plan.json --provider ollama --model <name> --db target/m1.db --yes`.

### M6: Handy und Workouts

Abnahmeprüfung:

```
python -m lastwerk_bridge push --plan target/plan.json --dry-run
cargo run -p lastwerk-cli -- export-ics target/plan.json --out target/plan.ics
```

Erfolg:

- `--dry-run` gibt je Einheit die Nutzlast aus und sendet nichts.
- Die Nutzlast je Vorlage entspricht der erwarteten Struktur (Test gegen aufgezeichnete Beispiele).
- Entscheidung PWA ist dokumentiert: Variante, Erreichbarkeit, Datenschutzfolge.

Umsetzung in T16 (Abweichungen): Es gibt keine aufgezeichneten echten Antworten oder Workouts, daher prüft der Test die Nutzlast gegen synthetische Beispiele, die aus Dokumentation und Quelltext der Bibliothek abgeleitet und handgerechnet sind (Q21, Q27, 14. Oktober 2026). Die Struktur bleibt bis zum ersten echten Versand eine Annahme. Die Abnahme führt den Befehl im Ordner `bridge/` mit `uv run` und dem Pfad `../target/plan.json` aus (Einzelheiten in 5.5, "Festlegungen aus M6"). Der echte Versand (`--yes`) ist implementiert und mit Attrappen getestet, wurde nie gegen ein Konto ausgeführt. Die PWA-Entscheidung ist eine Empfehlung, die der Nutzer noch bestätigen muss (O5, `docs/pwa-entscheidung.md`).

### M7: MCP-Server

Voraussetzung: M2 und M3. Der Server kommt ohne Garmin-Zugang aus.

Abnahmeprüfung:

```
cargo test -p lastwerk-mcp
cargo run -p lastwerk-mcp -- --db target/m1.db --list-tools
npx @modelcontextprotocol/inspector cargo run -p lastwerk-mcp -- --db target/m1.db
```

Erfolg:

- `--list-tools` nennt genau die Werkzeuge aus 5.4, jedes mit `readOnlyHint = true`. Ein Test schlägt fehl, sobald ein weiteres Werkzeug oder ein fehlender Hinweis auftaucht.
- Der Markertest findet in keiner Ausgabe Koordinaten, Aktivitätsnamen, Notizen, Seriennummern oder Dateipfade der synthetischen Datenbank.
- `get_recovery_days` liefert ohne Freigabe in den Einstellungen einen Fehler, mit Freigabe die Tageswerte.
- Der Server öffnet die Datenbank schreibgeschützt. Ein Test versucht ein `INSERT` über die Verbindung des Servers und erwartet einen Fehler.
- Der Paketname des MCP-Inspectors (`@modelcontextprotocol/inspector`) ist vor der ersten Nutzung zu prüfen. Alternativ gilt der Test über stdio ohne Inspector.
- Mit Claude Desktop oder Claude Code verbunden (Konfiguration im README) beantwortet ein Gespräch die Frage "Wie ist meine Form diese Woche?" mit den Werten aus `get_fitness_status`. Das ist ein manueller Rauchtest.
- Das Aufrufprotokoll enthält je Aufruf Werkzeug, Parameter und Zeilenzahl, aber keine Nutzdaten.

---

## 13. Zusatzprogramm (Ordner `bridge/`)

**Seit T20 eingefroren.** Die App holt Garmin-Daten selbst (5.6). Das Programm bleibt für Vollabzug, Archiv und `push` lauffähig, bekommt aber keine neuen Funktionen.

Arbeitsname: `lastwerk-bridge`. Python 3.12, im selben Repository wie der Kern, aber strikt getrennt: keine gemeinsamen Importe, Austausch nur über Dateien (5.4), eigene CI-Datei, eigene README mit Hinweis auf den inoffiziellen Zugang (1.5). Lizenz wie das Gesamtprojekt (MIT OR Apache-2.0).

| Punkt | Festlegung |
|---|---|
| Grundlage | python-garminconnect (MIT). Nutzt die mobile Anmeldung der Garmin-Connect-App, MFA über Rückruf, Tokens lokal gespeichert. Q21 |
| Garmin-Zugang | Inoffiziell. Kann jederzeit brechen. Schreibende Funktionen können Kontodaten ändern (laut Bibliothek), deshalb nur lesend (Allowlist, 5.5). Einzige Ausnahme: `push --yes` (M6) mit eigener Freigabe für `upload_workout` und `schedule_workout`, nur nach ausdrücklichem Aufruf und mit Protokoll |
| Offizieller Weg | Garmin Training API für Workouts nur für freigegebene Partner, daher nicht nutzbar |
| Funktionen | Vollabzug aller lesbaren Datenarten rückwirkend ab Kontoerstellung in ein Archiv (5.5), optional USB-Kopie, Normalisierung nach `exchange/out`, später Workouts aus `plan.json` anlegen und planen (M6) |
| Schichten der Vollständigkeit | 1. typisierte Methoden der Bibliothek, 2. freie Methoden `connectapi` und GraphQL für Endpunkte ohne Wrapper (Pfade aus der eigenen Browsersitzung, O27), 3. optional USB-Kopie für minutengenaue Wellness-Dateien der Uhr. Der Garmin-Datenexport gehört nicht dazu |
| Erwartete Lücken der Bibliothek | Rohdaten außerhalb der Connect-Oberfläche, minutengenaue Wellness-Dateien, Daten, die Garmin nicht mehr ausliefert. Abdeckung wird je Konto gemessen (`coverage`), nicht angenommen. Lücken werden nicht über den Datenexport geschlossen |
| Sicherheit | Token-Datei wie ein Passwort behandeln. Zugangsdaten nie in Lastwerk. Trockenlauf als Standard für Schreibaktionen |
| Austausch | Dateien nach Abschnitt 5.4 |
| Bibliothekslage | `garth` ist als veraltet markiert, weil Garmin den Anmeldeweg geändert hat. python-garminconnect nutzt es seit Version 0.3.0 nicht mehr (Q21) |
| Bibliotheksstand | python-garminconnect 0.3.17 (29. September 2026), Python 3.12 oder neuer, nahezu wöchentliche Releases. Die gepinnte Version wird erst nach Offline-Tests und einem Live-Rauchtest angehoben (Q21) |
| Bekannte Fehlerbilder | Cloudflare weist die Standard-Anfragebibliothek ab (403), erster SSO-Aufruf kann 429 liefern, Anmeldung scheitert bei manchen Windows-Rechnern, Tokens gehen nach Neustart verloren (Issues in python-garminconnect und garmin_mcp, Q21, Q23) |
| Gegenmaßnahmen | Zeitlimit und begrenzte Wiederholung mit Wartezeit, Aufrufe außerhalb des Event-Loops, eigene Token-Verzeichnisse, klare Fehlermeldung statt Endlosschleife |
| Rückfall bei Sperre | Der Kern liest FIT-Dateien aus dem Ordner `local/fit/`. Aktivitäten lassen sich ohne Garmin-Zugang per USB-Kopie dorthin bringen. Der Garmin-Datenexport ist ausdrücklich nicht vorgesehen |
| Ablageorte | Archiv `local/archive/garmin/`, Austausch `local/exchange/`, Sondenantworten `local/probe/`, Token im Benutzerprofil (`~/.garminconnect`) |
| Beziehung zu garmin_mcp | Gleiche Bibliothek, aber nicht dieselbe Version: garmin_mcp ist auf 0.3.2 gepinnt, Lastwerk braucht mindestens 0.3.17 (Trainingslast und Trainingsstatus gibt es erst ab 0.3.14). Die Sonde läuft deshalb direkt auf der Bibliothek, garmin_mcp ist nur optionaler Vergleich und keine Laufzeitabhängigkeit. Gründe: Pin, rund 150 Werkzeuge belasten den Kontext, schreibende und löschende Werkzeuge, HTTP-Modus ohne Authentifizierung (Q23) |
| Rohdaten-Server | Optional und nur lokal: schlanker lesender MCP-Server auf demselben Kern mit etwa 15 Werkzeugen. Wird nie mit einem Cloud-Client verbunden (9.6) |

Der Kern bleibt ohne dieses Programm vollständig nutzbar.

---

## 14. Offene Fragen

| Nr. | Frage | Warum offen | Entscheidung bei |
|---|---|---|---|
| O1 | Verbietet die Strava API Agreement die Nutzung von API-Daten mit KI-Modellen, einschließlich Prompts und RAG? | Abrufbarer Vertragstext (wirksam 1. Juni 2026) nennt keine KI-Klausel, mehrere Presseberichte behaupten ein Verbot. Möglicherweise lag nur ein unvollständiger Abruf vor | vor F33 |
| O2 | Unter welchen Bedingungen darf das FIT-Profil (Garmin FIT SDK) in einem Open-Source-Projekt weitergegeben werden? Enthält `fitparser` Garmin-Profildaten, und ist das mit MIT OR Apache-2.0 vereinbar? | **Entschieden am 13. Oktober 2026 (T03): eigener minimaler Decoder, kein `fitparser`, keine Garmin-Profildateien im Repository.** `fitparser` (MIT, Copyright 2020 Matthew Stadelman) enthält laut eigener Dokumentation aus Garmins `Profile.xlsx` erzeugten Profilcode ("pulled from ANT's official SDK"), die erzeugten Dateien sind im Repository eingecheckt. Die Lizenzseiten von Garmin (developer.garmin.com/fit/download, thisisant.developer.garmin.com FIT Protocol License, forums.garmin.com) waren aus der Cloud-Umgebung nicht erreichbar (DNS-Fehler). Suchzusammenfassungen nennen eine nicht exklusive, gebührenfreie Lizenz mit Beschränkungen bei der Weitergabe der SDK-Quelldateien ("internal business purposes"). Die Lage ist unklar, deshalb gilt der Standard aus Task T03. `lastwerk-fit` und `lastwerk-testkit` führen nur die benötigten Nachrichten- und Feldnummern als eigene Konstanten nach öffentlicher FIT-Protokoll- und Profilbeschreibung. Neu bewerten, sobald die Lizenz im Original gelesen werden kann (dann wäre ein Umstieg auf `fitparser` möglich, ist aber nicht nötig) | entschieden, erneut bei Klärung von Q19 |
| O3 | Welcher Kachelserver, falls Karten aktiviert werden? | Nutzungsbedingungen des Anbieters, Eigenbetrieb möglich | vor F30 |
| O4 | Pfad für Workout-Dateien auf Edge und Uhr (`GARMIN/NewFiles` oder `GARMIN/Workouts`)? | Je Gerätetyp zu prüfen. Stand 14. Oktober 2026 (T16): nicht berührt. `push` legt Workouts in Garmin Connect an (Kalender), schreibt keine Dateien auf ein Gerät und kopiert nichts aus `GARMIN/NewFiles` oder `GARMIN/Workouts` | vor F32 |
| O5 | PWA: Wie wird sie vom Handy erreicht, ohne Daten an Dritte zu geben? | Lokale App hat keine erreichbare Adresse. Optionen: LAN-Server, privater Tunnel, statische Veröffentlichung (gibt Daten weiter). **Stand 13. Oktober 2026 (T16): Empfehlung des Tasks, Entscheidung des Projekts offen.** Die PWA jetzt nicht bauen (F31 bleibt "Kann"; ICS und Garmin-Connect-App decken die Hauptnutzung ab). Wenn gebaut wird: Variante A (lesender Server im Heimnetz, standardmäßig aus, Token-Kopplung, TLS) als Grundlage, Variante B (privater Tunnel zwischen eigenen Geräten) als Betriebsoption des Nutzers, Variante C (statisch) nicht für Version 1. Begründung, Tabellen und offene Prüfpunkte in [pwa-entscheidung.md](pwa-entscheidung.md) | M6 (Empfehlung), Entscheidung offen |
| O6 | Markenrecherche für "Lastwerk" (DPMA, EUIPO), Namensraum auf GitHub | Nur crates.io und GitHub-Suche geprüft | vor Veröffentlichung |
| O7 | Zusatzprogramm: Umgang mit Nutzungsbedingungen von Garmin, MFA, regionalen Besonderheiten, Pflege der Bibliothek | Inoffizieller Zugang | M4a |
| O8 | Originalquellen für TRIMP-Konstanten (Banister 1991), Coggan-Zonengrenzen, Friel-Zonen, Steigungskorrektur im Laufen, Taper-Dauer und -Reduktion (Bosquet 2007), 7-Tage-Mittel für HRV (Plews 2013) | Bisher nur Sekundärquellen | vor Implementierung der jeweiligen Regel |
| O9 | Feldnamen und Struktur der Erholungsdaten aus dem Zusatzprogramm | Ausgabe der Bibliothek noch nicht gesichtet. Stand 13. Oktober 2026 (T14): `docs/garmin-feldabdeckung.md` aus L1 liegt nicht vor, der Entwurf aus 7.3 gilt und ist vorläufig. Die Zuordnung Schema-Feld zu Antwortfeld der Bibliothek (`FIELD_MAP` in `normalize.py`) ist eine Annahme und nach der Sonde zu prüfen und anzupassen Stand 14. Oktober 2026 (L1): `docs/garmin-feldabdeckung.md` liegt vor. `FIELD_MAP` in `normalize.py` und `schema/recovery-day.schema.json` sind gegen die erfassten Feldnamen noch zu prüfen, das ist ein eigener Schritt (T14 nacharbeiten). Stand 15. Oktober 2026 (T22): Der Abgleich in der App liest die Felder `hrvSummaries[].calendarDate/.lastNightAvg` (`hrv_range`), `allMetrics.metricsMap.WELLNESS_RESTING_HEART_RATE[].calendarDate/.value` oder die Liste `[].calendarDate/.value` (`rhr_range`) und `[].date` mit `bodyBatteryValuesArray[][1]` (`body_battery`). Der Bericht zeigt für die zwei Bereichsendpunkte nur Strukturen: `hrvSummaries` ohne Unterfelder (die Namen sind vom Tagesendpunkt übertragen) und `rhr_daily` als Liste (die Rohantwort stammt aus dem Quelltext von garminconnect 0.3.17). Beides ist eine **Annahme** bis zum Livetest in T23 und im Code (`crates/cli/src/garmin/parse.rs`) gekennzeichnet. | M4a (Sonde) |
| O10 | Toleranzen der Duplikatregel (120 s, 10 Prozent) | Konvention, an echten Dateipaaren prüfen | M1 (Plausibilität) |
| O11 | Diagrammbibliothek (z. B. uPlot, ECharts) | **Entschieden am 13. Oktober 2026 (T06): uPlot 1.6.32 (MIT).** Canvas, klein (rund 50 KB), keine Remote-Ressourcen, schnell bei langen Messreihen. ECharts 6.1.0 (Apache-2.0) wäre deutlich größer und bringt mehr, als die Übersichten brauchen. Die Last-Balken der Wochenübersicht sind HTML und CSS ohne Bibliothek | entschieden |
| O12 | i18n-Bibliothek für Svelte | **Entschieden am 13. Oktober 2026 (T06): keine Bibliothek, eigener Store `t(key, params)` über `catalog/locales`.** Zwei Sprachen und Schlüssel mit Punkten genügen, die Kataloge sind zur Build-Zeit eingebunden, Zahlen und Daten formatiert `Intl`. `svelte-i18n` 4.0.1 (MIT) bringt ICU-Nachrichten und Laden zur Laufzeit mit, beides unnötig | entschieden |
| O13 | Welches lokale Modell, welche Sprache des Prompts (Deutsch oder Englisch), welche Antwortzeit ist akzeptabel? | Hardware ohne dedizierte GPU, Messung nötig. Stand 13. Oktober 2026 (T15): Der Prompt liegt in beiden Sprachen vor (Sprache des Plans), das Zeitlimit je Aufruf steht auf 120 s (Faustregel, einstellbar mit `--timeout`). Welches Modell, welche Sprache besser trägt und welche Antwortzeit tragbar ist, zeigt erst der Live-Lauf mit 20 Läufen auf dem Zielrechner, der nicht in der Cloud-Umgebung läuft Stand 15. Oktober 2026 (T26): Das Gespräch in der App nutzt dasselbe Zeitlimit von 120 s je Aufruf (bis zu drei Aufrufe je Nachricht, `coach_cancel` bricht ab). Die Antwortzeit je Nachricht ist im Live-Lauf zu notieren: "Dienstag habe ich keine Zeit", "mehr Laufen", "warum ist meine Form so niedrig?" (Task T26, manuelle Schritte). Das Beispielmodell im Hinweis zur Einrichtung (`qwen3:4b`) ist die Vorzugsliste, nicht gemessen Stand 15. Oktober 2026 (T29): `catalog/models.json` (`qwen3:1.7b`, `4b`, `8b`, `14b`) und die Einstufung in `recommend.rs` sind Faustregel; das Startmodell `qwen3:4b` ist ein Vorschlag bis zur Messung. Zu messen auf dem Zielrechner: Antwortzeit je Modell und Hardware, ob die Schwelle von 3 GB für die CPU und die Werte `min_ram_gb` und `comfortable_vram_gb` stimmen, ob `qwen3:4b` die Pläne zuverlässig im Schema liefert. Die Dateigrößen im Katalog sind gegen `ollama.com/library/qwen3` zu prüfen (ungeprüft) | M5 (Live-Lauf offen) |
| O14 | Umrechnung der Session-RPE auf die Lastskala (Faustregel in 8.1) | Kein belegter Umrechnungsfaktor bekannt | M2, mit Nutzerdaten |
| O15 | Neuplanung nach verpassten Einheiten (F34) | Umfang und Verhalten ungeklärt | nach V1 |
| O16 | Taper- und Rampenparameter für den Autor persönlich (Standardwerte sind generisch) | Individuelle Erfahrung | M3 |
| O17 | Minimale Windows-Version und WebView2-Anforderung für die Veröffentlichung | Noch nicht festgelegt | M0 |
| O18 | Erlauben die Nutzungsbedingungen von Garmin den automatisierten Abruf über den inoffiziellen Zugang? Welches Risiko trägt das Konto (Sperre, Drosselung)? | Bedingungen nicht gelesen, Bibliothek nennt sich "unofficial" und warnt vor Schreibfunktionen. Stand 14. Oktober 2026 (T16): `push --yes` schreibt in das Konto (Workouts anlegen und planen) und darf erst nach Klärung dieser Frage zum ersten Mal ausgeführt werden | vor M4a, für `push --yes` vor dem ersten Versand |
| O19 | Dürfen Erholungswerte (HRV, Schlaf, Ruhepuls) an einen Cloud-Client gehen? Standard ist gesperrt. Freigabe pro Werkzeug, pro Sitzung oder gar nicht? | Gesundheitsdaten, besonders sensibel, nicht zurückholbar | M7 |
| O20 | Zeitangaben gegenüber MCP-Clients: nur Datum oder zusätzlich Tageszeitfenster (morgens, mittags, abends)? | Uhrzeiten verraten Gewohnheiten, Zeitfenster helfen der Planbesprechung | M7 |
| O21 | Läuft die Abhängigkeitskette des Zusatzprogramms mit Python 3.14 (Standard auf diesem Rechner), oder bleibt es bei 3.12 über `uv`? | Python 3.14.5 und 3.12.7 sind vorhanden. Vorläufige Festlegung: 3.12. Stand 13. Oktober 2026 (T11, Linux): `garminconnect 0.3.17` und `curl_cffi 0.16.3` installieren unter Linux auch mit CPython 3.14.6, alle Offline-Tests bestehen. Windows ungeprüft, deshalb bleibt es bei 3.12 (`bridge/.python-version`) | M4a |
| O22 | Wie stabil ist die Anmeldung unter Windows (Cloudflare-403, 429 beim ersten SSO-Aufruf, fehlgeschlagene Anmeldung in garmin_mcp)? Reicht die USB-Kopie als Rückfall? | Offene Issues in beiden Projekten, hängt von Garmin ab | M4a |
| O23 | Blockiert Smart App Control auf diesem Rechner `uv` oder `uvx`? | garmin_mcp-README nennt CodeIntegrity-Fehler 3077 als möglich. `uv --version` lief in dieser Sitzung ohne Blockade, `uvx`-Start eines Pakets ungeprüft | M4a |
| O24 | Wie viele Aufrufe pro Zeit toleriert das Konto, ohne gedrosselt oder gesperrt zu werden? Welcher Mindestabstand gilt als Startwert? | Die Bibliothek drosselt nicht und dokumentiert keine Grenzen. Startwert vorsichtig in der Sonde bestimmen und nur bei Erfolg lockern Stand 14. Oktober 2026 (L1): 271 Aufrufe mit 1,5 s Mindestabstand plus 0,5 s Zufall, keine 429, keine 403, keine Sperre. Startwert bleibt 1,5 s plus 0,5 s. | M4a |
| O25 | Bleibt die Anmeldung auf diesem Rechner stabil (429 beim ersten SSO-Aufruf laut Issue #453, 403 bei Datenaufrufen laut Issue #444)? Sonst USB-Kopie und FIT-Ordner als Hauptweg | Fingerabdruckbasierte Sperren sind berichtet, Verhalten hängt von Garmin ab | M4a |
| O26 | Größte zulässige Spanne je Bereichsendpunkt und tatsächliche Historientiefe je Datenart | Bibliothek dokumentiert es nicht. Bis zur Messung in L1 teilt `backfill` Bereiche in Spannen von 28 Tagen (Konvention, `max_span_days` im Register ist `null`) Stand 14. Oktober 2026 (L1): gemessen, siehe `bridge/registry.yaml` (`max_span_days`). Grenzen: 31 (body_battery), 336 (hrv_range, running_tolerance, weekly_intensity_minutes), 392 (endurance_score, hill_score), übrige Bereiche bis 1792 ohne Limit. Historie je Datenart im Bericht. | M4a |
| O27 | Welche nicht gewrappten Endpunkte über `connectapi` oder GraphQL sind für den Vollabzug nötig, und woher kommen ihre Pfade (eigene Browsersitzung mit den Entwicklerwerkzeugen)? | Erst nach Auswertung des Abdeckungsberichts sichtbar | M4b |
| O28 | Sicherung und Verschlüsselung des Archivs (BitLocker, getrennter Datenträger), Aufbewahrung von `sensitive/`. Ort ist entschieden: `local/archive/garmin/` im Projektordner | Archiv enthält vollständige Gesundheits- und Standortdaten und ist nicht aus FIT ableitbar. `git clean -x` würde es löschen (6.1) | vor M4b |
| O30 | Nimmt `diauth.garmin.com` den Tausch Ticket gegen Token ohne Browser-TLS an (reqwest mit rustls statt curl_cffi)? | Die Bibliothek sendet ihn mit nachgeahmtem Chrome-TLS, die Datenaufrufe dagegen mit der Standardbibliothek. Nachahmen widerspräche Abrufregel 5. Ausweg bei Ablehnung: Sitzungs-Cookie aus dem Anmeldefenster (wie der Rückfall `JWT_WEB` der Bibliothek), erst nach Entscheidung des Nutzers | Livetest T23 |
| O32 | Antwortformate des Vollabzugs (T39, T40) am echten Konto: Stimmen die Feldnamen aus dem Bericht der Sonde und aus Q28 bis Q33, die Werte von `gender` (`MALE`, `FEMALE` angenommen), die Namen der Zonenprofile (`DEFAULT`, `RUNNING`, `CYCLING`, `SWIMMING` angenommen), der Faktor 10 beim Schwellentempo, das Zeilenformat der Schwellenverläufe, die Phrasen des Trainingsstatus und die Formate von Gewicht, Blutdruck und Ernährung? | Seit T40 aus Sekundärquellen übernommen (Aufzeichnungen und Lesecode Dritter, Anhang A), am eigenen Konto ungeprüft. Weicht ein Format ab, verwirft der Parser den Wert (er fehlt dann, falsch wird er nicht) | Livetest T40 |
| O31 | Feste Stunden je Woche oder Minuten je Tag: wie verteilt der Planer mit drei Fragen die Zeit? | **Entschieden am 15. Oktober 2026 (T26 Teil B): gleichmäßig auf die gewählten Tage, ohne Wochenendanteil (Faustregel).** Die Wochenzeit wird auf 5 Minuten gerundet (wie die Dauerstufe des Planers) und in 5-Minuten-Schritten gleichmäßig verteilt, ein Rest von höchstens 5 Minuten je Tag geht an die späteren Tage (Summe bleibt). Gewählte Tage sind Obergrenze je Tag mit erlaubten harten Einheiten, die übrigen feste Ruhetage mit 0 Minuten (R5 gibt der Planer selbst, wenn alle sieben Tage gewählt sind). Begründung: Die Minuten sind die Obergrenze je Tag, nicht das Ziel; der Planer wählt die Dauern nach dem Lastziel und der Rampe (R1), setzt harte Tage nach R3 und R4 und legt die Langeinheit auf den Tag mit der größten Zeit, bei Gleichstand auf den späteren (`generate.rs`). Ein doppelter Wochenendanteil wäre eine Annahme über den Alltag des Nutzers, die weder die Eingabe noch eine Quelle trägt. Folge: Bei vielen Trainingstagen ist die Langeinheit auf den Durchschnitt je Tag begrenzt (7 Tage bei 7 Stunden: 60 Minuten); wer eine längere Einheit will, sagt es im Gespräch oder stellt die Zeit je Tag in den Feineinstellungen. Weniger als 20 Minuten je Tag (`structure.min_session_minutes`) ist `too_little_time`. Beide Fassungen der Rechnung (Rust `distribute_minutes`, Oberfläche `distributeMinutes`) haben dieselben Handrechnungen als Tests | entschieden |
| O29 | Wie zählt Kraft im Plan, solange die gemessene Last keine Kraft enthält? | Stand 14. Oktober 2026 (Integrationsprüfung, Fund B): Kraft aus FIT ist immer `missing_channel` (RPE kommt nur von Hand, 8.1), für `manual_session` gibt es keinen Eingabeweg. CTL, ATL und die Vorwochenlast enthalten deshalb keine Kraft. Der Planer rechnet Kraft aber mit Last (Session-RPE der Vorlage, 27 je Einheit bei 45 min und RPE 6) im Wochenziel, in `planned_load` und in der Vorausberechnung für R1. Folgen: Der Plan verdrängt Ausdauerlast durch Kraftlast, die später nie in der CTL erscheint, R1 verbraucht seinen Spielraum zum Teil mit Kraft, und Plan und Messung lassen sich nicht vergleichen. Optionen: (a) Eingabeweg für Session-RPE (CLI und App, je Aktivität oder als `manual_session`), dann zählt Kraft auch in der gemessenen Last; folgt 8.1, braucht Nutzereingaben und hängt an O14, ohne Eingabe bleibt die Last fehlend. (b) Kraft aus Wochenziel, `planned_load` der Woche und R1-Vorausberechnung herausnehmen (Kraft behält Dauer und zählt bei R2 und R7); Plan und Messung passen zusammen, die Ermüdung durch Kraft fehlt in ATL und TSB, alle Golden-Pläne ändern sich. (c) wie (b), aber als Parameter in `rules.default.json`, der wieder einschaltbar ist, sobald (a) besteht. Empfehlung des Prüfauftrags: (c) jetzt, (a) später. Die Minuten der Historie zählen seit 14. Oktober 2026 alle primären Aktivitäten (5.4, `week_history`), das ist entschieden | vor M3-Freigabe, Entscheidung des Nutzers |

---

## Anhang A: Quellen und Prüfstatus

Prüfstatus:

- **bibliographisch bestätigt**: Autoren, Zeitschrift, Jahr und Seiten über Verzeichnisse (PubMed, Fachdatenbanken) abgeglichen am 13. Oktober 2026. Volltext nicht gelesen, wo nichts anderes steht.
- **nur Sekundärquelle**: Wert oder Formel stammt aus Zusammenfassungen Dritter. Vor Übernahme in Code ist das Original zu beschaffen.
- **nicht geprüft**: Angabe aus Fachkenntnis, nicht abgeglichen.

| Nr. | Quelle | Verwendung | Prüfstatus |
|---|---|---|---|
| Q1 | Banister EW, Calvert TW, Savage MV, Bach T. A systems model of training for athletic performance. Aust J Sports Med 1975;7(3):57-61 | Grundlage Impuls-Antwort-Modell | bibliographisch bestätigt (Literaturlisten), Volltext nicht gelesen |
| Q2 | Calvert TW, Banister EW, Savage MV, Bach T. A systems model of the effects of training on physical performance. IEEE Trans Syst Man Cybern 1976 | Zeitkonstanten 50 und 15 Tage | Zeitkonstanten über Suchtreffer bestätigt, Bandangabe nicht geprüft |
| Q3 | Morton RH, Fitz-Clarke JR, Banister EW. Modeling human performance in running. J Appl Physiol 1990;69(3):1171-1177, DOI 10.1152/jappl.1990.69.3.1171 | Fitness-Ermüdungs-Modell, TRIMP-Zuschreibung | bibliographisch bestätigt (PubMed) |
| Q4 | Foster C, et al. A new approach to monitoring exercise training. J Strength Cond Res 2001;15(1):109-115 | Session-RPE | bibliographisch bestätigt |
| Q5 | Seiler S. What is best practice for training intensity and duration distribution in endurance athletes? Int J Sports Physiol Perform 2010;5(3):276-291, DOI 10.1123/ijspp.5.3.276 | Intensitätsverteilung (beschreibend, etwa 80 zu 20) | bibliographisch bestätigt (PubMed) |
| Q6 | Plews DJ, et al. Training adaptation and heart rate variability in elite endurance athletes: opening the door to effective monitoring. Sports Med 2013;43(9):773-781 | HRV-Überwachung | bibliographisch bestätigt. Empfehlung 7-Tage-Mittel nur Sekundärquelle |
| Q7 | Tanaka H, Monahan KD, Seals DR. Age-predicted maximal heart rate revisited. J Am Coll Cardiol 2001;37(1):153-156 | `208 - 0,7 * Alter` | bibliographisch bestätigt, Formel bestätigt |
| Q8 | Gabbett TJ. The training-injury prevention paradox: should athletes be training smarter and harder? Br J Sports Med 2016;50(5):273-280, DOI 10.1136/bjsports-2015-095788 | Herkunft des ACWR | bibliographisch bestätigt |
| Q9 | Impellizzeri FM, Tenan MS, Kempton T, Novak A, Coutts AJ. Acute:chronic workload ratio: conceptual issues and fundamental pitfalls. Int J Sports Physiol Perform 2020;15(6):907-913, DOI 10.1123/ijspp.2019-0864 | Kritik am ACWR | bibliographisch bestätigt (Repositorium) |
| Q10 | Mujika I, Padilla S. Scientific bases for precompetition tapering strategies. Med Sci Sports Exerc 2003;35(7):1182-1187 | Taper-Prinzip | bibliographisch bestätigt (PubMed) |
| Q11 | Bosquet L, Montpetit J, Arvisais D, Mujika I. Effects of tapering on performance: a meta-analysis. Med Sci Sports Exerc 2007;39(8):1358-1365 | Taper-Dauer und -Reduktion | bibliographisch bestätigt. Zahlenwerte nur Sekundärquelle |
| Q12 | Nielsen RO, et al. Excessive progression in weekly running distance and risk of running-related injuries. J Orthop Sports Phys Ther 2014 | Wochenanstieg Laufumfang | Ergebnis (HR 1,59, 95 % KI 0,96 bis 2,66, p = 0,07, kein signifikanter Unterschied) über Sekundärbericht. Bandangabe nicht geprüft |
| Q13 | Buist I, et al. Randomisierte Studie zu graduiertem Lauftraining bei Einsteigern (Verletzungsrate 20,3 und 20,8 Prozent, kein signifikanter Unterschied) | "10-Prozent-Regel" | nur Sekundärquelle. Jahr 2008 laut Suchtreffer, Zeitschrift und Jahr vor Zitat prüfen |
| Q14 | Karvonen MJ, Kentala E, Mustala O. The effects of training on heart rate: a longitudinal study. Ann Med Exp Biol Fenn 1957;35(3):307-315 | Herzfrequenzreserve | bibliographisch bestätigt (PubMed). Zuschreibung der Formel in dieser Form nicht bestätigt |
| Q15 | Wakayoshi K, et al. A simple method for determining critical speed as swimming fatigue threshold in competitive swimming. Int J Sports Med 1992;13(5):367-371. Wakayoshi K, et al. Determination and validity of critical velocity as an index of swimming performance in the competitive swimmer. Eur J Appl Physiol 1992;64:153-157 | CSS | erste: bibliographisch bestätigt. zweite: nur über Literaturliste bestätigt |
| Q16 | Allen H, Coggan AR. Training and Racing with a Power Meter. VeloPress | NP, IF, TSS, Leistungszonen | nur Sekundärquelle, Buch nicht eingesehen. Formeln über mehrere Darstellungen übereinstimmend |
| Q17 | Banister EW. Modeling elite athletic performance. In: Physiological Testing of the High-Performance Athlete, 1991. Edwards S. The Heart Rate Monitor Book, 1993 | TRIMP, Zonen-Summation | nur Sekundärquelle. Konstanten 1,92 und 1,67 sowie 0,64 und 0,86 vor Nutzung am Original prüfen |
| Q18 | Daniels J. Daniels' Running Formula | Möglicher späterer Ersatz für Laufzonen (VDOT) | nicht geprüft, nur als Option geführt |
| Q19 | Garmin FIT Protocol License und FIT SDK Lizenzvertrag (developer.garmin.com/fit, thisisant.developer.garmin.com) | Rechtsgrundlage für FIT-Nutzung | Suchzusammenfassung: nicht exklusive, gebührenfreie Lizenz, Weitergabebeschränkungen. Originalseiten beim Abruf nicht erreichbar (O2 am 13. Oktober 2026 ohne Profildateien entschieden, bei Klärung neu bewerten) |
| Q20 | Strava API Agreement, wirksam 1. Juni 2026 (strava.com/legal/api), Strava Brand Guidelines (developers.strava.com/guidelines, überarbeitet 29. September 2025) | Anzeige nur für den Nutzer, ein Token je Anwendung | Seiten abgerufen am 13. Oktober 2026 über eine Zusammenfassung des Abrufwerkzeugs. KI-Aussage widersprüchlich (offen O1) |
| Q21 | python-garminconnect (github.com/cyberjunky/python-garminconnect), garth (github.com/matin/garth) | Zusatzprogramm | README gelesen am 13. Oktober 2026: MIT, Workout-Upload und -Planung, garth veraltet. Release-Daten nicht ersichtlich |
| Q22 | Zonengrenzen nach Friel (Herzfrequenz, Prozent LTHR) | Herzfrequenzzonen | nicht geprüft, als Faustregel geführt |
| Q23 | taxuspt/garmin_mcp (github.com/taxuspt/garmin_mcp), README, CLAUDE.md, pyproject.toml, offene Issues, abgerufen am 13. Oktober 2026 | Erkundungswerkzeug, Befunde zu Fehlerbildern und Schwächen | direkt gelesen. MIT, Release 0.1.0, `garminconnect==0.3.2` gepinnt, `mcp>=1.28.1,<2`. Werkzeugzahl widersprüchlich (110+, 122, 131 nach Kategorien, rund 150 laut Issue #336). Quellcode der Werkzeuge nicht im Einzelnen geprüft |
| Q24 | rmcp, offizielles Rust-SDK der Model-Context-Protocol-Organisation (crates.io, Version 3.5.1, Apache-2.0) | MCP-Server in Rust | Metadaten über crates.io bestätigt. API in T10 am Quelltext in `~/.cargo/registry` geprüft: `#[tool_router]`, `#[tool_handler]`, `Parameters`, `Json`, Hinweise über `annotations(...)`, `serve(stdio())`, Initialisierung mit `protocolVersion` 2025-11-25 getestet |
| Q25 | Garmin Connect Developer Program (Berichte Dritter: Terra, Open Wearables, Sahha) | Offizieller Weg zu Garmin-Daten | nur Sekundärberichte. Aussage "nur für juristische Personen, Anträge von Privatpersonen werden abgelehnt" nicht gegen Garmins eigene Seiten geprüft |
| Q27 | python-garminconnect 0.3.17 (github.com/cyberjunky/python-garminconnect): Quelltext von `garminconnect/__init__.py` und `client.py`, `pyproject.toml`, README, Release-Notizen, Issues #444 und #453, abgerufen am 13. Oktober 2026 | Methodenliste, Anmeldung, Abhängigkeiten, Verhalten bei 429 und 403, Fehlerbilder | direkt gelesen. Methodenliste aus zwei Abrufen des Quelltexts (Zeichen 0 bis 100000 und ab 100000) zusammengestellt, per Zusammenfassung ausgewertet und nicht Zeile für Zeile geprüft. Verhalten der Endpunkte am echten Konto ungeprüft. Seit T39 (16. Oktober 2026): Pfade, Parameter und gelesene Felder der neuen Endpunkte im lokalen Quelltext 0.3.17 (`bridge/.venv`) Zeile für Zeile gelesen, Zeilen in `docs/tasks/T39-garmin-vollabzug.md`; Feldnamen gegen den Bericht der Sonde (`docs/garmin-feldabdeckung.md`) geprüft, Werte und Einheiten am echten Konto ungeprüft (O32) |
| Q28 | go-garmin (github.com/llehouerou/go-garmin), aufgezeichnete Antworten in `testdata/cassettes/biometric.yaml`, `metrics.yaml`, `weight.yaml`, `userprofile.yaml` (Januar 2026, Kennungen ersetzt), abgerufen am 16. Oktober 2026 | Zeilenformat der Schwellenverläufe (`from`, `until`, `series`, `value`), `latestLactateThreshold` als Liste, VO2max-Verlauf, Trainingsstatus, Belastungsfokus, Rennprognosen, Gewicht | nur Sekundärquelle: Aufzeichnung Dritter von einem echten Konto, nicht am eigenen Konto geprüft (O32) |
| Q29 | garth (github.com/matin/garth), Modelle `src/garth/data/weight.py`, `daily_summary.py`, `stats/training_status/daily.py` und Kassetten; Kassetten von python-garminconnect 0.2.x in Kopie (github.com/Benson0875/RunningGPT, `tests/cassettes/test_floors.yaml`), abgerufen am 16. Oktober 2026 | Etagen (Spalten über `floorsValueDescriptorDTOList`), Gewicht in Gramm, Typen im Trainingsstatus | nur Sekundärquelle (Code und Aufzeichnung Dritter) |
| Q30 | Taxuspt/garmin_mcp `src/garmin_mcp/training.py` (Faktor `_CM_PER_MS_TO_MPS = 10` mit Begründung aus der Web-App), gleichlautend praxys-run/praxys `sync/garmin_sync.py` und leonzzz435/garmin-ai-coach, abgerufen am 16. Oktober 2026 | Einheit des Schwellentempos: Garmins Wert × 10 = m/s | nur Sekundärquelle. Drei unabhängige Projekte übereinstimmend, Gegenprobe an Q28 plausibel (8.6); offizielle Angabe von Garmin fehlt |
| Q31 | fomalhaut84/pleiades `apps/fitness/docs/specs/378-garmin-fitness-metrics-history.md`, abgerufen am 16. Oktober 2026 | Grenze 366 Tage je Abruf der Schwellenverläufe (länger: HTTP 400) | nur Sekundärquelle, nicht geprüft |
| Q32 | lctayson/garmin-logger, aufgezeichnete Antwort `data/raw/daily/running_tolerance/2026/09/2026-09-23.json`, abgerufen am 16. Oktober 2026 | Felder der Lauftoleranz bei `aggregation=daily` (Meter, gleitend über 7 Tage) | nur Sekundärquelle (Aufzeichnung Dritter) |
| Q33 | cyberjunky/ha-garmin `src/ha_garmin/client.py` und python-garminconnect `demo.py` (Blutdruck, Ernährung); mlcousek/JirkasArc `docs/garmin-routes.json` (Schlüssel des Ernährungstagebuchs), abgerufen am 16. Oktober 2026 | Felder von Blutdruck und Ernährung | nur Sekundärquelle: Lesecode und Schlüsselliste, keine aufgezeichnete Antwort |
Nicht verwendete, aber geprüfte Aussagen: Die Aussage "Wochenanstieg von 10 Prozent schützt vor Verletzungen" ist nicht belegt (Q12, Q13). Die Aussage "ACWR zwischen 0,8 und 1,3 sei sicher" wird nicht übernommen (Q9).
