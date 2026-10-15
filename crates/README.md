# crates

Rust-Workspace, entsteht in M0 (Toolchain noch nicht installiert, siehe SPEC 1.1).

| Crate | Aufgabe |
|---|---|
| core | Reine Logik: Belastung, Zonen, Fitness/Ermüdung/Form, Planer, Leitplanken. Kein Ein- und Ausgabe |
| fit | FIT-Dateien einlesen |
| store | SQLite-Cache und Nutzerdaten, Migrationen |
| coach | Anbindung lokaler Sprachmodelle (Ollama, OpenAI-kompatible lokale Server) |
| cli | Kommandozeile `lastwerk` |
| mcp | MCP-Server, nur lesend, gefilterte Ausgabe (M7) |
| testkit | Nur Entwicklung: FIT-Encoder und Generatoren für synthetische Daten |

Schnittstellen: [SPEC 5.4](../docs/SPEC.md).
