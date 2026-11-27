//! File: dictionary/state.rs
//! Purpose: The dictionary window's lifecycle with no app handle, so its rules are tested without the OS: which lookup owns the window (a number carried by the session, the page's readiness and every close, so a cancelled or replaced lookup can change nothing), the ready/open handshake that sends the open event exactly once whichever side comes first, and the session check that makes a late or foreign command change nothing.
//! Contents: Phase / Press — what the window is doing and what a shortcut press does about it; Download — what the no-pack button leads to; Anchor — what a card points at; Mode — card or search window, the one record of it; Session — one open window; State — the lifecycle (`begin`, `publish`, `settle`, `close`, `close_if`), the handshake (`page_ready`, `claim`, `unsent`) `session`, `caller` (which lookup a calling window speaks for), and `download` (what the no-pack button does).

use std::time::{Duration, Instant};

use serde_json::Value;

use super::label::id_of;
use super::placement::{Kind, Placed};
use crate::platform::Rect;

/// A second press this soon after the one that started the lookup is a double tap, not a request to close.
const DEBOUNCE: Duration = Duration::from_millis(300);

/// What the window is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// No Lookup is running.
    Idle,
    /// A Lookup is reading the selection.
    Looking,
    /// A Card or Search window is open.
    Open,
}

/// What a press of the Dictionary shortcut does: start a Lookup, close the Card or Search window (or drop the Lookup), or nothing.
#[derive(Debug, PartialEq, Eq)]
pub enum Press {
    Start,
    Close,
    Ignore,
}

/// What the no-pack button leads to: nothing (a page that is not the open one), or closing the window and opening the settings.
#[derive(Debug, PartialEq)]
pub enum Download {
    Nothing,
    /// The window of this lookup was closed; the settings page opens.
    OpenSettings(u64),
}

/// What a window points at: the selection's visible lines (never their union) or the box round the pointer (the search window), what they are and the reading direction.
pub struct Anchor {
    pub lines: Vec<Rect>,
    pub kind: Kind,
    pub rtl: bool,
}

/// What an open window is: a word's card (never takes the keyboard, closes when the user leaves the word) or the search window (has the keyboard, handles Escape itself). Kept only in the session, so every lifecycle rule reads the same answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Card,
    Search,
}

/// One open window: its number (never reused; the window's label carries it, so a command learns its caller from the label), its mode, what it points at, the work area and scale of its monitor, the window the word is in and where the card is once placed.
pub struct Session {
    pub id: u64,
    pub mode: Mode,
    pub anchor: Option<Anchor>,
    pub work: Rect,
    pub scale: f32,
    pub source: isize,
    pub placed: Option<Placed>,
    /// The open event's payload, kept until it is sent.
    pub open: Option<Value>,
}

/// The lifecycle of the window and its session, all behind one lock so every change of hands is one step. Each lookup has a number (`current` at its press, never reused); the window, the session, the page's readiness and every close carry it, so late work of an older lookup finds nothing to change.
pub struct State {
    /// What the window is doing (`phase`).
    phase: Phase,
    /// The number of the newest lookup; bumped by every press and every close.
    current: u64,
    /// When the press that started the current lookup happened.
    started: Option<Instant>,
    /// The number of the window whose page has said it listens, 0 for none.
    ready: u64,
    session: Option<Session>,
    sent: bool,
}

impl State {
    /// The state of no lookup and no window.
    pub const CLOSED: State = State { phase: Phase::Idle, current: 0, started: None, ready: 0, session: None, sent: false };

    /// What the window is doing. Return: the phase.
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// What a press does now: starts a Lookup when none is up, else is ignored as a double tap within the debounce of the press that started the Lookup, else closes. Input: the press time. Return: the action; nothing changes until the caller acts on it.
    pub fn press(&self, now: Instant) -> Press {
        let since = self.started.map_or(Duration::MAX, |t| now.saturating_duration_since(t));
        match self.phase {
            Phase::Idle => Press::Start,
            _ if since < DEBOUNCE => Press::Ignore,
            _ => Press::Close,
        }
    }

