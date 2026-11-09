//! File: text/spans.rs
//! Purpose: What counts as a part of the text that must survive an edit: one place that finds links, emails, paths, commands, amounts and code, shared by the reply check (which compares them) and the masking (which lifts them out).
//! Contents: MASKABLE — the spans a model should never see; PROTECTED — those plus mentions, tags and every digit run.
//! Windows and Unix paths, bare commands (a known tool, an optional subcommand, then only option-like or path-like arguments) and currency amounts are covered; ordinary prose after a command is not swallowed.
//! shortcut: the command list is fixed to common tools; a rare tool's command line is protected only for its digits and links. Upgrade: a user-editable list.

use std::sync::LazyLock;

use regex::Regex;

const FENCE: &str = r"(?s)```.*?```";
const CODE: &str = r"`[^`\n]+`";
const URL: &str = r"https?://\S+";
const EMAIL: &str = r"[\w.+-]+@[\w-]+(?:\.[\w-]+)+";
const WINDOWS_PATH: &str = r#"[A-Za-z]:\\[^\s"'<>|*?]+|\\\\[\w.$-]+\\[^\s"'<>|*?]+"#;
const UNIX_PATH: &str = r"(?:~|\.{1,2})?/[\w.@+-]+(?:/[\w.@+-]+)+";
const COMMAND: &str = r"\b(?:git|npm|npx|pnpm|yarn|cargo|rustup|pip|uv|docker|kubectl|winget|choco|curl|ssh|sudo)(?: [a-z][\w-]*)?(?: (?:-{1,2}[\w-]+(?:=\S+)?|[\w.~]*[./\\:=@]\S*))*";
const AMOUNT: &str = r"[$€£¥]\s?\d(?:[\d.,]*\d)?";
const MENTION: &str = r"[@#]\w+";
const NUMBER: &str = r"\d(?:[\d.,:/-]*\d)?";

/// Builds a regex from alternatives, in order (the first that matches at a position wins). Input: the patterns. Returns the regex.
fn any_of(parts: &[&str]) -> Regex {
    Regex::new(&parts.iter().map(|p| format!("(?:{p})")).collect::<Vec<_>>().join("|")).expect("valid pattern")
}

/// The spans lifted out of a text before the model sees it: code, links, emails, paths, commands and currency amounts.
pub static MASKABLE: LazyLock<Regex> = LazyLock::new(|| any_of(&[FENCE, CODE, URL, EMAIL, WINDOWS_PATH, UNIX_PATH, COMMAND, AMOUNT]));

/// What must survive an edit untouched: the maskable spans, @mentions and #tags, and numbers with their separators.
pub static PROTECTED: LazyLock<Regex> = LazyLock::new(|| any_of(&[FENCE, CODE, URL, EMAIL, WINDOWS_PATH, UNIX_PATH, COMMAND, AMOUNT, MENTION, NUMBER]));
