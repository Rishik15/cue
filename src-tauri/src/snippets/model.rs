//! Purpose: What a snippet and a folder are: the stored rows, the drafts being saved, how a trigger expands and how its text goes in, and the rules a snippet and its folder come to together.
//! Contents: Snippet / Draft / Folder / FolderDraft — rows and drafts; Mode / When — when a trigger expands (as typed, or after a space) and what Auto resolves to; Insert — how the text goes in; Effective / effective — one trigger
//! with every option applied (the folder's prefix, apps and switch included). The limits and tidying of drafts are in `check`.

use serde::{Deserialize, Serialize};

use super::check::DELIMITERS;
use super::expand::pattern::PREFIX;

/// When a typed trigger turns into its snippet.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// As soon as the trigger is typed if it starts with a symbol (`:sig`), after a space or punctuation if it starts with a letter (`sig`).
    #[default]
    Auto,
    Immediate,
    Delimiter,
}

/// What `Mode` comes to for one trigger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum When {
    Immediate,
    Delimiter,
}

impl Mode {
    /// Auto fires at once when the trigger is already complete: it starts with a symbol (`:sig`) or ends with a space or punctuation (`i `); otherwise a trigger that starts with a letter (`sig`) waits for the space or punctuation that ends the word.
    pub fn when(self, trigger: &str) -> When {
        match self {
            Mode::Immediate => When::Immediate,
            Mode::Delimiter => When::Delimiter,
            Mode::Auto => {
                let starts_with_letter = trigger.trim().chars().next().is_some_and(char::is_alphanumeric);
                let ends_with_ending = trigger.chars().last().is_some_and(|c| DELIMITERS.contains(&c));
                if starts_with_letter && !ends_with_ending {
                    When::Delimiter
                } else {
                    When::Immediate
                }
            }
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Mode::Auto => "auto",
            Mode::Immediate => "immediate",
            Mode::Delimiter => "delimiter",
        }
    }

    pub fn parse(name: &str) -> Mode {
        match name {
            "immediate" => Mode::Immediate,
            "delimiter" => Mode::Delimiter,
            _ => Mode::Auto,
        }
    }
}

/// How the expansion is put into the app.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Insert {
    /// Typed when short and single-line, pasted otherwise.
    #[default]
    Auto,
    Paste,
    /// One character at a time with a short pause, for remote sessions and apps that drop fast input.
    Slow,
    /// The text is Markdown: pasted with formatting (bold, links, lists) for apps that take it, as plain text for the rest.
    Rich,
}

impl Insert {
    pub fn name(self) -> &'static str {
        match self {
            Insert::Auto => "auto",
            Insert::Paste => "paste",
            Insert::Slow => "slow",
            Insert::Rich => "rich",
        }
    }

    pub fn parse(name: &str) -> Insert {
        match name {
            "paste" => Insert::Paste,
            "slow" => Insert::Slow,
            "rich" => Insert::Rich,
            _ => Insert::Auto,
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Snippet {
    pub id: i64,
    pub name: String,
    /// What is typed to get the text; several are allowed.
    pub triggers: Vec<String>,
    pub body: String,
    pub uses: i64,
    /// When it was last used (seconds since 1970), 0 for never.
    pub last_used: i64,
    pub folder_id: Option<i64>,
    pub mode: Mode,
    pub match_case: bool,
    pub enabled: bool,
    pub insert: Insert,
    /// Characters that end a trigger for this snippet; empty means the usual ones (space and punctuation).
    pub ending: String,
    /// A trigger that starts with a letter may also start inside a word.
    pub inside_word: bool,
    /// `:Sig` types the text with a capital, `:SIG` in capitals (only when case does not matter).
    pub propagate_case: bool,
    /// When not empty, the only programs where this snippet expands.
    pub only_apps: Vec<String>,
    /// Programs where this snippet never expands.
    pub skip_apps: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Draft {
    pub name: String,
    #[serde(default)]
    pub triggers: Vec<String>,
    pub body: String,
    #[serde(default)]
    pub folder_id: Option<i64>,
    #[serde(default)]
    pub mode: Mode,
    #[serde(default)]
    pub match_case: bool,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub insert: Insert,
    #[serde(default)]
    pub ending: String,
    #[serde(default)]
    pub inside_word: bool,
    #[serde(default = "yes")]
    pub propagate_case: bool,
    #[serde(default)]
    pub only_apps: Vec<String>,
    #[serde(default)]
    pub skip_apps: Vec<String>,
}

fn yes() -> bool {
    true
}

impl Default for Draft {
    fn default() -> Self {
        Draft {
            name: String::new(),
            triggers: Vec::new(),
            body: String::new(),
            folder_id: None,
            mode: Mode::Auto,
            match_case: false,
            enabled: true,
            insert: Insert::Auto,
            ending: String::new(),
            inside_word: false,
            propagate_case: true,
            only_apps: Vec::new(),
            skip_apps: Vec::new(),
        }
    }
}

/// A group of snippets that share a switch, a trigger prefix and program lists.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Folder {
    pub id: i64,
    pub name: String,
    pub enabled: bool,
    /// Typed before every trigger of the folder's snippets (`;` makes `sig` into `;sig`).
    pub prefix: String,
    pub only_apps: Vec<String>,
    pub skip_apps: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FolderDraft {
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub only_apps: Vec<String>,
    #[serde(default)]
    pub skip_apps: Vec<String>,
}

/// One trigger with everything that applies to it worked out: the folder's prefix is in front, a disabled snippet or folder gives none.
#[derive(Clone, Debug, PartialEq)]
pub struct Effective {
    pub id: i64,
    pub trigger: String,
    pub mode: Mode,
    pub match_case: bool,
    pub inside_word: bool,
    pub propagate_case: bool,
    pub ending: String,
    pub only_apps: Vec<String>,
    pub skip_apps: Vec<String>,
}

/// The triggers a snippet gives inside its folder. The snippet's own list of allowed programs wins over the folder's; the lists of skipped programs add up.
pub fn effective(s: &Snippet, folder: Option<&Folder>) -> Vec<Effective> {
    if !s.enabled || folder.is_some_and(|f| !f.enabled) {
        return Vec::new();
    }
    let prefix = folder.map_or("", |f| f.prefix.as_str());
    let only_apps = if s.only_apps.is_empty() { folder.map(|f| f.only_apps.clone()).unwrap_or_default() } else { s.only_apps.clone() };
    let skip_apps = s.skip_apps.iter().chain(folder.iter().flat_map(|f| f.skip_apps.iter())).cloned().collect::<Vec<_>>();
    s.triggers
        .iter()
        .filter(|t| !t.is_empty())
        .map(|t| Effective {
            id: s.id,
            // A pattern is a regular expression, not typed text: a prefix in front would change what it matches.
            trigger: if t.starts_with(PREFIX) { t.clone() } else { format!("{prefix}{t}") },
            mode: s.mode,
            match_case: s.match_case,
            inside_word: s.inside_word,
            propagate_case: s.propagate_case && !s.match_case,
            ending: s.ending.clone(),
            only_apps: only_apps.clone(),
            skip_apps: skip_apps.clone(),
        })
        .collect()
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
