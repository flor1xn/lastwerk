# T06: Oberfläche: Tauri-Shell und Übersichten (app)

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | M2 |
| Voraussetzung | T05 |
| cloudfähig | ja (Linux-Build, Playwright siehe Hinweis) |
| Bereich | `app/src-tauri`, `app/ui`, `.github/workflows/ci.yml` |
| SPEC | 5.3, 10.3, 10.4, 4.1 (F7, F14), 12 (M2), 14 (O11, O12) |

## Ziel

Eine startende Desktop-App mit Wochenübersicht, Aktivitätsliste, Aktivitätsdetail (ohne Karte) und Einstellungen, in Deutsch und Englisch, dunkel zuerst, ohne Netzwerkzugriff.

## Umfang

1. **Spikes**: Diagrammbibliothek (O11, zum Beispiel uPlot gegen ECharts) und i18n-Bibliothek für Svelte (O12) je kurz vergleichen. Entscheidung mit Begründung und Lizenz in SPEC 14 eintragen. Keine Remote-Schriften, keine Remote-Skripte.
2. `app/src-tauri`: Tauri 2 als **eigenes Cargo-Projekt** (nicht Mitglied des Wurzel-Workspace, damit `cargo test --workspace` ohne Systembibliotheken läuft). Hängt per Pfad von `crates/*` ab. Eigenes `Cargo.lock`, eigene `deny`-Prüfung.
3. Tauri-Befehle als dünne Schicht über `lastwerk-store` und `lastwerk-core`: Ordner wählen, importieren (mit Fortschritt), Wochen- und Listen- und Detaildaten, Schwellenwerte lesen und setzen, Einstellungen.
4. Ansichten: Wochenübersicht (Last je Sportart, PMC-Verlauf), Aktivitätsliste (Filter, Duplikate sichtbar), Detail (Kennzahlen, Zonen, Leistungs- und HF-Verlauf, ohne Karte), Einstellungen (Ordner, Schwellenwerte mit Gültigkeitsdatum, Sprache).
5. Texte nur über `catalog/locales/de.json` und `en.json` mit gleichen Schlüsseln (`sh scripts/check-i18n.sh`).
6. Tests: Vitest für Komponenten, Playwright-Rauchtest (öffnet die vier Ansichten in beiden Sprachen ohne Konsolenfehler, blockiert externe Hosts, Test schlägt bei jeder externen Anfrage fehl).
7. CI: zusätzlicher Job `tauri` (Linux, Systembibliotheken per `apt`, `tauri build --debug` als Artefakt).

## Nicht im Umfang

Planansicht und Export (T09), Karte, Modell-Anbindung.

## Vorgaben

- Tauri-Capabilities minimal (SPEC 10.4): Ordnerdialog, Lesen des gewählten Ordners, Schreiben in App-Datenordner und Exportziel. Keine Shell-, keine HTTP-Berechtigung im Frontend. CSP ohne `unsafe-eval` und ohne Inline-Skripte, `img-src` ohne externe Hosts (Karten sind aus, SPEC 10.3).
- Nicht berechenbare Lasten erscheinen als "nicht berechenbar", nie als 0 (SPEC 5.4).
- Geschätzte Werte (HR_max aus Formel, rTSS, sTSS) sind in der Oberfläche als "geschätzt" gekennzeichnet.
- Dichtes, datenorientiertes Layout. Kein Marketing-Look.

## Abnahme

```
pnpm --dir app/ui install --frozen-lockfile
pnpm --dir app/ui lint
pnpm --dir app/ui check
pnpm --dir app/ui test
pnpm --dir app/ui exec playwright test
pnpm --dir app/ui exec tauri build --debug
sh scripts/check-i18n.sh
```

Erfolg (SPEC 12, M2): Playwright-Rauchtest grün ohne Konsolenfehler in Deutsch und Englisch, kein externer Netzwerkverkehr, Debug-Build entsteht, die Binärdatei läuft 5 Sekunden ohne Absturz (unter Linux mit `xvfb-run`).

