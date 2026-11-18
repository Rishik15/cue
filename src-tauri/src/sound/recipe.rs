//! File: sound/recipe.rs
//! Purpose: The pure part of the sound module: what each `Sound` is (a recipe of numbers), its samples made once at full level, and the WAV bytes for a volume.
//! Contents: Recipe — pitch, overtones, attack, decay, length and peak; Sound::recipe — the one table of recipes (a new sound is a new row); synthesize — a recipe's samples; wav — samples scaled to a volume as a mono 16-bit WAV file (tested).

use super::Sound;

/// Samples a second of every sound.
const RATE: u32 = 22_050;
/// Samples at the end that fade to zero, so a sound ends without a click.
const FADE_SAMPLES: f32 = 200.0;

/// What a sound is made of: a sine with overtones, a short linear attack and an exponential ring-out.
pub struct Recipe {
    /// The pitch in Hz.
    pub pitch: f32,
    /// Overtones as (multiple of the pitch, weight against the pitch itself).
    pub partials: &'static [(f32, f32)],
    /// Seconds the attack takes to reach full level.
    pub attack_secs: f32,
    /// The ring-out's decay constant in seconds: the level falls to 1/e after this long.
    pub decay_secs: f32,
    /// Seconds the whole sound lasts.
    pub total_secs: f32,
    /// The loudest sample as a share of full scale at volume 100; the system volume still applies.
    pub peak: f32,
}

/// The calm warm single tone ("warm single drop", .scratch/research/calm-completion-sound.md): 740 Hz with two faint overtones, 360 ms. Attack 5 ms (was 12 ms: the shortest that does not click, so the onset is heard sooner); peak 0.28 (-11.1 dBFS at volume 100, -14.2 dBFS at the default 70; was 0.20, -17.1 dBFS at 70) so the quicker onset is clearly audible; never above -10 dBFS.
const WARM_DROP: Recipe =
    Recipe { pitch: 740.0, partials: &[(2.0, 0.12), (3.0, 0.025)], attack_secs: 0.005, decay_secs: 0.105, total_secs: 0.36, peak: 0.28 };

impl Sound {
    /// The recipe of this sound. Timer and SnippetDone share one today and stay separate so either can change alone.
    pub fn recipe(self) -> &'static Recipe {
        match self {
            Sound::Timer | Sound::SnippetDone => &WARM_DROP,
        }
    }
}

/// The samples of a recipe at full level (volume 100): the sine and its overtones under the attack, the ring-out and the end fade.
pub fn synthesize(recipe: &Recipe) -> Vec<i16> {
    let samples = (RATE as f32 * recipe.total_secs) as usize;
    let weight: f32 = 1.0 + recipe.partials.iter().map(|p| p.1).sum::<f32>();
    let amp = recipe.peak * f32::from(i16::MAX);
    (0..samples)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let phase = std::f32::consts::TAU * recipe.pitch * t;
            let bell = phase.sin() + recipe.partials.iter().map(|&(k, w)| w * (k * phase).sin()).sum::<f32>();
            let left = (samples - i) as f32 / FADE_SAMPLES;
            let envelope = (-t / recipe.decay_secs).exp() * (t / recipe.attack_secs).min(1.0) * left.min(1.0);
            (bell / weight * envelope * amp) as i16
        })
        .collect()
}

/// A complete mono 16-bit PCM WAV file of `base` scaled to `volume` (0 to 100, more is 100; 0 is silence of the same length).
pub fn wav(base: &[i16], volume: u8) -> Vec<u8> {
    let gain = f32::from(volume.min(100)) / 100.0;
    let data_len = (base.len() * 2) as u32;
    let mut wav = Vec::with_capacity(44 + base.len() * 2);
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
    base.iter().for_each(|&s| wav.extend_from_slice(&((f32::from(s) * gain) as i16).to_le_bytes()));
    wav
}

#[cfg(test)]
mod tests {
    use super::{synthesize, wav, RATE};
    use crate::sound::Sound;

    /// The samples of a WAV after its 44-byte header.
    fn samples(wav: &[u8]) -> Vec<i16> {
        wav[44..].chunks(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect()
    }

    /// Every sound lasts its total, never passes its peak or -10 dBFS, starts and ends at zero (no click), and is not silent.
    #[test]
    fn every_sound_has_its_length_peak_and_a_quiet_end() {
        for sound in [Sound::Timer, Sound::SnippetDone] {
            let recipe = sound.recipe();
            let base = synthesize(recipe);
            assert_eq!(base.len(), (RATE as f32 * recipe.total_secs) as usize);
            assert!(base.iter().any(|&s| s != 0));
            assert!(recipe.peak <= 0.316, "above -10 dBFS");
            assert!(base.iter().all(|&s| f32::from(s.abs()) <= recipe.peak * f32::from(i16::MAX) + 1.0));
            assert_eq!((base.first(), base.last()), (Some(&0), Some(&0)));
        }
    }

    /// Volume 0 is silence of the same length (the timer's warm-up), volume 100 is the base, 50 is half, and more than 100 is 100.
    #[test]
    fn volume_scales_the_base_and_zero_is_silent() {
        let base = synthesize(Sound::Timer.recipe());
        let silent = wav(&base, 0);
        assert_eq!(silent.len(), 44 + base.len() * 2);
        assert!(samples(&silent).iter().all(|&s| s == 0));
        assert_eq!(samples(&wav(&base, 100)), base);
        assert_eq!(wav(&base, 200), wav(&base, 100));
        let loud = base.iter().map(|s| s.abs()).max().unwrap();
        let half = samples(&wav(&base, 50)).iter().map(|s| s.abs()).max().unwrap();
        assert!((i32::from(half) - i32::from(loud) / 2).abs() <= 1);
    }

    /// The WAV header names a mono 16-bit PCM file whose data length matches the samples.
    #[test]
    fn the_wav_header_matches_its_data() {
        let w = wav(&[0, 1, 2], 100);
        assert_eq!(&w[..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(w[40..44].try_into().unwrap()), 6);
        assert_eq!(u32::from_le_bytes(w[4..8].try_into().unwrap()), 36 + 6);
        assert_eq!(w.len(), 50);
    }
}
