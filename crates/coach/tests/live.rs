//! Live-Test gegen ein lokales Ollama (SPEC 11.4, "LLM (live)"). Läuft nur mit
//! `cargo test -p lastwerk-coach -- --ignored`, wenn Ollama auf `http://127.0.0.1:11434` läuft
//! und das Modell in `LASTWERK_OLLAMA_MODEL` gezogen ist. Der Eingang ist ein fester,
//! synthetischer Plan. Der Test prüft nicht den Inhalt der Antwort, sondern den Vertrag: Es
//! kommt entweder ein geprüfter Plan ohne harte Verletzung oder der unveränderte Regelplan.

mod common;

use std::time::Instant;

use common::{base_plan, context, rules};
use lastwerk_coach::{Provider, RefineConfig, RefineOutcome, refine_plan};
use lastwerk_core::plan::{Hardness, validate_plan};

#[test]
#[ignore = "braucht ein laufendes Ollama und LASTWERK_OLLAMA_MODEL"]
fn live_ollama_never_returns_a_plan_that_breaks_a_hard_rule() {
    let model = std::env::var("LASTWERK_OLLAMA_MODEL")
        .expect("LASTWERK_OLLAMA_MODEL nennt das gezogene Modell");
    let provider = Provider::ollama(None, &model, false).unwrap();
    let client = provider.client();
    let ctx = context();
    let base = base_plan(&ctx);
    let cfg = RefineConfig {
        max_corrections: 2,
        timeout_s: 600,
    };
    let started = Instant::now();
    let outcome = refine_plan(&base, &ctx, &rules(), client.as_ref(), &cfg);
    println!("Antwortzeit gesamt: {:?}", started.elapsed());
    match outcome {
        RefineOutcome::Applied {
            plan,
            explanation,
            attempts,
        } => {
            println!("Applied nach {attempts} Aufrufen: {explanation}");
            assert!(
                !validate_plan(&plan, &ctx, &rules())
                    .iter()
                    .any(|v| v.hardness == Hardness::Hard)
            );
        }
        RefineOutcome::Fallback {
            plan,
            reason,
            attempts,
        } => {
            println!("Fallback nach {attempts} Aufrufen: {}", reason.code());
            assert_eq!(plan, base);
        }
    }
}
