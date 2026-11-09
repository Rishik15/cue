//! File: actions/catalog.rs
//! Purpose: The one list of exact and model actions: the enum that rules store by name, and for each action its label, hint, where it applies, which palette section it belongs to and what runs it.
//! Contents: Action — every action; META — one row per action, in menu order, indexed by the action; Meta — a row; Kind — an exact function or a model task; Section — the palette section; Source — where the text came from (selected text or the screen).
//! The scope is for the backend only: it decides where an action is offered (after dictation, on selected text, on text read from the screen); the user is never shown it.

use serde::{Deserialize, Serialize};

use super::model::Task;
use super::{convert, extract, format, links, model, model_extra, speech};

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

/// What runs an action: a pure function over the text, or a task for the text model.
#[derive(Clone, Copy)]
pub enum Kind {
    Exact(fn(&str) -> String),
    Model(&'static Task),
}

/// One row of the action table: the action, its text for the editors and palettes, where it is offered, its palette section and what runs it.
pub struct Meta {
    pub action: Action,
    pub label: &'static str,
    pub hint: &'static str,
    pub(super) scope: u8,
    pub section: Section,
    pub kind: Kind,
}

/// Declares the enum and its table together, so an action cannot exist without a row. The table is in declaration order, so an action's discriminant is its row index.
macro_rules! catalog {
    ($($variant:ident => $label:literal, $hint:literal, $scope:expr, $section:ident, $kind:expr;)*) => {
        #[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
        #[serde(rename_all = "snake_case")]
        pub enum Action { $($variant),* }

        pub static META: &[Meta] = &[$(Meta { action: Action::$variant, label: $label, hint: $hint, scope: $scope, section: Section::$section, kind: $kind }),*];
    };
}

use Kind::{Exact, Model};

catalog! {
    CleanWhitespace => "Clean Whitespace", "Trim lines and collapse spaces and blank lines", D | T | C, Clean, Exact(format::clean_whitespace);
    JoinLines => "Join Broken Lines", "Rejoin hard-wrapped text, such as from a PDF", T | C, Clean, Exact(format::join_lines);
    RemoveFillers => "Remove Filler Words", "Remove um, uh, er and hmm", D | T, Clean, Exact(speech::remove_fillers);
    RemoveRepeats => "Remove Repeated Words", "Remove a word said twice in a row", D | T, Clean, Exact(speech::remove_repeats);
    PlainQuotes => "Use Plain Quotes", "Replace curly quotes with straight ones", T | C, Clean, Exact(convert::plain_quotes);
    CleanLinks => "Clean Links", "Strip tracking parameters from links", T | C, Clean, Exact(links::clean);
    DedupeLines => "Remove Duplicate Lines", "Keep the first of each repeated line", T | C, Clean, Exact(convert::dedupe_lines);
    UpperCase => "Uppercase", "Convert to capital letters", D | T | C, Convert, Exact(str::to_uppercase);
    LowerCase => "Lowercase", "Convert to small letters", D | T | C, Convert, Exact(str::to_lowercase);
    TitleCase => "Title Case", "Capitalize each word", D | T | C, Convert, Exact(format::title_case);
    SentenceCase => "Sentence Case", "Capitalize each sentence", D | T | C, Convert, Exact(format::sentence_case);
    CamelCase => "Camel Case", "One camelCase name per line", T | C, Convert, Exact(convert::camel_case);
    SnakeCase => "Snake Case", "One snake_case name per line", T | C, Convert, Exact(convert::snake_case);
    KebabCase => "Kebab Case", "One kebab-case name per line", T | C, Convert, Exact(convert::kebab_case);
    SplitBullets => "Split Into Bullets", "One bullet per line, or per sentence", D | T | C, Convert, Exact(format::split_bullets);
    SortLines => "Sort Lines", "Sort lines alphabetically", T | C, Convert, Exact(convert::sort_lines);
    SpokenNumbers => "Spoken Numbers to Digits", "Write numbers in digits, such as 25", D | T, Convert, Exact(speech::spoken_numbers);
    JsonPretty => "Format JSON", "Indent valid JSON", T | C, Convert, Exact(convert::json_pretty);
    JsonMinify => "Minify JSON", "Put valid JSON on one line", T | C, Convert, Exact(convert::json_minify);
    ExtractEmails => "Extract Emails", "Keep only email addresses", T | C, Extract, Exact(extract::emails);
    ExtractUrls => "Extract Links", "Keep only links", T | C, Extract, Exact(extract::urls);
    ExtractPhones => "Extract Phone Numbers", "Keep only phone numbers", T | C, Extract, Exact(extract::phones);
    Correct => "Correct Text", "Fix spelling, grammar and punctuation", D | T | C, Ai, Model(&model::CORRECT);
    MakeShorter => "Make Shorter", "Say the same in fewer words", D | T | C, Ai, Model(&model::SHORTER);
    MakeFormal => "Make Formal", "Rewrite in a formal tone", D | T | C, Ai, Model(&model::FORMAL);
    MakeFriendly => "Make Friendly", "Rewrite in a warm tone", D | T | C, Ai, Model(&model::FRIENDLY);
    TidySpeech => "Tidy Spoken Text", "Remove false starts, stutters and repeats", D | T | C, Ai, Model(&model::TIDY);
    BulletList => "Make Bullet List", "Turn the text into a bulleted list", D | T | C, Ai, Model(&model::BULLETS);
    Summarize => "Summarize", "Condense to a sentence or two", T | C, Ai, Model(&model::SUMMARY);
    Simplify => "Make Simpler", "Rewrite in plain words", T | C, Ai, Model(&model::SIMPLER);
    ExplainSimply => "Explain Simply", "Explain in plain words", T | C, Ai, Model(&model_extra::EXPLAIN);
    WriteReply => "Write a Reply", "Draft a short reply", T | C, Ai, Model(&model_extra::REPLY);
    FindTasks => "Find Action Items", "List the tasks the text asks for", D | T | C, Ai, Model(&model_extra::ACTION_ITEMS);
    TranslateEnglish => "Translate to English", "Translate into English", D | T | C, Ai, Model(&model_extra::TRANSLATE);
}

impl Meta {
    pub(super) fn offered_after_dictation(&self) -> bool {
        self.scope & D != 0
    }
}
