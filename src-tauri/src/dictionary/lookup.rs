//! Purpose: One dictionary lookup: copy the selection, find the word in the installed packs, and say why when there is nothing to show.
//! Contents: Pending — the word the card shows and what is kept to record it; lookup — copy, find, build the `Pending`; record — add the lookup to the Looked Up list; notice — a short message pill; find_word — the first pack file that knows the word; `Pending::sections` — the card's sections.

use std::path::PathBuf;
use std::time::Instant;

use tauri::AppHandle;

use super::catalog::{self, Pack};
use super::pack::{self, Block};
use super::{saved, words};
use crate::platform::{self, overlay, CardSection, Rect};
use crate::prefs::Prefs;
use crate::status;

/// A word found: what the card shows and what is kept to record the lookup.
pub(super) struct Pending {
    pub language: String,
    pub blocks: Vec<Block>,
    pub pack: &'static str,
    pub key: String,
    /// The selected text on the screen and the work area of its monitor.
    pub anchor: (Rect, Rect),
    /// The window the word was in: the card goes away when it is no longer the one in front.
    pub source: isize,
}

pub(super) fn notice(app: &AppHandle, text: &str) {
    crate::log::note(&format!("dictionary: {text}"));
    let prefs = Prefs::load(app);
    overlay::notice(text, prefs.overlay_top, prefs.theme());
}

struct Found {
    pack: &'static Pack,
    language: String,
    blocks: Vec<Block>,
}

/// The first pack file that knows the word. A pack that cannot be read is skipped; its error is returned only when no other pack has the word.
fn find_word(packs: &[(&'static Pack, PathBuf)], key: &str) -> Result<Option<Found>, String> {
    let mut unreadable = None;
    for (pack, path) in packs {
        match pack::open(path).and_then(|conn| Ok((pack::define(&conn, key)?, pack::language(&conn)))) {
            Ok((blocks, _)) if blocks.is_empty() => {}
            Ok((blocks, language)) => return Ok(Some(Found { pack, language: language.unwrap_or_else(|| pack.name.into()), blocks })),
            Err(e) => unreadable = Some(e),
        }
    }
    unreadable.map_or(Ok(None), Err)
}

/// UI Automation is asked for no more than this. The copy fallback returns whatever is selected, so `pack::normalize` is what refuses a longer selection.
const MAX_SELECTION: usize = 64;

/// The selected text and its place on the screen: UI Automation answers both at once without touching the clipboard or the keyboard; only an app that does not expose its text falls back to copying it.
fn read_selection(app: &AppHandle) -> Option<(String, Option<Rect>)> {
    if let Some(found) = platform::uia_selection(MAX_SELECTION) {
        return Some((found.text, found.rect));
    }
    let (text, _) = crate::selection::selected_text(app)?;
    // The first UI Automation call makes a browser build its accessibility tree, so the copy above may be answered by it a moment later: ask again for the place only.
    let rect = platform::uia_selection(MAX_SELECTION).filter(|found| found.text.trim() == text.trim()).and_then(|found| found.rect);
    Some((text, rect))
}

/// Read the selection and look it up; tells the user why when there is nothing to show.
pub(super) fn lookup(app: &AppHandle, wanted: &dyn Fn() -> bool) -> Option<Pending> {
    // A lookup that was cancelled (Escape, a dictation starting) stays quiet: its message would land on top of whatever came next.
    let say = |text: &str| {
        if wanted() {
            notice(app, text);
        }
    };
    let ms = |t: Instant| t.elapsed().as_millis();
    let source = platform::foreground_window();
    let t = Instant::now();
    let (text, selected) = read_selection(app)?;
    let read = ms(t);
    let Some(key) = pack::normalize(&text) else {
        say("Select one word");
        return None;
    };
    let Some(anchor) = platform::selection_anchor(selected) else {
        say("Could not find the selection on the screen");
        return None;
    };
    let packs: Vec<_> = catalog::installed_packs(app).into_iter().filter_map(|p| Some((p, catalog::path(app, p)?))).collect();
    if packs.is_empty() {
        say("Download a dictionary first");
        return None;
    }
    let t = Instant::now();
    let found = match find_word(&packs, &key) {
        Ok(Some(found)) => found,
        Ok(None) => {
            say(&format!("No definition for {key}"));
            return None;
        }
        Err(e) => {
            status::report(app, &e);
            return None;
        }
    };
    crate::log::note(&format!("dictionary: read {read} ms, find {} ms", ms(t)));
    let Found { pack, language, blocks } = found;
    let headword = blocks[0].word.to_lowercase();
    Some(Pending { language, blocks, pack: pack.id, key: headword, anchor, source })
}

/// Writes the lookup to the Looked Up list (first meaning only). A failure is only logged: the popup must show whatever happens to the list.
pub(super) fn record(app: &AppHandle, pending: &Pending) {
    let Some(block) = pending.blocks.first() else { return };
    let Some(sense) = block.senses.first() else { return };
    let row = words::Saved {
        id: 0,
        word: block.word.clone(),
        pos: block.pos.clone(),
        gloss: sense.gloss.clone(),
        example: sense.example.clone().unwrap_or_default(),
        language: pending.language.clone(),
        added: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64),
    };
    if let Err(e) = saved::words_conn(app).and_then(|conn| words::record(&conn, pending.pack, &pending.key, &row)) {
        crate::log::note(&format!("dictionary: {e}"));
    }
}

#[cfg(test)]
#[path = "lookup_test.rs"]
mod tests;

impl Pending {
    /// The card's sections: the word heads its first part of speech, and again whenever the word changes (Wiktionary keeps "Polish" and "polish" under one key).
    pub fn sections(&self) -> Vec<CardSection> {
        self.blocks
            .iter()
            .enumerate()
            .map(|(i, block)| CardSection {
                heading: (i == 0 || block.word != self.blocks[i - 1].word).then(|| block.word.clone()),
                label: block.pos.clone(),
                items: block.senses.iter().map(|s| (s.gloss.clone(), s.example.clone())).collect(),
                synonyms: block.synonyms.clone(),
            })
            .collect()
    }
}
