//! lastwerk-mcp: MCP-Server, nur lesend, gefilterte Ausgabe
//!
//! Die Schnittstellen stehen in docs/SPEC.md, Abschnitt 5.4, der Rahmen für Claude als
//! MCP-Client in Abschnitt 9.6.
//!
//! - [`views`]: Ausgabetypen mit fester Feldliste. Datenbankzeilen werden nie serialisiert.
//! - [`args`]: typisierte Argumente mit Bereichsgrenzen.
//! - [`tools`]: die neun Werkzeuge als reine Funktionen über einer lesenden Verbindung.
//! - [`server`]: Anbindung an `rmcp` (nur stdio), Hinweise `readOnlyHint` und Co., Protokoll.
//! - [`calllog`]: lokales Aufrufprotokoll ohne Nutzdaten.
//!
//! Die Datenbank wird schreibgeschützt geöffnet (`Store::open_read_only`). Der Server liest
//! nie `exchange/`, das Archiv oder das Garmin-Konto.

pub mod args;
pub mod calllog;
pub mod error;
pub mod rules;
pub mod server;
pub mod startup;
pub mod tools;
pub mod views;

pub use server::LastwerkMcp;
pub use tools::TOOL_NAMES;
