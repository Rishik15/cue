//! File: speech/tests.rs
//! Purpose: Protect the speech pipeline's pure parts: the pipe format to the worker survives hostile bytes, a take keeps only the audio the worker has not released and ignores a clause it already has, streaming text is cut into safe clauses, and the model list is sound.
//! Contents: tests of `protocol`, `take`, `phrase` and `catalog`; the session rules are in `session_tests`.

use std::collections::HashSet;

use super::catalog::{all, find, Model, DEFAULT, LIGHT, VAD};
use super::phrase::Phrases;
use super::protocol::{read_frame, write_frame, Adapter, EngineSpec, Kind, Reply, Request, VERSION};
use super::take::Take;
use crate::audio::RATE;

/// A request and its samples cross the pipe unchanged, several frames in a row, and a closed pipe between frames is a clean end.
#[test]
fn frames_cross_the_pipe_unchanged_and_end_cleanly() {
    let mut pipe = Vec::new();
    let load = Request::Load {
        dir: "C:\\models\\a".into(),
        vad: "v.onnx".into(),
        spec: EngineSpec::int8(Kind::Segmented, Adapter::OfflineTransducer, true, Some("nemo_transducer"), None),
        threads: 4,
        version: VERSION,
    };
    write_frame(&mut pipe, &load, &[]).expect("write");
    write_frame(&mut pipe, &Request::Audio, &[0.5, -0.25, f32::MIN_POSITIVE]).expect("write");
    write_frame(&mut pipe, &Reply::Partial { id: 7, index: 2, text: "h\u{e9}llo".into() }, &[]).expect("write");
    let mut input = pipe.as_slice();
    assert_eq!(read_frame::<Request>(&mut input).expect("read"), Some((load, vec![])));
    assert_eq!(read_frame::<Request>(&mut input).expect("read"), Some((Request::Audio, vec![0.5, -0.25, f32::MIN_POSITIVE])));
    assert_eq!(
        read_frame::<Reply>(&mut input).expect("read").map(|f| f.0),
        Some(Reply::Partial { id: 7, index: 2, text: "h\u{e9}llo".into() })
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

/// After a worker crash, only the audio the worker released is dropped: the replay carries the rest, numbers its clauses after the released
/// ones, does not repeat a clause already typed, and the worker's sample count restarts where the replay starts.
#[test]
fn a_crash_replays_only_what_was_not_released() {
    let mut take = Take::new(3);
    take.push(&[0.1; 10]);
    assert_eq!(take.commit(0, "hello".into()), Some(0));
    take.release(4);
    assert_eq!(take.commit(1, "big".into()), Some(1), "a clause found after the release is kept but its audio is too");
    let replay = take.replay(true);
    assert_eq!(
        replay.iter().map(|(r, s)| (r, s.len())).collect::<Vec<_>>(),
        [(&Request::Begin { id: 3, clause: 1 }, 0), (&Request::Audio, 6), (&Request::End { id: 3 }, 0)]
    );
    assert_eq!(take.commit(1, "other".into()), None, "the same clause again is ignored: the first text stands");
    assert_eq!(take.commit(2, "world".into()), Some(2));
    take.release(2);
    assert_eq!(take.text(), "hello big world");
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
        assert_engine_matches_files(model);
    }
    assert!(find(DEFAULT).is_some() && find(LIGHT).is_some());
    assert!(find("no-such-model").is_none());
}

/// A row's engine spec agrees with its files: a streaming model ships as an archive and uses the streaming config, a segmented one the
/// offline configs, and every file role the spec names exists among the files.
fn assert_engine_matches_files(model: &Model) {
    let spec = &model.engine;
    assert_eq!(spec.kind == Kind::Streaming, model.archive.is_some(), "{}", model.id);
    assert_eq!(spec.kind == Kind::Streaming, spec.adapter == Adapter::OnlineTransducer, "{}", model.id);
    let has = |suffix: &str| model.files.iter().any(|f| f.name.ends_with(suffix));
    assert!(has(&spec.encoder) && has(&spec.decoder) && has("tokens.txt"), "{}", model.id);
    assert_eq!(spec.joiner.as_ref().is_some_and(|j| has(j)), spec.joiner.is_some(), "{}", model.id);
}
