# Lastwerk

Lastwerk ist ein lokales Desktop-Werkzeug für Ausdauertraining. Es holt Aktivitäten und Erholungswerte aus Garmin Connect (oder liest FIT-Dateien), berechnet Belastung, Fitness, Ermüdung und Form, erzeugt regelbasierte Wochenpläne und exportiert sie als ICS und JSON. Ein optionales lokales Sprachmodell (Ollama) darf Pläne anpassen und erklären. Es rechnet nie, es wählt nur aus erlaubten Vorlagen, und jede Antwort wird von Lastwerk geprüft. Es gibt kein Konto, keinen Server und keine Telemetrie.

Das Projekt ist ein Hobbyprojekt mit Schaufenster-Charakter: Es gibt keinen Support und keine Zusage, dass Fehler behoben oder Wünsche umgesetzt werden.

## Status

- Aktuell ist Version 1.0.x. Jede Änderung am Programm auf `main` erscheint nach grünen Prüfungen automatisch als neuer Release.
- Installer gibt es für Windows (`-setup.exe`) und Linux (`.deb`, `.AppImage`). Die Update-Funktion ("Nach Update suchen" in den Einstellungen) gibt es nur unter Windows. Einen macOS-Installer gibt es seit dem 15. Oktober 2026 nicht mehr; aus dem Quelltext ist macOS baubar, aber nicht geprüft.
- Ehrlich zur Erprobung: Die Garmin-Verbindung und das Gespräch mit einem lokalen Modell sind gegen ein echtes Garmin-Konto und ein echtes lokales Modell nur eingeschränkt erprobt. Alles andere ist mit synthetischen Daten und Attrappen getestet. Rechnen Sie mit Fehlern und prüfen Sie Zahlen und Pläne selbst.
- Lastwerk ist kein Medizinprodukt und ersetzt keinen Trainer und keinen Arzt.

## Installation

Die Installer stehen unter "Releases" dieses Repositories. **Sie sind nicht signiert, und es gibt keinen Support.**

- Windows: `-setup.exe` ausführen. SmartScreen meldet eine unbekannte App: "Weitere Informationen", dann "Trotzdem ausführen". Wer früher die `.msi` installiert hat, deinstalliert sie einmal und nimmt die `-setup.exe`.
- Linux: Das AppImage mit `chmod +x` ausführbar machen, oder das deb-Paket installieren.
- Updates (Windows): in den Einstellungen "Nach Update suchen". Die App lädt und installiert nach Bestätigung, ohne Terminal. Die Signatur des Updates wird gegen einen öffentlichen Schlüssel in der App geprüft.

Beim ersten Start führt ein Assistent durch Sprache, Schwellenwerte, Ziel und die Verbindung mit Garmin ("Mit Garmin verbinden", Anmeldung mit MFA auf der Seite von Garmin in einem eigenen Fenster; Lastwerk sieht das Passwort nie). Schwellenwerte (FTP, Laufschwelle, CSS, Herzfrequenzen) sind nötig, sonst bleibt die Belastung "nicht berechenbar". Im Reiter "Plan" beantwortet man drei Fragen (Ziel, Stunden pro Woche, Trainingstage) und kann den Plan danach im Gespräch mit dem Coach anpassen, wenn ein lokales Modell läuft. FIT-Dateien liegen im Datenordner der App (unter Windows `%LOCALAPPDATA%\Lastwerk\fit`), einzelne Dateien kommen über "FIT-Dateien hinzufügen" in den Einstellungen dazu.

## Aus dem Quelltext bauen

Voraussetzungen:

- Git, Rust in der Version aus `rust-toolchain.toml` (1.99.0; `rustup` installiert sie beim ersten Aufruf von `cargo` selbst), Node 22 (mindestens 22.12) und pnpm 12 (Version in `app/ui/package.json` festgelegt, zum Beispiel über Corepack).
- Nur für die Desktop-App (`app/src-tauri`): unter Linux die Systembibliotheken von Tauri, `sudo apt-get install -y build-essential pkg-config libssl-dev libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev`; unter Windows die Visual-Studio-Build-Tools mit C++ und die WebView2-Laufzeit (siehe die Voraussetzungen der Tauri-Dokumentation).

