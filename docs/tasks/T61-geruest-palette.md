# T61: Gerüst, schmales Layout und Befehlspalette

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | nach 1.0 |
| cloudfähig | ja |
| Voraussetzung | T60 |
| Bereich | `app/ui` (`App.svelte`, `lib/state.ts`, `lib/components`), `catalog/locales`, `docs/SPEC.md` |
| SPEC | 5.5, 9.7 |

## Ziel

Eine schmale Leiste (56 px) mit Symbolen statt der Seitenleiste mit 208 px, unter 760 px eine Leiste unten. Strg+K öffnet eine Befehlspalette für alles (E14), Strg+J den Coach.

## Warum

Bei 420 px blieben dem Inhalt 150 px. Die Seitenleiste war zu 60 % leer. Der Coach war über Strg+K erreichbar, alles andere nur mit der Maus.

## Umfang

1. Leiste: Heute, Plan, Verlauf, unten Coach und Einstellungen; eigene Symbole als SVG im Code (keine Symbolbibliothek), Name und Kürzel als Tooltip und als `aria-label`. Aktive Ansicht mit Limettenmarke und `aria-current`. Statuspunkt des Garmin-Abgleichs an der Leiste mit Text im Tooltip.
2. Unter 760 px: Leiste unten, Inhalt mit 16 px Rand, kein waagerechtes Scrollen ab 360 px.
3. Befehlspalette (`lib/components/CommandPalette.svelte`, Befehle in `lib/commands.ts`): springen zu Ansichten und Reitern, Aktivitäten nach Datum und Sportart, Plan erstellen, Einstellungen je Reiter, Thema wechseln führt in den Reiter App. Filter rein in der Oberfläche, ohne Netz. Text ohne Treffer: Eintrag "Coach fragen" öffnet den Coach mit Prefill, sendet nichts. Ohne Bewegung beim Öffnen und Schließen. Muster "Combobox mit Listbox" (WAI-ARIA), Pfeiltasten, Eingabe, Escape, Fokus zurück an den Auslöser.
4. Kürzel: Strg+K Palette, Strg+J Coach, G dann H, P, V, E springt (nicht in Eingabefeldern). Kürzel stehen in der Palette rechts neben dem Befehl.
5. Texte de und en, Rauchtest erweitert.

## Nicht im Umfang

Neue Befehle im Rust-Teil.

## Abnahme

```
pnpm --dir app/ui lint && pnpm --dir app/ui check && pnpm --dir app/ui test
sh scripts/check-i18n.sh
```

Playwright: Palette per Tastatur bedienbar in de und en, Ansicht bei 360 px ohne waagerechtes Scrollen.

## Manuelle Schritte

Kürzel im Tauri-Fenster prüfen (Strg+J darf nicht mit einem Kürzel von WebView2 kollidieren).

## Blocker oder Vermerke

Keine.
