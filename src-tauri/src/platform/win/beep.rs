//! Purpose: The alert sound for a timer that finished, at the volume chosen in the notes settings (Windows has no volume for its own beep, so a short chime is made in memory).
//! Contents: beep — play the chime at 0 to 100 %; tone — the WAV bytes (tested).

use std::sync::Mutex;

use windows::core::PCWSTR;
use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

const RATE: u32 = 22_050;
/// Two bell notes, a rising fourth: each starts at full strength (no fade-in, so it is heard the moment it plays) and rings out.
const NOTES: [f32; 2] = [987.8, 1318.5];
const NOTE_SECS: f32 = 0.28;
/// Quiet enough that 100 % is not harsh; the system volume still applies.
const PEAK: f32 = 0.6;
/// The tone being played: `SND_ASYNC` reads it after the call returns, so it must outlive the call.
static PLAYING: Mutex<Vec<u8>> = Mutex::new(Vec::new());

pub fn beep(volume: u8) {
    if volume == 0 {
        return;
    }
    let wav = tone(volume);
    // SAFETY: `wav` is a complete WAV file; it is kept in `PLAYING` until the next tone replaces it (the new call stops the old sound first).
    let _ = unsafe { PlaySoundW(PCWSTR(wav.as_ptr().cast()), None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT) };
    *PLAYING.lock().unwrap_or_else(|e| e.into_inner()) = wav;
}

/// The chime as a mono 16-bit WAV file: each note a sine with a few overtones, a 2 ms attack and an exponential ring-out.
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
    wav.extend_from_slice(&[1, 0, 1, 0]); // PCM, one channel
    wav.extend_from_slice(&RATE.to_le_bytes());
    wav.extend_from_slice(&(RATE * 2).to_le_bytes());
    wav.extend_from_slice(&[2, 0, 16, 0]); // 2 bytes a frame, 16 bits
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..samples {
        let (pitch, t) = (NOTES[i / per_note], (i % per_note) as f32 / RATE as f32);
        let phase = std::f32::consts::TAU * pitch * t;
        let bell = phase.sin() + 0.35 * (2.0 * phase).sin() + 0.12 * (3.0 * phase).sin();
        let left = (per_note - i % per_note) as f32 / 200.0; // the last few samples fade, so a note ends without a click
        let envelope = (-t * 9.0).exp() * (t / 0.002).min(1.0) * left.min(1.0);
        wav.extend_from_slice(&((bell / 1.47 * envelope * amp) as i16).to_le_bytes());
    }
    wav
}

#[cfg(test)]
mod tests {
    use super::tone;

    #[test]
    fn a_louder_setting_makes_a_louder_tone() {
        let peak = |v| tone(v)[44..].as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b).unsigned_abs()).max().unwrap_or(0);
        assert!(peak(100) > peak(30) && peak(30) > 0);
        assert_eq!(&tone(50)[..4], b"RIFF");
    }
}
