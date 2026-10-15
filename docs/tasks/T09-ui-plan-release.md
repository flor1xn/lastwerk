# T09: Oberfläche für Pläne und Release-Workflow (Version 1)

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | M3 |
| Voraussetzung | T06, T08 |
| cloudfähig | ja |
| Bereich | `app/ui`, `app/src-tauri`, `.github/workflows/release.yml` |
| SPEC | 4.1 (F8 bis F11, F14), 11.6 (Release), 12 (M3) |

## Ziel

Plan erzeugen, ansehen, validieren und exportieren in der App. Ein Tag `v*` baut die Installer für drei Systeme als Artefakte. Damit ist Version 1 (M1 bis M3) fertig.

## Umfang

- Ziel anlegen (Termin oder fortlaufend), Verfügbarkeit je Wochentag, Plan erzeugen mit Anzeige der Wochen, Einheiten, Warnungen (Code, Text, Quelle oder "Faustregel") und Begründungen.
- Verlauf CTL, ATL, TSB der Planwochen (Vorausberechnung aus `pmc`).
- Export: ICS und Plan-JSON, Backup und Wiederherstellung der Nutzerdaten.
- Text in Plänen und Erklärungen wird als Text gerendert, nie als HTML (SPEC 9.5).
- `.github/workflows/release.yml`: bei Tag `v*` Tauri-Build auf Windows, macOS, Linux, Artefakte ohne Signierung, Hinweis "ohne Support" in der Release-Beschreibung (SPEC 11.6).
- README: Status, Installationshinweis, Datenschutz- und Markenhinweis auf aktuellem Stand (SPEC 1.5, Veröffentlichungs-Checkliste).

## Nicht im Umfang

Modell-Anbindung (T15), MCP (T10). Das Veröffentlichen des Repositories (privat zu öffentlich) ist nicht Teil dieses Tasks.

## Vorgaben

- Keine neuen Tauri-Berechtigungen ohne Vermerk (SPEC 10.4).
- Der Release-Workflow wird nicht ausgelöst. Den Tag `v0.1.0` setzt der Nutzer.

## Abnahme

```
pnpm --dir app/ui lint
pnpm --dir app/ui check
pnpm --dir app/ui test
pnpm --dir app/ui exec playwright test
pnpm --dir app/ui exec tauri build --debug
cargo test --workspace
sh scripts/check-i18n.sh
```

Erfolg: Playwright-Rauchtest deckt Planansicht und Export ab, Plan aus der Oberfläche besteht `validate`, Release-Workflow ist syntaktisch gültig (Prüfung per `gh workflow view` oder einem Linter in der CI).

## Manuelle Schritte

1. Tag `v0.1.0` setzen und prüfen, dass der Release-Lauf auf allen drei Systemen baut (SPEC 12, M3).
2. Veröffentlichung nur nach der Checkliste aus SPEC 1.5 und mit Rückfrage.

## Blocker oder Vermerke

Keine Blocker. Erledigt am 13. Oktober 2026. Die manuellen Schritte oben (Tag `v0.1.0`, Veröffentlichung) stehen aus, der Release-Workflow wurde nicht ausgelöst.

### Ergebnis

**Umfang.** Neue Ansicht "Plan" (`app/ui/src/views/PlanView.svelte`, Teile unter `views/plan/` und `lib/components/Plan*.svelte`): Ziel setzen (Termin mit Datum, Sportart, Priorität oder fortlaufend, Bezeichnung bleibt lokal), Verfügbarkeit je Wochentag, Plan erzeugen, gespeicherte Pläne anzeigen, aktivieren und archivieren, Anzeige mit Wochen, Phase, Ziel- und Planlast, Begründungen, Einheiten mit Vorlagenname, Dauer, Last und Härte, Warnungen mit Code, Text und Quelle (`rule.R**.name` und `.explanation`), Prüfung gegen Schema und Leitplanken und der Verlauf von Fitness, Ermüdung und Form der Planwochen. Export von ICS, Plan-JSON und Backup, Wiederherstellung mit Bestätigung und Hinweis auf den neuen Import. Elf neue Befehle (SPEC 5.4, App), Attrappe für Vitest und Playwright (`mock-plans.ts`), `release.yml`, README.

