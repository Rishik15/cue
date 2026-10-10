//! Purpose: The speech worker: this same executable started with `--speech-worker`. It owns the recognizer and the voice detector, so killing
//! the process returns every byte of the model to Windows, and a crash in the engine cannot take the tray app down. No window, no Tauri.
//! Contents: main — read requests from stdin, answer on stdout; Engine — recognizer plus detector for one utterance at a time:
//! speech segments are transcribed as soon as a pause closes them, so only the last one is left when the user stops.

use std::io::{self, BufReader};
use std::path::Path;
use std::time::Instant;

use sherpa_onnx::{
    OfflineCanaryModelConfig, OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig, OfflineWhisperModelConfig,
    SileroVadModelConfig, VadModelConfig, VoiceActivityDetector,
};

use super::protocol::{read_frame, write_frame, Family, Reply, Request};
use crate::audio::RATE;

/// A pause this long closes a segment. Long enough that cuts land between sentences, where Parakeet's punctuation is right on its own.
const PAUSE_SECS: f32 = 0.7;
/// The detector buffers audio it has not finished judging; the recorder caps at 60 s.
const VAD_BUFFER_SECS: f32 = 70.0;

/// The detector reports speech a little late and cuts it right at the last voiced frame, which clips the first and last sounds ("I begged"
/// became "begged"). Each segment is therefore re-cut from the recorded audio with some margin.
const PAD_BEFORE: usize = RATE as usize * 3 / 10;
const PAD_AFTER: usize = RATE as usize / 5;
/// Audio this loud with no speech found by the detector is decoded whole rather than dropped.
const LOUD: f32 = 0.03;
/// How many samples must be that loud: a click or a cough has a few hundred, speech thousands.
const MIN_LOUD: usize = RATE as usize / 10;

struct Engine {
    rec: OfflineRecognizer,
    vad: VoiceActivityDetector,
    text: Vec<String>,
    /// The utterance being recognised, and how many of its sentences have been reported as partial results.
    id: u64,
    reported: usize,
    /// Everything of the current utterance, so segments can be cut with margin.
    audio: Vec<f32>,
    /// End of the audio already handed to the recognizer; the next segment may not start before it.
    done_to: usize,
}

/// The range of the recording to decode for a segment of `n` samples starting at `start`: padded, and clamped so that it neither overlaps
/// what was already decoded nor runs past the audio received so far.
fn padded(start: usize, n: usize, done_to: usize, len: usize) -> (usize, usize) {
    let from = start.saturating_sub(PAD_BEFORE).max(done_to).min(len);
    let to = (start + n + PAD_AFTER).min(len).max(from);
    (from, to)
}

fn is_loud(samples: &[f32]) -> bool {
    samples.iter().filter(|s| s.abs() > LOUD).count() >= MIN_LOUD
}

/// The name of the file in `dir` that ends with `suffix` (Whisper's files carry a size prefix, "small-encoder.int8.onnx").
/// Names only, no folders: the worker works inside the model folder (see `Engine::load`).
fn file(dir: &str, suffix: &str) -> Option<String> {
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).find(|name| name.ends_with(suffix))
}

/// The voice detector path as seen from the model folder: beside the model files in tests, one level up in the app's data folder.
fn vad_relative(dir: &str, vad: &str) -> String {
    let name = Path::new(vad).file_name().map_or_else(|| vad.to_string(), |n| n.to_string_lossy().into_owned());
    if Path::new(vad).parent() == Some(Path::new(dir)) {
        name
    } else {
        format!("../{name}")
    }
}

fn recognizer(dir: &str, family: Family, threads: i32) -> Option<OfflineRecognizer> {
    let (encoder, decoder) = (file(dir, "encoder.int8.onnx"), file(dir, "decoder.int8.onnx"));
    let mut cfg = OfflineRecognizerConfig::default();
    let m = &mut cfg.model_config;
    m.tokens = Some(file(dir, "tokens.txt")?);
    m.num_threads = threads.max(1);
    match family {
        Family::Transducer => {
            m.transducer = OfflineTransducerModelConfig { encoder, decoder, joiner: file(dir, "joiner.int8.onnx") };
            m.model_type = Some("nemo_transducer".into());
        }
        Family::Canary => {
            m.canary =
                OfflineCanaryModelConfig { encoder, decoder, src_lang: Some("en".into()), tgt_lang: Some("en".into()), use_pnc: true }
        }
        Family::Whisper => {
            m.whisper = OfflineWhisperModelConfig {
                encoder,
                decoder,
                language: Some(String::new()),
                task: Some("transcribe".into()),
                ..Default::default()
            }
        }
    }
    // Greedy search is the default; beam search hallucinates or returns nothing on TDT models (sherpa-onnx #3267).
    OfflineRecognizer::create(&cfg)
}

fn detector(path: &str) -> Option<VoiceActivityDetector> {
    let silero = SileroVadModelConfig {
        model: Some(path.to_string()),
        threshold: 0.4,
        min_silence_duration: PAUSE_SECS,
        min_speech_duration: 0.15,
        window_size: 512,
        max_speech_duration: 25.0,
    };
    let cfg = VadModelConfig { silero_vad: silero, sample_rate: RATE as i32, num_threads: 1, ..Default::default() };
    VoiceActivityDetector::create(&cfg, VAD_BUFFER_SECS)
}

