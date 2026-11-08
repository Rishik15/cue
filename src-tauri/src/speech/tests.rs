//! File: speech/tests.rs
//! Purpose: Protect the speech pipeline's pure parts: the pipe format to the worker survives hostile bytes, a crash replays only the audio not yet recognised, streaming text is cut into safe clauses, and the model list is sound.
//! Contents: tests of `protocol`, `take`, `phrase` and `catalog`.

use std::collections::HashSet;

use super::catalog::{all, find, DEFAULT, LIGHT, VAD};
use super::phrase::Phrases;
use super::protocol::{read_frame, write_frame, Family, Reply, Request};
use super::take::{replay_all, Job, Session, Take};
use crate::audio::RATE;

/// A request and its samples cross the pipe unchanged, several frames in a row, and a closed pipe between frames is a clean end.
#[test]
fn frames_cross_the_pipe_unchanged_and_end_cleanly() {
    let mut pipe = Vec::new();
    let load = Request::Load { dir: "C:\\models\\a".into(), vad: "v.onnx".into(), family: Family::Transducer, threads: 4 };
    write_frame(&mut pipe, &load, &[]).expect("write");
    write_frame(&mut pipe, &Request::Audio, &[0.5, -0.25, f32::MIN_POSITIVE]).expect("write");
    write_frame(&mut pipe, &Reply::Partial { id: 7, index: 2, text: "h\u{e9}llo".into(), end: 9 }, &[]).expect("write");
    let mut input = pipe.as_slice();
    assert_eq!(read_frame::<Request>(&mut input).expect("read"), Some((load, vec![])));
    assert_eq!(read_frame::<Request>(&mut input).expect("read"), Some((Request::Audio, vec![0.5, -0.25, f32::MIN_POSITIVE])));
    assert_eq!(
        read_frame::<Reply>(&mut input).expect("read").map(|f| f.0),
        Some(Reply::Partial { id: 7, index: 2, text: "h\u{e9}llo".into(), end: 9 })
    );
    assert!(read_frame::<Request>(&mut input).expect("end").is_none());
}

