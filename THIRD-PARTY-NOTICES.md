# Fremde Bestandteile

Lastwerk selbst steht unter MIT OR Apache-2.0 ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)). Diese Datei nennt Bestandteile, die eine eigene Lizenz mitbringen und deren Hinweis weitergegeben werden muss. Die Zuordnung je Datei steht in [REUSE.toml](REUSE.toml) (Prüfung: `reuse lint`).

## Schrift Archivo (im Installer enthalten)

Die Oberfläche bündelt die Schrift Archivo über das npm-Paket `@fontsource-variable/archivo` 5.3.0, ohne Nachladen aus dem Netz (seit T60, vorher Cairo).

> Copyright 2020 The Archivo Project Authors (https://github.com/Omnibus-Type/Archivo)
>
> This Font Software is licensed under the SIL Open Font License, Version 1.1. This license is available with a FAQ at: http://scripts.sil.org/OFL

Der vollständige Lizenztext liegt im Paket (`node_modules/@fontsource-variable/archivo/LICENSE` nach `pnpm --dir app/ui install`). Die Schrift wird unverändert eingebunden und nicht einzeln verkauft.

## Skills für die Oberfläche (nur im Repository, nicht im Installer)

`.claude/skills/` enthält unveränderte Skills von Emil Kowalski (https://github.com/emilkowalski/skills, MIT). Lizenztext: [.claude/skills/LICENSE-emilkowalski-skills](.claude/skills/LICENSE-emilkowalski-skills), Herkunft und Commit: [.claude/skills/README.md](.claude/skills/README.md).

## Abhängigkeiten

- **Rust.** `cargo deny check bans licenses sources` (CI, Konfiguration in [deny.toml](deny.toml)) lässt nur Lizenzen zu, die zu MIT OR Apache-2.0 passen. Ausnahmen je Paket mit Begründung: MPL-2.0 für fünf Pakete der Tauri-Kette (ungeänderte Verlinkung, Pflichten nur für geänderte MPL-Dateien) und CDLA-Permissive-2.0 für Wurzelzertifikate (`webpki-root-certs`).
- **Oberfläche (npm, Laufzeit).** `@fontsource-variable/archivo` (OFL-1.1), `@tauri-apps/api` (Apache-2.0 OR MIT), `uplot` (MIT). Entwicklungswerkzeuge (Vite, Svelte, Vitest, Playwright und weitere) gelangen nicht in den Installer.
- **Zusatzprogramm `bridge/`** (eingefroren): Python-Abhängigkeiten laut `bridge/pyproject.toml` und `bridge/uv.lock`.

Die vollständige Liste mit Lizenztexten erzeugen `cargo deny list` bzw. `pnpm --dir app/ui licenses list --prod`.

## Marken

Garmin, Garmin Connect und Strava sind Marken ihrer Inhaber. Lastwerk ist weder mit ihnen verbunden noch von ihnen unterstützt oder autorisiert (siehe README).
