//! File: strip/state.rs
//! Purpose: The open strip's state with no app handle, so its rules are tested without the OS: the ready/open handshake that sends the open event exactly once whichever side comes first, the session check every command makes, the window place the page may ask for, and the result Paste may deliver (that of the run the page accepted).
//! Contents: Session — one open strip; State — the window's number, the page's readiness and the session, with `owns`, `retire`, the handshake, the session check, cancel's new run session and the first placement; Session::publish — keep a run's result only while the run is current; Row — a row as the open event carries it; rows — the rows to send, results only for a text under `EXACT_LIMIT`.

use std::time::Instant;

use serde::Serialize;

use super::bounds::{self, Dip};
use crate::palette::{Group, Item};
use crate::platform::{Rect, Target};

/// The longest text, in chars, whose exact results travel with the open event; a longer one's results are asked for row by row (`strip_run`), so a huge selection does not make the first event megabytes long.
pub const EXACT_LIMIT: usize = 20_000;

/// One open strip: the copied text and its rows, where the result goes, and where the selection is.
pub struct Session {
    /// The number the page carries on every command; never reused.
    pub id: u64,
    /// The palette session model runs belong to; renewed when a run is cancelled.
    pub epoch: u64,
    pub items: Vec<Item>,
    pub chips: Vec<usize>,
    pub text: String,
    pub program: String,
    pub target: Target,
    pub work: Rect,
    pub scale: f32,
    pub anchor: Rect,
    /// Text-model results as (row id, run number, text), so Paste delivers what Rust computed for the run the page accepted, never text the page sent.
    pub results: Vec<(String, u64, String)>,
    /// When the shortcut was pressed, for the timing log.
    pub pressed: Instant,
    pub shown: bool,
}

impl Session {
    /// Whether row `id` belongs to this session. Input: the id. Return: the answer.
    pub fn has_row(&self, id: &str) -> bool {
        self.items.iter().any(|i| i.id == id)
    }

    /// The exact result built with row `id`, if it has one. Input: the id. Return: the text.
    pub fn exact(&self, id: &str) -> Option<String> {
        self.items.iter().find(|i| i.id == id).and_then(|i| i.result.clone())
    }

    /// The result Rust holds for row `id` as run `seq` gave it: the exact result, else the text-model result of exactly that run. Input: the id and the run number. Return: the text.
    pub fn result_of(&self, id: &str, seq: u64) -> Option<String> {
        self.exact(id).or_else(|| self.results.iter().find(|(r, n, _)| r == id && *n == seq).map(|(_, _, t)| t.clone()))
    }

    /// Keeps `text` as row `id`'s result from run `seq` of palette session `epoch`, replacing an older one, only while that run is still current: no cancel moved the session to a new epoch and `wanted` (no newer request came in). The caller checks `wanted` and keeps under the one state lock, so a cancel or a newer result cannot land between the check and the keep. Input: the id, the run's epoch and number, whether it is still wanted, and the text. Return: whether it was kept.
    pub fn publish(&mut self, id: String, epoch: u64, seq: u64, wanted: bool, text: String) -> bool {
        if epoch != self.epoch || !wanted {
            return false;
        }
        self.results.retain(|(r, _, _)| *r != id);
        self.results.push((id, seq, text));
        true
    }
}

/// The page's readiness and the session; `sent` marks the event as sent, so it goes out exactly once.
pub struct State {
    /// The number of the window now built (`window_guard::fresh`), 0 for none; the watchdog's expiry must name it.
    pub window: u64,
    pub ready: bool,
    pub sent: bool,
    pub session: Option<Session>,
}

impl State {
    /// No page and no session: the state of a closed strip.
    pub const CLOSED: State = State { window: 0, ready: false, sent: false, session: None };

    /// Whether `owner` is the window now built. Input: the number. Return: the answer.
    pub fn owns(&self, owner: u64) -> bool {
        self.window == owner
    }

    /// Drops the session when `owner` is the window now built. Input: the number. Return: whether it was, so the caller may close that window.
    pub fn retire(&mut self, owner: u64) -> bool {
        let owned = self.owns(owner);
        if owned {
            self.session = None;
        }
        owned
    }

    /// Claims the open event when both the page and a session are there and it is not sent yet. Return: whether the caller must send it now.
    fn claim(&mut self) -> bool {
        let due = self.ready && !self.sent && self.session.is_some();
        self.sent |= due;
        due
    }

    /// The page listens for the open event. Return: whether the caller must send it now (the session came first).
    pub fn page_ready(&mut self) -> bool {
        self.ready = true;
        self.claim()
    }

    /// The copied text's session arrives. Input: the session. Return: whether the caller must send the event now (the page came first).
    pub fn store(&mut self, session: Session) -> bool {
        self.session = Some(session);
        self.sent = false;
        self.claim()
    }

    /// The claimed event could not be sent: it may be claimed again.
    pub fn unsent(&mut self) {
        self.sent = false;
    }

    /// The session numbered `id`, if it is the open one. Input: the number. Return: the session.
    pub fn session(&mut self, id: u64) -> Option<&mut Session> {
        self.session.as_mut().filter(|s| s.id == id)
    }

    /// Session `id`'s later model runs belong to palette session `epoch` (a cancel stopped the old one). Input: the number and the new epoch. Return: whether `id` was the open session.
    pub fn renew(&mut self, id: u64, epoch: u64) -> bool {
        self.session(id).map(|s| s.epoch = epoch).is_some()
    }

    /// The page of session `id` asks for the window `rect`: the physical bounds, whether this is the first placement (which shows the window) and when the shortcut was pressed. A bad rectangle changes nothing, so a corrected request still shows the window. Input: the number and the rectangle. Return: None for another session or a bad rectangle.
    pub fn place(&mut self, id: u64, rect: Dip) -> Option<(Rect, bool, Instant)> {
        let s = self.session(id)?;
        let physical = bounds::to_physical(rect, s.work, s.scale)?;
        let first = !std::mem::replace(&mut s.shown, true);
        Some((physical, first, s.pressed))
    }
}

/// A row as the open event carries it.
#[derive(Serialize, Clone)]
pub struct Row<'a> {
    id: &'a str,
    name: &'a str,
    hint: &'a str,
    group: Group,
    model: bool,
    diff: bool,
    result: Option<&'a str>,
}

/// The rows for the open event: exact results only when the text is under `EXACT_LIMIT` chars, else none (the page asks for each with `strip_run`). Input: the rows and the text. Return: the rows to send.
pub fn rows<'a>(items: &'a [Item], text: &str) -> Vec<Row<'a>> {
    let small = text.chars().nth(EXACT_LIMIT).is_none();
    items
        .iter()
        .map(|i| Row {
            id: &i.id,
            name: &i.name,
            hint: &i.hint,
            group: i.group,
            model: i.model,
            diff: i.diff,
            result: i.result.as_deref().filter(|_| small),
        })
        .collect()
}
