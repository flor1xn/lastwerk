//! Startpunkt der Desktop-App. Die Logik steht in der Bibliothek (`lib.rs`), damit sie ohne
//! Fenster testbar ist.

// Unter Windows kein zusätzliches Konsolenfenster im fertigen Programm.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    lastwerk_app::run();
}