impl Engine {
    fn load(dir: &str, vad: &str, family: Family, threads: i32) -> Result<Self, String> {
        // Native code reads model files through narrow (ANSI) paths on Windows, which break on a profile folder with non-ASCII letters.
        // Entering the folder and using plain file names sidesteps that.
        let vad = vad_relative(dir, vad);
        std::env::set_current_dir(dir).map_err(|e| format!("The speech model folder could not be opened: {e}"))?;
        let rec =
            recognizer(dir, family, threads).ok_or("The speech model could not be loaded. Delete it in Settings and download it again.")?;
        let vad =
            detector(&vad).ok_or("The voice detector could not be loaded. Delete the speech model in Settings and download it again.")?;
        Ok(Engine { rec, vad, text: Vec::new(), id: 0, reported: 0, audio: Vec::new(), done_to: 0 })
    }

    fn begin(&mut self, id: u64) {
        self.vad.reset();
        self.text.clear();
        (self.id, self.reported) = (id, 0);
        self.audio.clear();
        self.done_to = 0;
    }

    /// Recognises `audio[from..to]` and keeps the text.
    fn decode(&mut self, from: usize, to: usize) -> Result<(), String> {
        let stream = self.rec.create_stream();
        stream.accept_waveform(RATE as i32, &self.audio[from..to]);
        self.rec.decode(&stream);
        let result = stream.get_result().ok_or("The speech engine returned no result.")?;
        let text = result.text.trim();
        if !text.is_empty() {
            self.text.push(text.to_string());
        }
        self.done_to = to;
        Ok(())
    }

    /// Transcribes every segment the detector has closed.
    fn drain(&mut self) -> Result<(), String> {
        while let Some(segment) = self.vad.front() {
            let (from, to) = padded(segment.start().max(0) as usize, segment.n().max(0) as usize, self.done_to, self.audio.len());
            self.vad.pop();
            self.decode(from, to)?;
        }
        Ok(())
    }

    /// Sentences recognised since the last call, as partial results.
    fn partials(&mut self) -> Vec<Reply> {
        let fresh = self.text.iter().enumerate().skip(self.reported);
        let replies = fresh.map(|(index, text)| Reply::Partial { id: self.id, index, text: text.clone() }).collect();
        self.reported = self.text.len();
        replies
    }

    fn audio(&mut self, samples: &[f32]) -> Result<(), String> {
        self.audio.extend_from_slice(samples);
        self.vad.accept_waveform(samples);
        self.drain()
    }

    fn end(&mut self) -> Result<String, String> {
        self.vad.flush();
        self.drain()?;
        // The detector found no speech but the microphone heard something loud: trust the ears over the detector.
        if self.done_to == 0 && is_loud(&self.audio) {
            self.decode(0, self.audio.len())?;
        }
        Ok(self.text.join(" ")) // kept until the next `begin`, so the last sentence can still be reported as a partial
    }
}

fn answer(engine: &mut Option<Engine>, request: Request, samples: &[f32]) -> Option<Reply> {
    let error = |message: String| Some(Reply::Error { message });
    if let Request::Load { dir, vad, family, threads } = request {
        let started = Instant::now();
        return match Engine::load(&dir, &vad, family, threads) {
            Ok(e) => {
                *engine = Some(e);
                Some(Reply::Loaded { ms: started.elapsed().as_millis() as u64 })
            }
            Err(message) => error(message),
        };
    }
    let Some(engine) = engine.as_mut() else { return error("The speech model is not loaded.".into()) };
    match request {
        Request::Begin { id } => engine.begin(id),
        Request::Audio => return engine.audio(samples).err().and_then(error),
        Request::End { id } => return Some(engine.end().map_or_else(|message| Reply::Error { message }, |text| Reply::Text { id, text })),
        Request::Load { .. } => {}
    }
    None
}

/// Exit code for the process. Ends when the app closes our stdin (unload, quit, or the app died).
pub fn main() -> i32 {
    crate::platform::disable_power_throttling();
    let (mut input, mut output) = (BufReader::new(io::stdin().lock()), io::stdout().lock());
    let mut engine = None;
    loop {
        let (request, samples) = match read_frame::<Request>(&mut input) {
            Ok(Some(frame)) => frame,
            Ok(None) => return 0,
            Err(_) => return 1,
        };
        let reply = answer(&mut engine, request, &samples);
        // Sentences found while handling this request go out first, so the caption is current before the final text arrives.
        let partials = engine.as_mut().map(Engine::partials).unwrap_or_default();
        if partials.iter().chain(reply.iter()).any(|r| write_frame(&mut output, r, &[]).is_err()) {
            return 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{padded, vad_relative};

    #[test]
    fn segments_are_padded_without_overlap_or_overrun() {
        let r = crate::audio::RATE as usize;
        assert_eq!(padded(r, r, 0, 10 * r), (r - r * 3 / 10, 2 * r + r / 5)); // margin on both sides
        assert_eq!(padded(1000, 5000, 0, 90_000), (0, 6000 + r / 5)); // cannot start before the recording
        assert_eq!(padded(r, r, r + r / 2, 10 * r).0, r + r / 2); // does not repeat audio already decoded
        assert_eq!(padded(r, r, 0, 2 * r + 100).1, 2 * r + 100); // does not run past what has arrived
    }

    #[test]
    fn loud_audio_is_recognised_quiet_and_tiny_is_not() {
        assert!(super::is_loud(&vec![0.1; 8_000]));
        assert!(!super::is_loud(&vec![0.001; 8_000]));
        assert!(!super::is_loud(&[0.5; 100])); // a click
    }

    #[test]
    fn voice_detector_path_is_relative_to_the_model_folder() {
        assert_eq!(vad_relative("C:/data/models/parakeet-v2", "C:/data/models/silero_vad.onnx"), "../silero_vad.onnx");
        assert_eq!(vad_relative("C:/dev/model", "C:/dev/model/silero_vad.onnx"), "silero_vad.onnx");
    }
}
