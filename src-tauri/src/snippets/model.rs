//! File: snippets/model.rs
//! Purpose: What a snippet and a folder are: the stored rows, the drafts being saved, how a trigger expands and how its text goes in.
//! Contents: Snippet / Draft / Folder / FolderDraft — rows and drafts; Mode / When — when a trigger expands (as typed, or after a space) and what Auto resolves to; Insert — how the text goes in. The serde attributes of `Mode` and `Insert` are the one source of their names: `name` and `parse` read them.
//! The limits and tidying of drafts are in `check`; the triggers a snippet gives under the page's rules are in `policy`.

use serde::de::value::{Error, StrDeserializer};
use serde::de::{DeserializeOwned, IntoDeserializer};
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

/// The name serde gives a unit variant: the one spelling for the file, the database and the page.
fn name_of<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default()
}

/// The variant serde reads from a stored name (aliases included), or the default when none matches.
fn parse_name<T: DeserializeOwned + Default>(name: &str) -> T {
    let name: StrDeserializer<'_, Error> = name.into_deserializer();
    T::deserialize(name).unwrap_or_default()
}

/// What `Mode` comes to for one trigger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum When {
    Immediate,
    Delimiter,
}

/// Reading a mode and resolving Auto for one trigger.
impl Mode {
    /// Auto fires at once when the trigger is already complete: it starts with a symbol (`:sig`) or ends with a space or punctuation (`i `); otherwise a trigger that starts with a letter (`sig`) waits for the space or punctuation that ends the word. A pattern (`re:...`) has no first character to tell by, so Auto means as soon as it matches.
    pub fn when(self, trigger: &str) -> When {
        match self {
            Mode::Immediate => When::Immediate,
            Mode::Delimiter => When::Delimiter,
            Mode::Auto if trigger.starts_with(PREFIX) => When::Immediate,
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

    /// The mode a stored name stands for; an unknown name is Auto.
    pub fn parse(name: &str) -> Mode {
        parse_name(name)
    }
}

/// How the expansion is put into the app.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Insert {
    /// The text is pasted as plain text. Any other stored name reads as this.
    #[default]
    Paste,
    /// The text is Markdown: pasted with formatting (bold, links, lists) for apps that take it, as plain text for the rest.
    Rich,
}

/// Reading and writing an insert by its stored name.
impl Insert {
    /// The name stored and exported, as serde spells it.
    pub fn name(self) -> String {
        name_of(&self)
    }

    /// The insert a stored name stands for; an unknown name is Paste.
    pub fn parse(name: &str) -> Insert {
        parse_name(name)
    }
}

/// A stored snippet as the page and the expander read it.
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

/// A snippet being saved or imported, before it has an identity.
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

/// The serde default for switches that start on. Returns true.
fn yes() -> bool {
    true
}

/// An empty draft with the options a new snippet starts with.
impl Default for Draft {
    /// Returns the empty draft.
    fn default() -> Self {
        Draft {
            name: String::new(),
            triggers: Vec::new(),
            body: String::new(),
            folder_id: None,
            mode: Mode::Auto,
            match_case: false,
            enabled: true,
            insert: Insert::Paste,
            ending: String::new(),
            inside_word: false,
            propagate_case: true,
            only_apps: Vec::new(),
            skip_apps: Vec::new(),
        }
    }
}

/// A group of snippets that share a switch and program lists.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Folder {
    pub id: i64,
    pub name: String,
    pub enabled: bool,
    pub only_apps: Vec<String>,
    pub skip_apps: Vec<String>,
}

/// A folder being saved, before it has an identity.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FolderDraft {
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub only_apps: Vec<String>,
    #[serde(default)]
    pub skip_apps: Vec<String>,
}
