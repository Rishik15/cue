//! File: dictionary/commands.rs
//! Purpose: The dictionary page's commands, one per user action, each bound to the calling window's label and checked against the open session, so a late or foreign call changes nothing: the page is ready for the open event, search a typed prefix, open a word's entry, place the window, close it, open the Dictionary settings page.
//! Contents: dict_ready; dict_search (on the window's kept book); record_while_open; dict_define; dict_seen; dict_place (with Plan / plan / reveal); dict_close; dict_download; Placement — what the page learns from a placement.

use serde::Serialize;
use tauri::{AppHandle, Window};

use super::book::{Entry, Hit};
use super::label::id_of;
use super::placement::{self, Placed, Request, Side};
use super::state::{Download, Mode};
use super::{dismiss_if, installed_book, saved, send, state, window};

/// What a command answers to a page whose window is not the open one.
const CLOSED: &str = "This window is closed.";
/// The most hits a search returns to the page.
const HITS: usize = 5;

/// What the page learns from a placement: which side of the text it is on, where its arrow is along that edge, and the size it was given (smaller than asked when the work area is small: the page scrolls inside).
#[derive(Serialize)]
pub struct Placement {
    side: &'static str,
    arrow: Option<f64>,
    w: f64,
    h: f64,
}

/// The page has registered for the open event: it is sent now if the lookup is already done, else as soon as it is. The window's label names the page, so a page of a window already replaced marks nothing ready. Input: the app and the calling window. Return: none.
#[tauri::command]
pub fn dict_ready(app: AppHandle, window: Window) {
    let Some(id) = id_of(window.label()) else { return };
    let mut s = state();
    let now = s.page_ready(id);
    let at = s.since_press().map_or(0, |t| t.as_millis());
    crate::log::note(&format!("dictionary: page ready at {at} ms, send now {now}"));
    if now {
        send(&app, &mut s);
    }
}

/// The words that start with what was typed, at most five, alphabetical. Input: the calling window and the typed text (untrusted: length and characters are checked by the book). Return: the matches, empty for an empty query; Err for a query that is not a word's start or a window that is not the open one.
#[tauri::command]
pub async fn dict_search(app: AppHandle, window: Window, query: String) -> Result<Vec<Hit>, String> {
    let label = window.label().to_owned();
    let Some(id) = state().caller(&label) else { return Err(CLOSED.into()) };
    let found = crate::snippets::blocking(move || window::book(&app, id).search(&query, HITS)).await;
    if state().caller(&label).is_none() {
        window::release(id);
    }
    found
}

/// The full entry of a word the user opened (Enter or a click), held until the page says it shows it (`dict_seen`). A pack that cannot be read is reported to the user, so it never looks like a word with no entry. Input: the calling window and the word. Return: the entry, None when no pack has it; Err for a window that is not the open one or a damaged pack.
#[tauri::command]
pub async fn dict_define(app: AppHandle, window: Window, word: String) -> Result<Option<Entry>, String> {
    if state().caller(window.label()).is_none() {
        return Err(CLOSED.into());
    }
    let reporter = app.clone();
    let label = window.label().to_owned();
    let found = crate::snippets::blocking(move || {
        let found = installed_book(&app).lookup(&word)?;
        if let Some(entry) = &found {
            record_while_open(&label, || saved::hold(entry.clone()));
        }
        Ok(found)
    })
    .await;
    if let Err(why) = &found {
        crate::status::report(&reporter, why);
    }
    found
}

/// The page shows the entry of `word`: it goes to the Looked Up list. An entry the user left before it showed (Escape, a newer word) was never shown, so it is not recorded. Input: the calling window and the word. Return: none.
#[tauri::command]
pub fn dict_seen(app: AppHandle, window: Window, word: String) {
    let Some(entry) = saved::take_seen(&word) else { return };
    record_while_open(window.label(), || saved::record(&app, &entry));
}

/// Runs `record` only if window `label` is still the open session, checked as late as it can be: a slow lookup that outlived its window records nothing. Input: the caller's window label and the write. Return: whether it ran.
pub(super) fn record_while_open(label: &str, record: impl FnOnce()) -> bool {
    let open = state().caller(label).is_some();
    if open {
        record();
    }
    open
}

/// Where the window goes and what the page is told, worked out under the lock.
struct Plan {
    placed: Placed,
    first: bool,
    card: bool,
    source: isize,
    scale: f64,
}

/// Works out the placement of the open session `id` and records it. Input: the lookup's number and the card's size in dip. Return: the plan; Err for a closed session, a size that is not valid or no room.
fn plan(id: u64, w: f64, h: f64) -> Result<Plan, String> {
    let mut s = state();
    let Some(open) = s.session(id) else { return Err(CLOSED.into()) };
    let scale = f64::from(open.scale);
    let (Some(pw), Some(ph)) = (placement::to_px(w, scale), placement::to_px(h, scale)) else {
        return Err("The size is not valid.".into());
    };
    let placed = match (&open.placed, &open.anchor) {
        (Some(prev), _) => placement::resize(prev, (pw, ph), open.work, scale),
        (None, Some(a)) => placement::place(&Request { lines: &a.lines, kind: a.kind, work: open.work, size: (pw, ph), scale, rtl: a.rtl }),
        (None, None) => placement::centred(open.work, (pw, ph), scale),
    };
    let first = open.placed.is_none();
    let placed = placed.ok_or("There is no room on the screen.")?;
    open.placed = Some(placed);
    Ok(Plan { placed, first, card: open.mode == Mode::Card, source: open.source, scale })
}

