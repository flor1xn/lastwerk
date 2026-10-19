# T65: Einstellungen, Coach-Fenster und Einrichtung

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | nach 1.0 |
| cloudfähig | ja |
| Voraussetzung | T61, T62 |
| Bereich | `app/ui/src/views/settings`, `views/onboarding`, `lib/components/coach`, `catalog/locales` |
| SPEC | 5.5, 9.7 |

## Ziel

Die übrigen Flächen in der neuen Sprache und schmal bedienbar.

## Umfang

1. Einstellungen: Abschnitte mit Haarlinien, Formulare in einer Spalte bis 640 px Breite, Knöpfe nach T60.
2. Coach-Fenster: ab 1100 px rückt der Inhalt zur Seite (wie bisher), darunter liegt es über dem Inhalt, unter 760 px als Blatt über die volle Breite. Öffnen per Klick 200 ms, per Tastatur ohne Bewegung (wie bisher). Vorschlagskarte nach T60.
3. Einrichtung: Schritte schmal tauglich, Primäraktion mit Kürzel.

## Abnahme

```
pnpm --dir app/ui lint && pnpm --dir app/ui check && pnpm --dir app/ui test
```

## Blocker oder Vermerke

Keine.