## Manuelle Schritte

Optik und Bedienung durch den Nutzer prüfen (Screenshots aus Playwright als Artefakt der CI ablegen, nie mit echten Daten).

## Blocker oder Vermerke

- Playwright lädt Browser von einem Host, der in der Cloud-Umgebung "Trusted" möglicherweise nicht freigegeben ist. Dann in der Umgebung unter "Custom" den Host ergänzen (docs/remote-sessions.md) oder die Rauchtests mit einem per `apt` installierten Chromium ausführen. Der Blocker ist dann in dieser Datei einzutragen, nicht zu umgehen.
- Der Tauri-Build in der Cloud braucht die Systembibliotheken aus `scripts/cloud-env-setup.sh`.

### Ergebnis (13. Oktober 2026)

Keine Blocker. Alle Teile des Umfangs sind umgesetzt, die Prüfungen stehen unten.

**Entscheidungen der Spikes** (auch in SPEC 14 eingetragen):

- O11 Diagramme: **uPlot 1.6.32** (MIT). Canvas, rund 50 KB, ohne Remote-Ressourcen, schnell bei langen Messreihen (Leistung und Herzfrequenz einer mehrstündigen Fahrt). Alternative ECharts 6.1.0 (Apache-2.0): deutlich größer (hunderte KB gepackt) und mehr Funktionen, als die Übersichten brauchen. Die Last-Balken der Woche sind schlichtes HTML und CSS ohne Bibliothek.
- O12 i18n: **eigener Store ohne Bibliothek** (`app/ui/src/lib/i18n.ts`). Zwei Sprachen, Schlüssel mit Punkten, `t(key, params)` über `catalog/locales/de.json` und `en.json`, zur Build-Zeit eingebunden. Alternative `svelte-i18n` 4.0.1 (MIT): bringt ICU-Nachrichtenformat und Laden zur Laufzeit mit, beides unnötig, und eine Abhängigkeit mehr. Zahlen und Daten formatiert `Intl` (Dezimalkomma in de, Punkt in en).

**Neue Abhängigkeiten** (Lizenzen geprüft, passen zu MIT OR Apache-2.0):

| Ort | Name | Version | Lizenz | Grund |
|---|---|---|---|---|
| npm | uplot | 1.6.32 | MIT | Diagramme (O11) |
| npm | @tauri-apps/api | 2.12.3 | Apache-2.0 OR MIT | `invoke` und `listen` im Frontend |
| npm (dev) | @tauri-apps/cli | 2.12.2 | Apache-2.0 OR MIT | `tauri build` und `tauri dev`, über `lastwerk-tauri-shim` eingebunden |
| npm (dev) | lastwerk-tauri-shim | file:tools/tauri-shim | MIT OR Apache-2.0 | eigenes Skript, siehe Abweichungen |
| npm (dev) | @playwright/test | 1.56.1 | Apache-2.0 | Rauchtest. Version passend zu Chromium 1194 der Cloud-Umgebung gepinnt |
| npm (dev) | jsdom | 30.1.2 | MIT | DOM für Komponententests (Vitest) |
| npm (dev) | @testing-library/svelte | 5.4.2 | MIT | Komponententests |
| npm (dev) | @types/node | 22.20.5 | MIT | Typen für `vite.config.ts` und die Playwright-Dateien |
| Cargo | tauri | 2.12.3 | Apache-2.0 OR MIT | App-Hülle (SPEC 5.3), Standardfunktionen |
| Cargo | tauri-build | 2.7.2 | Apache-2.0 OR MIT | Build-Skript, erzeugt die Berechtigungen aus der Befehlsliste |
| Cargo | tauri-plugin-dialog | 2.8.2 | MIT OR Apache-2.0 | Ordnerdialog, nur aus Rust aufrufbar |
| Cargo | serde, chrono (Feature `clock`), serde_json (nur Tests) | 1.0.229, 0.4.45, 1.0.154 | MIT OR Apache-2.0 | DTOs, Datum von heute, Tests |