/// Moves, sizes and (the first time) shows window `id` and starts a card's click-away watch; a failure or a lookup closed meanwhile closes the window. The first time, the window the shortcut was pressed in must still be in front once the watch is listening (a switch before it started is not an event it can hear), else the user has moved on and the window is closed unseen, so a stale card never sits over another app and the search window never takes its keyboard. Input: the app, the lookup's number and the plan. Return: Err with the reason when the window is not up (`CLOSED`, which is not reported, when the user moved on).
fn reveal(app: &AppHandle, id: u64, plan: &Plan) -> Result<(), String> {
    let margin = (window::MARGIN * plan.scale).round() as i32;
    let bounds = placement::grown(plan.placed.rect, margin);
    if plan.first && plan.card && !window::follow(id, bounds, plan.source, super::left) {
        dismiss_if(app, id, |_| true);
        return Err("The card could not be watched, so it was not shown.".into());
    }
    if plan.first && !crate::platform::in_front(plan.source) {
        dismiss_if(app, id, |_| true);
        super::note_front("before-reveal", plan.source, true);
        return Err(CLOSED.into());
    }
    if !window::show(app, id, bounds, plan.first, !plan.card) {
        dismiss_if(app, id, |_| true);
        return Err("The window could not be shown.".into());
    }
    if state().session(id).is_none() {
        window::destroy(app, id);
        return Err(CLOSED.into());
    }
    Ok(())
}

/// The page measured its card (or the search window's fixed size) and asks for the window. The first call places and shows the window (a card without the keyboard, the search window with it); later calls resize it with the edge next to the text kept. The timing of the first is logged. A refusal other than a window already closed is shown to the user, because the page closes the window when it is refused and nothing else would say why. Input: the calling window and the card's size in dip. Return: the side, arrow and size to draw; Err for a window that is not the open one, a size that is not valid, or a window that could not be shown.
#[tauri::command]
pub fn dict_place(app: AppHandle, window: Window, w: f64, h: f64) -> Result<Placement, String> {
    let placed = place_for(&app, &window, w, h);
    if let Err(why) = &placed {
        crate::log::note(&format!("dictionary: placement refused: {why}"));
        if why != CLOSED {
            crate::status::report(&app, why);
        }
    }
    placed
}

/// The body of `dict_place`. Input: the app, the calling window and the size in dip. Return: what the page learns, Err with the reason when the window is not up.
fn place_for(app: &AppHandle, window: &Window, w: f64, h: f64) -> Result<Placement, String> {
    let id = id_of(window.label()).ok_or(CLOSED)?;
    let plan = plan(id, w, h)?;
    reveal(app, id, &plan)?;
    if plan.first {
        crate::log::note(&format!("dictionary: visible at {} ms", super::since_press()));
    }
    let (scale, rect) = (plan.scale, plan.placed.rect);
    let size = |px: i32| f64::from(px) / scale;
    Ok(Placement {
        side: side_name(plan.placed.side),
        arrow: plan.placed.arrow((placement::ARROW_INSET * scale).round() as i32).map(size),
        w: size(rect.right - rect.left),
        h: size(rect.bottom - rect.top),
    })
}

/// The page's word for a side. Input: the side. Return: where the card sits relative to the text.
fn side_name(side: Side) -> &'static str {
    match side {
        Side::Below => "below",
        Side::Above => "above",
        Side::Right => "right",
        Side::Left => "left",
        Side::Centre => "centre",
    }
}

/// The page closes its own window (Escape in the results, a click outside, or a page error); a page of a window already replaced closes nothing.
#[tauri::command]
pub fn dict_close(app: AppHandle, window: Window) {
    if let Some(id) = id_of(window.label()) {
        if dismiss_if(&app, id, |_| true) {
            crate::log::note("dictionary: the page closed its window");
        }
    }
}

/// The search window's no-pack button: closes the window and opens the Dictionary page of the settings, where the pack is downloaded. The whole action belongs to the open window: a page already replaced does neither. Asynchronous and on a blocking worker, never the main thread: building the settings webview there deadlocks on Windows, and `tray::open_settings_at` may wait for a closing settings window, which only the main thread can finish. Input: the calling window. Return: Err only when the worker could not run.
#[tauri::command]
pub async fn dict_download(app: AppHandle, window: Window) -> Result<(), String> {
    let Download::OpenSettings(id) = state().download(window.label()) else { return Ok(()) };
    crate::snippets::blocking(move || {
        window::destroy(&app, id);
        crate::tray::open_settings_at(&app, Some("words"));
        Ok(())
    })
    .await
}
