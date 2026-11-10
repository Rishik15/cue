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
use super::protocol::{read_frame, write_frame, Adapter, EngineSpec, Reply, Request, Segmented, VERSION};
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

/// The segmented engine for one loaded model: the recognizer and the voice detector, with the state of the current utterance.
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

/// Whether the audio has enough loud samples to be speech the detector missed. Input: the samples.
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

/// The offline recognizer config a spec describes. Inputs: the model folder, the spec, its adapter and the thread count. Returns None when the
/// tokens file is missing. One arm per sherpa-onnx config struct: a new model of a known struct is a catalog row, a new struct is one more arm here.
fn offline_config(dir: &str, spec: &EngineSpec, adapter: Segmented, threads: i32) -> Option<OfflineRecognizerConfig> {
    let (encoder, decoder) = (file(dir, &spec.encoder), file(dir, &spec.decoder));
    let language = spec.language.as_ref().map(|l| l.to_string());
    let mut cfg = OfflineRecognizerConfig::default();
    let m = &mut cfg.model_config;
    m.tokens = Some(file(dir, "tokens.txt")?);
    m.num_threads = threads.max(1);
    m.model_type = spec.model_type.as_ref().map(|t| t.to_string());
    match adapter {
        Segmented::Transducer => {
            let joiner = spec.joiner.as_ref().and_then(|j| file(dir, j));
            m.transducer = OfflineTransducerModelConfig { encoder, decoder, joiner };
        }
        Segmented::Canary => {
            m.canary = OfflineCanaryModelConfig { encoder, decoder, src_lang: language.clone(), tgt_lang: language, use_pnc: true }
        }
        Segmented::Whisper => {
            m.whisper = OfflineWhisperModelConfig { encoder, decoder, language, task: Some("transcribe".into()), ..Default::default() }
        }
    }
    Some(cfg)
}

/// Builds the offline recognizer for a spec (greedy search is the default; beam search hallucinates or returns nothing on TDT models, sherpa-onnx #3267).
/// Inputs as for `offline_config`. Returns None when the files are missing or the library refuses them.
fn recognizer(dir: &str, spec: &EngineSpec, adapter: Segmented, threads: i32) -> Option<OfflineRecognizer> {
    OfflineRecognizer::create(&offline_config(dir, spec, adapter, threads)?)
}

