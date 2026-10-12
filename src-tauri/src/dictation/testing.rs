//! Purpose: Debug-only end-to-end check of dictation without a microphone or a keyboard: `cue --dictate-test <wav> <hold|live>` feeds a 16 kHz
//! recording through the real speech path in real time, exactly as the recorder would, and finishes it like a stop. `live` is a toggled
//! recording (streaming model, text typed into the focused window as it comes); `hold` is a held key (offline model, text inserted at the end).
//! Used by `scripts/dictate-check.ps1`, which reads what landed in Notepad. Exits the process when done.
//! Contents: from_args — look for the flag after startup; run — the fake recording.

use std::sync::atomic::Ordering;
use std::time::Duration;

use tauri::AppHandle;

use super::{live, overlay, speech, uses_streaming, Mode, CAPTION_AFTER, LAST_MS, RECORDING, SPEECH_ID, TARGET};
use crate::platform;

pub fn from_args(app: &AppHandle) {
    let args: Vec<String> = std::env::args().collect();
    let Some(at) = args.iter().position(|a| a == "--dictate-test") else { return };
    let (Some(path), Some(kind)) = (args.get(at + 1), args.get(at + 2)) else { return };
    let Some(wave) = sherpa_onnx::Wave::read(path) else {
        eprintln!("cue: cannot read {path}");
        return app.exit(1);
    };
    let (app, samples, live) = (app.clone(), wave.samples().to_vec(), kind == "live");
    let _ = std::thread::Builder::new().name("dictate-test".into()).spawn(move || run(&app, samples, live));
}

fn run(app: &AppHandle, samples: Vec<f32>, live: bool) {
    std::thread::sleep(Duration::from_secs(2)); // the app finishes starting, and the target window keeps focus
    RECORDING.store(true, Ordering::Release);
    TARGET.store(platform::foreground_window(), Ordering::Release);
    let id = speech::begin(app, uses_streaming(if live { Mode::Toggle } else { Mode::Hold }));
    SPEECH_ID.store(id, Ordering::Release);
    match live {
        true => live::begin(app),
        false => overlay::caption_after(Some(CAPTION_AFTER)),
    }
    for chunk in samples.chunks(3_200) {
        speech::feed(id, chunk.to_vec());
        std::thread::sleep(Duration::from_millis(200));
    }
    speech::feed(id, vec![0.0; 8_000]); // the half second of room a real stop has
    RECORDING.store(false, Ordering::Release);
    LAST_MS.store(samples.len() as u64 * 1000 / 16_000, Ordering::Release);
    super::transcribe_and_insert(app, id, true);
    std::thread::sleep(Duration::from_secs(15)); // typing and the last insertion
    app.exit(0);
}
