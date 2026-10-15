# T19: Erststart-Assistent, Designsprache überall, Version in der App

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | nach M3 (Nutzerwunsch) |
| Voraussetzung | T17, T18 |
| cloudfähig | ja, Live manuell |
| Bereich | `app/ui`, `app/src-tauri`, `catalog`, `.github`, `docs` |
| SPEC | 5.4, 5.5 (Gestaltung der Oberfläche), 10.1, 10.2 |

## Ziel

Beim ersten Start führt ein Assistent durch die Einrichtung. Alle Ansichten folgen der Designsprache der Startseite "Heute". Die App zeigt ihre Version und die Änderungen.

## Warum

Ohne Führung muss der Nutzer Ordner, Schwellenwerte, Ziel und Verfügbarkeit selbst in den richtigen Ansichten finden. T18 hat die übrigen Ansichten bewusst nicht umgestaltet. Die Release-Notizen stehen fest in `release.yml`.

## Umfang

- Erststart-Assistent (Sprache und Erscheinungsbild, FIT-Ordner mit Import, Schwellenwerte und Athlet, Ziel und Verfügbarkeit, optional Garmin). Abschluss als Einstellung `onboarded`, wiederholbar in den Einstellungen.
- Garmin-Schritt: nur Erklärung, Startbefehl zum Kopieren und Status, ob `exchange/` Daten liefert (nur Dateien lesen, SPEC 5.4). Kein Anmeldeformular, keine Zugangsdaten in `app/`.
- Designsprache auf Woche, Plan, Aktivitätsliste, Aktivitätsdetail und Einstellungen.
- Version und "Neu in dieser Version" in den Einstellungen aus `catalog/changelog.json`. Der Release-Workflow nutzt dieselbe Quelle.

## Nicht im Umfang

Garmin-Anmeldung in der App, Änderungen an `bridge/`, Windows-Codesigning, Hintergrund-Updates, neue Planregeln.

## Vorgaben

- Neue Befehle an allen Stellen eintragen (`app/README.md`, "Neue Befehle oder Ansichten").
- Texte nur über `catalog/locales/de.json` und `en.json`.
- Skills `emil-design-eng`, `animate`, `review-animations`, `break-ui`. Keine neue Abhängigkeit ohne Vermerk, keine Netzwerkzugriffe aus dem Frontend.
- Alles mit Attrappen und synthetischen Daten testen. Es gibt keine echten Garmin-Zugriffe.

## Abnahme

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
(in app/src-tauri) cargo fmt --check, cargo clippy --all-targets -- -D warnings, cargo test
pnpm --dir app/ui lint
pnpm --dir app/ui check
pnpm --dir app/ui test
pnpm --dir app/ui exec playwright test
sh scripts/check-privacy.sh
sh scripts/check-isolation.sh
sh scripts/check-i18n.sh
```

## Manuelle Schritte

- Live-Test der Garmin-Anbindung über `bridge/` (Task L1).
- Release-Tag `v0.1.2` setzen und den Entwurf veröffentlichen.
- Optik im laufenden Programm prüfen.

## Blocker oder Vermerke

Die SPEC enthält zwei Abschnitte mit der Nummer 5.5 (Archiv des Zusatzprogramms, Gestaltung der Oberfläche). Verweise nennen den Titel dazu.

### Ergebnis

- Assistent mit fünf Schritten (`views/onboarding/Wizard.svelte`), Gate in `App.svelte` über die Einstellung `onboarded`, wiederholbar über "Einrichtung wiederholen".
- Neuer Befehl `get_bridge_status` und Einstellungen `onboarded` und `version` (SPEC 5.4, Block "Umsetzung in T19").
- Neue Abhängigkeiten: keine.
- Änderungsliste `catalog/changelog.json`, Release-Notizen über `scripts/release-notes.mjs` (neuer Job `notes` in `release.yml`, ungeprüft bis zum nächsten Tag).
- Abweichung: Die Designsprache wurde auf die übrigen Ansichten über Ein-Satz-Aussage, Bewegung und Abstände angewendet. Die Tabellen bleiben dicht. Der Stresstest mit `break-ui` war eine Durchsicht, keine Demo-Daten-Umschaltung.
- Der Standard des Exchange-Ordners gilt nur beim Start aus dem Repository (`LASTWERK_EXCHANGE` sonst).