Die Cargo-Kette von Tauri umfasst rund 430 Pakete. Lizenzen laut `cargo metadata`: überwiegend MIT und Apache-2.0, dazu Unicode-3.0, Zlib, BSD-3-Clause, ISC, CC0, 0BSD. **Fünf Pakete stehen unter MPL-2.0**: `cssparser` 0.37.0, `cssparser-macros` 0.7.1, `selectors` 0.38.0, `dtoa-short` 0.3.5 (alle über `tauri-utils` für die CSP-Verarbeitung) und `option-ext` 0.2.0 (über `dirs`). MPL-2.0 ist ein schwaches Copyleft auf Dateiebene: Unveränderte Abhängigkeiten dürfen in ein Programm unter MIT OR Apache-2.0 eingebunden werden, Pflichten entstehen nur für geänderte MPL-Dateien. Diese fünf Pakete sind in `deny.toml` als Ausnahmen je Paket eingetragen, nicht als allgemein erlaubte Lizenz. **Der Nutzer sollte das bestätigen** (offener Punkt unten).

**Abweichungen und Konventionen:**

- `crates/store`: kleine Ergänzung `read.rs` mit gezielten Lesezugriffen für die Oberfläche (`activity`, `activities_between`, `loads_between`, `activity_loads_of`, `zone_times_of`, `best_powers_of`, `source_file`). Die Gesamtlesungen hätten bei jedem Klick die ganze Datenbank gelesen. Die Zeilenumwandlung ist mit den bestehenden Methoden geteilt, die Ergebnisse sind in `tests/read.rs` gegen die gefilterte Gesamtlesung geprüft. Sonst nichts in `crates/*` geändert.
- Tauri-CLI und Ordnerlayout: Die CLI sucht `tauri.conf.json` nur im aktuellen Ordner und darunter. Mit `app/ui` und `app/src-tauri` als Nachbarn (SPEC 5.2) fände `pnpm --dir app/ui exec tauri build --debug` sie nicht. Das Paket `app/ui/tools/tauri-shim` stellt das Kommando `tauri` und trägt vor dem Aufruf der unveränderten CLI `TAURI_APP_PATH` und `TAURI_FRONTEND_PATH` ein. Damit gilt die Abnahme wörtlich. Bereits gesetzte Werte bleiben.
- Datenbankpfad: `<lokaler Datenordner>/Lastwerk/lastwerk.db` (Tauri `local_data_dir`, nicht `app_local_data_dir`), damit App und Kommandozeile dieselbe Datei nutzen (SPEC 6.1, `lastwerk`-Standardpfad). Unter macOS weicht die CLI ab (`~/.local/share`), dort sind die Pfade verschieden. `LASTWERK_DB` überschreibt den Pfad (Tests, Vorführung).
- Der Ordner mit den FIT-Dateien kommt nur über den Rust-Befehl `choose_folder` in die Einstellungen. `set_settings` nimmt keinen Pfad an. Das Frontend kann so keinen beliebigen Ordner lesen lassen.
- Fehler gehen als `{ code, message_key }` an das Frontend. Dateipfade des Imports verlassen den Rust-Teil nicht, die Zusammenfassung enthält Fehlercodes mit Anzahl.
- Der Verlauf von Leistung und Herzfrequenz kommt nicht aus dem Cache (er hat keine Messpunkte, SPEC 6.1), sondern aus der FIT-Datei, die `get_activity` erneut liest. Stimmt die SHA-256 nicht mehr zur importierten, gibt es keine Reihe (`file_changed`). Die Reihe ist auf höchstens 800 Punkte gemittelt, unplausible Werte (6.5) werden nicht gezeichnet.
- Die Wochenübersicht zeigt den Verlauf bis heute, nicht darüber hinaus (ein Verlauf ohne Plan wäre nur ein Abklingen). Ohne jede zählende Last gibt es keinen Verlauf statt Nullen.
- Dauerhaft geschätzt gekennzeichnet: rTSS, sTSS und Session-RPE (Methode), Zonen nach Herzfrequenzreserve auf der aus dem Alter geschätzten HR_max, die Schätzung der HR_max in den Einstellungen. hrTSS mit geschätzter HR_max trägt noch keine Kennzeichnung, weil `activity_load` sie nicht speichert (Vorschlag für später: Spalte `estimated`).
- Bündelung (`bundle.active`) ist aus. `tauri build --debug` erzeugt nur die Binärdatei. Installationspakete kommen mit dem Release-Workflow (T09).
- Playwright-Version 1.56.1 statt der neuesten, weil die Cloud-Umgebung Chromium in der dazu passenden Revision vorinstalliert hat. In der CI installiert `playwright install` den Browser selbst.

