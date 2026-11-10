//! File: actions/tests/perf.rs
//! Purpose: Measure the exact text actions on typical and on large Unicode input, so an allocation-heavy transform is fixed only when a number shows it costs something. Ignored by default: timings only mean something in a release build.
//! Contents: typical input p95 under 1 ms for every exact action; large input growing no faster than linearly; the sample builders.
//! Run: `cargo test --release perf -- --ignored --nocapture` (on Windows through `scripts/cue.ps1`); each test prints p50 and p95 per action.

use std::time::{Duration, Instant};

use crate::actions::Action;

/// The p95 every exact action must stay under on typical input.
const TYPICAL_P95: Duration = Duration::from_millis(1);

/// A paragraph with what real clipboard text has: Unicode letters, curly quotes, links with tracking, an email, a phone number, a list, a repeated word, fillers and a long line.
const SAMPLE: &str = "um so the the meeting with Zo\u{eb} at caf\u{e9} \u{201c}Nord\u{201d} moved to 3pm, see https://example.com/a?utm_source=x&id=2 or mail bob@acme.com / call +1 555 123 4567.\n\n- one\n- two\n- two\n\n\u{4e2d}\u{6587}\u{6587}\u{672c}  \u{3067}\u{3059}\u{3002}   Total twenty five dollars; MIXED Case Words here\u{2026}\n";

/// About 700 characters of typical text.
fn typical() -> String {
    SAMPLE.repeat(3)
}

/// The exact actions: every action that does not run the model.
fn exact() -> impl Iterator<Item = Action> {
    Action::all().filter(|a| !a.needs_model())
}

/// Times `runs` runs of `action` over `text`. Inputs: the action, the text and the run count. Returns the sorted durations.
fn time(action: Action, text: &str, runs: usize) -> Vec<Duration> {
    let mut times: Vec<Duration> = (0..runs)
        .map(|_| {
            let start = Instant::now();
            std::hint::black_box(action.apply(std::hint::black_box(text)));
            start.elapsed()
        })
        .collect();
    times.sort();
    times
}

/// The value at quantile `q` (0 to 1) of sorted durations.
fn quantile(sorted: &[Duration], q: f64) -> Duration {
    sorted[((sorted.len() - 1) as f64 * q).round() as usize]
}

/// Every exact action answers typical input within a millisecond at p95.
#[test]
#[ignore = "timing: run in release with --ignored --nocapture"]
fn typical_input_is_under_a_millisecond_at_p95() {
    let text = typical();
    let mut slow = Vec::new();
    for action in exact() {
        time(action, &text, 20);
        let times = time(action, &text, 400);
        let (p50, p95) = (quantile(&times, 0.5), quantile(&times, 0.95));
        println!("{:<22} p50 {:>9.1?}  p95 {:>9.1?}", action.id(), p50, p95);
        if p95 >= TYPICAL_P95 {
            slow.push(action.id());
        }
    }
    assert!(slow.is_empty(), "p95 over 1 ms: {slow:?}");
}

/// Four times the input costs at most about eight times the time (a small floor absorbs timer noise): no action is quadratic.
#[test]
#[ignore = "timing: run in release with --ignored --nocapture"]
fn large_input_grows_no_faster_than_linearly() {
    let small = SAMPLE.repeat(300);
    let large = SAMPLE.repeat(1200);
    let mut worse = Vec::new();
    for action in exact() {
        let (one, four) = (quantile(&time(action, &small, 5), 0.5), quantile(&time(action, &large, 5), 0.5));
        println!("{:<22} {} chars {:>9.1?}   {} chars {:>9.1?}", action.id(), small.chars().count(), one, large.chars().count(), four);
        if four > one * 8 + Duration::from_millis(2) {
            worse.push(action.id());
        }
    }
    assert!(worse.is_empty(), "grows faster than linearly: {worse:?}");
}
