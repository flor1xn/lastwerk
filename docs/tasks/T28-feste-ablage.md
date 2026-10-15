# T28: Feste Ablage der FIT-Dateien ohne Ordnerwahl

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | 1.0 |
| Voraussetzung | T23 |
| cloudfähig | ja |
| Bereich | `app/src-tauri`, `app/ui`, `crates/cli` (Pfade), `catalog/locales`, `docs/SPEC.md` |
| SPEC | 4.1 (F1), 5.4 (lastwerk-app), 5.6, 6.1 |

## Ziel

Der Nutzer wählt keinen FIT-Ordner mehr. Lastwerk legt alle FIT-Dateien selbst im Datenordner der App ab, neben der Datenbank.

## Warum

Entscheidung vom 15. Oktober 2026: Der Ordner wird nicht gewählt, die Konfiguration soll so klein wie möglich sein. Mit dem Garmin-Abgleich in der App (T23) kommen die Dateien ohnehin von Lastwerk selbst.

Ort: `<lokaler Datenordner>/Lastwerk/fit` (Windows `%LOCALAPPDATA%\Lastwerk\fit`), derselbe Stamm wie die Datenbank (5.4). Nicht der Programmordner der Installation: Den ersetzt jedes Update und die Deinstallation löscht ihn, und unter `Program Files` darf die App nicht schreiben. `LASTWERK_FIT` überschreibt den Ort für Tests (wie `LASTWERK_DB`).

## Umfang

1. Fester FIT-Stamm in `crates/cli/src/paths.rs` (`default_fit_root`, schon vorhanden aus T23) für App und Kommandozeile. Garmin-Dateien weiter in `fit/garmin/`. Die Einstellung `app.fit_dir` wird nicht mehr geschrieben und nur noch für die Übernahme gelesen.
2. Übernahme beim Start (einmalig, ohne Rückfrage): Ist `app.fit_dir` gesetzt, existiert und ist nicht der feste Stamm, kopiert Lastwerk alle `*.fit` (rekursiv) mit gleichem relativen Pfad in den festen Stamm (vorhandene gleiche Dateien überspringen, nichts löschen, nichts überschreiben), damit `source_file.rel_path` gültig bleibt. Danach `app.fit_dir` entfernen und Ergebnis (Anzahl) einmal in der Oberfläche melden. Ein fehlender alter Ordner wird übersprungen und gemeldet. Test mit synthetischen Dateien.
3. Befehle: `choose_folder` entfällt. `import_folder` liest immer den festen Stamm. Neu `add_fit_files`: Dateidialog im Rust-Teil (Mehrfachauswahl, nur `.fit`), kopiert die gewählten Dateien nach `fit/manual/` (Name aus SHA-256-Präfix plus Originalendung, keine Pfade aus der Eingabe, Größenlimit wie der Import) und importiert danach. Für Edge- oder USB-Dateien ohne Garmin Connect; eine einmalige Handlung, keine Einstellung.
4. Oberfläche: Erststart ohne Schritt "Ordner". Einstellungen: Abschnitt "Daten" zeigt nur noch, wo Lastwerk ablegt (Text, kein Pfadfeld zum Ändern; Pfad nur als Anzeige ist erlaubt, weil er dem Nutzer gehört), Knöpfe "Neu einlesen" und "FIT-Dateien hinzufügen". Texte de/en, entfallene Schlüssel entfernen.
5. SPEC: F1, 5.4 (Befehle, Zahl), 5.6, 6.1 mit Vermerk der Entscheidung; Rauchtest und Attrappe anpassen.

## Abnahme

```
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
(cd app/src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test)
pnpm --dir app/ui lint && pnpm --dir app/ui check && pnpm --dir app/ui test && pnpm --dir app/ui build
sh scripts/check-privacy.sh && sh scripts/check-isolation.sh && sh scripts/check-i18n.sh
```

## Vermerke

### Abhängigkeiten

Keine neue Fremdabhängigkeit. `cargo deny --manifest-path app/src-tauri/Cargo.toml --config deny.toml check`: `advisories ok, bans ok, licenses ok, sources ok`.

### Festlegungen und Abweichungen

Ausführlich in SPEC 5.4 ("Umsetzung in T28"), F1, 5.6 und 6.1. Kurz:

