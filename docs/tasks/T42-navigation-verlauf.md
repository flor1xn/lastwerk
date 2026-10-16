# T42: Navigation radikal aufräumen, Verlauf und Einstellungen in Reitern

| Feld | Wert |
|---|---|
| Status | offen |
| Meilenstein | nach 1.0 |
| cloudfähig | ja, Sichtprüfung im Tauri-Fenster manuell |
| Voraussetzung | T41 |
| Bereich | `app/ui`, `catalog/locales`, `docs/SPEC.md`, `app/README.md` |
| SPEC | 5.5 (Gestaltung der Oberfläche) |

## Ziel

Die Seitenleiste zeigt nur noch Heute, Plan und Verlauf, unten Einstellungen. Alles andere ist Tiefe innerhalb dieser Seiten. Keine doppelten Bedienelemente.

## Warum

Auftrag des Nutzers vom 16. Oktober 2026: "Alles in meinem Tool soll aufgeräumter und sortierter sein." Bisher elf flache Navigationspunkte, darunter "Detail" ohne eigene Bedeutung, Überschneidungen (Heute, Woche, Kalender; Erholung, Gesundheit), Sprache und Thema an zwei Stellen, Einstellungen als eine Spalte mit zehn Abschnitten.

## Entscheidung des Nutzers (16. Oktober 2026)

- **E8** Navigation radikal minimal: Heute, Plan, Verlauf. Der Coach kommt als Seitenfenster (T43).

## Umfang

1. `app/ui/src/lib/state.ts`: `VIEWS = ['today','plan','history']`, dazu `settings`; `detail` entfällt. Stores `historyTab` (`calendar`, `activities`, `progress`, `performance`, `body`), `bodyTab` (`recovery`, `health`), `settingsTab` (`data`, `coach`, `profile`, `app`), `selectedActivityId`. Helfer `openHistory(tab)`, `openSettings(tab)`, `openActivity(id)` (Verlauf, Reiter Aktivitäten, Detail an Ort und Stelle). Alle `view.set(...)` auf alte Ansichten umstellen; `coachSetupFocus` durch `settingsTab = 'coach'` ersetzen.
2. Neue Seite `views/HistoryView.svelte` mit Überschrift "Verlauf", Leitsatz des aktiven Reiters und den Reitern Kalender (Umschalter Monat/Woche, Woche ist die bisherige `WeekView` ohne doppelten PMC), Aktivitäten (Liste oder Detail mit "Zurück"), Fortschritt, Leistung, Körper (Umschalter Erholung/Gesundheit). Nur der aktive Reiter wird gerendert. Eingebettete Ansichten ohne eigenen Seitenkopf.
3. Neue Komponente `lib/components/Tabs.svelte` mit `role="tablist"`, Pfeiltasten, wanderndem Tabindex, Testkennungen `tab-<gruppe>-<wert>`. `Segmented` bleibt für Unterumschalter.
4. Einstellungen in vier Reitern: Daten und Garmin (Daten, Garmin, MCP), Coach (Einrichtung), Profil und Schwellen (Athlet, Schwellen, PMC), App (Darstellung, Update, Über).
5. Seitenleiste in `App.svelte`: Sprachwahl, Themenknopf und `CoachSetupLine` entfernen (bleiben in Einstellungen), `GarminSyncLine` bleibt.
6. Texte in `de.json` und `en.json`, veraltete Schlüssel entfernen.
7. Tests: Vitest (`state`, `Tabs`, angepasste Abschnittstests), Playwright (`smoke`, `garmin`, `coach-setup`) über `nav-history` und Reiter.
8. SPEC 5.5 und `app/README.md` nachziehen.

## Nicht im Umfang

- Coach-Seitenfenster (T43), neue Inhalte auf Heute (T44).
- Neue Tauri-Befehle, Änderungen in `crates/`.

## Vorgaben

SPEC 5.5: Aussage zuerst, eine primäre Aktion je Karte, Bewegung nur CSS 120 bis 200 ms mit `prefers-reduced-motion`.

## Abnahme

```
pnpm --dir app/ui lint && pnpm --dir app/ui check && pnpm --dir app/ui test
sh scripts/check-privacy.sh && sh scripts/check-isolation.sh && sh scripts/check-i18n.sh
sh scripts/check-all.sh
```

Erfolgskriterien: Seitenleiste hat genau drei Hauptpunkte plus Einstellungen; jede frühere Ansicht ist über Verlauf oder Einstellungen erreichbar; Smoke-Test in de und en grün.

## Manuelle Schritte

Sichtprüfung im echten Tauri-Fenster unter Windows.

## Neue Abhängigkeiten

Keine.

## Blocker oder Vermerke

Keine.
