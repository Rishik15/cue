//! File: speech/worker.rs
//! Purpose: The speech worker: this same executable started with `--speech-worker`. It owns the recognizer and the voice detector, so killing
//! the process returns every byte of the model to Windows, and a crash in the engine cannot take the tray app down. No window, no Tauri.
//! Contents: main — read requests from stdin, answer on stdout (every request after Load ends with a Release); Engine — recognizer plus detector for one utterance at a time:
//! speech segments are transcribed as soon as a pause closes them, so only the last one is left when the user stops.

use std::io::{self, BufReader};
use std::path::Path;
use std::time::Instant;

use sherpa_onnx::{
    OfflineCanaryModelConfig, OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig, OfflineWhisperModelConfig,
    SileroVadModelConfig, VadModelConfig, VoiceActivityDetector,
};

use super::online::Online;
use super::protocol::{read_frame, write_frame, Adapter, EngineSpec, Kind, Reply, Request, VERSION};
use crate::audio::RATE;

/// A pause this long closes a segment. Long enough that cuts land between sentences, where Parakeet's punctuation is right on its own.
const PAUSE_SECS: f32 = 0.7;
/// The detector buffers audio it has not finished judging. A segment is closed at the latest after `MAX_SPEECH_SECS` of speech.
const VAD_BUFFER_SECS: f32 = 70.0;
const MAX_SPEECH_SECS: f32 = 25.0;
/// The worker keeps this much of the latest audio (the recording can last an hour). Anything older is decoded or was silence, because
/// speech is cut into segments at least this often.
const KEEP: usize = RATE as usize * 40;

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
    /// The number the first clause of this run gets (a replay continues after the clauses released before the crash).
    first: usize,
    /// The latest audio of the current utterance, so segments can be cut with margin, and where it starts in the whole utterance.
    audio: Vec<f32>,
    base: usize,
    /// End of the audio already handed to the recognizer; the next segment may not start before it, and the app may drop the audio before it.
    done_to: usize,
}

/// The range of the recording to decode for a segment of `n` samples starting at `start`: padded, and clamped so that it neither overlaps
/// what was already decoded nor runs past the audio received so far.
fn padded(start: usize, n: usize, done_to: usize, len: usize) -> (usize, usize) {
    let from = start.saturating_sub(PAD_BEFORE).max(done_to).min(len);
    let to = (start + n + PAD_AFTER).min(len).max(from);
    (from, to)
}

