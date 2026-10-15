# app

Desktop-Oberfläche: Tauri 2 mit Svelte 5, Vite und TypeScript. Ruhige Gestaltung nach SPEC 5.5, dunkel zuerst, Deutsch und Englisch. Ansichten: Heute (Start), Kalender (T35), Wochenübersicht, Fortschritt (T32), Leistung (T33), Erholung (T34), Plan, Aktivitätsliste, Detail ohne Karte, Einstellungen. "Heute" ist seit T25 ein Dashboard (Befehl `get_dashboard`). Die Attrappe liefert dafür mit `?dashboard=rich|low|sparse|empty|extreme|gaps` im Browser verschiedene erfundene Datenlagen für die Sichtprüfung.

| Ordner | Inhalt |
|---|---|
| src-tauri | Rust-Hülle (eigenes Cargo-Projekt, nicht im Wurzel-Workspace), Capabilities, Content-Security-Policy |
| ui | Svelte-Oberfläche mit Attrappe, Vitest und Playwright |

Vorgaben: [SPEC 5.3, 5.4 und 10.4](../docs/SPEC.md). Keine Netzwerkzugriffe aus dem Frontend, keine Telemetrie.

## Starten

Im Repository-Wurzelverzeichnis, nach `pnpm --dir app/ui install --frozen-lockfile`:

| Zweck | Befehl |
|---|---|
| Oberfläche im Browser mit Attrappe (synthetische Daten, kein Rust nötig) | `pnpm --dir app/ui dev`, dann http://127.0.0.1:5173 |
| App mit echtem Rust-Teil und Datenbank | `pnpm --dir app/ui exec tauri dev` |
| Debug-Build der App | `pnpm --dir app/ui exec tauri build --debug`, Programm unter `app/src-tauri/target/debug/lastwerk-app` |
| Installer (wie der Release-Workflow) | `pnpm --dir app/ui exec tauri build --config app/src-tauri/tauri.release.conf.json --bundles deb` (unter Windows `nsis`, unter Linux auch `appimage`; macOS ist seit dem 15. Oktober 2026 aus CI und Release entfallen, SPEC 11.6), Ergebnis unter `app/src-tauri/target/release/bundle/` |
| Installer lokal (Windows, nsis) | `powershell -File scripts/build-installer.ps1` (mit `TAURI_SIGNING_PRIVATE_KEY` auch `.sig`) |
| Tests der Oberfläche | `pnpm --dir app/ui lint`, `check`, `test` |
| Rauchtest im Browser (Deutsch und Englisch, ohne Netzwerk) | `pnpm --dir app/ui exec playwright test`, Screenshots unter `app/ui/test-results/screens/` (nicht einchecken) |
| Tests der Hülle | in `app/src-tauri`: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` |

Die Bündelung ist in `tauri.conf.json` aus, damit der Debug-Build nur die Binärdatei baut. `tauri.release.conf.json` schaltet sie ein, `.github/workflows/release.yml` nutzt diese Überlagerung bei einem Tag `v*`. Die Installer sind nicht signiert.

Update auf Knopfdruck (T17): `plugins.updater` in `tauri.conf.json` trägt Endpunkt und öffentlichen Schlüssel, `tauri.release.conf.json` schaltet `createUpdaterArtifacts` ein. Der private Schlüssel liegt nie im Repository, sondern als GitHub-Secret `TAURI_SIGNING_PRIVATE_KEY` (und `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, leer erlaubt). Der Release-Workflow legt `latest.json` und die `.sig` an den Release. Ein Update läuft nur über die Befehle `check_update` und `install_update`.

Die Oberfläche erkennt selbst, ob sie in Tauri läuft (`window.__TAURI_INTERNALS__`). Sonst nutzt sie die Attrappe `ui/src/lib/backend/mock.ts`. Beide stehen hinter dem Interface `Backend` (`ui/src/lib/backend/types.ts`).

`tauri` im Ordner `ui` ist ein kleines Skript (`ui/tools/tauri-shim`), das der unveränderten Tauri-CLI die Pfade von `ui` und `src-tauri` mitgibt. Die CLI sucht ihre Konfiguration sonst nur im aktuellen Ordner und darunter.

