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
    /// The scope bit of this source. Return: `T` for a selection, `C` for a capture.
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
    /// The stable name stored in rules: the serde name of the variant (a test keeps them equal).
    pub id: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    pub(super) scope: u8,
    pub section: Section,
    pub kind: Kind,
}

/// Declares the enum and its table together, so an action cannot exist without a row. The table is in declaration order, so an action's discriminant is its row index.
macro_rules! catalog {
    ($($variant:ident => $id:literal, $label:literal, $hint:literal, $scope:expr, $section:ident, $kind:expr;)*) => {
        #[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
        #[serde(rename_all = "snake_case")]
        pub enum Action { $($variant),* }

        pub static META: &[Meta] = &[$(Meta { action: Action::$variant, id: $id, label: $label, hint: $hint, scope: $scope, section: Section::$section, kind: $kind }),*];
    };
}

use Kind::{Exact, Model};

catalog! {
    CleanWhitespace => "clean_whitespace", "Clean Whitespace", "Trim lines; collapse spaces and blank lines", D | T | C, Clean, Exact(format::clean_whitespace);
    JoinLines => "join_lines", "Join Lines", "Rejoin hard-wrapped text from PDFs", T | C, Clean, Exact(format::join_lines);
    RemoveFillers => "remove_fillers", "Remove Fillers", "Remove um, uh, er and hmm", D | T, Clean, Exact(speech::remove_fillers);
    RemoveRepeats => "remove_repeats", "Remove Repeats", "Remove adjacent repeated words", D | T, Clean, Exact(speech::remove_repeats);
    PlainQuotes => "plain_quotes", "Plain Quotes", "Straighten curly quotes", T | C, Clean, Exact(convert::plain_quotes);
    CleanLinks => "clean_links", "Clean Links", "Remove tracking parameters", T | C, Clean, Exact(links::clean);
    DedupeLines => "dedupe_lines", "Dedupe Lines", "Keep first of each repeated line", T | C, Clean, Exact(convert::dedupe_lines);
    UpperCase => "upper_case", "Uppercase", "Convert to capital letters", D | T | C, Convert, Exact(str::to_uppercase);
    LowerCase => "lower_case", "Lowercase", "Convert to small letters", D | T | C, Convert, Exact(str::to_lowercase);
    TitleCase => "title_case", "Title Case", "Capitalize each word", D | T | C, Convert, Exact(format::title_case);
    SentenceCase => "sentence_case", "Sentence Case", "Capitalize each sentence", D | T | C, Convert, Exact(format::sentence_case);
    CamelCase => "camel_case", "Camel Case", "One camelCase name per line", T | C, Convert, Exact(convert::camel_case);
    SnakeCase => "snake_case", "Snake Case", "One snake_case name per line", T | C, Convert, Exact(convert::snake_case);
    KebabCase => "kebab_case", "Kebab Case", "One kebab-case name per line", T | C, Convert, Exact(convert::kebab_case);
    SplitBullets => "split_bullets", "Split Into Bullets", "One bullet per line, or per sentence", D | T | C, Convert, Exact(format::split_bullets);
    SortLines => "sort_lines", "Sort Lines", "Alphabetize lines", T | C, Convert, Exact(convert::sort_lines);
    SpokenNumbers => "spoken_numbers", "Numbers to Digits", "Write numbers as digits, e.g. 25", D | T, Convert, Exact(speech::spoken_numbers);
    JsonPretty => "json_pretty", "Format JSON", "Indent valid JSON", T | C, Convert, Exact(convert::json_pretty);
    JsonMinify => "json_minify", "Minify JSON", "Put valid JSON on one line", T | C, Convert, Exact(convert::json_minify);
    ExtractEmails => "extract_emails", "Extract Emails", "Keep only email addresses", T | C, Extract, Exact(extract::emails);
    ExtractUrls => "extract_urls", "Extract Links", "Keep only links", T | C, Extract, Exact(extract::urls);
    ExtractPhones => "extract_phones", "Extract Phones", "Keep only phone numbers", T | C, Extract, Exact(extract::phones);
    Correct => "correct", "Correct Text", "Fix spelling, grammar and punctuation", D | T | C, Ai, Model(&model::CORRECT);
    MakeShorter => "make_shorter", "Make Shorter", "Say the same in fewer words", D | T | C, Ai, Model(&model::SHORTER);
    MakeFormal => "make_formal", "Make Formal", "Rewrite in a formal tone", D | T | C, Ai, Model(&model::FORMAL);
    MakeFriendly => "make_friendly", "Make Friendly", "Rewrite in a warm tone", D | T | C, Ai, Model(&model::FRIENDLY);
    TidySpeech => "tidy_speech", "Tidy Speech", "Remove false starts, stutters and repeats", D | T | C, Ai, Model(&model::TIDY);
    BulletList => "bullet_list", "Bullet List", "Turn text into a bulleted list", D | T | C, Ai, Model(&model::BULLETS);
    Summarize => "summarize", "Summarize", "Condense to a sentence or two", T | C, Ai, Model(&model::SUMMARY);
    Simplify => "simplify", "Make Simpler", "Rewrite in plain words", T | C, Ai, Model(&model::SIMPLER);
    ExplainSimply => "explain_simply", "Explain Simply", "Explain in plain words", T | C, Ai, Model(&model_extra::EXPLAIN);
    WriteReply => "write_reply", "Draft Reply", "Write a short reply", T | C, Ai, Model(&model_extra::REPLY);
    FindTasks => "find_tasks", "Find Tasks", "List tasks the text asks for", D | T | C, Ai, Model(&model_extra::ACTION_ITEMS);
    TranslateEnglish => "translate_english", "Translate to English", "Translate into English", D | T | C, Ai, Model(&model_extra::TRANSLATE);
    Quote => "quote", "Quote", "Prefix each line with >", T, Convert, Exact(format::quote);
    Unquote => "unquote", "Unquote", "Remove one > per line", T, Convert, Exact(format::unquote);
}

impl Meta {
    /// Whether the action can be a step of a dictation rule. Return: the answer.
    pub(super) fn offered_after_dictation(&self) -> bool {
        self.scope & D != 0
    }
}
