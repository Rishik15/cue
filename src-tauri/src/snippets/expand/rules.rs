//! File: snippets/expand/rules.rs
//! Purpose: The rules every snippet follows (when a trigger expands, whether case matters, how the text goes in), read from the settings on the Snippets page instead of being kept per snippet.
//! Contents: Rules — the values; load — read them from the settings; apply — a trigger with the rules applied; insert_for — how one snippet's text goes in.

use super::super::model::{Effective, Insert, Mode};
use crate::settings;

/// The settings keys of the Snippets page.
pub const MODE: &str = "snippets_mode";
pub const MATCH_CASE: &str = "snippets_match_case";
pub const FOLLOW_CASE: &str = "snippets_follow_case";
pub const INSIDE_WORDS: &str = "snippets_inside_words";
pub const ENDING: &str = "snippets_ending";
pub const INSERT: &str = "snippets_insert";

/// The values read from the settings: when a trigger expands, whether case matters, what ends a trigger and how the text goes in.
pub struct Rules {
    mode: Mode,
    match_case: bool,
    follow_case: bool,
    inside_words: bool,
    ending: String,
    insert: Insert,
}

/// Reads the rules from the settings. Returns them; a hand-edited file gets the same limits `settings::validate` gives the page.
pub fn load() -> Rules {
    Rules {
        mode: Mode::parse(&settings::get_str(MODE).unwrap_or_default()),
        match_case: settings::get_bool(MATCH_CASE, false),
        follow_case: settings::get_bool(FOLLOW_CASE, true),
        inside_words: settings::get_bool(INSIDE_WORDS, false),
        ending: settings::get_str(ENDING).unwrap_or_default().chars().filter(|c| !c.is_control()).take(20).collect(),
        insert: insert_setting(settings::get_str(INSERT).as_deref()),
    }
}

/// How the text goes in: Auto (typed in one batch with the erase when short, pasted otherwise) unless the setting says paste or slow. The platform layer makes Auto paste on weak and remote machines, where a typed burst is dropped (`insert::place`).
fn insert_setting(value: Option<&str>) -> Insert {
    match value {
        Some("paste") => Insert::Paste,
        Some("slow") => Insert::Slow,
        _ => Insert::Auto,
    }
}

impl Rules {
    /// A trigger with the rules applied. Input: the snippet's own settings. Returns them with the rules in force.
    pub fn apply(&self, e: Effective) -> Effective {
        Effective {
            mode: self.mode,
            match_case: self.match_case,
            inside_word: self.inside_words,
            propagate_case: self.follow_case && !self.match_case,
            ending: self.ending.clone(),
            ..e
        }
    }

    /// A formatted snippet stays formatted; every other one goes in the way the rules say.
    pub fn insert_for(&self, own: Insert) -> Insert {
        if own == Insert::Rich {
            own
        } else {
            self.insert
        }
    }
}
