# T41: Einrichtung neu, in drei Schritten zum fertigen Plan

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | nach 1.0 |
| cloudfähig | ja, **Livetest manuell** (erster Abgleich mit echtem Konto) |
| Voraussetzung | T39, T40 |
| Bereich | `app/ui` (Assistent, Heute, Planer), `app/src-tauri` (`get_planning_inputs` mit Historie), `catalog/locales`, `docs` |
| SPEC | 5.4 (`lastwerk-app`), 7.2, 9.2, 9.7 |

## Ziel

Vom ersten Start in wenigen Klicks zu einem fertig eingerichteten Werkzeug: Garmin verbunden, Daten unterwegs, Freigaben gesetzt, Plan aktiv.

## Warum

Auftrag des Nutzers vom 16. Oktober 2026: "die ganze Logik rund um mein Tool neu denken, wie der Einrichtungsprozess aussieht und man dann effizient und einfach zu seinem fertig eingerichteten Tool kommt". Bisher endete der Erststart nach Sprache und Garmin auf einer leeren Startseite; den Plan musste man selbst finden, mit Standardantworten statt der eigenen Historie, und der Coach lag versteckt in den Einstellungen.

## Entscheidung des Nutzers (16. Oktober 2026)

- **E6** Geführte Einrichtung bis zum Plan. Die Sprache kommt vom System. Ein Schritt Garmin; während der Abgleich läuft, Freigaben und Coach; danach die drei Fragen des Planers, vorbelegt aus der Garmin-Historie. Keine Fragen nach Schwellen. Löst die Festlegung "Erststart nur Sprache und Garmin" aus T39 ab.
- **E7** (aus T40) S3-Freigaben je Bereich auch in der Einrichtung, Standard aus.

## Umfang

1. Assistent (`app/ui/src/views/onboarding/Wizard.svelte`) mit den Schritten Garmin, Freigaben und Coach, Plan.
2. Vorschlag aus der Historie: `PlanningInputs.history` aus `service::plans::history_defaults` (Faustregeln dort), nur in `get_planning_inputs`.
3. Startseite: Starttag eines Plans, der erst später beginnt; leere Abschnitte Form und Bereitschaft während des Abgleichs.
4. Texte: veraltete Hinweise (`ui.today.no_plan_hint`, `ui.about.repeat_hint`, `ui.dash.form.empty`) angepasst.

## Nicht im Umfang

- Die Freigabe des Gesprächs mit dem Coach bleibt je Sitzung (SPEC 9.7, Punkt 4). Sie dauerhaft zu speichern wäre eine Änderung der Spezifikation und ist nicht Teil des Auftrags.
- Vorschläge für Schwellen, die Garmin nicht liefert (CSS), bleiben in den Einstellungen.

## Abnahme

```
cargo fmt --manifest-path app/src-tauri/Cargo.toml --check && cargo clippy --manifest-path app/src-tauri/Cargo.toml --all-targets --locked -- -D warnings && cargo test --manifest-path app/src-tauri/Cargo.toml --locked
pnpm --dir app/ui lint && pnpm --dir app/ui check && pnpm --dir app/ui test
pnpm --dir app/ui exec playwright test
sh scripts/check-i18n.sh && sh scripts/check-all.sh
```

Erfolgskriterien:

- Vitest: drei Schritte ohne Sprach- oder Schwellenschritt, Sprache vom System nur beim ersten Start, kein Passwortfeld, S3 je Bereich standardmäßig aus, Plan aus der Historie erzeugt und aktiv, Abschluss ohne Garmin und ohne Plan möglich.
- Rust: Handrechnungen zu `history_defaults` (Median, Tage, Untergrenze 20 Minuten je Tag, mindestens vier Wochen).
- Playwright: Einrichtung in Deutsch und Englisch bis zur Startseite mit Plan.

## Manuelle Schritte

- Erster Start mit echtem Konto: Dauer des ersten Abgleichs, ob Last und Form vor den Tageswerten erscheinen, ob der Vorschlag aus der Historie zum eigenen Training passt.

## Neue Abhängigkeiten

Keine.

## Blocker oder Vermerke

- 16. Oktober 2026: Der Plan aus der Einrichtung beginnt wie im Planer am nächsten Montag (T26). Heute nennt bis dahin den Starttag.
