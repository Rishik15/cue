// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|a| a == "--speech-worker") {
        std::process::exit(cue_lib::run_speech_worker());
    }
    cue_lib::run()
}
