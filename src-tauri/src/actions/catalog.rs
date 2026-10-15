//! Purpose: The one list of exact and model actions: the enum that rules store by name, and for each action its label, hint, where it applies and which palette section it belongs to.
//! Contents: Action — every action; META — one row per action, in menu order; Meta — a row; Section — the palette section; Source — where the text came from (selected text or the screen).
//! The scope is for the backend only: it decides where an action is offered (after dictation, on selected text, on text read from the screen); the user is never shown it.

use serde::{Deserialize, Serialize};

const D: u8 = 1; // dictated text
const T: u8 = 2; // selected text
const C: u8 = 4; // text read from the screen

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    Selection,
    Capture,
}

impl Source {
    pub(super) fn bit(self) -> u8 {
        match self {
            Source::Selection => T,
            Source::Capture => C,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Section {
    Clean,
    Convert,
    Extract,
    Ai,
}

pub struct Meta {
    pub action: Action,
    pub label: &'static str,
    pub hint: &'static str,
    pub(super) scope: u8,
    pub section: Section,
}

/// Declares the enum and its table together, so an action cannot exist without a row.
macro_rules! catalog {
    ($($variant:ident => $label:literal, $hint:literal, $scope:expr, $section:ident;)*) => {
        #[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
        #[serde(rename_all = "snake_case")]
        pub enum Action { $($variant),* }

        pub static META: &[Meta] = &[$(Meta { action: Action::$variant, label: $label, hint: $hint, scope: $scope, section: Section::$section }),*];
    };
}

catalog! {
    CleanWhitespace => "Clean Whitespace", "Trim lines and collapse spaces and blank lines", D | T | C, Clean;
    JoinLines => "Join Broken Lines", "Rejoin hard-wrapped text, such as from a PDF", T | C, Clean;
    RemoveFillers => "Remove Filler Words", "Remove um, uh, er and hmm", D | T, Clean;
    RemoveRepeats => "Remove Repeated Words", "Remove a word said twice in a row", D | T, Clean;
    PlainQuotes => "Use Plain Quotes", "Replace curly quotes with straight ones", T | C, Clean;
    CleanLinks => "Clean Links", "Strip tracking parameters from links", T | C, Clean;
    DedupeLines => "Remove Duplicate Lines", "Keep the first of each repeated line", T | C, Clean;
    UpperCase => "Uppercase", "Convert to capital letters", D | T | C, Convert;
    LowerCase => "Lowercase", "Convert to small letters", D | T | C, Convert;
    TitleCase => "Title Case", "Capitalize each word", D | T | C, Convert;
    SentenceCase => "Sentence Case", "Capitalize each sentence", D | T | C, Convert;
    CamelCase => "Camel Case", "One camelCase name per line", T | C, Convert;
    SnakeCase => "Snake Case", "One snake_case name per line", T | C, Convert;
    KebabCase => "Kebab Case", "One kebab-case name per line", T | C, Convert;
    SplitBullets => "Split Into Bullets", "One bullet per line, or per sentence", D | T | C, Convert;
    SortLines => "Sort Lines", "Sort lines alphabetically", T | C, Convert;
    SpokenNumbers => "Spoken Numbers to Digits", "Write numbers in digits, such as 25", D | T, Convert;
    JsonPretty => "Format JSON", "Indent valid JSON", T | C, Convert;
    JsonMinify => "Minify JSON", "Put valid JSON on one line", T | C, Convert;
    ExtractEmails => "Extract Emails", "Keep only email addresses", T | C, Extract;
    ExtractUrls => "Extract Links", "Keep only links", T | C, Extract;
    ExtractPhones => "Extract Phone Numbers", "Keep only phone numbers", T | C, Extract;
    Correct => "Correct Text", "Fix spelling, grammar and punctuation", D | T | C, Ai;
    MakeShorter => "Make Shorter", "Say the same in fewer words", D | T | C, Ai;
    MakeFormal => "Make Formal", "Rewrite in a formal tone", D | T | C, Ai;
    MakeFriendly => "Make Friendly", "Rewrite in a warm tone", D | T | C, Ai;
    TidySpeech => "Tidy Spoken Text", "Remove false starts, stutters and repeats", D | T | C, Ai;
    BulletList => "Make Bullet List", "Turn the text into a bulleted list", D | T | C, Ai;
    Summarize => "Summarize", "Condense to a sentence or two", T | C, Ai;
    Simplify => "Make Simpler", "Rewrite in plain words", T | C, Ai;
    ExplainSimply => "Explain Simply", "Explain in plain words", T | C, Ai;
    WriteReply => "Write a Reply", "Draft a short reply", T | C, Ai;
    FindTasks => "Find Action Items", "List the tasks the text asks for", D | T | C, Ai;
    TranslateEnglish => "Translate to English", "Translate into English", D | T | C, Ai;
}

impl Meta {
    pub(super) fn offered_after_dictation(&self) -> bool {
        self.scope & D != 0
    }
}
