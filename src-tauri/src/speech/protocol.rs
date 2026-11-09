//! Purpose: Wire format between the app and its speech worker process (pipes): one JSON header plus raw samples per frame.
//! Contents: VERSION — the protocol revision both ends must share; Request / Reply — the messages (Load, then Begin, Audio..., End per utterance; every
//! request after Load is answered by exactly one Release); write_frame / read_frame — `[u32 json_len][json][u32 sample_count][f32 samples]`, little endian.
//! Lengths are checked before allocating, so a corrupt pipe cannot ask for gigabytes.

use std::borrow::Cow;
use std::io::{self, Read, Write};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// A reply can carry the whole text of an hour-long recording (about 60 KB), so this is generous.
const MAX_JSON: u32 = 1024 * 1024;
/// 10 minutes of 16 kHz audio in one frame. The app sends 200 ms pieces, and replays at most the audio not yet recognised.
const MAX_SAMPLES: u32 = 16_000 * 600;

/// The protocol revision. Both ends are the same executable, so this only guards against a stale worker process: `Load` carries it and a worker that
/// disagrees refuses the model.
pub const VERSION: u32 = 2;

/// How a model is fed: pieces cut at pauses by a voice detector (Segmented) or the whole recording as one stream that keeps its own state (Streaming).
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy)]
pub enum Kind {
    Segmented,
    Streaming,
}

/// Which sherpa-onnx config struct a model needs. A model of an existing struct is a catalog row; a new struct needs one more arm in the worker's
/// `recognizer` (offline) or `config` (streaming) and one variant here.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy)]
pub enum Adapter {
    /// `OfflineTransducerModelConfig`: Parakeet TDT.
    OfflineTransducer,
    /// `OfflineCanaryModelConfig`: Canary flash.
    OfflineCanary,
    /// `OfflineWhisperModelConfig`: Whisper.
    OfflineWhisper,
    /// `OnlineTransducerModelConfig`: Nemotron, a cache-aware transducer for long dictation.
    OnlineTransducer,
}

/// Everything the worker needs to wire a model folder's files together: read from the catalog row, never matched on a model name.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct EngineSpec {
    pub kind: Kind,
    pub adapter: Adapter,
    /// Ends of the file names holding each role (Whisper's carry a size prefix); the joiner exists only in transducers.
    pub encoder: Cow<'static, str>,
    pub decoder: Cow<'static, str>,
    pub joiner: Option<Cow<'static, str>>,
    /// The `model_type` sherpa-onnx wants for the family, when it wants one.
    pub model_type: Option<Cow<'static, str>>,
    /// The spoken language: a code fixes it, an empty string or "auto" lets the model detect it, None means the model has no such option.
    pub language: Option<Cow<'static, str>>,
}

impl EngineSpec {
    /// A row's spec with the usual int8 file names. Inputs: kind, adapter, whether there is a joiner, the model type and the language option.
    pub const fn int8(
        kind: Kind,
        adapter: Adapter,
        joiner: bool,
        model_type: Option<&'static str>,
        language: Option<&'static str>,
    ) -> Self {
        EngineSpec {
            kind,
            adapter,
            encoder: Cow::Borrowed("encoder.int8.onnx"),
            decoder: Cow::Borrowed("decoder.int8.onnx"),
            joiner: if joiner { Some(Cow::Borrowed("joiner.int8.onnx")) } else { None },
            model_type: match model_type {
                Some(t) => Some(Cow::Borrowed(t)),
                None => None,
            },
            language: match language {
                Some(l) => Some(Cow::Borrowed(l)),
                None => None,
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "type")]
pub enum Request {
    Load {
        dir: String,
        vad: String,
        spec: EngineSpec,
        threads: i32,
        version: u32,
    },
    /// A new utterance starts; speech detection restarts from silence. `clause` is the number the first clause it recognises gets: a replay
    /// after a crash starts at the audio not yet released, so it continues the numbering of the clauses released before.
    Begin {
        id: u64,
        clause: usize,
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
        version: u32,
    },
    Text {
        id: u64,
        text: String,
    },
    /// A clause was recognised while the user is still talking; `index` is its number in the utterance. Text only: it says nothing about audio.
    Partial {
        id: u64,
        index: usize,
        text: String,
    },
    /// Answers every request after Load, after its Partials and Text: the audio before sample `up_to` (counted from the start of what this worker
    /// was sent) will never be needed again, so the app may stop keeping it. Also proof that the worker is alive and making progress.
    Release {
        id: u64,
        up_to: usize,
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