## Systembibliotheken (Linux)

Tauri braucht WebKitGTK und GTK: `libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev` (siehe `scripts/cloud-env-setup.sh`). Die Wurzel kommt ohne sie aus, `cargo test --workspace` baut `app/src-tauri` nicht.

## Daten und Pfade

- Datenbank: `<lokaler Datenordner>/Lastwerk/lastwerk.db`, derselbe Pfad wie bei der Kommandozeile `lastwerk` (Linux `~/.local/share`, macOS `~/Library/Application Support`, Windows `%LOCALAPPDATA%`). Die Umgebungsvariable `LASTWERK_DB` überschreibt ihn (in der App wie in der Kommandozeile), zum Beispiel für Versuche mit synthetischen Daten (`cargo run -p lastwerk-testkit --bin gen-fixtures -- --out target/fixtures`).
- FIT-Dateien: fester Stamm `<lokaler Datenordner>/Lastwerk/fit` (T28), neben der Datenbank und nicht im Programmordner, den Updates ersetzen. Der Nutzer wählt keinen Ordner. `LASTWERK_FIT` überschreibt den Ort (Tests, Vorführungen). Unterordner `garmin/` (Abgleich) und `manual/` ("FIT-Dateien hinzufügen"). Nur dieser Pfad erreicht die Oberfläche, zur Anzeige in den Einstellungen. Ein früher gewählter Ordner (`app.fit_dir`) wird beim Start einmalig in den Stamm kopiert (nichts gelöscht, nichts überschrieben), die Zahlen stehen in `Settings.fit_migration`.
- Garmin (T23): Der Abgleich legt FIT-Dateien unter `<FIT-Stamm>/garmin/` ab und liest danach den ganzen Stamm ein. Token liegen nur im Schlüsselbund (Dienst `lastwerk-garmin`), die Sperrdatei unter `<lokaler Datenordner>/Lastwerk/garmin/lockout.json`. In der Attrappe gelingt die Anmeldung sofort, der Abgleich ist synthetisch.
- Coach einrichten (T29): Die Zustandsdatei `ollama-setup.json` des Einrichtungsskripts liegt neben der Datenbank. Die Befehle `coach_setup_status`, `coach_install_ollama`, `coach_pull_model` und `coach_pull_cancel` lesen sie, die Oberfläche bekommt nie einen Pfad. Die Attrappe kennt `?ollama=fehlt|laeuft|laedt` (Ollama fehlt, läuft mit Modellen, lädt gerade das Startmodell).
- Liegt die Datenbank in einem Sync-Ordner (OneDrive, Dropbox, iCloud und weitere), warnt der Einstellungsdialog.

## Pläne und Export

Die Planung nutzt `lastwerk-cli` als Bibliothek (Kontext aus der Datenbank, Erzeugung, Schemaprüfung, Leitplanken, ICS), die App rechnet und prüft also genau wie `lastwerk plan` und `lastwerk validate`. Dateidialoge für ICS, Plan-JSON, Backup, Wiederherstellung und "FIT-Dateien hinzufügen" öffnet der Rust-Befehl, die Oberfläche bekommt nie einen Pfad und hat kein Dateisystem-Plugin. Texte aus Plänen und Erklärungen zeigt die Oberfläche als Text, nie als HTML. Im Browser mit der Attrappe protokolliert `MockBackend.calls` die Exporte (im Browser als `window.__lastwerkMockCalls`).

## Neue Befehle oder Ansichten

Ein neuer Befehl steht an vier Stellen: Funktion in `src-tauri/src/lib.rs` samt `COMMAND_NAMES` und `generate_handler!`, Befehlsliste in `src-tauri/build.rs`, Berechtigung `allow-<befehl>` in `src-tauri/capabilities/default.json`, Methode im `Backend` mit Umsetzung in `tauri.ts` und `mock.ts`. Ein Test prüft, dass die drei Rust-Stellen übereinstimmen. Texte kommen nur aus `catalog/locales/de.json` und `en.json` (`sh scripts/check-i18n.sh`).
