//! File: main.rs
//! Purpose: Process entry point: picks the speech worker, the debug insert test or the app.
//! Contents: main. The `windows_subsystem` attribute below keeps release builds from opening a console window; do not remove it.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Starts Cue: the speech worker when asked, the debug insert test, or the app. Returns nothing; the process exits through the called entry point.
fn main() {
    cue_lib::harden_process();
    if std::env::args().any(|a| a == "--speech-worker") {
        std::process::exit(cue_lib::run_speech_worker());
    }
    #[cfg(debug_assertions)]
    if let Some(at) = std::env::args().position(|a| a == "--insert-test") {
        let args: Vec<String> = std::env::args().skip(at + 1).collect();
        std::process::exit(cue_lib::insert_test(args.first().map_or("", String::as_str), args.get(1).is_some_and(|a| a == "paste")));
    }
    cue_lib::run()
}
