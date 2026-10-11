//! Purpose: The registry of exact text actions: ordinary code that turns text into text, no model involved. Rules and the selection
//! shortcut run actions by name; a model action (Correct, Rewrite) will join this enum in a later milestone.
//! Contents: Action — one transform, with its stable id, label and hint; apply — run one; ALL — every action in menu order;
//! list_actions — the catalogue for the Rules page. Transforms live in `format`, `extract` and `links`.

mod extract;
mod format;
mod links;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    CleanWhitespace,
    JoinLines,
    UpperCase,
    LowerCase,
    TitleCase,
    SentenceCase,
    CleanLinks,
    ExtractEmails,
    ExtractUrls,
}

impl Action {
    pub const ALL: [Action; 9] = [
        Action::CleanWhitespace,
        Action::JoinLines,
        Action::UpperCase,
        Action::LowerCase,
        Action::TitleCase,
        Action::SentenceCase,
        Action::CleanLinks,
        Action::ExtractEmails,
        Action::ExtractUrls,
    ];

    /// The stable name stored in rules (the serde name, so the file and the UI agree).
    pub fn id(self) -> String {
        serde_json::to_value(self).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default()
    }

    pub fn label(self) -> &'static str {
        match self {
            Action::CleanWhitespace => "Clean Whitespace",
            Action::JoinLines => "Join Broken Lines",
            Action::UpperCase => "Uppercase",
            Action::LowerCase => "Lowercase",
            Action::TitleCase => "Title Case",
            Action::SentenceCase => "Sentence Case",
            Action::CleanLinks => "Clean Links",
            Action::ExtractEmails => "Extract Emails",
            Action::ExtractUrls => "Extract Links",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Action::CleanWhitespace => "Trims lines and removes repeated spaces and blank lines.",
            Action::JoinLines => "Rejoins text that was hard-wrapped, such as copied from a PDF.",
            Action::UpperCase => "Makes every letter capital.",
            Action::LowerCase => "Makes every letter small.",
            Action::TitleCase => "Capitalizes the first letter of each word.",
            Action::SentenceCase => "Capitalizes the first letter of each sentence.",
            Action::CleanLinks => "Removes tracking parameters from links and keeps the rest.",
            Action::ExtractEmails => "Keeps only the email addresses, one per line.",
            Action::ExtractUrls => "Keeps only the links, one per line.",
        }
    }

    /// The transformed text, or None when there is nothing to do (no change, or nothing found), so callers can skip the write.
    pub fn apply(self, text: &str) -> Option<String> {
        let out = match self {
            Action::CleanWhitespace => format::clean_whitespace(text),
            Action::JoinLines => format::join_lines(text),
            Action::UpperCase => text.to_uppercase(),
            Action::LowerCase => text.to_lowercase(),
            Action::TitleCase => format::title_case(text),
            Action::SentenceCase => format::sentence_case(text),
            Action::CleanLinks => links::clean(text),
            Action::ExtractEmails => extract::emails(text),
            Action::ExtractUrls => extract::urls(text),
        };
        (out != text && !out.is_empty()).then_some(out)
    }
}

#[derive(Serialize)]
pub struct ActionInfo {
    id: String,
    label: &'static str,
    hint: &'static str,
}

#[tauri::command]
pub fn list_actions() -> Vec<ActionInfo> {
    Action::ALL.iter().map(|a| ActionInfo { id: a.id(), label: a.label(), hint: a.hint() }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_snake_case_and_round_trip() {
        for a in Action::ALL {
            let back: Action = serde_json::from_str(&format!("\"{}\"", a.id())).unwrap();
            assert_eq!(a, back);
        }
        assert_eq!(Action::CleanLinks.id(), "clean_links");
    }

    #[test]
    fn nothing_to_do_is_none() {
        assert_eq!(Action::UpperCase.apply("ABC"), None);
        assert_eq!(Action::ExtractEmails.apply("no address here"), None);
        assert_eq!(Action::LowerCase.apply("ABC").as_deref(), Some("abc"));
    }
}