/// A corrupt or hostile pipe fails with an error instead of allocating gigabytes, hanging or returning half a frame.
#[test]
fn hostile_frames_are_refused_before_any_big_allocation() {
    let huge_header = u32::MAX.to_le_bytes();
    assert_eq!(read_frame::<Request>(&mut huge_header.as_slice()).expect_err("big header").to_string(), "header too large");
    let json = br#"{"type":"Audio"}"#;
    let frame = |count: u32, body: &[u8]| [&(json.len() as u32).to_le_bytes()[..], json, &count.to_le_bytes(), body].concat();
    assert_eq!(read_frame::<Request>(&mut frame(u32::MAX, &[]).as_slice()).expect_err("big audio").to_string(), "audio too long");
    assert!(read_frame::<Request>(&mut frame(4, &[0; 8]).as_slice()).is_err(), "audio cut short");
    let mut bad_json = [&5u32.to_le_bytes()[..], b"{bad}", &0u32.to_le_bytes()].concat();
    assert!(read_frame::<Request>(&mut bad_json.as_slice()).is_err());
    bad_json.truncate(6);
    assert!(read_frame::<Request>(&mut bad_json.as_slice()).is_err(), "header cut short");
    let unknown = [&16u32.to_le_bytes()[..], br#"{"type":"Reboot"}"#.as_slice(), &0u32.to_le_bytes()].concat();
    assert!(read_frame::<Request>(&mut unknown.as_slice()).is_err());
}

/// After a worker crash, only the audio not yet recognised is replayed, the sentences already typed are not repeated, and the worker's sample count restarts where the replay starts.
#[test]
fn a_crash_replays_only_what_was_not_recognised() {
    let mut take = Take::new(3);
    take.push(&[0.1; 10]);
    assert_eq!(take.commit("hello".into(), 4), 0);
    let replay = take.replay(true);
    assert_eq!(
        replay.iter().map(|(r, s)| (r, s.len())).collect::<Vec<_>>(),
        [(&Request::Begin { id: 3 }, 0), (&Request::Audio, 6), (&Request::End { id: 3 }, 0)]
    );
    assert_eq!(take.commit("world".into(), 2), 1, "the new worker counts from 0 again");
    assert_eq!(take.text(), "hello world");
    assert_eq!(take.replay(false).len(), 2, "a recording still going on is not ended");
    assert!((take.seconds() - 4.0 / RATE as f32).abs() < f32::EPSILON);
}

/// A long recording keeps only the latest minute of unrecognised audio, so memory stays flat.
#[test]
fn unrecognised_audio_is_bounded_however_long_the_recording() {
    let mut take = Take::new(1);
    let second = vec![0.0; RATE as usize];
    for _ in 0..600 {
        take.push(&second);
    }
    assert!(take.seconds() <= 65.0 && take.seconds() >= 60.0, "{}", take.seconds());
}

/// Every unfinished dictation is replayed to a fresh worker: the finished ones are ended, the one still recorded is not.
#[test]
fn replay_all_covers_finished_parked_and_current_takes() {
    let job = |id| Job { take: Take::new(id), done: Box::new(|_| {}) };
    let session = |id| Session { take: Take::new(id), failed: None };
    let mut running = std::collections::VecDeque::from([job(1)]);
    let mut parked = [session(2)];
    let mut current = session(3);
    let requests = replay_all(&mut running, &mut parked, Some(&mut current));
    let ends: Vec<_> = requests.iter().filter_map(|(r, _)| matches!(r, Request::End { .. }).then_some(r)).collect();
    assert_eq!(ends, [&Request::End { id: 1 }]);
    let begins: Vec<_> = requests.iter().filter_map(|(r, _)| if let Request::Begin { id } = r { Some(*id) } else { None }).collect();
    assert_eq!(begins, [1, 2, 3]);
}

/// Streaming text is handed out in clauses once settled: at sentence ends, at commas after four words, and at ten words, never the last word that can still grow.
#[test]
fn settled_words_leave_in_clauses_and_the_last_word_waits() {
    let mut phrases = Phrases::default();
    assert_eq!(phrases.next("Hello there. How are you", false), ["Hello there."]);
    assert!(phrases.next("Hello there. How are you", false).is_empty(), "nothing is handed out twice");
    assert_eq!(phrases.next("Hello there. How are you doing today.", true), ["How are you doing today."]);
    phrases.reset();
    assert_eq!(phrases.next("one two three four, five six", false), ["one two three four,"]);
    phrases.reset();
    assert_eq!(phrases.next("a b, c d e f g h i j k l", false), ["a b, c d e f g h i j"]);
    phrases.reset();
    assert_eq!(phrases.next("so and then", true), ["so and then"]);
    assert!(phrases.next("so", false).is_empty(), "a shorter hypothesis hands out nothing");
}

/// The model list is sound: ids are unique, every file has a size and a SHA-256, the defaults exist and ask for one download each.
#[test]
fn the_model_list_is_sound() {
    let mut seen = HashSet::new();
    for model in all() {
        assert!(seen.insert(model.id), "duplicate id {}", model.id);
        assert!(model.total_size() > 0 && (1..=5).contains(&model.accuracy) && (1..=5).contains(&model.speed), "{}", model.id);
        let files = model.files.iter().chain(model.archive.iter().map(|a| &a.file)).chain(std::iter::once(&VAD));
        for file in files {
            assert!(
                file.size > 0 && file.sha256.len() == 64 && file.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                "{} {}",
                model.id,
                file.name
            );
        }
        for file in model.files {
            assert!(model.url(file).starts_with("https://huggingface.co/"), "{}", model.id);
        }
        assert_eq!(find(model.id).map(|m| m.id), Some(model.id));
    }
    assert!(find(DEFAULT).is_some() && find(LIGHT).is_some());
    assert!(find("no-such-model").is_none());
}
