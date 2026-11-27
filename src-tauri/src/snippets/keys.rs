//! File: snippets/keys.rs
//! Purpose: The settings of the Snippets page in one place: each key, its default and its limits, read by the rules loader (`policy`), the expander (`expand`) and the settings door (`settings_door::validate`), so a key is spelled and bounded once.
//! Contents: the key names and defaults; MAX_MS / MAX_ENDING / MAX_SKIP_APPS — the limits; check — whether a write to one of these keys is allowed.
//! The page itself (`src/snippets.ts`, `SETTING`) spells the same keys and defaults; `tests` pins that the two lists agree.

use serde_json::Value;

use super::model::Mode;

pub const EXPAND: &str = "snippets_expand";
pub const SKIP_APPS: &str = "snippets_skip_apps";
pub const UNDO: &str = "snippets_undo";
pub const CONFIRM_DELETE: &str = "snippets_confirm_delete";
/// Plays the Notes alert tone, at the Notes alert volume, after an expansion was delivered (off by default).
pub const SOUND: &str = "snippets_sound";
pub const MODE: &str = "snippets_mode";
pub const MATCH_CASE: &str = "snippets_match_case";
pub const FOLLOW_CASE: &str = "snippets_follow_case";
pub const INSIDE_WORDS: &str = "snippets_inside_words";
pub const ENDING: &str = "snippets_ending";
/// Injection Delay in milliseconds (the page offers none, 150, 300 and 600): how long a completed trigger waits for a longer one. Decided, not measured: a fast typist's gap between keys is 100 to 200 ms, so 150 catches a trigger typed in one go, 600 one typed slowly.
pub const DELAY_MS: &str = "snippets_delay_ms";
/// Response Time in milliseconds (the page offers none, 100, 250 and 600): how much longer a paste keeps the text for the target. Decided, not measured; `shortcut:` a target that needs more than 600 ms still loses the paste, upgrade by measuring one on the bench.
pub const RESPONSE_MS: &str = "snippets_response_ms";

pub const DEFAULT_EXPAND: bool = true;
pub const DEFAULT_UNDO: bool = false;
pub const DEFAULT_SOUND: bool = false;
pub const DEFAULT_MATCH_CASE: bool = false;
pub const DEFAULT_FOLLOW_CASE: bool = true;
pub const DEFAULT_INSIDE_WORDS: bool = false;

/// The most a delay or a response time may be, in milliseconds.
pub const MAX_MS: u64 = 1000;
/// The most characters of the ending setting.
pub const MAX_ENDING: usize = 20;
const MAX_SKIP_APPS: usize = 4_000;

/// The keys that hold on/off values.
const BOOLEANS: &[&str] = &[EXPAND, UNDO, CONFIRM_DELETE, SOUND, MATCH_CASE, FOLLOW_CASE, INSIDE_WORDS];

/// Whether a write to one of the page's keys is allowed. Inputs: key and value. Returns None when the key is not one of them, else whether the value is allowed.
pub fn check(key: &str, value: &Value) -> Option<bool> {
    Some(match key {
        k if BOOLEANS.contains(&k) => value.is_boolean(),
        SKIP_APPS => value.as_str().is_some_and(|v| v.chars().count() <= MAX_SKIP_APPS),
        ENDING => value.as_str().is_some_and(|v| v.chars().count() <= MAX_ENDING && !v.chars().any(char::is_control)),
        MODE => serde_json::from_value::<Mode>(value.clone()).is_ok(),
        DELAY_MS | RESPONSE_MS => value.as_u64().is_some_and(|v| v <= MAX_MS),
        _ => return None,
    })
}