    /// Whether the open window is a card that never took the keyboard (typing in the word's window closes it). Return: the answer.
    pub fn is_card(&self) -> bool {
        self.open_mode() == Some(Mode::Card)
    }

    /// Whether the open window is the search window (it has the keyboard and handles Escape itself). Return: the answer.
    pub fn is_search(&self) -> bool {
        self.open_mode() == Some(Mode::Search)
    }

    /// The mode of the open window, from its session. Return: None when no window is open.
    fn open_mode(&self) -> Option<Mode> {
        self.session.as_ref().filter(|_| self.phase == Phase::Open).map(|s| s.mode)
    }

    /// Time since the press that started the current lookup, None before the first. Return: the elapsed time.
    pub fn since_press(&self) -> Option<Duration> {
        self.started.map(|t| t.elapsed())
    }

    /// Starts a lookup if none is up. Input: the press time. Return: its number, None when one is already up.
    pub fn begin(&mut self, now: Instant) -> Option<u64> {
        if self.phase != Phase::Idle {
            return None;
        }
        self.phase = Phase::Looking;
        self.current += 1;
        self.started = Some(now);
        Some(self.current)
    }

    /// Whether lookup `id` is still the wanted one. Input: the number. Return: the answer.
    pub fn wanted(&self, id: u64) -> bool {
        self.current == id
    }

    /// Turns lookup `id` into the open window with its session (which says its mode), unless it was cancelled or replaced first. Input: the lookup's number and its session. Return: None when it no longer owns the window, else whether the open event should be sent now (the page is ready).
    pub fn publish(&mut self, id: u64, session: Session) -> Option<bool> {
        if self.current != id || self.phase != Phase::Looking {
            return None;
        }
        self.phase = Phase::Open;
        Some(self.store(session))
    }

    /// Ends lookup `id`'s work when it shows nothing. Input: the number. Return: whether its window stays (it was opened and is still the open one).
    pub fn settle(&mut self, id: u64) -> bool {
        if self.current != id {
            return false;
        }
        if self.phase == Phase::Looking {
            self.phase = Phase::Idle;
        }
        self.phase == Phase::Open
    }

    /// Ends whatever is up: a new number (a lookup in flight finds itself replaced), idle, no session, a page that has to say it listens again. Return: the number of the window that was up, for the caller to destroy.
    pub fn close(&mut self) -> u64 {
        let id = self.current;
        *self = State { current: id + 1, started: self.started, ..State::CLOSED };
        id
    }

    /// Closes the lookup or window `id` when it is still the current one and `allowed` says so. Input: the number and the condition on this state. Return: its number when closed.
    pub fn close_if(&mut self, id: u64, allowed: impl FnOnce(&State) -> bool) -> Option<u64> {
        (self.current == id && self.phase != Phase::Idle && allowed(self)).then(|| self.close())
    }

    /// Stores a new session. Input: the session. Return: whether the open event should be sent now (its page is ready).
    fn store(&mut self, session: Session) -> bool {
        let ready = self.ready == session.id;
        self.session = Some(session);
        self.sent = false;
        ready
    }

    /// The page of window `id` listens. Input: the number. Return: whether the open event should be sent now (its session waits for it).
    pub fn page_ready(&mut self, id: u64) -> bool {
        if self.current != id {
            return false;
        }
        self.ready = id;
        self.session.as_ref().is_some_and(|s| s.id == id) && !self.sent
    }

    /// Takes the open event once. Return: the session number and payload, None when it was sent already or there is none.
    pub fn claim(&mut self) -> Option<(u64, Value)> {
        let session = self.session.as_mut()?;
        if self.sent {
            return None;
        }
        let payload = session.open.clone()?;
        self.sent = true;
        Some((session.id, payload))
    }

    /// A send that failed: the event may be claimed again. Input: none. Return: none.
    pub fn unsent(&mut self) {
        self.sent = false;
    }

    /// The lookup the calling window speaks for: what a command may touch is named by the caller's label, never by a number it supplies. Input: the caller's window label. Return: the number when the label names the open session, None for another window, a malformed label or a window that is not the open one.
    pub fn caller(&self, label: &str) -> Option<u64> {
        let id = id_of(label)?;
        self.session.as_ref().filter(|s| s.id == id).map(|s| s.id)
    }

