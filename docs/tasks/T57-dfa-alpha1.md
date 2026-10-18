# T57: Hinweis auf die aerobe Schwelle aus HRV (DFA alpha1)

| Feld | Wert |
|---|---|
| Status | offen |
| Meilenstein | nach 1.0 (Marktabgleich 18. Oktober 2026) |
| cloudfähig | ja |
| Voraussetzung | T31, T55 (gleicher Ordner `crates/fit`) |
| Bereich | `crates/fit`, `crates/testkit`, `crates/core`, `app/src-tauri`, `app/ui`, `catalog/locales`, `docs/SPEC.md` (8.6, Anhang A) |
| SPEC | 4.3 (F54), 8.6, 2.3, Anhang A (Q38) |

## Ziel

Enthält eine Aktivität RR-Intervalle, zeigt das Detail DFA alpha1 im Verlauf neben Herzfrequenz und Leistung oder Tempo. Wo alpha1 nach Rogers 2021 (Q38) die Marke für die aerobe Schwelle kreuzt, nennt Lastwerk Herzfrequenz und Leistung oder Tempo als Hinweis. Der Schwellenwert wird nicht automatisch gesetzt.

## Warum

Vorbilder: Runalyze schätzt die aerobe Schwelle aus HRV, intervals.icu rechnet DFA alpha1 aus RR-Daten. Lastwerk hat Schwellen für FTP und LTHR, aber keinen Hinweis auf die untere Schwelle, die für lockere Einheiten zählt.

## Umfang

1. `crates/fit`: Nachricht `hrv` (RR-Intervalle) lesen, Nummer und Skalierung nach öffentlicher Profilbeschreibung (O2). Geräte schreiben sie nur mit eingeschalteter Aufzeichnung; ohne Daten bleibt der Abschnitt weg.
2. Quelle: Fensterlänge, Schrittweite, Artefaktkorrektur und Grenzwert aus dem Volltext von Q38 (frei über PubMed Central, PMC7845545). Jede Zahl mit Seitenangabe im Kommentar.
3. `core`: DFA (Profil, Boxen, lineare Trendbereinigung, Fluktuationsfunktion, Steigung im Bereich kurzer Boxen nach Q38). Fenster mit zu vielen Artefakten liefern keinen Wert.
4. App: alpha1 als Linie, Grenzwert als Hilfslinie, Hinweistext "Hinweis, keine Messung im Labor".

## Nicht im Umfang

Automatisches Setzen von Schwellen oder Zonen, Aussagen über Gesundheit (SPEC 2.3), HRV in Ruhe (bleibt Erholung, T34).

## Vorgaben

- Referenz (SPEC 11.1): Teilschritte von Hand an einer Reihe von 16 Werten (Profil, Trend je Box, F(n) für zwei Boxgrößen, Steigung im Log-Log). Zusätzlich statistische Probe: weißes Rauschen aus festem Startwert ergibt alpha1 nahe 0,5 mit Toleranz, im Test ohne Zufall zur Laufzeit (Reihe als Konstante).
- Artefakte über der Grenze aus Q38 ergeben `None`.

## Erkundung (optional, nur lokal, Regeln in [../marktabgleich.md](../marktabgleich.md))

Keine REA-Analyse. Das Verfahren ist veröffentlicht.

## Abnahme

```
sh scripts/check-all.sh
```

- Referenz und Artefaktfälle grün, Datei ohne `hrv` unverändert.
- Attrappe mit Stufentest und ohne RR-Daten, Sichtprüfung.

## Manuelle Schritte

Stufentest mit Brustgurt und eingeschalteter HRV-Aufzeichnung durch den Nutzer, Ergebnis gegen das eigene Gefühl und die Zonen im Vermerk.

## Vermerke
