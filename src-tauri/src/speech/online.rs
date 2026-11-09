//! File: speech/online.rs
//! Purpose: The streaming speech engine for long dictation. A cache-aware streaming model (Nemotron) keeps its encoder caches and decoder
//! state between chunks, so every audio frame is processed once and a recording can run for an hour in constant memory, with no cuts and
//! no seams to stitch. Settled words come out as clauses (see `phrase`); a pause of 1.2 s ends the utterance and starts a fresh one.
//! Contents: Online — the recognizer and the stream of one dictation: load, begin, audio, end, partials (the same four calls the offline
//! `Engine` answers, so the worker treats them alike).

use sherpa_onnx::{OnlineRecognizer, OnlineRecognizerConfig, OnlineStream, OnlineTransducerModelConfig};

use super::phrase::Phrases;
use super::protocol::{EngineSpec, Reply};
use crate::audio::RATE;

/// A pause this long after speech ends the utterance: what is left of it is handed out and the decoder starts fresh.
const PAUSE_SECS: f32 = 1.2;

pub struct Online {
    rec: OnlineRecognizer,
    stream: OnlineStream,
    phrases: Phrases,
    /// The language option of the stream, when the model has one ("auto": detect it per stream).
    language: Option<String>,
    pub id: u64,
    /// Clauses handed out so far, how many were reported, the number the first one gets (a replay continues the numbering), and how much audio has been fed.
    text: Vec<String>,
    reported: usize,
    first: usize,
    fed: usize,
    /// Audio before this sample is used up for good: the decoder was reset at a pause, so a replay may start here. Only an endpoint moves it.
    pub released: usize,
}

/// The recognizer config a spec describes. Inputs: `folder`, the model folder with a trailing slash or empty when the process is already inside
/// it, and the spec. One config struct is supported (`Adapter::OnlineTransducer`); another streaming struct is one more function like this one.
fn config(folder: &str, spec: &EngineSpec) -> OnlineRecognizerConfig {
    let mut cfg = OnlineRecognizerConfig::default();
    let file = |name: &str| Some(format!("{folder}{name}"));
    cfg.model_config.transducer = OnlineTransducerModelConfig {
        encoder: file(&spec.encoder),
        decoder: file(&spec.decoder),
        joiner: spec.joiner.as_ref().and_then(|j| file(j)),
    };
    cfg.model_config.tokens = file("tokens.txt");
    cfg.model_config.model_type = spec.model_type.as_ref().map(|t| t.to_string());
    cfg.decoding_method = Some("greedy_search".into()); // the only method the Nemotron path supports
    cfg.enable_endpoint = true;
    (cfg.rule1_min_trailing_silence, cfg.rule2_min_trailing_silence) = (PAUSE_SECS * 2.0, PAUSE_SECS);
    cfg.rule3_min_utterance_length = 3600.0; // never cut speech that keeps going: that is the seam this engine exists to avoid
    cfg
}

impl Online {
    /// `dir` holds `encoder.int8.onnx`, `decoder.int8.onnx`, `joiner.int8.onnx` and `tokens.txt`. Plain file names inside the folder, for the
    /// same reason as the offline engine: native code reads paths in the ANSI code page.
    pub fn load(dir: &str, threads: i32, spec: &EngineSpec) -> Result<Self, String> {
        std::env::set_current_dir(dir).map_err(|e| format!("The speech model folder could not be opened: {e}"))?;
        let mut cfg = config("", spec);
        cfg.model_config.num_threads = threads;
        let rec = OnlineRecognizer::create(&cfg)
            .ok_or("The streaming speech model could not be loaded. Delete it in Settings and download it again.")?;
        let stream = rec.create_stream();
        Ok(Online {
            rec,
            stream,
            phrases: Phrases::default(),
            language: spec.language.as_ref().map(|l| l.to_string()),
            id: 0,
            text: Vec::new(),
            reported: 0,
            first: 0,
            fed: 0,
            released: 0,
        })
    }

    /// A new utterance (or the replay of one). Inputs: its id and the number its first clause gets.
    pub fn begin(&mut self, id: u64, clause: usize) {
        self.stream = self.rec.create_stream();
        if let Some(language) = &self.language {
            self.stream.set_option("language", language);
        }
        self.phrases.reset();
        (self.id, self.reported, self.first, self.fed, self.released) = (id, 0, clause, 0, 0);
        self.text.clear();
    }

    /// More audio for the utterance. Input: 16 kHz samples.
    pub fn audio(&mut self, samples: &[f32]) {
        self.fed += samples.len();
        self.stream.accept_waveform(RATE as i32, samples);
        self.pump(false, samples.len());
    }

    /// Decodes whatever the stream has, hands out the clauses that became ready, and starts a new utterance at a pause. Inputs: whether
    /// nothing more is coming, and how much audio the latest chunk added. The audio before that chunk is released at a pause: the chunk itself
    /// may hold the start of the next words, which the decoder has not settled, so it stays for a replay.
    /// shortcut: sherpa-onnx `Reset` after an endpoint keeps the encoder caches and the last tokens as decoder context (checked in the sherpa-onnx
    /// `OnlineRecognizerTransducerImpl::Reset` docs), so a fresh stream replayed from the release point lacks that context and may word the first
    /// clause slightly differently; the clauses typed before the crash stand and the replay continues after them. Exact replay needs the worker
    /// to keep a copy of the stream state, which the library does not expose.
    fn pump(&mut self, last: bool, chunk: usize) {
        while self.rec.is_ready(&self.stream) {
            self.rec.decode(&self.stream);
        }
        let paused = self.rec.is_endpoint(&self.stream);
        let hypothesis = self.rec.get_result(&self.stream).map(|r| r.text).unwrap_or_default();
        for clause in self.phrases.next(&hypothesis, last || paused) {
            self.text.push(clause);
        }
        if paused {
            self.rec.reset(&self.stream);
            self.phrases.reset();
            self.released = self.fed - chunk;
        }
    }

    /// The utterance is over. Returns its whole text.
    pub fn end(&mut self) -> String {
        // The model commits a word only after it has heard about 0.7 s more audio (its right context): without this the last word of a
        // recording that ends right after it comes out cut ("đất n" for "đất nước").
        self.stream.accept_waveform(RATE as i32, &vec![0.0; RATE as usize * 7 / 10]);
        self.stream.input_finished();
        self.pump(true, 0);
        self.text.join(" ") // kept until the next `begin`, so the last clause can still be reported
    }

    /// Clauses handed out since the last call, as partial results.
    pub fn partials(&mut self) -> Vec<Reply> {
        let fresh = self.text.iter().enumerate().skip(self.reported);
        let replies = fresh.map(|(index, text)| Reply::Partial { id: self.id, index: self.first + index, text: text.clone() }).collect();
        self.reported = self.text.len();
        replies
    }
}
