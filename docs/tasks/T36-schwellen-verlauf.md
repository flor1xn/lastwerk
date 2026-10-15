# T36: Verlauf der Schwellenwerte

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | nach 1.0 (Auswertung) |
| cloudfähig | ja |
| Voraussetzung | T33 |
| Bereich | `app/src-tauri` (Teil von `get_performance`), `app/ui` (Ansicht "Leistung"), `catalog/locales` |
| SPEC | 4.1 (F4), 4.2 (F43), 8.6 |

## Ziel

Auf der Seite "Leistung" zeigt ein Abschnitt, wie sich FTP, LTHR und Schwellentempo über die Zeit entwickelt haben: gesetzte Werte als Stufenlinie mit Gültigkeitsdatum, dazu der FTP-Vorschlag aus der besten 20-Minuten-Leistung je Kalendermonat (8.6) als Punkte. Ein Vorschlag über dem gesetzten Wert verweist in die Einstellungen.

## Warum

Andere Werkzeuge zeigen den Verlauf der Schwellen (WKO, TrainingPeaks, intervals.icu eFTP). In Lastwerk stehen die Werte nur als Liste in den Einstellungen.

## Umfang

1. `get_performance` liefert zusätzlich `thresholds`: je Art die gesetzten Werte (Wert, gültig ab, Herkunft) und je Monat den FTP-Vorschlag aus `suggest_ftp` über die beste 20-Minuten-Leistung des Monats.
2. Abschnitt "Schwellen" mit Stufenlinie (uPlot) je Art, Tabelle der Änderungen, Hinweis bei Vorschlag über dem aktuellen Wert.

## Nicht im Umfang

Übernahme eines Vorschlags aus dieser Ansicht (bleibt in den Einstellungen), neue Schätzverfahren.

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

- `get_performance` liefert `thresholds` (FTP, LTHR, Laufschwelle, CSS bis heute, unabhängig vom Zeitraum, nach Art und Datum) und `ftp_suggestions` (je Kalendermonat des Zeitraums die beste 20-Minuten-Leistung der Radeinheiten, Vorschlag über `suggest_ftp`, SPEC 8.6). Integrationstest mit den Fixtures.
- Seite "Leistung", Abschnitt "Schwellen": Aussage aus dem Zustand (`thresholdView`: kein Wert, nur andere Arten, kein FTP mit Vorschlag, aktueller FTP, Vorschlag darüber), Knopf in die Einstellungen, wenn dort etwas zu tun ist, FTP als Stufenlinie mit den Monatsvorschlägen als Punkte (uPlot), Tabelle aller gesetzten Werte mit Herkunft.
- Abweichung: Die Stufenlinie zeigt nur FTP. LTHR, Laufschwelle und CSS ändern sich selten und stehen in der Tabelle; ein eigenes Diagramm je Art brächte wenig.
- Übernehmen bleibt in den Einstellungen; die Seite schreibt nichts.