Kern, Kommandozeile und MCP-Server (Rust-Workspace im Wurzelverzeichnis, braucht die Systembibliotheken nicht):

```
git clone https://github.com/flor1xn/lastwerk.git
cd lastwerk
cargo build --workspace --locked
cargo test --workspace --locked
cargo run -p lastwerk-cli -- --help
```

Oberfläche im Browser mit Attrappe (synthetische Daten, kein Rust nötig):

```
pnpm --dir app/ui install --frozen-lockfile
pnpm --dir app/ui build
pnpm --dir app/ui dev
```

`dev` öffnet die Oberfläche unter http://127.0.0.1:5173.

Desktop-App mit Rust-Teil (eigenes Cargo-Projekt in `app/src-tauri`, nicht Teil des Wurzel-Workspace und von `cargo test --workspace` nicht erfasst):

```
pnpm --dir app/ui install --frozen-lockfile
pnpm --dir app/ui exec tauri dev
pnpm --dir app/ui exec tauri build --debug
```

Weitere Befehle (Installer, Tests der Hülle, Pfade) stehen in [app/README.md](app/README.md). Alle Prüfungen der CI in einem Lauf: `sh scripts/check-all.sh`.

## Kommandozeile und MCP-Server

- [docs/cli.md](docs/cli.md): das Programm `lastwerk` mit denselben Funktionen ohne Oberfläche (Import, Pläne, Export, Backup, Garmin-Abgleich, Gespräch mit dem lokalen Coach).
- [docs/mcp.md](docs/mcp.md): `lastwerk-mcp`, ein schreibgeschützter MCP-Server für Claude Desktop und Claude Code. Das ist die einzige Stelle, an der ein Cloud-Modell Daten aus Lastwerk sieht, und nur, wenn man den Server selbst verbindet.

## Datenschutz

Lastwerk arbeitet lokal: keine Telemetrie, kein Konto, keine Netzwerkzugriffe aus der Oberfläche. Die Daten bleiben auf dem eigenen Rechner. Die Datenbank liegt unter `<lokaler Datenordner>/Lastwerk/lastwerk.db`, Exportdateien (ICS, Plan, Backup) schreibt Lastwerk nur dorthin, wo man sie im Dateidialog ablegt. Die Garmin-Token liegen nur im Schlüsselbund des Betriebssystems, nie in der Datenbank, in einem Backup oder in einem Log. Eine ICS-Datei enthält Titel, Zeit und Dauer der Einheiten, keine Orte.

Backups, Pläne und FIT-Dateien können persönliche Angaben enthalten (zum Beispiel die Bezeichnung eines Ziels, Orte und Gerätekennungen) und gehören nicht in ein öffentliches Repository. Lastwerk ruft keine Cloud-API auf und speichert keine API-Schlüssel; ein Sprachmodell wird nur lokal angesprochen (Standard: Loopback).

## Hinweis zu Garmin und Strava

Lastwerk ist nicht mit Garmin oder Strava verbunden und wird von ihnen nicht unterstützt, gefördert oder autorisiert. Die Namen werden nur beschreibend verwendet, Logos gibt es nicht. Der Garmin-Zugang der App nutzt eine inoffizielle Schnittstelle. Sie kann sich jederzeit ändern, und der automatisierte Abruf kann gegen die Nutzungsbedingungen von Garmin verstoßen. Die Nutzung geschieht auf eigenes Risiko. Ohne Garmin-Konto arbeitet Lastwerk mit FIT-Dateien.

## Lizenz

