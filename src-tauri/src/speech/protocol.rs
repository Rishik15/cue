//! Purpose: Wire format between the app and its speech worker process (pipes): one JSON header plus raw samples per frame.
//! Contents: Request / Reply — the messages (Load, then Begin, Audio..., End per utterance); write_frame / read_frame — `[u32 json_len][json][u32 sample_count][f32 samples]`, little endian.
//! Lengths are checked before allocating, so a corrupt pipe cannot ask for gigabytes.

use std::io::{self, Read, Write};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

const MAX_JSON: u32 = 64 * 1024;
/// 10 minutes of 16 kHz audio; the recorder caps far below this.
const MAX_SAMPLES: u32 = 16_000 * 600;

/// Which sherpa-onnx model family a folder holds; decides how the worker wires its files together.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy)]
pub enum Family {
    /// NVIDIA Parakeet TDT: encoder, decoder, joiner.
    Transducer,
    /// NVIDIA Canary flash: encoder, decoder.
    Canary,
    /// OpenAI Whisper: encoder, decoder.
    Whisper,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "type")]
pub enum Request {
    Load {
        dir: String,
        vad: String,
        family: Family,
        threads: i32,
    },
    /// A new utterance starts; speech detection restarts from silence.
    Begin {
        id: u64,
    },
    /// More audio for the current utterance, in the frame body. Finished speech segments are transcribed at once.
    Audio,
    /// The utterance is over: transcribe what is left and answer with its text.
    End {
        id: u64,
    },
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "type")]
pub enum Reply {
    Loaded {
        ms: u64,
    },
    Text {
        id: u64,
        text: String,
    },
    /// A sentence was recognised while the user is still talking; `index` counts sentences of this utterance from 0.
    Partial {
        id: u64,
        index: usize,
        text: String,
    },
    Error {
        message: String,
    },
}

fn invalid(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what.to_string())
}

pub fn write_frame<T: Serialize>(out: &mut impl Write, msg: &T, samples: &[f32]) -> io::Result<()> {
    let json = serde_json::to_vec(msg)?;
    let mut buf = Vec::with_capacity(8 + json.len() + samples.len() * 4);
    buf.extend_from_slice(&(json.len() as u32).to_le_bytes());
    buf.extend_from_slice(&json);
    buf.extend_from_slice(&(samples.len() as u32).to_le_bytes());
    buf.extend(samples.iter().flat_map(|s| s.to_le_bytes()));
    out.write_all(&buf)?; // one write: a frame is never interleaved
    out.flush()
}

fn read_u32(input: &mut impl Read) -> io::Result<u32> {
    let mut b = [0u8; 4];
    input.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

/// `Ok(None)` is a clean end of stream (the other side closed its pipe between frames).
pub fn read_frame<T: DeserializeOwned>(input: &mut impl Read) -> io::Result<Option<(T, Vec<f32>)>> {
    let json_len = match read_u32(input) {
        Ok(n) => n,
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    };
    if json_len > MAX_JSON {
        return Err(invalid("header too large"));
    }
    let mut json = vec![0u8; json_len as usize];
    input.read_exact(&mut json)?;
    let count = read_u32(input)?;
    if count > MAX_SAMPLES {
        return Err(invalid("audio too long"));
    }
    let mut raw = vec![0u8; count as usize * 4];
    input.read_exact(&mut raw)?;
    let samples = raw.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect();
    Ok(Some((serde_json::from_slice(&json)?, samples)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_header_and_samples() {
        let mut pipe = Vec::new();
        write_frame(&mut pipe, &Request::Audio, &[0.5, -1.0]).unwrap();
        write_frame(&mut pipe, &Reply::Loaded { ms: 3 }, &[]).unwrap();
        let mut r = pipe.as_slice();
        assert_eq!(read_frame::<Request>(&mut r).unwrap().unwrap(), (Request::Audio, vec![0.5, -1.0]));
        assert_eq!(read_frame::<Reply>(&mut r).unwrap().unwrap(), (Reply::Loaded { ms: 3 }, vec![]));
        assert!(read_frame::<Reply>(&mut r).unwrap().is_none());
    }

    #[test]
    fn rejects_oversized_lengths() {
        let mut huge = (MAX_JSON + 1).to_le_bytes().to_vec();
        assert!(read_frame::<Reply>(&mut huge.as_slice()).is_err());
        huge = 2u32.to_le_bytes().to_vec();
        huge.extend_from_slice(b"{}");
        huge.extend_from_slice(&(MAX_SAMPLES + 1).to_le_bytes());
        assert!(read_frame::<Reply>(&mut huge.as_slice()).is_err());
    }
}
