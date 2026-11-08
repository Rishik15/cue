//! File: beep.rs
//! Purpose: The alert sound for a timer that finished, at the volume chosen in the notes settings (Windows has no volume for its own beep, so a short chime is made in memory).
//! Contents: beep — play the chime at 0 to 100 %; tone — the WAV bytes (tested).

use std::sync::Mutex;

use windows::core::PCWSTR;
use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

const RATE: u32 = 22_050;
/// Two soft notes, a rising major third (C5, E5): nearly pure sines with a short, gentle attack and a slow ring-out.
const NOTES: [f32; 2] = [523.3, 659.3];
const NOTE_SECS: f32 = 0.45;
/// Quiet enough that 100 % is not harsh; the system volume still applies.
const PEAK: f32 = 0.6;
/// The tone being played: `SND_ASYNC` reads it after the call returns, so it must outlive the call.
static PLAYING: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// Volume 0 plays silence: it still wakes the audio device, which is how a timer warms it up before its alert. Safety: `playing` is a complete WAV file; PlaySoundW stops the previous sound before this new buffer replaces the old one, and the lock is held across both so two beeps cannot interleave play and store.
pub fn beep(volume: u8) {
    let wav = tone(volume);
    let mut playing = PLAYING.lock().unwrap_or_else(|e| e.into_inner());
    let _ = unsafe { PlaySoundW(PCWSTR(wav.as_ptr().cast()), None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT) };
    *playing = wav;
}

/// The chime as a mono 16-bit WAV file: each note a sine with a few overtones, a 2 ms attack and an exponential ring-out. The last few samples fade, so a note ends without a click. 2 bytes a frame, 16 bits. PCM, one channel.
fn tone(volume: u8) -> Vec<u8> {
    let per_note = (RATE as f32 * NOTE_SECS) as usize;
    let samples = per_note * NOTES.len();
    let amp = f32::from(volume.min(100)) / 100.0 * PEAK * f32::from(i16::MAX);
    let data_len = (samples * 2) as u32;
    let mut wav = Vec::with_capacity(44 + samples * 2);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&[1, 0, 1, 0]);
    wav.extend_from_slice(&RATE.to_le_bytes());
    wav.extend_from_slice(&(RATE * 2).to_le_bytes());
    wav.extend_from_slice(&[2, 0, 16, 0]);
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..samples {
        let (pitch, t) = (NOTES[i / per_note], (i % per_note) as f32 / RATE as f32);
        let phase = std::f32::consts::TAU * pitch * t;
        let bell = phase.sin() + 0.08 * (2.0 * phase).sin();
        let left = (per_note - i % per_note) as f32 / 200.0;
        let envelope = (-t * 5.0).exp() * (t / 0.012).min(1.0) * left.min(1.0);
        wav.extend_from_slice(&((bell / 1.08 * envelope * amp) as i16).to_le_bytes());
    }
    wav
}
