# T46: CI straffen und Schlüsselbund ohne zbus

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | nach 1.0 |
| cloudfähig | ja |
| Voraussetzung | T38 |
| Bereich | `.github`, `scripts`, `crates/garmin`, `app/ui`, `docs`, `CLAUDE.md` |
| SPEC | 5.3, 5.4 (`lastwerk-garmin`), 11.6 |

## Warum

Analyse vom 18. Oktober 2026: 13 von 58 Läufen rot, keiner durch Code auf `main`. Ursachen:

- Der tägliche Lauf der Sicherheitshinweise wurde ohne Commit rot (zurückgezogene Crates, dann RUSTSEC-2026-0370 bis 0372: `zvariant`, `zbus_names`, `zvariant_derive` nicht mehr gepflegt, über `keyring` 4.2 und zbus) und blockierte damit jeden Release.
- Dependabot-PRs scheiterten: npm-PRs schrieben das Shim-Paket von `file:` zu `link:` um (Tauri-CLI fehlte), `bridge/` stand als `pip` statt `uv` drin (Lockdatei nicht mitgezogen).
- gitleaks lief lokal nicht mit.
- `ubuntu-latest` wechselt ab 19. Oktober 2026 auf Ubuntu 26.

Entscheidungen des Nutzers vom 18. Oktober 2026: Ursache beheben statt ausnehmen; danach "straffen, nicht mit zu vielen Regeln und Mauern verkomplizieren".

## Umgesetzt

1. `crates/garmin`: `keyring-core` mit einem Backend je Plattform statt `keyring` 4.2; Linux über `dbus-secret-service-keyring-store` (libdbus). zbus ist aus beiden Lockdateien verschwunden. CI-Job `rust` installiert `libdbus-1-dev`.
2. Linux-Runner fest auf `ubuntu-24.04`.
3. `advisories.yml`: nur cargo-deny, bei geänderter Lockdatei und vor jedem Release, kein täglicher Lauf.
4. Tauri-Shim als Skript `tauri` in `app/ui/package.json` statt als eigenes Paket; Aufruf `pnpm --dir app/ui tauri ...`. Damit kann Dependabot nichts mehr umschreiben.
5. Dependabot monatlich, gruppiert, Tauri-Projekt dabei, `bridge/` als `uv`.
6. `check-all.sh` mit gitleaks, `pnpm audit`, `ruff format`; zwei Symlink-Tests in `bridge/` werden übersprungen, wo das System keine Symlinks erlaubt (Windows).
7. Session-Start meldet rote Läufe auf `main`.

Beim Straffen wieder entfernt: Job `scope` für Doku-Commits, `docs/ci.md`, Werkzeug-Installation in `cloud-env-setup.sh`, Sonderprüfungen in `check-all.sh`.

## Abhängigkeiten

| Paket | Version | Lizenz | Grund |
|---|---|---|---|
| keyring-core | 1.0.1 | MIT OR Apache-2.0 | Kern des Schlüsselbunds, vorher indirekt über `keyring` |
| windows-native-keyring-store | 1.1.0 | MIT OR Apache-2.0 | Windows, unverändert, jetzt direkt |
| apple-native-keyring-store | 1.0.2 | MIT OR Apache-2.0 | macOS, unverändert, jetzt direkt |
| dbus-secret-service-keyring-store | 1.0.1 | MIT OR Apache-2.0 | Linux, ersetzt zbus; vom Projekt hinter `keyring` |
| dbus-secret-service, dbus, libdbus-sys | 4.1.0, 0.9.12, 0.2.7 | MIT OR Apache-2.0 | darunter; `dbus` stand schon über Tauri im Lock von `app/src-tauri` |
| @tauri-apps/cli (npm, dev) | 2.12.2 | Apache-2.0 OR MIT | unverändert, jetzt direkt in `app/ui` statt im Shim-Paket |

Entfernt: `keyring`, `zbus` mit `zvariant`, `zbus_names` und der async-Kette, `secret-service`, das Paket `lastwerk-tauri-shim`. Neu im Wurzel-Lock die ältere RustCrypto-Reihe von `dbus-secret-service` (`aes` 0.8, `cbc` 0.1, `hkdf` 0.12, `sha2` 0.10 und Zubehör, MIT OR Apache-2.0).

## Abnahme

`cargo deny check advisories` und `bans licenses sources` in beiden Workspaces grün; `keyring_round_trip_live` unter Windows gegen die echte Anmeldeinformationsverwaltung bestanden; `pnpm --dir app/ui tauri build --debug` lokal gebaut; `check-all.sh` Exit 0; CI auf `main` grün, Release v1.2.1 automatisch veröffentlicht.

## Manuelle Schritte

Linux mit Secret Service: `cargo test -p lastwerk-garmin --lib -- --ignored keyring_round_trip_live` einmal von Hand, falls Lastwerk unter Linux genutzt wird.
