//! Purpose: Helpers shared by the worker integration tests: the pipe protocol from the outside, a worker process handle, WAV reading.
//! Contents: Worker — start / send / utterance; wav_samples — 16 kHz mono 16-bit WAV to f32.

#![allow(dead_code)] // each test file uses a different part of these helpers

use std::io::{Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

fn frame(json: &str, samples: &[f32]) -> Vec<u8> {
    let mut out = (json.len() as u32).to_le_bytes().to_vec();
    out.extend_from_slice(json.as_bytes());
    out.extend_from_slice(&(samples.len() as u32).to_le_bytes());
    out.extend(samples.iter().flat_map(|s| s.to_le_bytes()));
    out
}

fn read_reply(out: &mut impl Read) -> serde_json::Value {
    let mut len = [0u8; 4];
    out.read_exact(&mut len).expect("reply length");
    let mut json = vec![0u8; u32::from_le_bytes(len) as usize];
    out.read_exact(&mut json).expect("reply body");
    out.read_exact(&mut len).expect("sample count");
    serde_json::from_slice(&json).expect("reply json")
}

pub fn wav_samples(bytes: &[u8]) -> Vec<f32> {
    let at = bytes.windows(4).position(|w| w == b"data").expect("data chunk") + 8;
    bytes[at..].as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b) as f32 / 32768.0).collect()
}

pub struct Worker {
    pub child: Child,
    pub input: ChildStdin,
    output: ChildStdout,
}

impl Worker {
    pub fn start(model: &str, family: &str) -> Worker {
        let mut child = Command::new(env!("CARGO_BIN_EXE_cue"))
            .arg("--speech-worker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("start worker");
        let (input, output) = (child.stdin.take().unwrap(), child.stdout.take().unwrap());
        let mut worker = Worker { child, input, output };
        let dir = serde_json::to_string(&model.replace('\\', "/")).unwrap();
        let vad = serde_json::to_string(&format!("{}/silero_vad.onnx", model.replace('\\', "/"))).unwrap();
        worker.send(&format!(r#"{{"type":"Load","dir":{dir},"vad":{vad},"family":"{family}","threads":2}}"#), &[]);
        assert_eq!(read_reply(&mut worker.output)["type"], "Loaded");
        worker
    }

    fn send(&mut self, json: &str, samples: &[f32]) {
        self.input.write_all(&frame(json, samples)).unwrap();
    }

    /// One utterance, streamed in 200 ms pieces; returns the text and the sentences reported as partial results on the way.
    pub fn utterance_with_partials(&mut self, id: u64, audio: &[f32]) -> (String, Vec<String>) {
        self.send(&format!(r#"{{"type":"Begin","id":{id}}}"#), &[]);
        for piece in audio.chunks(3_200) {
            self.send(r#"{"type":"Audio"}"#, piece);
        }
        let stopped = std::time::Instant::now();
        self.send(&format!(r#"{{"type":"End","id":{id}}}"#), &[]);
        let mut partials = Vec::new();
        loop {
            let reply = read_reply(&mut self.output);
            assert_eq!(reply["id"], id, "{reply}");
            match reply["type"].as_str() {
                Some("Partial") => {
                    assert_eq!(reply["index"], partials.len(), "partials arrive in order: {reply}");
                    partials.push(reply["text"].as_str().unwrap().to_string());
                }
                Some("Text") => {
                    eprintln!(
                        "utterance {id}: {:.1} s of audio, text {} ms after stop",
                        audio.len() as f32 / 16_000.0,
                        stopped.elapsed().as_millis()
                    );
                    return (reply["text"].as_str().unwrap().to_string(), partials);
                }
                _ => panic!("unexpected reply {reply}"),
            }
        }
    }

    pub fn utterance(&mut self, id: u64, audio: &[f32]) -> String {
        self.utterance_with_partials(id, audio).0
    }
}

#[cfg(windows)]
impl Worker {
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
}
