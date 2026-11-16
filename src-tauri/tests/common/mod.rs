//! File: tests/common/mod.rs
//! Purpose: Helpers shared by the worker integration tests: the pipe protocol from the outside, a worker process handle that timestamps every reply, WAV reading.
//! Contents: stats — the benches' arithmetic; wav — the WAV format check; Family / spec_json — the engine spec a family name stands for (mirrors the catalog rows); Worker (and Job: its tree peak) — start / utterance with the timings of one dictation; Outcome — what one dictation returned and when; read_wav — a checked 16 kHz mono 16-bit WAV file to f32 (the parsing is wav.rs).
//! The wire format is protocol v2 (src/speech/protocol.rs): every request after Load is answered by one Release, so a reader thread drains the pipe and stamps each reply on arrival.

#![allow(dead_code)] // each test file uses a different part of these helpers

pub mod stats;
pub mod wav;

use std::io::{Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

/// The protocol revision this helper speaks; the worker refuses a Load that disagrees.
const VERSION: u32 = 2;
/// The recorder sends 200 ms pieces.
const PIECE: usize = 3_200;
const RATE: f64 = 16_000.0;

/// What one dictation returned and when, all measured from the moment its Begin was sent unless noted.
pub struct Outcome {
    pub text: String,
    /// Arrival of each Partial (a clause) since Begin, in order.
    pub partials: Vec<(Duration, String)>,
    /// Time from sending End to receiving the final text.
    pub stop_to_text: Duration,
    /// Time from Begin to the final text.
    pub total: Duration,
}

/// The engine spec JSON a family name stands for: the same wiring as the catalog rows (Transducer: Parakeet, Canary, Whisper, Streaming: Nemotron).
pub fn spec_json(family: &str) -> String {
    let (adapter, joiner, model_type, language) = match family {
        "Transducer" => (r#"{"Segmented":"Transducer"}"#, true, r#""nemo_transducer""#, "null"),
        "Canary" => (r#"{"Segmented":"Canary"}"#, false, "null", r#""en""#),
        "Whisper" => (r#"{"Segmented":"Whisper"}"#, false, "null", r#""""#),
        "Streaming" => (r#"{"Streaming":"Transducer"}"#, true, r#""nemotron""#, "null"),
        other => panic!("unknown family {other}: Transducer, Canary, Whisper or Streaming"),
    };
    let joiner = if joiner { r#""joiner.int8.onnx""# } else { "null" };
    format!(
        r#"{{"adapter":{adapter},"encoder":"encoder.int8.onnx","decoder":"decoder.int8.onnx","joiner":{joiner},"model_type":{model_type},"language":{language}}}"#
    )
}

/// One pipe frame: `[u32 json_len][json][u32 sample_count][f32 samples]`, little endian.
fn frame(json: &str, samples: &[f32]) -> Vec<u8> {
    let mut out = (json.len() as u32).to_le_bytes().to_vec();
    out.extend_from_slice(json.as_bytes());
    out.extend_from_slice(&(samples.len() as u32).to_le_bytes());
    out.extend(samples.iter().flat_map(|s| s.to_le_bytes()));
    out
}

/// Reads one reply frame. Returns None when the worker closed its pipe.
fn read_reply(out: &mut impl Read) -> Option<serde_json::Value> {
    let mut len = [0u8; 4];
    out.read_exact(&mut len).ok()?;
    let mut json = vec![0u8; u32::from_le_bytes(len) as usize];
    out.read_exact(&mut json).ok()?;
    out.read_exact(&mut len).ok()?;
    serde_json::from_slice(&json).ok()
}

/// Reads a WAV file from disk as 16 kHz mono samples. Input: the path. Panics, naming the file, when it is missing or not 16 kHz mono 16-bit PCM.
pub fn read_wav(path: &str) -> Vec<f32> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    wav::wav_samples(&bytes).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// A Windows job object holding the worker and anything it starts; its peak is the largest committed private memory of the whole tree at one moment.
#[cfg(windows)]
struct Job(windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for Job {
    /// Closes the job handle.
    fn drop(&mut self) {
        let _ = unsafe { windows::Win32::Foundation::CloseHandle(self.0) };
    }
}

pub struct Worker {
    #[cfg(windows)]
    job: Job,
    pub child: Child,
    pub input: ChildStdin,
    replies: Receiver<(Instant, serde_json::Value)>,
    /// How long the worker says it took to load, and how long spawn plus Load took from the outside (cold start as a user waits for it).
    pub reported_load_ms: u64,
    pub load: Duration,
}

impl Worker {
    /// Starts a worker and loads a model. Inputs: the model folder (with `silero_vad.onnx` beside the model files for the segmented families),
    /// the family name (see `spec_json`) and the inference thread count. Panics when the worker refuses the model.
    pub fn start(model: &str, family: &str, threads: i32) -> Worker {
        let spawned = Instant::now();
        let mut child = Command::new(env!("CARGO_BIN_EXE_cue"))
            .arg("--speech-worker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("start worker");
        #[cfg(windows)]
        let job = Job::holding(&child);
        let (mut input, mut output) = (child.stdin.take().unwrap(), child.stdout.take().unwrap());
        let dir = serde_json::to_string(&model.replace('\\', "/")).unwrap();
        let vad = serde_json::to_string(&format!("{}/silero_vad.onnx", model.replace('\\', "/"))).unwrap();
        let load =
            format!(r#"{{"type":"Load","dir":{dir},"vad":{vad},"spec":{},"threads":{threads},"version":{VERSION}}}"#, spec_json(family));
        input.write_all(&frame(&load, &[])).unwrap();
        let loaded = read_reply(&mut output).expect("worker reply to Load");
        assert_eq!(loaded["type"], "Loaded", "{loaded}");
        let load = spawned.elapsed();
        let (tx, replies) = channel();
        std::thread::spawn(move || {
            while let Some(reply) = read_reply(&mut output) {
                if tx.send((Instant::now(), reply)).is_err() {
                    break;
                }
            }
        });
        Worker {
            #[cfg(windows)]
            job,
            child,
            input,
            replies,
            reported_load_ms: loaded["ms"].as_u64().unwrap_or(0),
            load,
        }
    }

    /// Sends one frame to the worker.
    fn send(&mut self, json: &str, samples: &[f32]) {
        self.input.write_all(&frame(json, samples)).unwrap();
    }

    /// One dictation: Begin, the audio in 200 ms pieces, End. Inputs: its id, the audio and `paced`: send each piece when it would have been
    /// recorded (so first-caption and stop-to-text are what a user waits) instead of as fast as the pipe takes it (so the decode speed shows).
    /// Returns the text and the timings.
    pub fn dictate(&mut self, id: u64, audio: &[f32], paced: bool) -> Outcome {
        while self.replies.try_recv().is_ok() {}
        let begun = Instant::now();
        self.send(&format!(r#"{{"type":"Begin","id":{id},"clause":0}}"#), &[]);
        for (n, piece) in audio.chunks(PIECE).enumerate() {
            if paced {
                let due = begun + Duration::from_secs_f64(((n + 1) * PIECE) as f64 / RATE);
                std::thread::sleep(due.saturating_duration_since(Instant::now()));
            }
            self.send(r#"{"type":"Audio"}"#, piece);
        }
        let stopped = Instant::now();
        self.send(&format!(r#"{{"type":"End","id":{id}}}"#), &[]);
        self.collect(id, begun, stopped)
    }

    /// Reads replies until the dictation's final text. Inputs: its id, when Begin and End were sent. Panics on an Error or a reply for another id.
    fn collect(&mut self, id: u64, begun: Instant, stopped: Instant) -> Outcome {
        let mut partials = Vec::new();
        loop {
            let (at, reply) = self.replies.recv_timeout(Duration::from_secs(600)).expect("a reply within 10 minutes");
            match reply["type"].as_str() {
                Some("Release") => {}
                Some("Partial") => {
                    assert_eq!(reply["id"], id, "{reply}");
                    partials.push((at - begun, reply["text"].as_str().unwrap().to_string()));
                }
                Some("Text") => {
                    assert_eq!(reply["id"], id, "{reply}");
                    return Outcome {
                        text: reply["text"].as_str().unwrap().to_string(),
                        partials,
                        stop_to_text: at.saturating_duration_since(stopped),
                        total: at - begun,
                    };
                }
                _ => panic!("unexpected reply {reply}"),
            }
        }
    }

    /// One dictation sent as fast as the pipe takes it; returns the final text.
    pub fn utterance(&mut self, id: u64, audio: &[f32]) -> String {
        self.dictate(id, audio, false).text
    }

    /// Closes the worker's input and waits for it to exit.
    pub fn finish(mut self) {
        drop(self.input);
        let _ = self.child.wait();
    }
}

#[cfg(windows)]
impl Job {
    /// Puts a freshly spawned child into a new job object. Input: the child. Panics when the job cannot be made (the bench would report no tree peak).
    fn holding(child: &Child) -> Job {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::JobObjects::{AssignProcessToJobObject, CreateJobObjectW};
        let job = Job(unsafe { CreateJobObjectW(None, None) }.expect("create job object"));
        unsafe { AssignProcessToJobObject(job.0, HANDLE(child.as_raw_handle())) }.expect("assign worker to job");
        job
    }

    /// Peak committed private memory of every process the job has held, at one moment, in MB.
    fn peak_mb(&self) -> f64 {
        use windows::Win32::System::JobObjects::{
            JobObjectExtendedLimitInformation, QueryInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        };
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        let size = std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;
        unsafe { QueryInformationJobObject(Some(self.0), JobObjectExtendedLimitInformation, &mut info as *mut _ as *mut _, size, None) }
            .expect("query job object");
        info.PeakJobMemoryUsed as f64 / 1e6
    }
}

#[cfg(windows)]
impl Worker {
    /// Peak private memory of the worker and everything it started, over its whole life so far, in MB (sampled by the system, so a short spike during decode is caught).
    pub fn peak_tree_mb(&self) -> f64 {
        self.job.peak_mb()
    }

    /// Private memory of the worker process in MB (what Task Manager calls "Memory").
    pub fn private_mb(&self) -> f64 {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
        let mut counters = PROCESS_MEMORY_COUNTERS_EX::default();
        let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
        let _ = unsafe { K32GetProcessMemoryInfo(HANDLE(self.child.as_raw_handle()), &mut counters as *mut _ as *mut _, size) };
        counters.PrivateUsage as f64 / 1e6
    }

    /// Processor time the worker has used so far (user plus kernel), in seconds.
    pub fn cpu_secs(&self) -> f64 {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::Foundation::{FILETIME, HANDLE};
        use windows::Win32::System::Threading::GetProcessTimes;
        let (mut created, mut exited, mut kernel, mut user) =
            (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        let read = unsafe { GetProcessTimes(HANDLE(self.child.as_raw_handle()), &mut created, &mut exited, &mut kernel, &mut user) };
        let ticks = |t: FILETIME| ((u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime)) as f64 * 100e-9;
        if read.is_ok() {
            ticks(kernel) + ticks(user)
        } else {
            0.0
        }
    }
}