MIT OR Apache-2.0, nach Wahl. Siehe [LICENSE-MIT](LICENSE-MIT) und [LICENSE-APACHE](LICENSE-APACHE). Das Repository folgt der REUSE-Spezifikation (Lizenzzuordnung in `REUSE.toml`, Lizenztexte in `LICENSES/`). Lizenzen der verwendeten Fremdpakete und die mitgelieferte Schrift: [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).

## Sicherheit

Schwachstellen bitte nicht als öffentliches Issue melden, sondern auf dem Weg, den [SECURITY.md](SECURITY.md) beschreibt.

## Dokumentation und Struktur

Die Spezifikation steht in [docs/SPEC.md](docs/SPEC.md) und gilt bei Widerspruch. Jede Formel und jede Leitplanke nennt dort ihre Quelle oder ist ausdrücklich als Faustregel gekennzeichnet. Die Arbeitsaufträge, nach denen das Projekt entstanden ist, stehen in [docs/tasks/](docs/tasks/README.md), eine Übersicht in [docs/README.md](docs/README.md).

| Ordner | Inhalt |
|---|---|
| [docs/](docs/) | Spezifikation, Arbeitsaufträge, Anleitungen |
| [crates/](crates/) | Rust-Workspace: Kern, FIT-Einlesen, Speicher, Garmin-Zugang, Kommandozeile, Coach (lokales Sprachmodell), MCP-Server |
| [app/](app/) | Desktop-Oberfläche (Svelte 5, Tauri 2, Schrift Cairo) |
| [bridge/](bridge/README.md) | Python-Zusatzprogramm für Vollabzug und Workouts, seit T20 eingefroren, die App braucht es nicht |
| [schema/](schema/) | JSON-Schemas für Plan, Planänderung, Erholungstag, Archiv-Manifest |
| [catalog/](catalog/) | Trainingsvorlagen, Sprachdateien, Regelparameter |
| [scripts/](scripts/) | Datenschutz-, Isolations- und Sprachprüfung, Gesamtprüfung |
| `local/` | Private Daten (FIT-Dateien, Archiv, Austausch), nicht versioniert |

## Mitentwickeln

Regeln für Beiträge und für Sessions mit Claude Code stehen in [CLAUDE.md](CLAUDE.md), die Aufträge und ihre Reihenfolge in [docs/tasks/README.md](docs/tasks/README.md). Der Ordner `local/` ist von Git ausgeschlossen. Ein Hook (`.githooks/pre-commit`) und die CI verhindern, dass FIT-Dateien, Datenbanken oder Zugangsdaten in das Repository gelangen; aktivieren einmal pro Klon mit `git config core.hooksPath .githooks`. `git clean` nie mit `-x` oder `-X` ausführen: Es würde ignorierte Dateien in `local/` löschen.

## In English

Lastwerk is a local desktop tool for endurance training. It syncs activities and recovery data (HRV, resting HR, sleep, Body Battery) from Garmin Connect, read-only and throttled, or reads FIT files. It computes training load, fitness, fatigue and form, shows a dashboard, and builds rule-based weekly plans that can be exported as ICS and JSON. An optional local language model (Ollama) may suggest plan changes in a chat; every suggestion is validated and needs your confirmation, the model never computes. No account, no server, no telemetry, no cloud model. Data stays on your machine, Garmin tokens live only in the OS keychain.

Status: version 1.0.x, installers for Windows and Linux under "Releases", unsigned and without support. The Garmin connection and the local model chat have been tried against a real Garmin account and a real model only to a limited extent. The Garmin access is unofficial, may break at any time and may violate Garmin's terms; use it at your own risk. Lastwerk is not affiliated with, supported by or endorsed by Garmin or Strava. Build from source: see "Aus dem Quelltext bauen" above (Rust 1.99.0 via `rust-toolchain.toml`, Node 22, pnpm 12; `cargo build --workspace --locked`, `pnpm --dir app/ui install --frozen-lockfile`, `pnpm --dir app/ui build`). License: MIT OR Apache-2.0. Security reports: see [SECURITY.md](SECURITY.md). The specification is in [docs/SPEC.md](docs/SPEC.md) (German).
