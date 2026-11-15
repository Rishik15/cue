//! File: strip/tests.rs
//! Purpose: Protect the window geometry that crosses the page boundary (dip and physical pixels, refusing a bad or far request, keeping the card on the work area, a negative work-area origin, windows that share a corner sharing it in pixels) and the session state (the open event sent exactly once, another session's command finding nothing, cancel renewing the epoch, the first good placement showing, Paste delivering Rust's text of the run it names, a late run never replacing the accepted one, results left out for a long text).
//! Contents: tests of `bounds::to_dip`, `bounds::to_physical` and `state`.

use std::time::Instant;

use super::bounds::{to_dip, to_physical, Dip, MAX_OFFSET, MAX_SIDE};
use super::state::{rows as open_rows, Session, State, EXACT_LIMIT};
use crate::palette::{Group, Item};
use crate::platform::{Rect, Target};

/// A work area at (100, 50), 1000 by 800 physical pixels.
const WORK: Rect = Rect { left: 100, top: 50, right: 1100, bottom: 850 };

/// The four sides of a rectangle, to compare.
fn sides(r: Rect) -> (i32, i32, i32, i32) {
    (r.left, r.top, r.right, r.bottom)
}

/// A selection is handed over in dip relative to the work area, and a window asked for in dip comes back in physical pixels at the same place.
#[test]
fn geometry_round_trips_through_dip() {
    let selection = Rect { left: 400, top: 350, right: 700, bottom: 380 };
    assert_eq!(to_dip(selection, WORK, 1.5), Dip { x: 200.0, y: 200.0, w: 200.0, h: 20.0 });
    let window = to_physical(Dip { x: 200.0, y: 200.0, w: 200.0, h: 20.0 }, WORK, 1.5).unwrap();
    assert_eq!(sides(window), sides(selection));
}

/// A request that is not finite, empty, negative or larger than `MAX_SIDE` is refused.
#[test]
fn a_bad_request_is_refused() {
    for bad in [f64::NAN, f64::INFINITY, 0.0, -4.0, MAX_SIDE + 1.0] {
        assert!(to_physical(Dip { x: 0.0, y: 0.0, w: bad, h: 40.0 }, WORK, 1.0).is_none(), "{bad}");
        assert!(to_physical(Dip { x: 0.0, y: 0.0, w: 40.0, h: bad }, WORK, 1.0).is_none(), "{bad}");
    }
    assert!(to_physical(Dip { x: f64::NAN, y: 0.0, w: 40.0, h: 40.0 }, WORK, 1.0).is_none());
}

/// A window asked for far off the work area is moved back until its middle is on it, keeping its size; one inside it is left alone.
#[test]
fn the_window_stays_on_the_work_area() {
    let far = to_physical(Dip { x: 5000.0, y: -5000.0, w: 400.0, h: 200.0 }, WORK, 1.0).unwrap();
    assert_eq!(sides(far), (1100 - 200, 50 - 100, 1100 + 200, 50 + 100));
    let inside = to_physical(Dip { x: 10.0, y: 20.0, w: 400.0, h: 200.0 }, WORK, 1.0).unwrap();
    assert_eq!(sides(inside), (110, 70, 510, 270));
}

/// A corner far past any screen is refused instead of overflowing the physical sums.
#[test]
fn a_far_corner_is_refused() {
    for far in [1e12, -1e12, MAX_OFFSET + 1.0] {
        assert!(to_physical(Dip { x: far, y: 0.0, w: 40.0, h: 40.0 }, WORK, 2.0).is_none(), "{far}");
        assert!(to_physical(Dip { x: 0.0, y: far, w: 40.0, h: 40.0 }, WORK, 2.0).is_none(), "{far}");
    }
    assert!(to_physical(Dip { x: MAX_OFFSET, y: -MAX_OFFSET, w: MAX_SIDE, h: MAX_SIDE }, WORK, 4.0).is_some());
}

/// A work area left of and above the primary monitor (negative origin) hands the selection over relative to itself.
#[test]
fn a_negative_work_area_is_measured_from_its_corner() {
    let work = Rect { left: -1920, top: -200, right: 0, bottom: 880 };
    let at = Rect { left: -1720, top: 100, right: -1520, bottom: 130 };
    assert_eq!(to_dip(at, work, 2.0), Dip { x: 100.0, y: 150.0, w: 100.0, h: 15.0 });
    let back = to_physical(Dip { x: 100.0, y: 150.0, w: 100.0, h: 15.0 }, work, 2.0).unwrap();
    assert_eq!(sides(back), sides(at));
}

/// The page replays strip, list, result, list, strip as windows that share one corner in dip and differ only in size (the larger card while easing, then the trim); at every common scale, on a monitor with a negative origin and from a selection whose dip corner is fractional (a physical pixel over the scale, as `to_dip` sends it, less the 4 dip margin), every window keeps the same physical corner and its own rounded size, so the card inside it never moves by a rounding pixel.
#[test]
fn windows_sharing_a_corner_share_it_in_pixels() {
    let work = Rect { left: -2400, top: -300, right: 0, bottom: 1000 };
    let sizes = [(438.0, 44.0), (438.0, 368.0), (388.0, 368.0), (388.0, 188.0), (388.0, 368.0), (438.0, 368.0), (438.0, 44.0)];
    for scale in [1.0_f32, 1.25, 1.5, 1.75, 2.0] {
        let s = f64::from(scale);
        let (x, y) = (1723.0 / s - 4.0, 517.0 / s - 4.0);
        let first = to_physical(Dip { x, y, w: sizes[0].0, h: sizes[0].1 }, work, scale).unwrap();
        for (w, h) in sizes {
            let r = to_physical(Dip { x, y, w, h }, work, scale).unwrap();
            assert_eq!((r.left, r.top), (first.left, first.top), "scale {scale} size {w}x{h}");
            assert_eq!((r.right - r.left, r.bottom - r.top), ((w * s).round() as i32, (h * s).round() as i32), "scale {scale}");
        }
    }
}

