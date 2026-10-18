//! Purpose: The rules every snippet follows (when a trigger expands, whether case matters, how the text goes in), read from the settings on the Snippets page instead of being kept per snippet.
//! Contents: Rules — the values; load — read them from the settings; apply — a trigger with the rules applied; insert_for — how one snippet's text goes in.

use tauri::AppHandle;

use super::super::model::{Effective, Insert, Mode};
use crate::settings;

pub const MODE: &str = "snippets_mode";
pub const MATCH_CASE: &str = "snippets_match_case";
pub const FOLLOW_CASE: &str = "snippets_follow_case";
pub const INSIDE_WORDS: &str = "snippets_inside_words";
pub const ENDING: &str = "snippets_ending";
pub const INSERT: &str = "snippets_insert";

pub struct Rules {
    mode: Mode,
    match_case: bool,
    follow_case: bool,
    inside_words: bool,
    ending: String,
    insert: Insert,
}

pub fn load(app: &AppHandle) -> Rules {
    Rules {
        mode: Mode::parse(&settings::get_str(app, MODE).unwrap_or_default()),
        match_case: settings::get_bool(app, MATCH_CASE, false),
        follow_case: settings::get_bool(app, FOLLOW_CASE, true),
        inside_words: settings::get_bool(app, INSIDE_WORDS, false),
        // A hand-edited store gets the same limits `settings::validate` gives the page.
        ending: settings::get_str(app, ENDING).unwrap_or_default().chars().filter(|c| !c.is_control()).take(20).collect(),
        insert: Insert::parse(&settings::get_str(app, INSERT).unwrap_or_default()),
    }
}

impl Rules {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> Rules {
        Rules { mode: Mode::Immediate, match_case: true, follow_case: true, inside_words: false, ending: "-".into(), insert: Insert::Slow }
    }

    #[test]
    fn the_rules_replace_a_trigger_s_own_options_and_case_following_needs_case_not_to_matter() {
        let e = Effective {
            id: 1,
            trigger: ":a".into(),
            mode: Mode::Auto,
            match_case: false,
            inside_word: true,
            propagate_case: true,
            ending: String::new(),
            only_apps: vec!["code".into()],
            skip_apps: vec![],
        };
        let applied = rules().apply(e);
        assert_eq!((applied.mode, applied.match_case, applied.inside_word, applied.propagate_case), (Mode::Immediate, true, false, false));
        assert_eq!((applied.ending.as_str(), applied.only_apps.len()), ("-", 1), "programs still come from the snippet and its folder");
    }

    #[test]
    fn a_formatted_snippet_stays_formatted() {
        assert_eq!((rules().insert_for(Insert::Rich), rules().insert_for(Insert::Auto)), (Insert::Rich, Insert::Slow));
    }
}
