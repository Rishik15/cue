//! File: snippets/policy.rs
//! Purpose: The one place that decides which triggers a snippet gives and how each behaves: the snippet's folder (switch and programs) and the rules of the Snippets page (when a trigger expands, whether case matters, what ends a trigger) come together here, for the expander's index and for the save and import clash checks alike.
//! Contents: Effective — one trigger with everything worked out; Policy — the page's rules; load — read them from the settings; Policy::resolve — a snippet and its folder as triggers.
//! The per-snippet mode, case, ending and inside-word fields are still stored, imported and exported (older files and other tools carry them) but are not read here: the page's rules are the only authority.

use super::keys;
use super::model::{Folder, Mode, Snippet, When};
use crate::settings;

/// One trigger with everything that applies to it worked out: a disabled snippet or folder gives none.
#[derive(Clone, Debug, PartialEq)]
pub struct Effective {
    pub id: i64,
    pub trigger: String,
    /// Whether it expands as soon as typed or waits for the character that ends the word.
    pub when: When,
    pub match_case: bool,
    pub inside_word: bool,
    pub propagate_case: bool,
    pub ending: String,
    pub only_apps: Vec<String>,
    pub skip_apps: Vec<String>,
}

/// The rules of the Snippets page, shared by every snippet.
#[derive(Clone, Debug, PartialEq)]
pub struct Policy {
    mode: Mode,
    match_case: bool,
    follow_case: bool,
    inside_words: bool,
    ending: String,
}

impl Default for Policy {
    /// The page's defaults: Auto timing, case ignored and followed, no inside-word matching, the usual endings.
    fn default() -> Self {
        Policy {
            mode: Mode::Auto,
            match_case: keys::DEFAULT_MATCH_CASE,
            follow_case: keys::DEFAULT_FOLLOW_CASE,
            inside_words: keys::DEFAULT_INSIDE_WORDS,
            ending: String::new(),
        }
    }
}

/// Reads the rules from the settings. Returns them; a hand-edited file gets the same limits `settings::validate` gives the page.
pub fn load() -> Policy {
    let defaults = Policy::default();
    Policy {
        mode: Mode::parse(&settings::get_str(keys::MODE).unwrap_or_default()),
        match_case: settings::get_bool(keys::MATCH_CASE, defaults.match_case),
        follow_case: settings::get_bool(keys::FOLLOW_CASE, defaults.follow_case),
        inside_words: settings::get_bool(keys::INSIDE_WORDS, defaults.inside_words),
        ending: settings::get_str(keys::ENDING).unwrap_or_default().chars().filter(|c| !c.is_control()).take(keys::MAX_ENDING).collect(),
    }
}

impl Policy {
    /// The triggers a snippet gives inside its folder, under these rules. Input: the snippet and its folder. Returns one per non-empty trigger, none when the snippet or the folder is off. The snippet's own list of allowed programs wins over the folder's; the lists of skipped programs add up.
    pub fn resolve(&self, s: &Snippet, folder: Option<&Folder>) -> Vec<Effective> {
        if !s.enabled || folder.is_some_and(|f| !f.enabled) {
            return Vec::new();
        }
        let only_apps = if s.only_apps.is_empty() { folder.map(|f| f.only_apps.clone()).unwrap_or_default() } else { s.only_apps.clone() };
        let skip_apps = s.skip_apps.iter().chain(folder.iter().flat_map(|f| f.skip_apps.iter())).cloned().collect::<Vec<_>>();
        s.triggers
            .iter()
            .filter(|t| !t.is_empty())
            .map(|t| Effective {
                id: s.id,
                trigger: t.clone(),
                when: self.mode.when(t),
                match_case: self.match_case,
                inside_word: self.inside_words,
                propagate_case: self.follow_case && !self.match_case,
                ending: self.ending.clone(),
                only_apps: only_apps.clone(),
                skip_apps: skip_apps.clone(),
            })
            .collect()
    }

    /// A policy with only the timing set: tests with a chosen global mode, and the check of a timing change before it is saved. Input: the mode. Returns the defaults with that mode.
    pub fn with_mode(mode: Mode) -> Policy {
        Policy { mode, ..Policy::default() }
    }
}