1. **Ort.** `paths::default_fit_root_from(platform, lastwerk_fit, ...)` mit `LASTWERK_FIT` (ganzer Ordnerpfad, nicht leer, geht vor). `LASTWERK_GARMIN_FIT` und `paths::garmin_fit_override` entfallen. Begründung für den Datenordner der App statt des Installationsordners steht in der SPEC: Updates ersetzen den Programmordner, die Deinstallation löscht ihn, `Program Files` ist nicht beschreibbar.
2. **Übernahme** (`crates/cli/src/fit_migration.rs`, neu, damit App und Kommandozeile dieselbe Regel haben): kopiert `*.fit` rekursiv mit gleichem relativen Pfad, überspringt gleiche, überschreibt und löscht nie, folgt keinen Verknüpfungen, kopiert über `.part`. `app.fit_dir` wird danach entfernt, außer bei fehlgeschlagenem Kopieren (nächster Start versucht es erneut). Fehlender alter Ordner: gemeldet, Einstellung entfällt. Die App führt sie im `setup` aus, `lastwerk garmin sync` ebenfalls.
3. **Meldung an die Oberfläche** über `Settings.fit_migration` (Zahlen, nie ein Pfad), zusammen mit `Settings.fit_root` (nur Anzeige). Der Wert lebt im Prozess und bleibt bis zum Programmende stehen, die Oberfläche zeigt eine ausblendbare Notiz. Gegenüber einem Ereignis (kann vor dem `listen` verloren gehen) und einem Merker in der Datenbank ist das die einfachste robuste Lösung. Beide Felder setzt der Befehl (`AppState::decorate`, auch für `set_athlete`), die Datenbank kennt sie nicht.
4. **Abweichung:** Die Aufgabe nannte eine einmalige Meldung. Die Notiz bleibt bis zum Ausblenden oder Programmende; wer vorher schließt, verpasst nur die Meldung, nicht die Dateien.
5. **`Store::delete_setting`** ist neu in `crates/store` (nicht im Umfang genannt, aber nötig, um `app.fit_dir` zu entfernen). Test `delete_setting_removes_only_that_key`.
6. **Befehle:** 29, `choose_folder` entfällt, `add_fit_files` ist neu. Entfallen sind außerdem die Fehlercodes `no_folder` und `garmin_fit_dir_missing`, der Messreihenstatus `no_folder`, `GarminSyncSummary.fit_dir_assigned`, `SyncError::FitDirMissing`, `service::remember_fit_dir`, `service::fit_dir`. Neu: Fehlercode `fit_root_io`. `service::import`, `set_threshold`, `set_athlete` und `get_activity` bekommen den Stamm als Parameter.
7. **`add_fit_files`:** Name `manual/<32 Hexzeichen SHA-256>.fit`, Größenlimit und Lesen wie der Import (`read_fit_file`, 64 MiB), keine weitere Prüfung der Aktivität (das meldet der Import je Datei). Ergebnis `{ cancelled, selected, copied, already_present, rejected, import }`.
8. **Oberfläche:** Wizard mit vier Schritten, `FolderSection.svelte` ersetzt durch `DataSection.svelte` (Baustein `Section`), neu `MigrationNotice.svelte` und der Speicher `fitMigration` in `state.ts`. Die Attrappe bekommt `addFitFiles`, die Option `fitMigration` und im Browser `?uebernahme=fertig|fehlt`. Texte: neue Schlüssel `ui.settings.*` (statement, location, location_hint, rescan, rescan_hint, add_files, add_files_hint, add_result, add_rejected) und `ui.migration.*`; entfallen sind `ui.settings.fit_folder`, `no_folder`, `choose_folder`, `import`, `ui.onboarding.folder.*`, `ui.error.no_folder`, `ui.error.garmin_fit_dir_missing`, `ui.garmin.fit_dir_assigned`, `ui.detail.series_no_folder`; umformuliert sind Texte, die den Ordner nannten.

### Tests

- Rust, Wurzel: `cargo test --workspace` 756 Tests grün (vorher 746). Neu: `crates/cli/tests/fit_migration.rs` (7), `paths.rs` (`LASTWERK_FIT`), `fit_location_creates_the_fixed_root_with_a_garmin_subfolder`, `delete_setting_removes_only_that_key`.
- Rust, App: 78 Tests grün. Neu: `tests/fit_files.rs` (5), geänderte Tests für Import, Abgleich über den festen Stamm, Einstellungen.
- Vitest: 185 Tests grün, neu `views/settings/DataSection.test.ts` (10), `Wizard.test.ts` mit vier Schritten.
- Playwright: 16 Tests grün, neu Neu einlesen ohne Ordnerwahl, Dateien hinzufügen, Notiz zur Übernahme.

### Nicht geprüft

- `add_fit_files` mit dem echten Dateidialog (unter Linux ohne Fenster nicht auslösbar); getestet ist die Ablage (`service::fit_files`) und die Attrappe der Oberfläche.
- Die Übernahme in einer laufenden App mit einer echten Datenbank aus einer Vorversion (getestet ist die Funktion mit synthetischen Dateien und einer Datenbank im Speicher).
- Windows und macOS (gebaut und getestet ist nur Linux).
