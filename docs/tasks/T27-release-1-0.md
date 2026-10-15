# T27: Version 1.0

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | 1.0 |
| Voraussetzung | T21 bis T26 |
| cloudfähig | ja, Release manuell |
| Bereich | Wurzel, `app`, `catalog/changelog.json`, `docs`, `README.md` |
| SPEC | 11.6, 12 |

## Ziel

Lastwerk 1.0.0 ist gebaut, beschrieben und geprüft.

## Umfang

1. Version 1.0.0 in `Cargo.toml` (Workspace), `app/src-tauri/Cargo.toml`, `tauri.conf.json`, `app/ui/package.json`; Lockfiles neu schreiben lassen.
2. `catalog/changelog.json` (de, en): Garmin in der App, Dashboard, Cairo, Planer mit Gespräch.
3. `README.md`, `docs/remote-sessions.md`, `docs/README.md`, `app/README.md`: Garmin ohne Python, `bridge/` eingefroren.
4. Rauchtest (`app/ui/e2e`): Dashboard, Garmin-Schritt, Planer mit drei Fragen, Gespräch (Attrappe) in de und en.
5. `break-ui` auf Dashboard und Gespräch, Befunde beheben.
6. Status von T17 klären (in Prüfung, manueller Release-Test offen).

## Abnahme

Alle Befehle aus `CLAUDE.md` "Befehle", dazu `pnpm --dir app/ui build`.

## Manuelle Schritte

Tag `v1.0.0`, Release-Entwurf prüfen und veröffentlichen, Update aus 0.1.0 testen, Livetests aus T23 und T26.

## Vermerke

- 15. Oktober 2026: Die Versionsnummer steht nicht in den Dateien (`0.0.0` in `Cargo.toml`, `tauri.conf.json`, `package.json`), sondern kommt wie bisher aus dem Tag (`release.yml`, `scripts/build-installer.ps1`). Deshalb keine Anhebung in den Dateien, `v1.0.0` setzt die Version beim Bau.
- `catalog/changelog.json` hat den Eintrag `1.0.0` in de und en; `node scripts/release-notes.mjs v1.0.0 <ordner>` erzeugt daraus die Notizen. README (de, en), CLI-Übersicht und Beschreibung im Release-Overlay nennen Garmin in der App, Dashboard, Planer mit Gespräch und die feste Ablage.
- Rauchtest: 20 Playwright-Tests in de und en decken Dashboard, Garmin-Schritt und -Einstellungen, Planer mit drei Fragen, Gespräch (Attrappe), Datenabschnitt und Übernahme ab. break-ui lief in T24, T25, T26 und T28 je Ansicht.
- Abnahme auf dem Arbeitszweig grün: Workspace (fmt, clippy, Tests), App (fmt, clippy, Tests), `cargo deny` für beide, Oberfläche (lint, check, 235 Vitest, build), Datenschutz, Isolation, Sprachen.
- T17 bleibt in Prüfung (Update von 0.1.0 auf eine neuere Version ist nicht live geprüft). Status in Prüfung, bis die Livetests aus T23 (Garmin, O30, O9) und T26 (Ollama, O13) gelaufen sind und der Tag gesetzt ist.
- 15. Oktober 2026: Der Tag-Push aus der Cloud-Session bricht in der Umgebung ab. Deshalb hat `release.yml` einen manuellen Start mit Tag-Eingabe, und `publish-release.yml` veröffentlicht den geprüften Entwurf (SPEC 11.6). Freigabe für Release und Veröffentlichung liegt vor.
- 15. Oktober 2026: Release `v1.0.0` veröffentlicht (https://github.com/flor1xn/lastwerk/releases/tag/v1.0.0), Tag auf 8e12ee7. Gebaut über `release.yml` per manuellem Start (Lauf 38741008406, alle Jobs grün, Sicherheitshinweise eingeschlossen), veröffentlicht über `publish-release.yml`. Der Entwurf hatte `.msi`, `-setup.exe`, `.exe.sig`, `latest.json` (Version 1.0.0, `windows-x86_64` mit Signatur), `.dmg`, `.deb`, `.AppImage` und die Notizen in de und en. T27 ist damit erledigt; die Livetests liegen bei den Tasks, die sie betreffen: T17 (Update aus 0.1.x), T23 (Garmin), T26 und T29 (Ollama, Installer unter Windows).
- Stand 15. Oktober 2026: Das Repository ist privat. Release und `latest.json` sind nur mit Anmeldung erreichbar, die Update-Prüfung einer installierten App (`releases/latest/download/latest.json`, SPEC 11.6) bekommt ohne Anmeldung einen Fehler. Updates also bis auf Weiteres von Hand installieren. Ein öffentlicher Ort nur für Release-Dateien wäre eine Entscheidung des Projekts nach 1.0 gewesen; mit der Umstellung auf ein öffentliches Repository (T38) entfällt das.
- Der Release `v0.1.1` ist auf GitHub weiterhin ein Entwurf, obwohl der Tag existiert. Nicht angefasst.