**Entscheidung zum Bibliotheks-Crate.** `lastwerk-cli` ist jetzt Bibliothek und Binary (eigener erster Commit, nur Modulpfade und Sichtbarkeit, die insta-Dateien heißen nach dem Crate `lastwerk_cli__*`). Ein neues Crate `crates/planning` wäre mehr Verschiebung gewesen. Preis: Die App zieht `clap` mit, das nur das Binary nutzt. Vermerk in SPEC 5.4. Für T14: Neue insta-Snapshots in `crates/cli` tragen den Präfix `lastwerk_cli__`, `src/main.rs` enthält keine `mod`-Zeilen mehr (neue Module stehen in `src/lib.rs`).

**Neue Abhängigkeiten.** npm: keine. Cargo (nur `app/src-tauri`):

| Name | Version | Lizenz | Grund |
|---|---|---|---|
| lastwerk-cli (Pfad) | 0.0.0 | MIT OR Apache-2.0 | gemeinsame Planungslogik, Schemaprüfung, ICS, Sicherung. Zieht `jsonschema`, `icalendar`, `clap` mit (alle in der Wurzel schon geprüft) |
| uuid, nur Feature `v4` | 1.28.0 | Apache-2.0 OR MIT | Plan-ID, wie in der Kommandozeile |
| serde_json | 1.0 | MIT OR Apache-2.0 | war nur Dev-Abhängigkeit, jetzt für die Schemaprüfung nötig |

`cargo deny check` (Wurzel und `app/src-tauri` mit `deny.toml`): grün.

**Abweichungen** (alle in SPEC 5.4, 10.4 und 11.6 vermerkt):

- Der Planer rechnet mit den Standardwerten des Verlaufs (42 und 7 Tage), auch wenn die Einstellungen `pmc.*` abweichen. Die Planansicht weist darauf hin. Die Anbindung der Einstellungen an `planning::generate` ist ein Vorschlag, T09 änderte keine Logik der CLI. Erledigt am 14. Oktober 2026 (SPEC 5.4, Hinweis `pmc_settings_differ` entfernt).
- Die Fehlerzuordnung der App kannte die seit T08 neuen Varianten des Speichers nicht (`PlanNotFound`, `InvalidInput`), das Projekt übersetzte nicht. Sie ist ergänzt.
- Release: `bundle.active` bleibt in `tauri.conf.json` aus, `tauri.release.conf.json` schaltet die Bündelung für den Release ein (Überlagerung per `--config`). Lokal bestätigt: `tauri build --debug` baut weiter nur die Binärdatei, `tauri build --debug --config tauri.release.conf.json --bundles deb` erzeugt das deb-Paket. Windows, macOS und die AppImage-Bündelung konnten hier nicht laufen, das zeigt erst der Lauf auf einem Tag.
- Der macOS-Installer ist ein dmg für Apple Silicon (`macos-latest`).
- Der Release-Workflow nutzt `gh run download` und `gh release create` statt einer fremden Release-Action. Das Schreibrecht hat nur der Job `release`.
- Nur der Lauf mit einem Tag zeigt, ob die Aktionen `actions/*@v7` und `pnpm/action-setup@v6` wie in der CI laufen (gleiche Versionen wie `ci.yml`).

**Prüfungen** (alle grün): `pnpm --dir app/ui lint`, `check`, `test` (77 Tests in 10 Dateien, 34 neu: Plananzeige, Warnungstexte mit Quelle, Text statt HTML mit `<script>`, Formatierung, Ansicht mit Export und Wiederherstellung, Attrappe), `exec playwright test` (9 Tests, davon 2 neue für die Planansicht in Deutsch und Englisch mit Export), `exec tauri build --debug` (Start unter Xvfb: Exit-Code 124), in `app/src-tauri` `cargo fmt --check`, `clippy -D warnings`, `cargo test` (31 Tests, darunter 9 neue in `tests/plans.rs`: Plan aus `generate_plan` besteht Schema und `validate_plan` gegen die Testdatenbank, Verlauf nach Handrechnung, Zustände, Export, Backup mit Wiederherstellung), Wurzel `cargo fmt --all --check`, `clippy --workspace`, `cargo test --workspace`, `sh scripts/check-i18n.sh`, `check-privacy.sh`, `check-isolation.sh`, `actionlint` (v1.7.7) über alle Workflows (auch als Schritt im Job `guards` der CI).

**Offen für den Nutzer.** Tag `v0.1.0` setzen und prüfen, dass der Release-Lauf auf allen drei Systemen baut. Optik der Planansicht ansehen (Screenshots im CI-Artefakt `ui-screenshots`). Danach Veröffentlichung nur nach der Checkliste aus SPEC 1.5.
