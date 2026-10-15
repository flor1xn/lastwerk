# Kommandozeile

Dieselbe Logik steht ohne Oberfläche als `lastwerk` bereit (SPEC 5.4):

```
lastwerk import --dir <ordner>                 FIT-Dateien einlesen
lastwerk import-recovery --dir <ordner>        Erholungstage aus dem Zusatzprogramm (exchange/out/recovery)
lastwerk plausibility --dir <ordner>           Kennzahlen eines Ordners prüfen, ohne zu speichern
lastwerk rebuild --dir <ordner>                Cache neu aufbauen
lastwerk thresholds set|list|suggest           Schwellenwerte
lastwerk athlete set                           Geschlecht, Geburtsjahr
lastwerk pmc --from <datum> --to <datum>       Fitness, Ermüdung, Form
lastwerk goal set|list                         Ziele
lastwerk availability set|list                 Verfügbarkeit je Wochentag
lastwerk plan --week-start <montag> --out <plan.json>
lastwerk plans                                 gespeicherte Pläne auflisten
lastwerk plan-status --id <plan_id> --status draft|active|archived
                                               Planstatus setzen (active archiviert den bisherigen)
lastwerk validate <plan.json>                  Schema und Leitplanken prüfen
lastwerk refine <plan.json> --provider ollama|openai-compatible --model <name>
                                               Plan von einem lokalen Modell anpassen lassen
lastwerk export-ics <plan.json> --out <plan.ics>
lastwerk backup --out <datei.json>             Nutzerdaten sichern
lastwerk restore --in <datei.json>             Nutzerdaten einspielen
lastwerk garmin status|sync|logout             Garmin-Abgleich (Anmeldung nur in der App)
lastwerk chat --message <text> [--show-input]  eine Nachricht an den lokalen Coach
```

`lastwerk --help` und `lastwerk <befehl> --help` zeigen die Optionen. `refine` zeigt vor dem Senden genau den Text, der an das Modell geht, und fragt nach (`--yes` für Skripte). Erlaubt sind nur Server auf dem eigenen Rechner (Standard `http://127.0.0.1:11434`), im lokalen Netz nur mit `--allow-lan`. Das Modell wählt aus erlaubten Vorlagen, gerechnet und geprüft wird in Lastwerk; besteht die Antwort die Prüfung nach zwei Korrekturen nicht, bleibt der Regelplan. Ohne `--db` gilt `<lokaler Datenordner>/Lastwerk/lastwerk.db`, dieselbe Datei wie in der Oberfläche (Windows `%LOCALAPPDATA%`, macOS `~/Library/Application Support`, Linux `$XDG_DATA_HOME` oder `~/.local/share`). Die Umgebungsvariable `LASTWERK_DB` (voller Dateipfad) ersetzt diesen Pfad in der Kommandozeile wie in der App, `--db` geht beiden vor.

Bauen: `cargo build --release -p lastwerk-cli`, das Programm liegt danach unter `target/release/lastwerk`. Ohne Bauen: `cargo run -p lastwerk-cli -- <befehl>`. Die MCP-Anbindung für Claude steht in [mcp.md](mcp.md).
