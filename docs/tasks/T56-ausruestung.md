# T56: Ausrüstung mit Kilometerstand

| Feld | Wert |
|---|---|
| Status | offen |
| Meilenstein | nach 1.0 (Marktabgleich 18. Oktober 2026) |
| cloudfähig | ja |
| Voraussetzung | T42 |
| Bereich | `crates/store` (Migration, Backup), `crates/cli`, `app/src-tauri`, `app/ui`, `catalog/locales`, `schema` |
| SPEC | 4.3 (F53), 6.2, 7 (Backup) |

## Ziel

Der Nutzer legt Schuhe und Räder an, ordnet Aktivitäten zu und sieht je Teil Strecke, Zeit und Zahl der Aktivitäten. Je Sportart gilt ein Standardteil für neue Aktivitäten. Optional eine selbst gesetzte Grenze in Kilometern mit Hinweis beim Erreichen.

## Warum

Vorbilder: Strava, Garmin Connect, intervals.icu und Runalyze führen Ausrüstung mit Kilometerstand. Für den Nutzer mit Edge und Uhr praktisch, in Lastwerk fehlt es ganz.

## Umfang

1. Speicher: Tabellen für Teile (Name, Art, Sportart, Startdatum, optional Ausmusterung, optionale Grenze, Anfangsstand) und Zuordnung je Aktivität. Nutzerdaten, im Backup und im Schema, Neuaufbau aus FIT behält sie (SPEC 6.1).
2. Standard je Sportart, angewandt beim Import neuer Aktivitäten; nachträgliches Umhängen einzelner Aktivitäten.
3. Kommandozeile `lastwerk gear add|list|retire|assign`.
4. App: Reiter "Ausrüstung" in den Einstellungen, Auswahl im Aktivitätsdetail, Hinweis bei erreichter Grenze in der Wochen-Story.

## Nicht im Umfang

- Ausrüstung aus Garmin übernehmen: kein belegter Endpunkt in der Allowlist (5.6). Wird er später belegt, eigener Task.
- Eine Standardgrenze: Für die Lebensdauer von Schuhen gibt es in Anhang A keine Quelle, deshalb setzt nur der Nutzer eine Grenze.

## Vorgaben

- Test: zwei Teile, Standardwechsel zu einem Datum, Summen je Teil.
- Test: Backup, leere Datenbank, Wiederherstellung, gleiche Summen.

## Erkundung (optional, nur lokal, Regeln in [../marktabgleich.md](../marktabgleich.md))

Keine REA-Analyse nötig.

## Abnahme

```
sh scripts/check-all.sh
```

- Tests grün, `break-ui` für die Liste (lange Namen, 50 Teile, keins).

## Manuelle Schritte

Eigene Teile im Tauri-Fenster anlegen.

## Vermerke
