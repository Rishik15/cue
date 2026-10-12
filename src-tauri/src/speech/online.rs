//! Purpose: The streaming speech engine for long dictation. A cache-aware streaming model (Nemotron) keeps its encoder caches and decoder
//! state between chunks, so every audio frame is processed once and a recording can run for an hour in constant memory, with no cuts and
//! no seams to stitch. Settled words come out as clauses (see `phrase`); a pause of 1.2 s ends the utterance and starts a fresh one.
//! Contents: Online — the recognizer and the stream of one dictation: load, begin, audio, end, partials (the same four calls the offline
//! `Engine` answers, so the worker treats them alike).

use sherpa_onnx::{OnlineRecognizer, OnlineRecognizerConfig, OnlineStream, OnlineTransducerModelConfig};

use super::phrase::Phrases;
use super::protocol::Reply;
use crate::audio::RATE;

/// A pause this long after speech ends the utterance: what is left of it is handed out and the decoder starts fresh.
const PAUSE_SECS: f32 = 1.2;

pub struct Online {
    rec: OnlineRecognizer,
    stream: OnlineStream,
    phrases: Phrases,
    /// The multilingual model detects the language per stream.
    detect_language: bool,
    id: u64,
    /// Clauses handed out so far (and the sample count at each), how many were reported, and how much audio has been fed.
    text: Vec<String>,
    ends: Vec<usize>,
    reported: usize,
    fed: usize,
}

/// `folder` is the model folder with a trailing slash, or empty when the process is already inside it.
fn config(folder: &str) -> OnlineRecognizerConfig {
    let mut cfg = OnlineRecognizerConfig::default();
    let file = |name: &str| Some(format!("{folder}{name}"));
    cfg.model_config.transducer = OnlineTransducerModelConfig {
        encoder: file("encoder.int8.onnx"),
        decoder: file("decoder.int8.onnx"),
        joiner: file("joiner.int8.onnx"),
    };
    cfg.model_config.tokens = file("tokens.txt");
    cfg.model_config.model_type = Some("nemotron".into());
    cfg.decoding_method = Some("greedy_search".into()); // the only method the Nemotron path supports
    cfg.enable_endpoint = true;
    (cfg.rule1_min_trailing_silence, cfg.rule2_min_trailing_silence) = (PAUSE_SECS * 2.0, PAUSE_SECS);
    cfg.rule3_min_utterance_length = 3600.0; // never cut speech that keeps going: that is the seam this engine exists to avoid
    cfg
}

impl Online {
    /// `dir` holds `encoder.int8.onnx`, `decoder.int8.onnx`, `joiner.int8.onnx` and `tokens.txt`. Plain file names inside the folder, for the
    /// same reason as the offline engine: native code reads paths in the ANSI code page.
    pub fn load(dir: &str, threads: i32, detect_language: bool) -> Result<Self, String> {
        std::env::set_current_dir(dir).map_err(|e| format!("The speech model folder could not be opened: {e}"))?;
        let mut cfg = config("");
        cfg.model_config.num_threads = threads;
        let rec = OnlineRecognizer::create(&cfg)
            .ok_or("The streaming speech model could not be loaded. Delete it in Settings and download it again.")?;
        let stream = rec.create_stream();
        Ok(Online {
            rec,
            stream,
            phrases: Phrases::default(),
            detect_language,
            id: 0,
            text: Vec::new(),
            ends: Vec::new(),
            reported: 0,
            fed: 0,
        })
    }

    pub fn begin(&mut self, id: u64) {
        self.stream = self.rec.create_stream();
        if self.detect_language {
            self.stream.set_option("language", "auto");
        }
        self.phrases.reset();
        (self.id, self.reported, self.fed) = (id, 0, 0);
        self.text.clear();
        self.ends.clear();
    }

    pub fn audio(&mut self, samples: &[f32]) {
        self.fed += samples.len();
        self.stream.accept_waveform(RATE as i32, samples);
        self.pump(false);
    }

    /// Decodes whatever the stream has, hands out the clauses that became ready, and starts a new utterance at a pause.
    fn pump(&mut self, last: bool) {
        while self.rec.is_ready(&self.stream) {
            self.rec.decode(&self.stream);
        }
        let paused = self.rec.is_endpoint(&self.stream);
        let hypothesis = self.rec.get_result(&self.stream).map(|r| r.text).unwrap_or_default();
        for clause in self.phrases.next(&hypothesis, last || paused) {
            self.text.push(clause);
            self.ends.push(self.fed); // all audio so far is used up: nothing of it is needed again
        }
        if paused {
            self.rec.reset(&self.stream);
            self.phrases.reset();
        }
    }

    pub fn end(&mut self) -> String {
        // The model commits a word only after it has heard about 0.7 s more audio (its right context): without this the last word of a
        // recording that ends right after it comes out cut ("đất n" for "đất nước").
        self.stream.accept_waveform(RATE as i32, &vec![0.0; RATE as usize * 7 / 10]);
        self.stream.input_finished();
        self.pump(true);
        self.text.join(" ") // kept until the next `begin`, so the last clause can still be reported
    }

    /// Clauses handed out since the last call, as partial results.
    pub fn partials(&mut self) -> Vec<Reply> {
        let fresh = self.text.iter().enumerate().skip(self.reported);
        let replies = fresh.map(|(index, text)| Reply::Partial { id: self.id, index, text: text.clone(), end: self.ends[index] }).collect();
        self.reported = self.text.len();
        replies
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every successive hypothesis keeps all of the previous one's words except the last, which may still grow: that is what makes it safe to type
    /// settled words. Skipped unless CUE_TEST_STREAM_MODEL is set (see `scripts/winsync stream`).
    #[test]
    fn a_hypothesis_only_ever_grows_at_its_last_word() {
        let Ok(dir) = std::env::var("CUE_TEST_STREAM_MODEL") else { return };
        let dir = dir.replace('\\', "/");
        let mut cfg = config(&format!("{dir}/"));
        cfg.model_config.num_threads = 2;
        let rec = OnlineRecognizer::create(&cfg).expect("model loads");
        for name in ["0.wav", "1.wav"] {
            let wave = sherpa_onnx::Wave::read(&format!("{dir}/test_wavs/{name}")).expect("wav");
            let stream = rec.create_stream();
            let mut previous: Vec<String> = Vec::new();
            for piece in wave.samples().chunks(3_200) {
                stream.accept_waveform(RATE as i32, piece);
                while rec.is_ready(&stream) {
                    rec.decode(&stream);
                }
                let words: Vec<String> =
                    rec.get_result(&stream).map(|r| r.text).unwrap_or_default().split_whitespace().map(String::from).collect();
                let settled = previous.len().saturating_sub(1);
                assert_eq!(words.get(..settled), Some(&previous[..settled]), "{name}: {previous:?} then {words:?}");
                previous = words;
            }
        }
    }
}
