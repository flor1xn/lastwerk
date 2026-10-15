//! lastwerk-mcp: MCP-Server über stdio, nur lesend, gefilterte Ausgabe (SPEC 5.4, 9.6).
//!
//! Aufruf: `lastwerk-mcp --db <datei> [--log <datei>] [--list-tools]`. Die Standardausgabe
//! gehört dem Protokoll, Meldungen stehen auf der Fehlerausgabe. Exit-Codes: 0 Erfolg,
//! 2 Eingabefehler (auch fehlendes `--db`), 3 interner Fehler.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use lastwerk_mcp::startup::{list_tools_json, open_server};
use rmcp::ServiceExt;
use rmcp::transport::stdio;

#[derive(Debug, Parser)]
#[command(
    name = "lastwerk-mcp",
    version,
    about = "Read-only MCP server for Lastwerk"
)]
struct Cli {
    /// Lastwerk database (SQLite file). Opened read-only.
    #[arg(long, value_name = "FILE")]
    db: PathBuf,
    /// Call log (default: mcp-calls.log next to the database).
    #[arg(long, value_name = "FILE")]
    log: Option<PathBuf>,
    /// Print the tools with their hints as JSON and exit without starting the server.
    #[arg(long)]
    list_tools: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    if cli.list_tools {
        println!(
            "{}",
            serde_json::to_string_pretty(&list_tools_json()).unwrap_or_default()
        );
        return ExitCode::SUCCESS;
    }
    let server = match open_server(&cli.db, cli.log.as_deref()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("lastwerk-mcp: {}", e.message());
            return ExitCode::from(e.exit_code());
        }
    };
    let running = match server.serve(stdio()).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("lastwerk-mcp: the server could not start: {e}");
            return ExitCode::from(3);
        }
    };
    match running.waiting().await {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("lastwerk-mcp: the connection ended with an error: {e}");
            ExitCode::from(3)
        }
    }
}