**Prüfungen:**

- `pnpm --dir app/ui lint`, `check`, `test` (43 Tests in 5 Dateien), `exec playwright test` (7 Tests), `exec tauri build --debug`, Start des Binaries mit `xvfb-run -a timeout 5` (Exit-Code 124, läuft ohne Absturz), `sh scripts/check-i18n.sh`, `check-privacy.sh`, `check-isolation.sh`.
- In `app/src-tauri`: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (22 Tests: Befehle gegen eine Datenbank mit den synthetischen Fixtures, Konfigurationsprüfungen für CSP, Capabilities und Fehlertexte). Wurzel: `cargo test --workspace` bleibt ohne Systembibliotheken grün.
- `cargo deny check` (Version 0.20.2, lokal installiert) mit der `deny.toml` der Wurzel: grün für `app/src-tauri` und für die Wurzel (Lizenzen, Quellen, Verbote, Hinweise). Dafür trägt `deny.toml` die fünf MPL-2.0-Ausnahmen und den Hinweis RUSTSEC-2024-0370 (`proc-macro-error`, nicht gepflegt, nur Makro zur Übersetzungszeit in den GTK3-Bindungen von Tauri, keine sichere Aktualisierung) mit Begründung. `cargo audit` für `app/src-tauri/Cargo.lock` läuft in der CI noch nicht (der Job `supply-chain` prüft nur die Wurzel), er würde denselben Hinweis melden.
- `tauri dev` unter Xvfb gestartet: Vite und App laufen.
- Das Programm wurde unter Xvfb gestartet und der Bildschirm aufgenommen: Die Oberfläche lädt unter der echten CSP, die Befehle antworten.

**Offen für den Nutzer:**

- Optik und Bedienung ansehen (Screenshots aus dem Rauchtest liegen als Artefakt `ui-screenshots` der CI).
- MPL-2.0 der fünf Pakete bestätigen (siehe oben).
- Die Verbindung zur Garmin-Welt gibt es hier nicht, nichts davon wurde berührt.

**Hinweise für T09 (Planansicht):** Neue Ansicht in `app/ui/src/views`, Eintrag in `VIEWS` (`lib/state.ts`) und `ui.nav.*`. Neue Befehle in `src-tauri/src/lib.rs` (`COMMAND_NAMES`, `generate_handler!`), `build.rs` und `capabilities/default.json` gleichzeitig eintragen, ein Test prüft die drei Stellen. Dazu eine Methode im `Backend`-Interface (`lib/backend/types.ts`) mit Umsetzung in `tauri.ts` und `mock.ts`. Exportziel schreiben: Dateidialog im Rust-Befehl öffnen (wie `choose_folder`), kein `fs`-Plugin im Frontend freigeben. Schlüssel `ui.*` bleiben bei dieser Oberfläche, `template.`, `reason.`, `rule.` kommen aus T07.
