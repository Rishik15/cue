//! File: main.rs
//! Purpose: Process entry point: picks the data cleanup helper, the speech worker, the lab commands, the debug insert test or the app.
//! Contents: main — runs `--remove-data` first, then the speech worker, `--lab` (scripts/lab reads the app through it), the debug insert test or the app. The `windows_subsystem` attribute below keeps release builds from opening a console window; do not remove it.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Starts Cue: the data cleanup, the speech worker or the lab commands when asked, the debug insert test, or the app. Returns nothing; the process exits through the called entry point.
fn main() {
    cue_lib::harden_process();
    let args: Vec<String> = std::env::args().collect();
    if let Some(code) = cue_lib::run_cleanup(&args) {
        std::process::exit(code);
    }
    if args.iter().any(|a| a == "--speech-worker") {
        std::process::exit(cue_lib::run_speech_worker());
    }
    if let Some(at) = args.iter().position(|a| a == "--lab") {
        std::process::exit(cue_lib::run_lab(&args[at + 1..]));
    }
    #[cfg(debug_assertions)]
    if let Some(at) = args.iter().position(|a| a == "--insert-test") {
        let rest = &args[at + 1..];
        std::process::exit(cue_lib::insert_test(rest.first().map_or("", String::as_str), rest.get(1).is_some_and(|a| a == "paste")));
    }
    cue_lib::run()
}
