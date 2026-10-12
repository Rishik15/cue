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

use super::join::{stitch, words_in};
use super::protocol::{read_frame, write_frame, Family, Reply, Request};
use super::segment::{is_loud, padded, quietest, trim_point, CUT_LOOK_SECS, CUT_SECS};
use crate::audio::RATE;

/// A pause this long closes a segment. Long enough that cuts land between sentences, where Parakeet's punctuation is right on its own.
const PAUSE_SECS: f32 = 0.7;
/// While typing live, text should follow the speaker closely, so a breath between phrases is enough.
const LIVE_PAUSE_SECS: f32 = 0.45;
/// The detector buffers audio it has not finished judging. A segment is closed at the latest after `MAX_SPEECH_SECS` of speech.
const VAD_BUFFER_SECS: f32 = 70.0;
const MAX_SPEECH_SECS: f32 = 14.0;
/// What one decode is for (see `Engine::decode_part`).
struct Part {
    keep_until: usize,
    used: usize,
    open: bool,
}

/// How far each side of a cut reaches into the other, in samples.
const OVERLAP: usize = RATE as usize;

struct Engine {
    rec: OfflineRecognizer,
    vad: VoiceActivityDetector,
    /// Where the detector was loaded from and the pause it was built with, so a live session can swap in the shorter one.
    vad_path: String,
    pause: f32,
    family: Family,
    /// After a cut in the middle of speech: words of the next piece that start before this sample were the other side's, and the piece
    /// continues a sentence. Both are consumed by the next decode.
    skip_before: usize,
    carry_on: bool,
    text: Vec<String>,
    /// The utterance being recognised, and how many of its sentences have been reported as partial results.
    id: u64,
    reported: usize,
    /// The latest audio of the current utterance, so segments can be cut with margin, and where it starts in the whole utterance.
    audio: Vec<f32>,
    base: usize,
    /// End of the audio already handed to the recognizer; the next segment may not start before it. `ends[i]` is where sentence `i` ended.
    done_to: usize,
    ends: Vec<usize>,
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

fn detector(path: &str, pause: f32) -> Option<VoiceActivityDetector> {
    let silero = SileroVadModelConfig {
        model: Some(path.to_string()),
        threshold: 0.4,
        min_silence_duration: pause,
        min_speech_duration: 0.15,
        window_size: 512,
        max_speech_duration: MAX_SPEECH_SECS,
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
        let detected = detector(&vad, PAUSE_SECS)
            .ok_or("The voice detector could not be loaded. Delete the speech model in Settings and download it again.")?;
        Ok(Engine {
            rec,
            vad: detected,
            vad_path: vad,
            pause: PAUSE_SECS,
            family,
            skip_before: 0,
            carry_on: false,
            text: Vec::new(),
            id: 0,
            reported: 0,
            audio: Vec::new(),
            base: 0,
            done_to: 0,
            ends: Vec::new(),
        })
    }

    fn begin(&mut self, id: u64, live: bool) {
        let pause = if live { LIVE_PAUSE_SECS } else { PAUSE_SECS };
        match detector(&self.vad_path, pause).filter(|_| pause != self.pause) {
            Some(vad) => (self.vad, self.pause) = (vad, pause),
            None => self.vad.reset(), // same pause as before (or the new detector could not load): keep the one we have
        }
        self.text.clear();
        (self.id, self.reported) = (id, 0);
        self.audio.clear();
        (self.base, self.done_to) = (0, 0);
        (self.skip_before, self.carry_on) = (0, false);
        self.ends.clear();
    }

    /// Recognises the part of the utterance from sample `from` to `to` and keeps the text.
    fn decode(&mut self, from: usize, to: usize) -> Result<(), String> {
        self.decode_part(from, to, Part { keep_until: usize::MAX, used: to, open: false })
    }

    /// Decodes one piece. Words starting after `keep_until` are the next piece's; `used` is how much of the audio is done with; `open` says
    /// the piece was cut in the middle of speech.
    fn decode_part(&mut self, from: usize, to: usize, part: Part) -> Result<(), String> {
        let stream = self.rec.create_stream();
        stream.accept_waveform(RATE as i32, &self.audio[from - self.base..to - self.base]);
        self.rec.decode(&stream);
        let result = stream.get_result().ok_or("The speech engine returned no result.")?;
        let skip_before = std::mem::take(&mut self.skip_before);
        let seconds = |sample: usize| sample.saturating_sub(from) as f32 / RATE as f32;
        let mut text = match result.timestamps.as_deref() {
            Some(times) if skip_before > 0 || part.keep_until != usize::MAX => {
                words_in(&result.tokens, times, seconds(skip_before), seconds(part.keep_until))
            }
            _ => result.text.trim().to_string(),
        };
        text = stitch(&text, self.text.last().map_or("", String::as_str), std::mem::take(&mut self.carry_on), part.open);
        self.carry_on = part.open;
        if !text.is_empty() {
            self.text.push(text);
            self.ends.push(part.used);
        }
        self.done_to = part.used;
        Ok(())
    }

    /// Transcribes every segment the detector has closed.
    fn drain(&mut self) -> Result<(), String> {
        while let Some(segment) = self.vad.front() {
            let floor = self.done_to.max(self.base);
            let (from, to) = padded(segment.start().max(0) as usize, segment.n().max(0) as usize, floor, self.base + self.audio.len());
            self.vad.pop();
            self.decode(from, to)?;
        }
        Ok(())
    }

    /// Sentences recognised since the last call, as partial results.
    fn partials(&mut self) -> Vec<Reply> {
        let fresh = self.text.iter().enumerate().skip(self.reported);
        let replies = fresh.map(|(index, text)| Reply::Partial { id: self.id, index, text: text.clone(), end: self.ends[index] }).collect();
        self.reported = self.text.len();
        replies
    }

    /// Speech that has run `CUT_SECS` without a pause is decoded up to its quietest moment near the end, instead of waiting for the detector's
    /// clock-driven cut. A transducer reports when each word starts, so the piece is decoded a second past the cut and the next one starts a
    /// second before it: a word on the seam is heard whole by one side, and each side keeps only the words that start on its own side.
    fn cut_long_speech(&mut self) -> Result<(), String> {
        let (total, floor) = (self.base + self.audio.len(), self.done_to.max(self.base));
        if !self.vad.detected() || total - floor < RATE as usize * CUT_SECS {
            return Ok(());
        }
        let look = total - RATE as usize * CUT_LOOK_SECS;
        let (offset, pause) = quietest(&self.audio[look - self.base..]);
        let cut = look + offset;
        let open = !pause; // a real pause is a real boundary: the model's punctuation and capitals on both sides stand
        if self.family != Family::Transducer {
            return self.decode_part(floor, cut, Part { keep_until: usize::MAX, used: cut, open });
        }
        let (to, used) = ((cut + OVERLAP).min(total), cut.saturating_sub(OVERLAP).max(floor));
        self.decode_part(floor, to, Part { keep_until: cut + OVERLAP / 10, used, open })?;
        self.skip_before = cut;
        Ok(())
    }

    fn audio(&mut self, samples: &[f32]) -> Result<(), String> {
        self.audio.extend_from_slice(samples);
        self.vad.accept_waveform(samples);
        self.drain()?;
        self.cut_long_speech()?;
        let from = trim_point(self.base + self.audio.len(), self.base);
        self.audio.drain(..from - self.base);
        self.base = from;
        Ok(())
    }

    fn end(&mut self) -> Result<String, String> {
        self.vad.flush();
        self.drain()?;
        // The detector found no speech but the microphone heard something loud: trust the ears over the detector.
        if self.done_to == 0 && is_loud(&self.audio) {
            self.decode(self.base, self.base + self.audio.len())?;
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
        Request::Begin { id, live } => engine.begin(id, live),
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
    use super::vad_relative;

    #[test]
    fn voice_detector_path_is_relative_to_the_model_folder() {
        assert_eq!(vad_relative("C:/data/models/parakeet-v2", "C:/data/models/silero_vad.onnx"), "../silero_vad.onnx");
        assert_eq!(vad_relative("C:/dev/model", "C:/dev/model/silero_vad.onnx"), "silero_vad.onnx");
    }
}