/// A session numbered `id` over rows with these (id, exact result) pairs.
fn session(id: u64, rows: &[(&str, Option<&str>)], text: &str) -> Session {
    let items = rows
        .iter()
        .map(|(row, result)| Item {
            id: row.to_string(),
            name: row.to_string(),
            hint: String::new(),
            group: Group::Clean,
            model: result.is_none(),
            diff: false,
            result: result.map(str::to_string),
        })
        .collect();
    Session {
        id,
        epoch: id,
        items,
        chips: Vec::new(),
        text: text.to_string(),
        program: String::new(),
        target: Target::default(),
        work: WORK,
        scale: 1.0,
        anchor: WORK,
        results: Vec::new(),
        pressed: Instant::now(),
        shown: false,
    }
}

/// The open event is claimed exactly once whichever side comes first, again after a failed send, and again for a new session.
#[test]
fn the_open_event_goes_out_exactly_once() {
    let mut page_first = State::CLOSED;
    assert!(!page_first.page_ready());
    assert!(page_first.store(session(1, &[], "a")));
    assert!(!page_first.page_ready());
    let mut text_first = State::CLOSED;
    assert!(!text_first.store(session(1, &[], "a")));
    assert!(text_first.page_ready());
    assert!(!text_first.page_ready());
    text_first.unsent();
    assert!(text_first.page_ready());
    assert!(text_first.store(session(2, &[], "a")));
}

/// A command of another session finds nothing: no run, no place, no cancel, no result.
#[test]
fn another_sessions_command_finds_nothing() {
    let mut s = State::CLOSED;
    s.store(session(7, &[("action:a", Some("A"))], "a"));
    assert!(s.session(8).is_none());
    assert!(!s.renew(8, 99));
    assert!(s.place(8, Dip { x: 0.0, y: 0.0, w: 40.0, h: 40.0 }).is_none());
    assert_eq!(s.session(7).map(|x| x.epoch), Some(7));
    assert_eq!(s.session(7).and_then(|x| x.result_of("action:a", 0)), Some("A".into()));
    assert!(s.session(7).is_some_and(|x| !x.has_row("action:other")));
}

/// Cancel moves the session's later model runs to the new palette session.
#[test]
fn cancel_renews_the_epoch() {
    let mut s = State::CLOSED;
    s.store(session(3, &[], "a"));
    assert!(s.renew(3, 40));
    assert_eq!(s.session(3).map(|x| x.epoch), Some(40));
}

/// Only the first good placement shows the window; a bad rectangle before it does not use it up.
#[test]
fn only_the_first_good_placement_shows() {
    let mut s = State::CLOSED;
    s.store(session(5, &[], "a"));
    assert!(s.place(5, Dip { x: 0.0, y: 0.0, w: f64::NAN, h: 40.0 }).is_none());
    assert!(s.place(5, Dip { x: 0.0, y: 0.0, w: 40.0, h: 40.0 }).is_some_and(|(_, first, _)| first));
    assert!(s.place(5, Dip { x: 0.0, y: 0.0, w: 40.0, h: 40.0 }).is_some_and(|(_, first, _)| !first));
}

/// A model result is kept per row and run, Paste finds only the run it names, and an exact result wins over it.
#[test]
fn paste_delivers_what_rust_holds() {
    let mut x = session(1, &[("action:exact", Some("E")), ("action:model", None)], "a");
    assert_eq!(x.result_of("action:model", 1), None);
    assert!(x.publish("action:model".into(), 1, 1, true, "one".into()));
    assert!(x.publish("action:model".into(), 1, 2, true, "two".into()));
    assert_eq!(x.result_of("action:model", 2), Some("two".into()));
    assert_eq!(x.result_of("action:model", 1), None);
    assert!(x.publish("action:exact".into(), 1, 3, true, "other".into()));
    assert_eq!(x.result_of("action:exact", 3), Some("E".into()));
}

/// Run A finishes after a cancel and run B: its late publication is refused, so the row keeps B and Paste naming A finds nothing.
#[test]
fn a_late_run_never_replaces_the_accepted_one() {
    let mut s = State::CLOSED;
    s.store(session(4, &[("action:model", None)], "a"));
    assert!(s.renew(4, 9));
    let x = s.session(4).expect("open");
    assert!(x.publish("action:model".into(), 9, 2, true, "B".into()));
    assert!(!x.publish("action:model".into(), 4, 1, true, "A".into()), "a cancelled epoch is refused");
    assert!(!x.publish("action:model".into(), 9, 1, false, "A".into()), "an older request of the same epoch is refused");
    assert_eq!(x.result_of("action:model", 2), Some("B".into()));
    assert_eq!(x.result_of("action:model", 1), None);
}

/// Exact results travel with the open event only for a text under the limit.
#[test]
fn a_long_text_sends_no_results() {
    let items = session(1, &[("action:a", Some("A"))], "").items;
    let short = serde_json::to_value(open_rows(&items, "short")).unwrap();
    assert_eq!(short[0]["result"], "A");
    assert_eq!(short[0]["diff"], false);
    let long = serde_json::to_value(open_rows(&items, &"x".repeat(EXACT_LIMIT + 1))).unwrap();
    assert!(long[0]["result"].is_null());
    assert_eq!(serde_json::to_value(open_rows(&items, &"x".repeat(EXACT_LIMIT))).unwrap()[0]["result"], "A");
}
