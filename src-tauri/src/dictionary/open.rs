//! File: dictionary/open.rs
//! Purpose: Opening the window for a decided lookup: the card beside the selection, the search window beside the pointer, and handing the session and the open event to the window.
//! Contents: RECENT / OPEN_EVENT — the words the search lists and the page's event; on_display — the monitor the lines are on; open_card / open_search — build the session; search_place — where the search window opens; publish — stores the session and sends the event once the page is ready; send — the event itself.

use serde_json::json;
use tauri::AppHandle;

use super::book::Entry;
use super::flow::say;
use super::lookup::Selected;
use super::state::{Anchor, Mode, Session, State};
use super::{installed_book, label, placement, saved, state};

/// The open event the page receives.
const OPEN_EVENT: &str = "dict-open";
/// How many recent words the empty search window lists.
const RECENT: usize = 8;

/// The monitor the lines are on (the one they overlap most, else the nearest) and the lines cut to its work area. Input: the lines. Return: the lines, the work area and the scale, None when Windows lists no monitor.
fn on_display(lines: &[crate::platform::Rect]) -> Option<(Vec<crate::platform::Rect>, crate::platform::Rect, f32)> {
    let displays = crate::platform::displays();
    let works: Vec<_> = displays.iter().map(|d| d.0).collect();
    let (work, scale) = displays[placement::choose_display(lines, &works)?];
    Some((placement::clip_lines(lines, work), work, scale))
}

/// Opens the Card for a selected word: its Entry, the place of the selection, one session; the Entry is recorded in Looked Up once the Card is published. Says why when the selection cannot be found on the screen. Input: the app, the lookup's number, the word's window, the selection and its Entry.
pub(super) fn open_card(app: &AppHandle, id: u64, source: isize, selected: &Selected, entry: Entry) {
    let caret = crate::platform::caret_in(source);
    let Some((lines, kind)) = placement::anchor(&selected.lines, caret, crate::platform::pointer_box()) else {
        return say(id, "Could not find the selection on the screen");
    };
    let Some((lines, work, scale)) = on_display(&lines) else { return say(id, "Could not find the selection on the screen") };
    let anchor = Anchor { lines, kind, rtl: placement::is_rtl(&selected.text) };
    let open = json!({ "session": id, "mode": "card", "theme": crate::settings::get_theme(), "entry": entry });
    let session = Session { id, mode: Mode::Card, anchor: Some(anchor), work, scale, source, placed: None, open: Some(open) };
    if publish(app, id, session) {
        saved::record(app, &entry);
    }
}

/// Opens the search window next to the pointer, listing the words looked up lately: below and to the right of it (above or to its left when there is no room) on the pointer's monitor, as a card with no arrow; in the middle of the monitor the user is working on only when the pointer cannot be read. Input: the app, the lookup's number and the window the shortcut was pressed in.
pub(super) fn open_search(app: &AppHandle, id: u64, source: isize) {
    let Some((anchor, work, scale)) = search_place(source) else {
        crate::log::note("dictionary: the search window has no screen");
        return say(id, "Could not find the screen");
    };
    let pack = !installed_book(app).is_empty();
    let recent = saved::recent(app, RECENT);
    let open = json!({ "session": id, "mode": "search", "theme": crate::settings::get_theme(), "recent": recent, "pack": pack });
    let session = Session { id, mode: Mode::Search, anchor, work, scale, source, placed: None, open: Some(open) };
    publish(app, id, session);
}

/// Where the search window opens: the box round the pointer as a pointer anchor on the pointer's monitor, else no anchor (centred) on the monitor of the shortcut's window or the first one. Input: the window the shortcut was pressed in. Return: the anchor, the work area and the scale; None when Windows lists no monitor. shortcut: the monitor is the work area the pointer's box overlaps most, so a pointer on a taskbar seam between stacked monitors can open Search on the neighbour, resolve the monitor from the pointer point if it is seen.
fn search_place(source: isize) -> Option<(Option<Anchor>, crate::platform::Rect, f32)> {
    if let Some((lines, work, scale)) = crate::platform::pointer_box().and_then(|p| on_display(&[p])) {
        return Some((Some(Anchor { lines, kind: placement::Kind::Cursor, rtl: false }), work, scale));
    }
    let (work, scale) = crate::platform::work_area_of(source).or_else(|| crate::platform::displays().first().copied())?;
    Some((None, work, scale))
}

/// Stores the session and sends the open event once the page is ready; the lookup becomes an open window. A lookup cancelled or replaced meanwhile opens nothing: the check, the change of phase and the store are one step under the lock. Input: the app, the lookup's number and the session (which says its mode). Return: whether the window was opened.
fn publish(app: &AppHandle, id: u64, session: Session) -> bool {
    let mut s = state();
    let Some(ready) = s.publish(id, session) else {
        crate::log::note("dictionary: not published, the lookup was closed or replaced meanwhile");
        return false;
    };
    let at = s.since_press().map_or(0, |t| t.as_millis());
    crate::log::note(&format!("dictionary: window ready at {at} ms, page ready {ready}"));
    if ready {
        send(app, &mut s);
    }
    true
}

/// Sends the open event the caller claimed to the window it is for; a failed send leaves it to be claimed again. Input: the app and the locked state.
pub(super) fn send(app: &AppHandle, s: &mut State) {
    let Some((id, payload)) = s.claim() else { return };
    let at = s.since_press().map_or(0, |t| t.as_millis());
    if !crate::window_guard::send_open(app, "dictionary", &label::label(id), OPEN_EVENT, payload, at) {
        s.unsent();
    }
}
