# T13: Zusatzprogramm, USB-Kopie und Abgleich (bridge, optional)

| Feld | Wert |
|---|---|
| Status | erledigt |
| Meilenstein | M4c |
| Voraussetzung | T11 |
| cloudfähig | ja |
| Bereich | `bridge/` |
| SPEC | 4.2 (F28), 12 (M4c) |

## Ziel

Rückfall für den Fall, dass der Garmin-Zugang gesperrt ist: FIT-Dateien aus den Geräteordnern (Activity, Monitor, Sleep) unverändert in das Archiv übernehmen und mit den API-Aktivitäten abgleichen.

## Umfang

- `import-device <ordner> --root <archiv>`: übernimmt Dateien unverändert, vermerkt sie im Manifest, Prüfbericht mit Anzahl erkannter, unbekannter und übersprungener Dateien.
- `reconcile --root <archiv>`: Liste der Aktivitäten, die nur in der API oder nur auf dem Gerät vorkommen, ohne Werte.
- Ohne vorhandenes Gerät oder Ordner: klare Meldung, Exit-Code 2.
- Synthetische Gerätefixtures unter `bridge/tests/fixtures/device/` (aus dem Test heraus erzeugt, keine `*.fit` im Index: Dateien zur Laufzeit in ein temporäres Verzeichnis schreiben).

## Nicht im Umfang

Der Garmin-Datenexport (ZIP aus dem Konto) ist ausdrücklich keine Quelle (SPEC 2.3).

## Abnahme

```
cd bridge
uv run pytest -q tests/test_usb_import.py tests/test_reconcile.py
uv run pytest -q
cd ..
sh scripts/check-privacy.sh
```

Erfolg (SPEC 12, M4c): `import-device` übernimmt Dateien unverändert und liefert den Prüfbericht, `reconcile` meldet Lücken in beide Richtungen, fehlender Ordner endet mit Exit-Code 2, `git ls-files "*.fit"` bleibt leer.

## Manuelle Schritte

Optionaler Test mit dem echten Gerät (Datenträgerpfad je Gerät prüfen, offen O4 für Workout-Pfade).

## Blocker oder Vermerke

Keine Blocker. Erledigt am 13. Oktober 2026 in einer Cloud-Session (Linux), vollständig offline mit synthetischen Geräteordnern und gesperrtem Socket.

### Abhängigkeiten

Keine neuen. Nur Standardbibliothek.

### Festlegungen und Abweichungen (in SPEC 12 M4c nachgetragen)

- **Fixtures zur Laufzeit.** `bridge/tests/fixtures/device/` entfällt, weil `*.fit` nicht im Index stehen dürfen. `tests/device_fixture.py` erzeugt die Geräteordner unter `tmp_path`: FIT-Kopf (14 Byte, `.FIT`, Kopf-CRC), wenige erfundene Datenbytes, Datei-CRC. Der Abnahmebefehl in SPEC 12 M4c nennt statt der Einzelbefehle gegen den Fixture-Ordner nur noch die beiden Testdateien; beide führen die Befehle auch als Unterprozess gegen ein temporäres Verzeichnis aus.
- **Erkennung ohne FIT-Parser.** Ordnername (`Activity`, `Monitor`, `Sleep`, Groß- und Kleinschreibung egal, direkt oder unter `GARMIN/`), Endung `.fit` (Groß- und Kleinschreibung egal), Kopf (Byte 0 gleich 12 oder 14, `.FIT` an Byte 8 bis 11). Alles andere in diesen Ordnern, `.fit` in fremden Ordnern und Symlinks sind `unbekannt` und werden nicht kopiert. Die Pfade `GARMIN/NewFiles` und `GARMIN/Workouts` (SPEC 14 O4) betrifft die Kopie nicht: sie zählen als fremde Ordner.
- **Kopie.** `device_usb/<geraet>/<Ordner>/<dateiname>`, atomar, nie überschrieben. Gleiche SHA-256 unter demselben Schlüssel ergibt `übersprungen`; gleicher Name mit anderem Inhalt eine neue Datei mit Zeitstempel im Namen (`abweichend`). Liegt die Datei schon im Archiv und fehlt nur der Eintrag (Abbruch dazwischen), wird nur vermerkt. Das Archiv darf nicht im Geräteordner liegen (Exit-Code 2). `lib_version` im Manifest ist `lastwerk-bridge <Version>`, weil keine Garmin-Bibliothek beteiligt ist.
- **Berichte.** `device-import-report.json` und `reconcile-report.json` sind abgeleitet, stehen in `DERIVED_FILES` und haben keinen Manifest-Eintrag. Sie enthalten nur Zählungen, Kennungen und Dateinamen. Die Schlüssel von `reconcile-report.json` heißen wie im Auftrag `nur_api`, `nur_geraet`, `beide` (Zahl), dazu `nicht_zuordenbar`; die übrigen Schlüssel sind englisch.
- **Zuordnung.** Dateiname mit Zeitstempel `JJJJ-MM-TT-hh-mm-ss` (so benennt Garmin Aktivitätsdateien auf dem Gerät) oder mit Kennung (mindestens sechs Ziffern, optional `_ACTIVITY`). Zeitstempel gilt als UTC, `--name-offset-minutes` rechnet Ortszeit um; Vergleich mit `startTimeGMT`, Toleranz 120 s (Konvention, `--tolerance`); je Aktivität und Datei höchstens eine Zuordnung, kleinste Abweichung zuerst; Muster über `--name-pattern`. Ob echte Geräte so benennen und in welcher Zeitzone, ist bis zum Test am Gerät offen (O4). Dateien mit unpassendem Namen sind `nicht_zuordenbar` und nicht `nur_geraet`.
- **Startzeit der API.** Aus `summary.json` (`startTimeGMT`, auch unter `summaryDTO`), sonst aus der Listenseite. `ActivityRef` hat dafür das Feld `start_utc`. Der Ort des Feldes in `get_activity` ist bis zur Sonde in L1 eine Annahme.
- **`coverage`.** Kleiner Eingriff: Aktivitäten ohne `original.fit` mit zugeordneter Gerätedatei erhalten `via_device` im JSON und den Zusatz "über Gerät vorhanden" in der Ausgabe; der Block `device` im JSON erscheint nur, wenn das Archiv eine USB-Kopie hat. `coverage` nutzt die Standardwerte der Zuordnung.
- **Exit-Codes.** `import-device`: 2 bei fehlendem Ordner, fehlenden Unterordnern, fehlendem `--root`, unbrauchbarem Gerätenamen, Archiv im Geräteordner. `reconcile`: 2 ohne Archivordner, ohne Aktivitätsdateien der USB-Kopie, bei ungültigem Muster. Lücken sind kein Fehler (Exit-Code 0).
- **Offen für den Nutzer (manuell):** Test mit dem echten Gerät (Datenträgerpfad, tatsächliche Dateinamen und Zeitzone der Namen, Pfade für Workouts, O4). Stimmt das Namensmuster nicht, genügt `--name-pattern` und `--name-offset-minutes`.
