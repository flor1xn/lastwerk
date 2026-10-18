# CI und Sicherheitsprüfungen

Welche Prüfung wann läuft, warum es sie gibt und was zu tun ist, wenn sie rot wird. Entscheidungen und Verlauf stehen in SPEC 11.6, die Analyse vom 18. Oktober 2026 in [tasks/T46-ci-haertung.md](tasks/T46-ci-haertung.md).

## Grundsätze

1. **Vor jedem Push** `sh scripts/check-all.sh`, Ergebnis Exit 0. Exit 3 heißt: Werkzeug fehlt, Teillauf, nicht pushen. Das Skript prüft lokal alles, was die CI unter Linux prüft.
2. **Zu Beginn jeder Session** zeigt der SessionStart-Hook Workflows, deren letzter Lauf auf `main` rot ist. Ohne Hook: `gh run list --branch main --limit 10`. Ein roter Lauf auf `main` geht jeder anderen Arbeit vor, auch der tägliche Lauf der Sicherheitshinweise, denn er blockiert jeden Release.
3. **Ursache beheben, nicht ausnehmen.** Ein nicht mehr gepflegtes oder unsicheres Crate wird ersetzt, wenn es einen Ersatz gibt. Eine Ausnahme in `deny.toml` braucht Grund, Datum und den Vermerk, warum es keinen Ersatz gibt.
4. **Feste Versionen.** Runner (`ubuntu-24.04`), Rust (`rust-toolchain.toml`), Werkzeuge (cargo-deny 0.20.2, actionlint 1.7.7, gitleaks 8.18.4, zizmor 1.31.0) und Actions (per Commit-SHA). Ein Wechsel geschieht bewusst in einem eigenen Commit, nie über ein `-latest`.
5. **Ein Push je Session**, zuerst auf den Arbeitszweig `claude/...`, nach grüner CI auf `main` (CLAUDE.md, Schritt 6).

## Workflows

| Datei | Auslöser | Prüft | Warum nötig |
|---|---|---|---|
| `ci.yml` | Push auf `main` und `claude/**`, PR, Knopfdruck | `scope`: nur Doku geändert? `guards`: Datenschutz, Verlaufsprüfung, Isolation, Sprachdateien, actionlint, zizmor, gitleaks. `rust` (Ubuntu, Windows): fmt, clippy, Tests. `supply-chain`: Lizenzen, Verbote, Quellen. `ui`: Lint, Typen, Tests, `pnpm audit`. `tauri`: Playwright, Tauri-Projekt, Debug-Build, Start. `ollama-setup`, `installer-hook`: Windows-Skript und NSIS-Hook | Kern der Qualität. Bei reiner Doku laufen nur `scope` und `guards` |
| `advisories.yml` | täglich 06:17 UTC, Push mit geänderter Lockdatei, `Cargo.toml` oder `deny.toml`, Knopfdruck, vor jedem Release | `cargo deny check advisories` für Wurzel und Tauri-Projekt | Neue RUSTSEC-Hinweise erscheinen ohne Codeänderung; getrennt von der Push-CI, damit sie nicht jeden Push rot machen |
| `bridge.yml` | Push oder PR mit Änderung in `bridge/`, `catalog/`, `schema/` | ruff, pytest | `bridge/` ist eingefroren (T20), wird aber weiter getestet |
| `auto-release.yml` | grüne CI eines Pushes auf `main` | Produktänderung seit dem letzten Tag? Dann `release.yml` | Installer und Update immer aktuell (T30) |
| `release.yml` | Tag `v*`, Knopfdruck, Aufruf aus `auto-release.yml` | Tag, Sicherheitshinweise, Installer Windows und Linux, Notizen, Release | Auslieferung |
| `publish-release.yml` | Knopfdruck | veröffentlicht einen Entwurf | Cloud-Sessions können keine Tags pushen |
| Dependabot | wöchentlich, Cargo (Wurzel und Tauri), npm, uv, Actions | kleine Aktualisierungen gruppiert, Hauptversionen einzeln | Lockdateien frisch halten; zurückgezogene Crates fallen sonst erst in `advisories.yml` auf |

Bewusst behalten: Die CI läuft für denselben Commit auf `claude/...` und danach auf `main`. Der erste Lauf ist das Vorab-Signal, der zweite löst `auto-release.yml` aus. Im öffentlichen Repository kosten die Standard-Runner nichts.

## Wenn es rot ist

| Meldung | Vorgehen |
|---|---|
| `error[yanked]` in `advisories.yml` | `cargo update -p <crate>` im betroffenen Workspace (Wurzel oder `--manifest-path app/src-tauri/Cargo.toml`). Gibt es keine neuere passende Version, das Crate ersetzen, das es einbindet. |
| `error[unmaintained]` oder `error[vulnerability]` | `cargo tree -i <crate> --target all` in beiden Workspaces: Wer bindet es ein? Neuere Version des Einbinders, sonst Ersatz (Beispiel T46: `keyring` mit zbus → `keyring-core` mit dbus). Nur wenn es nachweislich keinen Ersatz gibt: Ausnahme in `deny.toml` mit Grund und Datum. |
| `warning[advisory-not-detected]` | Ausnahme in `deny.toml` greift nicht mehr. Gilt sie auch im anderen Workspace nicht, löschen. `RUSTSEC-2024-0370` steckt nur im Tauri-Projekt und bleibt deshalb. |
| gitleaks: `leaks found` | Lokal `gitleaks detect -v --redact`. Echtes Geheimnis: sofort widerrufen, nie nur löschen (der Verlauf ist öffentlich). Fehlalarm: Fingerabdruck mit Begründung in `.gitleaksignore`. |
| Dependabot-PR rot | In einer Session übernehmen: Änderung in einem eigenen Commit nachbauen und reparieren, PR mit Hinweis schließen. Kein Merge-Commit. |
| npm-PR von Dependabot rot im Job `tauri`: `Cannot find module '@tauri-apps/cli/main'` | Dependabot hat `file:tools/tauri-shim` zu `link:` umgeschrieben. In `app/ui/package.json` wieder `file:` setzen und `pnpm install` (Beispiel 1f3f564). Die CLI nicht direkt in `app/ui` eintragen, sonst startet `tauri` ohne Shim (T46). |
| Dependabot-PR für `bridge/` rot in `test_allowlist` | Gewollt: Die Garmin-Bibliothek bringt neue Methoden mit. Jede wird in `bridge/` als lesend oder schreibend eingestuft, bevor das Update übernommen wird. Schreibende Methoden bleiben gesperrt. |
| Dependabot-Sicherheitsupdate `security_update_not_possible` | Der Einbinder hält die alte Version (Beispiel `glib` 0.18 über Tauri GTK3). Hinweis prüfen; trifft er Lastwerk nicht, auf GitHub mit Grund verwerfen. |
| `Automatischer Release` rot im Job `advisories` | Folge eines roten `advisories.yml`, dort beheben; der nächste Push auf `main` löst den Release erneut aus. |
| Job `scope` überspringt zu viel | Er überspringt nur, wenn ausschließlich `docs/`, `*.md` oder `.claude/` geändert sind. Ein Test, der eine solche Datei liest, gehört nicht dorthin. |

## Lokale Werkzeuge

Windows: `cargo install cargo-deny@0.20.2 --locked`, actionlint und gitleaks über Scoop, `pip install zizmor==1.31.0`. Cloud: `scripts/cloud-env-setup.sh` installiert alles mit Prüfsumme. Nach einer Änderung an diesem Skript den Inhalt erneut in die Cloud-Umgebung einfügen ([remote-sessions.md](remote-sessions.md)).
