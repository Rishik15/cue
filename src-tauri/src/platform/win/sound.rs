//! File: sound.rs
//! Purpose: Plays a finished WAV file through Windows without waiting for it to end (the OS end of the `sound` module; it knows nothing about recipes).
//! Contents: play_wav — `PlaySoundW` from memory, asynchronous, the buffer kept alive until the next sound replaces it.

use std::sync::Mutex;

use windows::core::PCWSTR;
use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

/// The sound being played: `SND_ASYNC` reads it after the call returns, so it must outlive the call.
static PLAYING: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// Plays `wav`, a complete WAV file, and returns at once. Safety: PlaySoundW stops the previous sound before this new buffer replaces the old one, and the lock is held across both so two sounds cannot interleave play and store. `SND_NODEFAULT` keeps Windows from substituting its default sound when the call fails.
pub fn play_wav(wav: Vec<u8>) {
    let mut playing = PLAYING.lock().unwrap_or_else(|e| e.into_inner());
    let _ = unsafe { PlaySoundW(PCWSTR(wav.as_ptr().cast()), None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT) };
    *playing = wav;
}
