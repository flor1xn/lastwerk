# T62: Bausteine und Diagrammstil

| Feld | Wert |
|---|---|
| Status | offen |
| Meilenstein | nach 1.0 |
| cloudfähig | ja |
| Voraussetzung | T60 |
| Bereich | `app/ui/src/lib/components`, `app/ui/src/lib/charts*.ts`, `app.css` |
| SPEC | 5.5 |

## Ziel

Bausteine der Instrumentsprache und ein ruhiger Diagrammstil, bevor die Ansichten umgebaut werden.

## Umfang

1. `Metric`: Zahl in schmaler Breite, Einheit und Marke in normaler; `StatRow` unverändert im Verhalten.
2. Zustandsmarke statt Pille (`.state-mark`): quadratische Marke und Text, Farbe aus den Formtokens.
3. `BandScale.svelte`: Formzustand als Skala mit vier benannten Bereichen und Marke auf dem TSB. Grenzen aus `lib/today.ts` (dieselben wie `core`, SPEC 5.5), Bereichsnamen als Text, aktiver Bereich fett. Die Marke gleitet 200 ms, wenn sich der Wert ändert (`transform`), ohne Bewegung bei reduzierter Bewegung.
4. `DeviationRow.svelte`: Wert gegen eigene Basis als Balken um eine Mitte, Prozent als Text. Skala fest ±25 % (Faustregel, nur Darstellung), größere Werte stoßen an und zeigen den Wert im Text.
5. `WeekStrip.svelte`: sieben Tagessäulen mit Zustand als Form (gemacht voll, geplant gestrichelt, verpasst mit Kreuz, heute mit Limettenrand) und Kurztext für Bildschirmleser.
6. Diagramme (`PmcChart`, `charts.ts`): höchstens vier waagerechte Gitterlinien, Zeitachse an Monatswechseln beschriftet, Bandnamen außerhalb der Zeichenfläche, Fitness neutral statt Limette, Bänder sehr leise. Der Zeiger schreibt den Wert des Tages in die Kopfzeile des Diagramms.
7. Unbekannter Vorlagenname zeigt die ID lesbar statt des rohen Schlüssels (Befund `today-extreme`).

## Abnahme

```
pnpm --dir app/ui lint && pnpm --dir app/ui check && pnpm --dir app/ui test
```

Komponententests für BandScale (Marke im richtigen Bereich an den Grenzen −30, −10, +5), DeviationRow und WeekStrip.

## Blocker oder Vermerke

Keine.