    /// What the no-pack button does: only the open window's own page closes it and leads to the settings. Input: the caller's window label. Return: `OpenSettings` with the number closed; `Nothing` for a label that is not the current lookup's (a replaced page, a foreign or malformed label, no window), so it cannot go on to open the settings.
    pub fn download(&mut self, label: &str) -> Download {
        match id_of(label).and_then(|id| self.close_if(id, |_| true)) {
            Some(id) => Download::OpenSettings(id),
            None => Download::Nothing,
        }
    }

    /// The session `id` names. Input: the number. Return: the session, None when it is not the open one.
    pub fn session(&mut self, id: u64) -> Option<&mut Session> {
        self.session.as_mut().filter(|s| s.id == id)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use serde_json::json;

    use super::{Download, Mode, Phase, Press, Session, State};
    use crate::dictionary::label::label;
    use crate::platform::Rect;
    use crate::window_guard::Focus;
    use std::time::Duration;

    /// A session numbered `id` in `mode` with a payload.
    fn session_in(id: u64, mode: Mode) -> Session {
        let work = Rect { left: 0, top: 0, right: 100, bottom: 100 };
        Session { id, mode, anchor: None, work, scale: 1.0, source: 0, placed: None, open: Some(json!({ "session": id })) }
    }

    /// A card session numbered `id`.
    fn session(id: u64) -> Session {
        session_in(id, Mode::Card)
    }

    /// A search-window session numbered `id`.
    fn search(id: u64) -> Session {
        session_in(id, Mode::Search)
    }

    /// A state with lookup 1 begun and published, its page ready.
    fn open_one() -> State {
        let mut s = State::CLOSED;
        let id = s.begin(Instant::now()).expect("begin");
        s.page_ready(id);
        assert_eq!(s.publish(id, session(id)), Some(true));
        s
    }

    /// The open event goes out exactly once, whether the page or the session comes first, and a failed send may be claimed again.
    #[test]
    fn the_open_event_is_sent_once_whichever_side_comes_first() {
        let mut page_first = State::CLOSED;
        let a = page_first.begin(Instant::now()).unwrap();
        assert!(!page_first.page_ready(a));
        assert_eq!(page_first.publish(a, search(a)), Some(true));
        assert_eq!(page_first.claim().map(|(id, _)| id), Some(a));
        assert!(page_first.claim().is_none());
        page_first.unsent();
        assert!(page_first.claim().is_some());

        let mut session_first = State::CLOSED;
        let b = session_first.begin(Instant::now()).unwrap();
        assert_eq!(session_first.publish(b, search(b)), Some(false));
        assert!(session_first.page_ready(b));
        assert_eq!(session_first.claim().map(|(id, _)| id), Some(b));
        assert!(!session_first.page_ready(b));
    }

    /// Only the open session answers to its number; a stale or invented one finds nothing.
    #[test]
    fn a_command_of_another_session_finds_nothing() {
        let mut s = open_one();
        assert!(s.session(1).is_some());
        assert!(s.session(0).is_none() && s.session(2).is_none());
        s.close();
        assert!(s.session(1).is_none());
    }

    /// A lookup cancelled before it publishes opens nothing, even when a newer lookup has started and is looking: the old one's number no longer owns the window.
    #[test]
    fn a_cancelled_lookup_cannot_publish_into_its_successor() {
        let mut s = State::CLOSED;
        let old = s.begin(Instant::now()).unwrap();
        assert_eq!(s.close(), old);
        assert_eq!(s.phase(), Phase::Idle);
        assert_eq!(s.publish(old, session(old)), None);
        let newer = s.begin(Instant::now()).unwrap();
        assert_ne!(old, newer);
        assert_eq!(s.publish(old, session(old)), None);
        assert_eq!(s.phase(), Phase::Looking);
        assert!(s.session(old).is_none());
        assert_eq!(s.publish(newer, session(newer)), Some(false));
        assert_eq!(s.phase(), Phase::Open);
    }

    /// A cancel that wins against a publish leaves nothing stored: no session, idle, and the late publisher is refused.
    #[test]
    fn a_cancel_before_publish_leaves_no_session() {
        let mut s = State::CLOSED;
        let id = s.begin(Instant::now()).unwrap();
        s.close();
        assert!(s.publish(id, search(id)).is_none());
        assert!(s.session(id).is_none());
        assert!(!s.settle(id));
    }

    /// An older lookup ending (its `settle`) never touches a newer lookup, and a lookup that opened keeps its window.
    #[test]
    fn settling_an_old_lookup_changes_nothing_of_a_newer_one() {
        let mut s = State::CLOSED;
        let old = s.begin(Instant::now()).unwrap();
        s.close();
        let newer = s.begin(Instant::now()).unwrap();
        assert!(!s.settle(old));
        assert_eq!(s.phase(), Phase::Looking);
        assert_eq!(s.publish(newer, session(newer)), Some(false));
        assert!(s.settle(newer));
        assert_eq!(s.phase(), Phase::Open);
    }

    /// A close that names an older window (a watch thread, a watchdog, a focus event of a window already gone) does nothing to the window that replaced it.
    #[test]
    fn a_late_close_cannot_close_the_next_window() {
        let mut s = open_one();
        assert_eq!(s.close(), 1);
        let next = s.begin(Instant::now()).unwrap();
        assert_eq!(s.publish(next, session(next)), Some(false));
        assert_eq!(s.close_if(1, |_| true), None);
        assert_eq!(s.phase(), Phase::Open);
        assert_eq!(s.close_if(next, |st| st.is_card()), Some(next));
        assert_eq!(s.phase(), Phase::Idle);
    }

    /// The condition of a close is checked in the same step: the leave of a card does not close the search window, and a window that never held the keyboard does not close on losing it.
    #[test]
    fn a_close_checks_its_condition_under_the_same_lock() {
        let (mut s, focus) = (State::CLOSED, Focus::new());
        let id = s.begin(Instant::now()).unwrap();
        s.publish(id, search(id));
        assert_eq!(s.close_if(id, |st| st.is_card()), None);
        assert_eq!(s.close_if(id, |_| focus.lost(id, false)), None);
        focus.gained(id);
        assert_eq!(s.close_if(id, |_| focus.lost(id, false)), Some(id));
    }

    /// A focus loss closes the search window only once it has held the keyboard and is no longer in front: the bounce while it takes the keyboard (still in front) keeps it, and an older window's loss closes nothing.
    #[test]
    fn a_focus_loss_closes_only_a_window_no_longer_in_front() {
        let (mut s, focus) = (State::CLOSED, Focus::new());
        let id = s.begin(Instant::now()).unwrap();
        s.publish(id, search(id));
        focus.gained(id);
        assert_eq!(s.close_if(id, |_| focus.lost(id, true)), None);
        assert!(s.is_search());
        assert_eq!(s.close_if(id - 1, |_| focus.lost(id - 1, false)), None);
        assert_eq!(s.close_if(id, |_| focus.lost(id, false)), Some(id));
    }

    /// A command is bound to the calling window: the page of window A cannot act while session B is open, and a label that names no lookup (other windows, malformed or non-canonical numbers) acts on nothing.
    #[test]
    fn a_command_is_bound_to_the_calling_window() {
        let mut s = State::CLOSED;
        let a = s.begin(Instant::now()).unwrap();
        s.close();
        let b = s.begin(Instant::now()).unwrap();
        s.publish(b, search(b));
        assert_eq!(s.caller(&label(a)), None);
        assert_eq!(s.caller(&label(b)), Some(b));
        for bad in ["main", "dictionary", "dictionary-", "dictionary-x", "dictionary-+2", "dictionary-02", "dictionary-2 ", " dictionary-2"]
        {
            assert_eq!(s.caller(bad), None, "{bad}");
        }
        s.close();
        assert_eq!(s.caller(&label(b)), None);
    }

    /// The no-pack button leads to the settings only from the open window's own page: the page of a replaced window, a foreign or malformed label and an idle state get nothing, and the open page gets it once.
    #[test]
    fn only_the_open_pages_download_opens_the_settings() {
        let mut s = open_one();
        assert_eq!(s.download("main"), Download::Nothing);
        assert_eq!(s.download("dictionary-+1"), Download::Nothing);
        assert_eq!(s.phase(), Phase::Open);
        s.close();
        assert_eq!(s.download(&label(1)), Download::Nothing);
        let next = s.begin(Instant::now()).unwrap();
        s.publish(next, search(next));
        assert_eq!(s.download(&label(1)), Download::Nothing);
        assert_eq!(s.phase(), Phase::Open);
        assert_eq!(s.download(&label(next)), Download::OpenSettings(next));
        assert_eq!(s.phase(), Phase::Idle);
        assert_eq!(s.download(&label(next)), Download::Nothing);
    }

    /// With session 3 open, the spellings `+3` and `03` of its own number are refused by the canonical parsing alone (they parse to 3, which is the open session).
    #[test]
    fn a_non_canonical_spelling_of_the_open_number_is_refused() {
        let mut s = State::CLOSED;
        s.begin(Instant::now()).unwrap();
        s.close();
        let three = s.begin(Instant::now()).unwrap();
        assert_eq!(three, 3);
        s.publish(three, search(three));
        assert_eq!(s.caller("dictionary-3"), Some(3));
        for alias in ["dictionary-+3", "dictionary-03", "dictionary-003"] {
            assert_eq!(s.caller(alias), None, "{alias}");
            assert_eq!(s.download(alias), Download::Nothing, "{alias}");
        }
        assert_eq!(s.phase(), Phase::Open);
    }

    /// The gate of a page's whole action: only the open window's own page closes it (and so may go on to open the settings); the page of a replaced window closes nothing and the successor stays open.
    #[test]
    fn a_replaced_page_cannot_close_its_successor() {
        let mut s = open_one();
        s.close();
        let next = s.begin(Instant::now()).unwrap();
        s.publish(next, search(next));
        assert_eq!(s.close_if(1, |_| true), None);
        assert_eq!(s.phase(), Phase::Open);
        assert_eq!(s.close_if(next, |_| true), Some(next));
    }

    /// A page of a window that is gone cannot mark the next window's page ready, so the next session's open event waits for its own page.
    #[test]
    fn a_stale_page_does_not_make_the_next_page_ready() {
        let mut s = open_one();
        s.close();
        let next = s.begin(Instant::now()).unwrap();
        assert!(!s.page_ready(1));
        assert_eq!(s.publish(next, session(next)), Some(false));
        assert!(s.page_ready(next));
    }

    /// The mode is the session's alone: a card session makes the open window a card (closed by the user leaving the word, by typing and by the global Escape), a search session makes it the search window (none of those close it), and with no open window neither holds.
    #[test]
    fn the_window_is_what_its_session_says() {
        let mut s = State::CLOSED;
        let card = s.begin(Instant::now()).unwrap();
        assert!(!s.is_card() && !s.is_search());
        s.publish(card, session(card));
        assert!(s.is_card() && !s.is_search());
        s.close();
        let find = s.begin(Instant::now()).unwrap();
        s.publish(find, search(find));
        assert!(s.is_search() && !s.is_card());
        assert_eq!(s.close_if(find, State::is_card), None);
        s.close();
        assert!(!s.is_card() && !s.is_search());
    }

    /// A press starts a Lookup when idle, is ignored as a double tap right after, and closes the Card or Search window later.
    #[test]
    fn a_press_starts_ignores_a_double_tap_or_closes() {
        let t0 = Instant::now();
        let (soon, later) = (t0 + Duration::from_millis(100), t0 + Duration::from_millis(900));
        let mut s = State::CLOSED;
        assert_eq!(s.press(t0), Press::Start);
        let id = s.begin(t0).unwrap();
        assert_eq!(s.press(soon), Press::Ignore);
        assert_eq!(s.press(later), Press::Close);
        s.publish(id, session(id));
        assert_eq!(s.press(soon), Press::Ignore);
        assert_eq!(s.press(later), Press::Close);
        s.close();
        assert_eq!(s.press(later), Press::Start);
    }
}
