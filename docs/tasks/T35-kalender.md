# T35: Trainingskalender je Monat

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | nach 1.0 (Auswertung) |
| cloudfähig | ja |
| Voraussetzung | T25, T26 |
| Bereich | `app/src-tauri` (Befehl `get_month`), `app/ui` (Ansicht "Kalender"), `catalog/locales` |
| SPEC | 4.2 (F42), 5.5, 7.1 |

## Ziel

Eine Ansicht "Kalender" zeigt einen Monat als Raster (Montag zuerst): je Tag die geplanten Einheiten des aktiven Plans und die tatsächlichen Aktivitäten, mit Zustand erledigt, verpasst, zusätzlich oder Ruhetag, und je Woche Soll gegen Ist (Last und Dauer). Klick auf eine Aktivität öffnet das Detail.

## Warum

Der Kalender mit Plan gegen Ist ist die Hauptansicht von TrainingPeaks und Final Surge. Lastwerk zeigt Plan und Aktivitäten bisher getrennt.

## Umfang

1. Befehl `get_month(month, today)` (`YYYY-MM`): Tage des Rasters (ganze Wochen), je Tag Aktivitäten (ID, Sportart, Dauer, Strecke, Last) und geplante Einheiten (Vorlage, Dauer, hart, Gründe), je Woche Summen und `planned_load`.
2. Zustand je Tag in der Oberfläche aus den Daten (vergangener Tag mit Plan ohne Aktivität: verpasst; Tag ohne Plan mit Aktivität: zusätzlich). Heute und Zukunft gelten nie als verpasst. Reine Zählung, keine Schwelle.
3. Ansicht "Kalender" in der Navigation: Monatswechsel, Raster mit Wochen-Summe am Rand, unter 760 px eine Liste der Tage.

## Nicht im Umfang

Einheiten verschieben oder bearbeiten (Planänderung bleibt beim Coach), Neuplanung nach verpassten Einheiten (F34).

## Abnahme

```
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo fmt --manifest-path app/src-tauri/Cargo.toml --check && cargo clippy --manifest-path app/src-tauri/Cargo.toml --all-targets --locked -- -D warnings && cargo test --manifest-path app/src-tauri/Cargo.toml --locked
pnpm --dir app/ui lint && pnpm --dir app/ui check && pnpm --dir app/ui test
sh scripts/check-privacy.sh && sh scripts/check-isolation.sh && sh scripts/check-i18n.sh
sh scripts/check-all.sh
```

Dazu: Attrappe mit synthetischen Werten für jede neue Ansicht (auch leer und extrem), Vitest für Deutung und Formatierung, Playwright-Rauchtest Deutsch und Englisch, Sichtprüfung hell und dunkel, schmal und breit (`break-ui`).

## Manuelle Schritte

Sichtprüfung im echten Tauri-Fenster mit eigener Datenbank durch den Nutzer.

## Vermerke

### Geliefert (15. Oktober 2026)

- **Abweichung: kein neuer Befehl `get_month`.** Alles Nötige liefern vorhandene Befehle: `list_activities` für das Raster (ganze Wochen um den Monat) und `list_plans` mit `get_plan` für den aktiven Plan, wie die Startseite. Ein eigener Befehl hätte dieselben Daten nur umgepackt.
- `lib/calendar.ts`: Raster (`monthRange`, `shiftMonth`), Zustand je Tag (`dayState`: erledigt, teilweise, verpasst, zusätzlich, trainiert ohne Plan, geplant, Ruhetag, leer; heute und Zukunft nie verpasst), Wochensummen Ist gegen Soll (Last nur aus berechenbaren Werten), Zählung für die Aussage. Duplikate zählen nicht (SPEC 6.4), Planeinträge mit Sportart `rest` sind keine Einheit. Tests mit Handrechnung.
- Ansicht "Kalender" in der Navigation nach "Heute": Monat blättern, Raster mit geplanten Einheiten (gestrichelt, harte mit Akzentrand, Gründe im `title`) und Aktivitäten (Rand in Sportfarbe, Dauer, Last, Klick öffnet das Detail), Zustand als Text (verpasst, teilweise, zusätzlich) und verpasste Tage schraffiert, Wochenspalte Last und Stunden Ist / Soll. Unter 760 px Seitenbreite eine Liste der Tage mit Einträgen.
- Attrappe: `?plan=laufend` legt einen aktiven Plan an, der vor drei Wochen begann.