/// Builds the Silero voice detector. Input: its model path. Returns None when it cannot be loaded.
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
    /// Loads the recognizer and the voice detector. Inputs: the model folder, the detector path, the spec, its adapter and the thread count.
    fn load(dir: &str, vad: &str, spec: &EngineSpec, adapter: Segmented, threads: i32) -> Result<Self, String> {
        // Native code reads model files through narrow (ANSI) paths on Windows, which break on a profile folder with non-ASCII letters.
        // Entering the folder and using plain file names sidesteps that.
        let vad = vad_relative(dir, vad);
        std::env::set_current_dir(dir).map_err(|e| format!("The speech model folder could not be opened: {e}"))?;
        let rec = recognizer(dir, spec, adapter, threads)
            .ok_or("The speech model could not be loaded. Delete it in Settings and download it again.")?;
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

    /// More audio: kept for padding, fed to the detector, and every segment it closed is decoded. Input: the samples. Returns an error sentence if decoding failed.
    fn audio(&mut self, samples: &[f32]) -> Result<(), String> {
        self.audio.extend_from_slice(samples);
        self.vad.accept_waveform(samples);
        self.drain()?;
        let from = trim_point(self.base + self.audio.len(), self.base);
        self.audio.drain(..from - self.base);
        self.base = from;
        Ok(())
    }

    /// The utterance is over: decodes what is left. Returns the whole text or an error sentence.
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
    /// Loads the engine the spec describes. Inputs: the model folder, the detector path, the spec and the thread count. Returns the engine or an error sentence.
    fn load(dir: &str, vad: &str, spec: &EngineSpec, threads: i32) -> Result<Self, String> {
        match spec.adapter {
            Adapter::Streaming(adapter) => Online::load(dir, threads, spec, adapter).map(Loaded::Online),
            Adapter::Segmented(adapter) => Engine::load(dir, vad, spec, adapter, threads).map(Loaded::Offline),
        }
    }

    /// The clauses found since the last call, as Partial replies.
    fn partials(&mut self) -> Vec<Reply> {
        match self {
            Loaded::Offline(e) => e.partials(),
            Loaded::Online(o) => o.partials(),
        }
    }

    /// The answer every request gets last: the audio of the current utterance the app may stop keeping, counted from the worker's first sample.
    fn release(&self) -> Reply {
        match self {
            Loaded::Offline(e) => Reply::Release { id: e.id, up_to: e.done_to, clauses: e.first + e.text.len() },
            Loaded::Online(o) => Reply::Release { id: o.id, up_to: o.released, clauses: o.released_clauses },
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
    ordered(loaded.partials(), answer, loaded.release())
}

/// Puts one request's replies in the order the app needs: the clauses found, then the Text or Error, then the Release that closes the request,
/// which an Error replaces (the app kills a worker that failed).
fn ordered(partials: Vec<Reply>, answer: Option<Reply>, release: Reply) -> Vec<Reply> {
    let failed = matches!(answer, Some(Reply::Error { .. }));
    let mut out = partials;
    out.extend(answer);
    if !failed {
        out.push(release);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech::catalog::all;

    /// A Partial reply for the tests.
    fn partial(index: usize) -> Reply {
        Reply::Partial { id: 1, index, text: "w".into() }
    }

    /// A Release reply for the tests.
    fn release() -> Reply {
        Reply::Release { id: 1, up_to: 0, clauses: 0 }
    }

    /// Clauses come first, then the Text, and the Release closes the request.
    #[test]
    fn a_release_closes_every_answered_request() {
        let text = Reply::Text { id: 1, text: "w w".into() };
        assert_eq!(
            ordered(vec![partial(0), partial(1)], Some(text), release()),
            [partial(0), partial(1), Reply::Text { id: 1, text: "w w".into() }, release()]
        );
        assert_eq!(ordered(Vec::new(), None, release()), [release()], "audio with nothing recognised is still acknowledged");
    }

    /// Nothing follows an Error: the app treats it as the end of that worker.
    #[test]
    fn nothing_is_sent_after_an_error() {
        let error = Reply::Error { message: "bad".into() };
        assert_eq!(ordered(vec![partial(0)], Some(error), release()), [partial(0), Reply::Error { message: "bad".into() }]);
    }

    /// Each segmented catalog row maps to the offline config its adapter names, with the files, model type and language the row declares.
    #[test]
    fn every_segmented_row_maps_to_its_config() {
        for model in all().filter(|m| !m.engine.is_streaming()) {
            let dir = tempfile::tempdir().expect("dir");
            for file in model.files {
                std::fs::write(dir.path().join(file.name), b"x").expect("write");
            }
            let Adapter::Segmented(adapter) = model.engine.adapter else { unreachable!() };
            let cfg = offline_config(&dir.path().to_string_lossy(), &model.engine, adapter, 2).expect(model.id);
            let m = &cfg.model_config;
            assert_eq!(m.model_type.as_deref(), model.engine.model_type.as_deref(), "{}", model.id);
            assert!(m.tokens.as_deref().is_some_and(|t| t.ends_with("tokens.txt")), "{}", model.id);
            let (encoder, decoder) = match adapter {
                Segmented::Transducer => (&m.transducer.encoder, &m.transducer.decoder),
                Segmented::Canary => (&m.canary.encoder, &m.canary.decoder),
                Segmented::Whisper => (&m.whisper.encoder, &m.whisper.decoder),
            };
            assert!(encoder.as_deref().is_some_and(|f| f.ends_with(&*model.engine.encoder)), "{}", model.id);
            assert!(decoder.as_deref().is_some_and(|f| f.ends_with(&*model.engine.decoder)), "{}", model.id);
            assert_eq!(m.transducer.joiner.is_some(), model.engine.joiner.is_some(), "{}", model.id);
            let language = model.engine.language.as_deref();
            match adapter {
                Segmented::Canary => assert_eq!((m.canary.src_lang.as_deref(), m.canary.tgt_lang.as_deref()), (language, language)),
                Segmented::Whisper => assert_eq!(m.whisper.language.as_deref(), language),
                Segmented::Transducer => {}
            }
        }
    }
}
