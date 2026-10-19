# T66: Prüfung der Designsprache und Abgleich mit REA

| Feld | Wert |
|---|---|
| Status | offen |
| Meilenstein | nach 1.0 |
| cloudfähig | Teil A ja, Teil B nur lokal |
| Voraussetzung | T60 bis T65 |
| Bereich | `app/ui/e2e`, `docs/designsprache.md`, `docs/SPEC.md` |
| SPEC | 5.5, 10.6, 11.4 |

## Ziel

Nachweis, dass die neue Oberfläche hält, was SPEC 5.5 verspricht, und Gegenprobe der Bewegungs- und Abstandswerte an installierten Programmen.

## Umfang

### Teil A (Prüfung)

1. `sh scripts/check-all.sh`.
2. Je Ansicht Attrappe im Normalfall, leer und extrem, 360, 420 und 1440 px, hell und dunkel; break-ui mit langen Namen und großen Zahlen.
3. Rauchtest Deutsch und Englisch, Bewegung mit `review-animations` geprüft, Bildrate der Marke auf der Bandskala und des Coach-Fensters gemessen (`animation-performance`).
4. Kontrast aller Textpaare beider Themen gerechnet; Bündelgröße vorher und nachher im Vermerk.

### Teil B (REA, nur lokal, nur nach Einrichtung durch den Nutzer)

Regeln aus [../marktabgleich.md](../marktabgleich.md). Der Nutzer richtet REA selbst ein (`npx rea-agents setup`), nichts davon kommt ins Repository. Vor jedem Ziel: Lizenz oder Nutzungsbedingungen prüfen, entfällt das Ziel, wenn sie Reverse Engineering verbieten.

Fragen: Dauer und Kurve beim Öffnen von Palette und Seitenfenster, Abstufung der Abstände und Zeilenhöhen in dichten Listen, Zustände von Knöpfen (Hover, Drücken, Fokus). Befund mit Datum, Ziel, Version, Frage, Antwort und Grenzen in `docs/designsprache.md`. Kein Code, keine Texte, Grafiken, Symbole oder Schriften werden übernommen, keine Screenshots fremder Oberflächen ins Repository.

## Abnahme

Vermerk in `docs/designsprache.md` mit Ergebnissen; Abweichungen in SPEC 5.5 nachgetragen.

## Manuelle Schritte

Sichtprüfung im echten Tauri-Fenster (Schrift, Kürzel, Coach-Fenster, schmales Fenster). Teil B durch den Nutzer angestoßen.

## Blocker oder Vermerke

- 19. Oktober 2026: REA noch nicht eingerichtet. Teil B wartet auf den Nutzer.
