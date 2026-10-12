//! Purpose: Live typing for a toggled recording (a lecture, a long thought): every sentence is typed at the cursor as soon as it is recognised,
//! so nothing waits for the end. Hold-to-talk never goes through here: it types once on release. No key is held while a toggled recording
//! runs, so typed characters cannot turn into shortcuts. Sentences are never revised once typed, which is why live typing is possible at all.
//! Contents: begin / push / finish / cancel / active — one live session and its typing thread; route — the focus guard (pure, tested); run — the thread.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;

use tauri::AppHandle;

use super::clean;
use crate::platform::{self, overlay, InsertOptions};
use crate::prefs::Prefs;
use crate::rules::{self, model::Trigger};
use crate::{stats, status};

enum Item {
    Sentence(String),
    End,
}

/// The live session in progress: the way to its typing thread.
static SESSION: Mutex<Option<Sender<Item>>> = Mutex::new(None);
/// Escape: what is still queued is thrown away.
static CANCELLED: AtomicBool = AtomicBool::new(false);

pub fn active() -> bool {
    SESSION.lock().unwrap().is_some()
}

/// Starts typing sentences as they arrive. The overlay shows only the bars: the text is landing in the document.
pub fn begin(app: &AppHandle) {
    CANCELLED.store(false, Ordering::Release);
    let (tx, rx) = mpsc::channel();
    let handle = app.clone();
    // One thread keeps the sentences in order, which separate threads per sentence could not.
    let spawned = std::thread::Builder::new().name("live-typing".into()).stack_size(512 * 1024).spawn(move || run(&handle, rx));
    if spawned.is_ok() {
        *SESSION.lock().unwrap() = Some(tx);
        overlay::caption_after(None);
    }
}

pub fn push(sentence: &str) {
    if let Some(tx) = SESSION.lock().unwrap().as_ref() {
        let _ = tx.send(Item::Sentence(sentence.to_string()));
    }
}

/// The recording is over and `text` is everything that was said: record it and let the thread type what is left.
pub fn finish(app: &AppHandle, text: &str, secs: f32) {
    let Some(tx) = SESSION.lock().unwrap().take() else { return };
    let prefs = Prefs::load(app);
    let said = clean::clean(text, &prefs.vocabulary, prefs.remove_fillers);
    if said.is_empty() {
        overlay::notice("Didn't catch any speech", prefs.overlay_top, prefs.theme());
    } else {
        stats::record(app, said.split_whitespace().count() as u32, secs);
    }
    let _ = tx.send(Item::End);
}

/// Escape: stop typing at once. What was already typed stays.
pub fn cancel() {
    CANCELLED.store(true, Ordering::Release);
    SESSION.lock().unwrap().take();
}

/// Adds `sentence` to what waits. Returns what to type now: everything waiting, once focus is back on the window dictation started in.
fn route(waiting: &mut Vec<String>, sentence: String, on_target: bool) -> String {
    waiting.push(sentence);
    if on_target {
        return std::mem::take(waiting).join(" ");
    }
    String::new()
}

fn type_now(app: &AppHandle, text: String, prefs: &Prefs) {
    let options = InsertOptions { always_paste: prefs.always_paste, restore_clipboard: prefs.restore_clipboard };
    super::insert_text(app, text, &options, prefs.context_paste, "Disabled"); // never submits: the speaker is not done
}

fn run(app: &AppHandle, rx: Receiver<Item>) {
    let mut waiting = Vec::new();
    for item in rx {
        if CANCELLED.load(Ordering::Acquire) {
            return;
        }
        let prefs = Prefs::load(app);
        match item {
            Item::Sentence(text) => {
                let cleaned = clean::clean(&text, &prefs.vocabulary, prefs.remove_fillers);
                if cleaned.is_empty() {
                    continue;
                }
                // A rule's Preview or Copy output cannot work sentence by sentence, so only its changes to the text count.
                let typed = route(&mut waiting, rules::run(Trigger::Dictation, &cleaned).text, super::on_target());
                if !typed.is_empty() {
                    type_now(app, typed, &prefs);
                }
            }
            Item::End => return leave_rest(app, waiting, &prefs),
        }
    }
}

/// Sentences still waiting at the end: typed if focus is back, otherwise on the clipboard so they are not lost.
fn leave_rest(app: &AppHandle, waiting: Vec<String>, prefs: &Prefs) {
    if waiting.is_empty() {
        return;
    }
    let text = waiting.join(" ");
    if super::on_target() {
        return type_now(app, text, prefs);
    }
    match platform::copy_text(&text) {
        Ok(()) => status::report(app, "You switched windows, so the last of the text was copied to the clipboard. Paste it with Ctrl+V."),
        Err(e) => status::report(app, &format!("You switched windows and the last of the text could not be copied: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::route;

    #[test]
    fn sentences_wait_while_focus_is_elsewhere_and_come_out_in_order() {
        let mut waiting = Vec::new();
        assert_eq!(route(&mut waiting, "One.".into(), true), "One.");
        assert_eq!(route(&mut waiting, "Two.".into(), false), "");
        assert_eq!(route(&mut waiting, "Three.".into(), false), "");
        assert_eq!(route(&mut waiting, "Four.".into(), true), "Two. Three. Four.");
        assert!(waiting.is_empty());
    }
}
