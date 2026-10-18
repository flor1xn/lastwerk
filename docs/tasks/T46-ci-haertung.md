# T46: CI-Härtung und Schlüsselbund ohne zbus

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | nach 1.0 |
| cloudfähig | ja |
| Voraussetzung | T38 |
| Bereich | `.github`, `scripts`, `crates/garmin`, `docs`, `CLAUDE.md` |
| SPEC | 5.3, 5.4 (`lastwerk-garmin`), 10.2, 11.6 |

## Ziel

Keine roten Läufe mehr ohne eigenen Fehler, und jede Prüfung der CI läuft vor dem Push auch lokal. Der Garmin-Schlüsselbund kommt unter Linux ohne die nicht mehr gepflegte zbus-Kette aus.

## Warum

Analyse vom 18. Oktober 2026 über 58 Läufe, davon 13 rot. Die Push-CI auf `main` war grün, rot waren die Prüfungen um die Commits herum:

| Fehlerbild | Läufe | Ursache |
|---|---|---|
| Sicherheitshinweise, täglich | 16., 17., 18. Oktober | Ohne Codeänderung: zurückgezogene Crates (`zip`, `inout`), dann RUSTSEC-2026-0370, 0371, 0372 (`zvariant`, `zbus_names`, `zvariant_derive` nicht mehr gepflegt) über `keyring` 4.2 → `secret-service` → `zbus` 5. Keine Aktualisierung möglich, alle Glieder in der neuesten Version. |
| Automatischer Release | 16., 17. Oktober | Folge davon: `release.yml` prüft die Hinweise als Vorbedingung. Die zurückgezogenen Crates kamen per Lockdatei herein, die Push-CI prüft keine Hinweise. |
| gitleaks auf `claude/eager-cori-xg7wwt` | 2 × 16. Oktober | Fehlalarm. `check-all.sh` führte gitleaks nicht aus. |
| Dependabot-PRs | zip 9, `@types/node` 26, Playwright 1.64 | Hauptversionen; die Korrektur für Playwright lag grün auf `claude/happy-edison-ey4ytb`, kam aber nie auf `main`. |
| Dependabot-Sicherheitsupdate `glib` | 2 × 16. Oktober | Tauri hält glib 0.18 (GTK3). Hinweis inzwischen als "not_used" verworfen. |
| Droht | ab 19. Oktober | `ubuntu-latest` wechselt auf Ubuntu 26 (Hinweis im Lauf 39121647895). Tauri-Systempakete und die glibc-Basis von AppImage und deb hängen am Image. |

Entscheidung des Nutzers vom 18. Oktober 2026: Veraltetes restlos entfernen und an der Ursache beheben statt Ausnahmen eintragen; liegengebliebene Commits übernehmen, alte Zweige und PRs aufräumen.

## Umfang

