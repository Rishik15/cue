//! File: sound/mod.rs
//! Purpose: The one way Cue makes a sound: a named `Sound` played at a volume, from anywhere (Notes timers, snippets, later features). Nothing else may call the operating system's player.
//! Contents: Sound — the sounds Cue has (a new one is a variant and a recipe row in `recipe`); play — start one without waiting, at 0 to 100 %. The samples of each sound are made once and only scaled per call; the OS call is `platform::play_wav`. No audio stream stays open, so idle costs nothing.

mod recipe;

use std::sync::OnceLock;

/// The sounds Cue has. Each is a recipe (see `recipe`): pitch, overtones, attack, decay, length and peak.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    /// A timer or pomodoro phase ended.
    Timer,
    /// A snippet's text was sent.
    SnippetDone,
}

/// The full-level samples of each sound, made on first use.
static BASES: [OnceLock<Vec<i16>>; 2] = [OnceLock::new(), OnceLock::new()];

/// Starts `sound` and returns at once; it never waits for the sound to end. Input: the sound and the volume, 0 to 100 (more is 100). Volume 0 plays silence and still wakes the audio device, which is how a timer warms it up before its alert; the system volume applies on top.
pub fn play(sound: Sound, volume: u8) {
    let base = BASES[sound as usize].get_or_init(|| recipe::synthesize(sound.recipe()));
    crate::platform::play_wav(recipe::wav(base, volume));
}
