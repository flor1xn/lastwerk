# MCP-Server: Claude Desktop und Claude Code

`lastwerk-mcp` ist ein schreibgeschützter MCP-Server über stdio. Er beantwortet Fragen zu Form, Wochenlast, einzelnen Aktivitäten, Plan, Schwellenwerten und Planregeln, damit sich Training im Gespräch mit Claude besprechen lässt. Das ist die einzige Stelle, an der ein Cloud-Modell Daten aus Lastwerk sieht (SPEC 9.6).

**Folge für die Daten.** Jede Antwort eines Werkzeugs wird Teil des Gesprächs und geht an den Anbieter des Clients. Sie ist danach nicht zurückholbar. Wer das nicht will, verbindet den Server nicht. Der Server liefert nur Kennzahlen: keine Koordinaten, Streckenpunkte, Aktivitätsnamen, Notizen, Gerätekennungen, Dateipfade und keine Zielbezeichnung. Uhrzeiten gibt er nicht aus, nur Daten. Erholungswerte (HRV, Ruhepuls, Schlaf) sind gesperrt, bis die Einstellung `mcp.recovery_enabled` auf `true` steht. Die Freigabe ist ein Schalter in den Einstellungen der App (Abschnitt "Claude als Client", Standard aus, das Einschalten fragt nach). Sie gilt für alle Clients, die den Server starten, und lässt sich dort jederzeit wieder sperren. Schon ausgegebene Werte holt das nicht zurück.

Werkzeuge (alle nur lesend): `get_fitness_status`, `get_weekly_load`, `list_activities`, `get_activity`, `get_current_plan`, `get_plan_week`, `get_thresholds`, `explain_rule`, `get_recovery_days`. Die Liste mit den Hinweisen gibt `lastwerk-mcp --db <datei> --list-tools` aus. `get_current_plan` und `get_plan_week` brauchen einen aktiven Plan: `lastwerk plans` zeigt die gespeicherten Pläne, `lastwerk plan-status --id <plan_id> --status active` setzt einen aktiv.

Bauen und prüfen:

```
cargo build --release -p lastwerk-mcp
target/release/lastwerk-mcp --db <pfad>/lastwerk.db --list-tools
```

`--db` ist Pflicht (kein Standardpfad, damit ein Client nie unbemerkt eine andere Datenbank liest). Die Datenbank ist die der Kommandozeile und der App (unter Windows `%LOCALAPPDATA%\Lastwerk\lastwerk.db`, unter macOS `~/Library/Application Support/Lastwerk/lastwerk.db`, sonst `~/.local/share/Lastwerk/lastwerk.db`), in Konfigurationsdateien mit vollem Pfad. Der Server öffnet sie nur zum Lesen und legt nichts an, die Datenbank muss also schon existieren (`lastwerk import`). Jeder Aufruf steht mit Werkzeug, Parametern und Zeilenzahl, aber ohne Werte der Antwort, in `mcp-calls.log` neben der Datenbank (anderer Ort: `--log <datei>`).

**Claude Desktop.** In `claude_desktop_config.json` (Einstellungen, Entwickler, Konfiguration bearbeiten) unter `mcpServers` eintragen, danach Claude Desktop neu starten:

```json
{
  "mcpServers": {
    "lastwerk": {
      "command": "C:\\Pfad\\zu\\lastwerk-mcp.exe",
      "args": ["--db", "C:\\Users\\Name\\AppData\\Local\\Lastwerk\\lastwerk.db"]
    }
  }
}
```

**Claude Code.**

```
claude mcp add lastwerk -- /pfad/zu/lastwerk-mcp --db /pfad/zu/lastwerk.db
```

Rauchtest: verbinden und fragen "Wie ist meine Form diese Woche?". Die Antwort muss die Werte aus `get_fitness_status` nennen (CTL, ATL, TSB). Den Server des Zusatzprogramms in `bridge/` (Garmin-Rohdaten) nie mit einem Cloud-Client verbinden.