1. `crates/garmin`: `keyring` 4.2 ersetzt durch `keyring-core` mit einem Backend je Plattform. Linux nutzt `dbus-secret-service-keyring-store` (libdbus) statt `zbus-secret-service-keyring-store`. `store.rs` bildet die Auswahl aus `keyring::v1` nach. zbus, zvariant, secret-service und die async-Kette sind aus beiden Lockdateien verschwunden.
2. Alle Workflows auf `ubuntu-24.04` statt `ubuntu-latest`.
3. `advisories.yml`: nur noch cargo-deny (cargo-audit las dieselbe Datenbank), zusätzlich bei jedem Push, der eine Lockdatei, ein Manifest oder `deny.toml` ändert.
4. `ci.yml`: Job `scope` überspringt bei reinen Dokuänderungen die schweren Jobs; `guards` läuft immer. Doppelte Schritte der Oberfläche im Job `tauri` und die doppelte Datenschutzprüfung unter Linux entfallen. Job `rust` installiert unter Linux `libdbus-1-dev`.
5. `dependabot.yml`: Tauri-Projekt aufgenommen, kleine Aktualisierungen gruppiert, `@types/node` ohne Hauptversionen. pnpm/action-setup 6.1.0 (PR #1) übernommen, Commits von `claude/happy-edison-ey4ytb` (PR #4) übernommen.
6. `check-all.sh`: gitleaks, `pnpm audit --prod`, `ruff format --check`, Sicherheitshinweise bei geänderter Lockdatei. `cloud-env-setup.sh` installiert cargo-deny, actionlint, gitleaks (feste Versionen mit Prüfsumme) und zizmor, damit `check-all.sh` auch in der Cloud mit Exit 0 enden kann.
7. `cloud-session-start.sh` zeigt zu Beginn jeder Session (lokal und Cloud) Workflows, deren letzter Lauf auf `main` rot ist.
8. `docs/ci.md` mit allen Workflows und dem Vorgehen bei Rot; Abschnitt in `CLAUDE.md`.

## Abhängigkeiten

| Paket | Version | Lizenz | Grund |
|---|---|---|---|
| keyring-core | 1.0.1 | MIT OR Apache-2.0 | Kern des Schlüsselbunds, schon bisher über `keyring` im Baum, jetzt direkt |
| windows-native-keyring-store | 1.1.0 | MIT OR Apache-2.0 | Windows, unverändert, jetzt direkt |
| apple-native-keyring-store | 1.0.2 | MIT OR Apache-2.0 | macOS (ungeprüft seit SPEC 11.6), unverändert, jetzt direkt |
| dbus-secret-service-keyring-store | 1.0.1 | MIT OR Apache-2.0 | Linux, ersetzt die zbus-Kette; vom selben Projekt wie `keyring` |
| dbus-secret-service | 4.1.0 | MIT OR Apache-2.0 | darunter, Funktion `crypto-rust` |
| dbus, libdbus-sys | 0.9.12, 0.2.7 | Apache-2.0/MIT | Bindung an libdbus; im Lock von `app/src-tauri` schon vorher über Tauri |

Neu im Wurzel-Lock außerdem die ältere RustCrypto-Reihe, die `dbus-secret-service` nutzt: `aes` 0.8.4, `cbc` 0.1.2, `cipher` 0.4.4, `inout` 0.1.4, `block-padding` 0.3.3, `hkdf` 0.12.4, `hmac` 0.12.1, `sha2` 0.10.9, `digest` 0.10.7, `generic-array` 0.14.7, `crypto-common` 0.1.7, `block-buffer` 0.10.4, `cpufeatures` 0.2.17, `zeroize_derive` 1.5.0 (alle MIT OR Apache-2.0). Entfernt: `keyring`, `zbus-secret-service-keyring-store`, `secret-service`, `zbus`, `zbus_macros`, `zbus_names`, `zvariant`, `zvariant_derive`, `zvariant_utils`, `endi`, `enumflags2`, die async-Kette (`async-io`, `async-lock`, `async-executor`, `async-process`, `blocking`, `polling`, `event-listener` und weitere) und die neue RustCrypto-Reihe 0.9/0.13.

Systembibliothek: libdbus-1 unter Linux zur Laufzeit. Jeder Linux-Desktop mit Secret Service hat sie, und die App linkte sie über Tauri schon vorher.

## Abnahme

- `cargo tree -i zbus` und `cargo tree -i zvariant` in beiden Workspaces: kein Treffer.
- `cargo deny check advisories` und `cargo deny check bans licenses sources` in beiden Workspaces grün.
- `cargo test -p lastwerk-garmin` grün; `keyring_round_trip_live` am 18. Oktober 2026 unter Windows gegen die echte Anmeldeinformationsverwaltung bestanden.
- `sh scripts/check-all.sh` mit Exit 0.
- CI auf dem Arbeitszweig und auf `main` grün, einschließlich `Rust (ubuntu-24.04)` mit dem dbus-Backend; danach `Automatischer Release` und `Sicherheitshinweise` grün.

## Manuelle Schritte

- Cloud-Umgebung: den Inhalt von `scripts/cloud-env-setup.sh` erneut in das Feld "Setup script" auf claude.ai/code einfügen (docs/remote-sessions.md), damit neue Sessions die Prüfwerkzeuge haben.
- Linux mit echtem Secret Service: `cargo test -p lastwerk-garmin --lib -- --ignored keyring_round_trip_live` einmal von Hand, falls Lastwerk unter Linux genutzt wird.

## Blocker oder Vermerke

Keine. Abnahme am 18. Oktober 2026: `check-all.sh` Exit 0 unter Windows; CI, Bridge und Sicherheitshinweise auf `claude/ci-haertung` und `main` grün, darunter `Rust (ubuntu-24.04)` mit dem dbus-Backend; automatischer Release v1.2.1 veröffentlicht. Nachgezogen: Dependabot führt `bridge/` als `uv`-Projekt, weil `pip` die Lockdatei nicht mitzog (PR #5). PRs #1, #4 und #5 geschlossen, alte Zweige `claude/*` gelöscht.

Offen (18. Oktober 2026): Dependabot schreibt bei npm-PRs `file:tools/tauri-shim` zu `link:` um; `link:` installiert die Abhängigkeit des Shims auf `@tauri-apps/cli` nicht, der Debug-Build scheitert (PR #4, PR #7). Der Versuch, die CLI direkt in `app/ui` einzubinden (0a0af80), ist zurückgenommen (db7eca4): Dann gibt es zwei Programme `tauri`, pnpm verlinkt das der CLI, und die Pfade des Shims fehlen. Bis zu einer Lösung (zum Beispiel ein eigener Befehlsname für den Shim) setzt eine Session bei solchen PRs `file:` von Hand zurück, wie in 1f3f564. `check-all.sh` prüft seitdem, dass `tauri` auf den Shim zeigt.
