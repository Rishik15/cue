// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
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
