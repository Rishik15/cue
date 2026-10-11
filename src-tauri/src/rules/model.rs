//! Purpose: The stored shape of a rule: what starts it, which texts it matches, what it does, and where the result goes.
//! Contents: Rule — one `trigger -> conditions -> actions -> output` rule; Trigger / Match / Condition / Kind / Output — its parts.
//! This is the file format of `rules.json`, so names are stable: change them only with a version bump.

use serde::{Deserialize, Serialize};

use crate::actions::Action;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Rule {
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    pub trigger: Trigger,
    /// Program names the rule is limited to ("slack", "chrome"); empty means any program.
    #[serde(default)]
    pub apps: Vec<String>,
    #[serde(default)]
    pub when: Match,
    #[serde(default)]
    pub conditions: Vec<Condition>,
    /// Run in order; each gets the previous result.
    pub actions: Vec<Action>,
    #[serde(default)]
    pub output: Output,
    /// No later rule runs for this text once this one has matched.
    #[serde(default)]
    pub stop: bool,
}

fn yes() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    /// A transcript is ready, before it is typed.
    Dictation,
    /// The selection shortcut was pressed on some selected text.
    Selection,
}

/// How the conditions combine. A rule with no conditions always matches.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Match {
    #[default]
    All,
    Any,
    None,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Condition {
    /// Text conditions ignore letter case.
    Contains {
        value: String,
    },
    StartsWith {
        value: String,
    },
    EndsWith {
        value: String,
    },
    Matches {
        pattern: String,
    },
    LongerThan {
        chars: usize,
    },
    ShorterThan {
        chars: usize,
    },
    LooksLike {
        kind: Kind,
    },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Url,
    Email,
    Json,
    List,
}

/// Where the final text goes. Dictation inserts by default; the selection shortcut replaces the selection.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Output {
    #[default]
    Insert,
    Copy,
}