/// Where the kept audio starts once `total` samples have arrived: never earlier than it already does.
fn trim_point(total: usize, base: usize) -> usize {
    total.saturating_sub(KEEP).max(base)
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

/// Builds the offline recognizer a spec describes. Inputs: the model folder, its spec and the thread count. Returns None when the files are missing or a streaming spec is given.
/// One arm per sherpa-onnx config struct: a new model of a known struct is a catalog row, a new struct is one more arm here.
fn recognizer(dir: &str, spec: &EngineSpec, threads: i32) -> Option<OfflineRecognizer> {
    let (encoder, decoder) = (file(dir, &spec.encoder), file(dir, &spec.decoder));
    let language = spec.language.as_ref().map(|l| l.to_string());
    let mut cfg = OfflineRecognizerConfig::default();
    let m = &mut cfg.model_config;
    m.tokens = Some(file(dir, "tokens.txt")?);
    m.num_threads = threads.max(1);
    m.model_type = spec.model_type.as_ref().map(|t| t.to_string());
    match spec.adapter {
        Adapter::OfflineTransducer => {
            let joiner = spec.joiner.as_ref().and_then(|j| file(dir, j));
            m.transducer = OfflineTransducerModelConfig { encoder, decoder, joiner };
        }
        Adapter::OfflineCanary => {
            m.canary = OfflineCanaryModelConfig { encoder, decoder, src_lang: language.clone(), tgt_lang: language, use_pnc: true }
        }
        Adapter::OfflineWhisper => {
            m.whisper = OfflineWhisperModelConfig { encoder, decoder, language, task: Some("transcribe".into()), ..Default::default() }
        }
        Adapter::OnlineTransducer => return None,
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
        max_speech_duration: MAX_SPEECH_SECS,
    };
    let cfg = VadModelConfig { silero_vad: silero, sample_rate: RATE as i32, num_threads: 1, ..Default::default() };
    VoiceActivityDetector::create(&cfg, VAD_BUFFER_SECS)
}

impl Engine {
    fn load(dir: &str, vad: &str, spec: &EngineSpec, threads: i32) -> Result<Self, String> {
        // Native code reads model files through narrow (ANSI) paths on Windows, which break on a profile folder with non-ASCII letters.
        // Entering the folder and using plain file names sidesteps that.
        let vad = vad_relative(dir, vad);
        std::env::set_current_dir(dir).map_err(|e| format!("The speech model folder could not be opened: {e}"))?;
        let rec =
            recognizer(dir, spec, threads).ok_or("The speech model could not be loaded. Delete it in Settings and download it again.")?;
        let vad =
            detector(&vad).ok_or("The voice detector could not be loaded. Delete the speech model in Settings and download it again.")?;
        Ok(Engine { rec, vad, text: Vec::new(), id: 0, reported: 0, first: 0, audio: Vec::new(), base: 0, done_to: 0 })
    }

    /// A new utterance (or the replay of one). Inputs: its id and the number its first clause gets.
    fn begin(&mut self, id: u64, clause: usize) {
        self.vad.reset();
        self.text.clear();
        (self.id, self.reported, self.first) = (id, 0, clause);
        self.audio.clear();
        (self.base, self.done_to) = (0, 0);
    }

    /// Recognises the part of the utterance from sample `from` to `to` and keeps the text.
    fn decode(&mut self, from: usize, to: usize) -> Result<(), String> {
        let stream = self.rec.create_stream();
        stream.accept_waveform(RATE as i32, &self.audio[from - self.base..to - self.base]);
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
        let replies = fresh.map(|(index, text)| Reply::Partial { id: self.id, index: self.first + index, text: text.clone() }).collect();
        self.reported = self.text.len();
        replies
    }

    fn audio(&mut self, samples: &[f32]) -> Result<(), String> {
        self.audio.extend_from_slice(samples);
        self.vad.accept_waveform(samples);
        self.drain()?;
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

/// What the worker has loaded: an offline model that is fed pieces cut at pauses, or a streaming one that is fed the whole recording.
enum Loaded {
    Offline(Engine),
    Online(Online),
}

impl Loaded {
    fn load(dir: &str, vad: &str, spec: &EngineSpec, threads: i32) -> Result<Self, String> {
        match spec.kind {
            Kind::Streaming => Online::load(dir, threads, spec).map(Loaded::Online),
            Kind::Segmented => Engine::load(dir, vad, spec, threads).map(Loaded::Offline),
        }
    }

    fn partials(&mut self) -> Vec<Reply> {
        match self {
            Loaded::Offline(e) => e.partials(),
            Loaded::Online(o) => o.partials(),
        }
    }

    /// The answer every request gets last: the audio of the current utterance the app may stop keeping, counted from the worker's first sample.
    fn release(&self) -> Reply {
        match self {
            Loaded::Offline(e) => Reply::Release { id: e.id, up_to: e.done_to },
            Loaded::Online(o) => Reply::Release { id: o.id, up_to: o.released },
        }
    }
}

/// Loads the model a `Load` asks for. Inputs: the engine slot and the request's fields. Returns `Loaded`, or the error to report.
fn load(engine: &mut Option<Loaded>, dir: &str, vad: &str, spec: &EngineSpec, threads: i32, version: u32) -> Reply {
    if version != VERSION {
        return Reply::Error { message: "The speech engine is out of date. Restart Cue.".into() };
    }
    let started = Instant::now();
    match Loaded::load(dir, vad, spec, threads) {
        Ok(loaded) => {
            *engine = Some(loaded);
            Reply::Loaded { ms: started.elapsed().as_millis() as u64, version: VERSION }
        }
        Err(message) => Reply::Error { message },
    }
}

/// Handles one request after Load. Inputs: the loaded engine, the request and its audio. Returns the Text or Error reply, if any.
fn answer(loaded: &mut Loaded, request: Request, samples: &[f32]) -> Option<Reply> {
    match (request, loaded) {
        (Request::Begin { id, clause }, Loaded::Offline(e)) => e.begin(id, clause),
        (Request::Begin { id, clause }, Loaded::Online(o)) => o.begin(id, clause),
        (Request::Audio, Loaded::Offline(e)) => return e.audio(samples).err().map(|message| Reply::Error { message }),
        (Request::Audio, Loaded::Online(o)) => o.audio(samples),
        (Request::End { id }, Loaded::Offline(e)) => {
            return Some(e.end().map_or_else(|message| Reply::Error { message }, |text| Reply::Text { id, text }));
        }
        (Request::End { id }, Loaded::Online(o)) => return Some(Reply::Text { id, text: o.end() }),
        _ => {}
    }
    None
}

/// Every reply to one request, in the order the app needs them: clauses found, the Text or Error, then the Release that closes the request.
fn replies(engine: &mut Option<Loaded>, request: Request, samples: &[f32]) -> Vec<Reply> {
    if let Request::Load { dir, vad, spec, threads, version } = request {
        return vec![load(engine, &dir, &vad, &spec, threads, version)];
    }
    let Some(loaded) = engine.as_mut() else { return vec![Reply::Error { message: "The speech model is not loaded.".into() }] };
    let answer = answer(loaded, request, samples);
    let mut out = loaded.partials();
    let failed = matches!(answer, Some(Reply::Error { .. }));
    out.extend(answer);
    if !failed {
        out.push(loaded.release());
    }
    out
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
        if replies(&mut engine, request, &samples).iter().any(|r| write_frame(&mut output, r, &[]).is_err()) {
            return 1;
        }
    }
}
