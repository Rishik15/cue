//! File: tests/common/wav.rs
//! Purpose: Read a WAV file for the speech benches and refuse anything but 16 kHz mono 16-bit PCM, so a wrong recording fails loudly instead of being timed and scored as speech.
//! Contents: wav_samples — checked RIFF chunk walk.

/// 16 kHz mono 16-bit PCM WAV bytes to samples in -1..1. Walks the RIFF chunks by their declared sizes (word aligned), checks the `fmt ` chunk and decodes only the `data` payload.
/// Input: the file bytes. Returns the samples, or why the file is not usable (not RIFF/WAVE, a wrong format, missing or truncated chunks).
pub fn wav_samples(bytes: &[u8]) -> Result<Vec<f32>, String> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a RIFF/WAVE file".into());
    }
    let (mut at, mut format_ok) = (12usize, false);
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        let body = at + 8;
        let end = body.checked_add(size).filter(|e| *e <= bytes.len()).ok_or("a chunk runs past the end of the file")?;
        match id {
            b"fmt " => format_ok = check_format(&bytes[body..end])?,
            b"data" if !format_ok => return Err("data chunk before the fmt chunk".into()),
            b"data" => return Ok(bytes[body..end].as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b) as f32 / 32768.0).collect()),
            _ => {}
        }
        at = end + (size & 1);
    }
    Err("no data chunk".into())
}

/// Checks a `fmt ` chunk body is 16 kHz mono 16-bit integer PCM. Input: the chunk body. Returns Ok(true), or what is wrong.
fn check_format(fmt: &[u8]) -> Result<bool, String> {
    if fmt.len() < 16 {
        return Err("fmt chunk too short".into());
    }
    let tag = u16::from_le_bytes([fmt[0], fmt[1]]);
    let channels = u16::from_le_bytes([fmt[2], fmt[3]]);
    let rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
    let bits = u16::from_le_bytes([fmt[14], fmt[15]]);
    if (tag, channels, rate, bits) != (1, 1, 16_000, 16) {
        return Err(format!("need 16 kHz mono 16-bit PCM, got tag {tag}, {channels} channel(s), {rate} Hz, {bits} bit"));
    }
    Ok(true)
}
