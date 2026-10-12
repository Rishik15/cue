//! Purpose: The registry of exact text actions: ordinary code that turns text into text, no model involved. Rules and the selection
//! shortcut run actions by name; the model actions (Correct and the rewrites) use the text model, their tasks live in `model`.
//! Contents: Action — one transform, with its stable id, label and hint; apply — run one; needs_model — whether it uses the text model; ALL — every action in menu order;
//! for_dictation — whether an action is offered after dictation; list_actions — the catalogue the action editors read. Transforms live in `format`, `extract` and `links`.

mod extract;
mod format;
mod links;
mod model;

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
    Correct,
    MakeShorter,
    MakeFormal,
    MakeFriendly,
    TidySpeech,
    BulletList,
    Summarize,
    Simplify,
}

impl Action {
    pub const ALL: [Action; 17] = [
        Action::CleanWhitespace,
        Action::JoinLines,
        Action::UpperCase,
        Action::LowerCase,
        Action::TitleCase,
        Action::SentenceCase,
        Action::CleanLinks,
        Action::ExtractEmails,
        Action::ExtractUrls,
        Action::Correct,
        Action::MakeShorter,
        Action::MakeFormal,
        Action::MakeFriendly,
        Action::TidySpeech,
        Action::BulletList,
        Action::Summarize,
        Action::Simplify,
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
            Action::Correct => "Correct Text",
            Action::MakeShorter => "Make Shorter",
            Action::MakeFormal => "Make Formal",
            Action::MakeFriendly => "Make Friendly",
            Action::TidySpeech => "Tidy Spoken Text",
            Action::BulletList => "Make Bullet List",
            Action::Summarize => "Summarize",
            Action::Simplify => "Make Simpler",
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
            Action::Correct => "Fixes spelling, grammar and punctuation with the text model.",
            Action::MakeShorter => "Says the same in fewer words with the text model.",
            Action::MakeFormal => "Rewrites in a formal, professional tone with the text model.",
            Action::MakeFriendly => "Rewrites in a warm, friendly tone with the text model.",
            Action::TidySpeech => "Removes false starts, stutters and repeated words from speech with the text model.",
            Action::BulletList => "Turns the text into a short bulleted list with the text model.",
            Action::Summarize => "Boils the text down to a sentence or two with the text model.",
            Action::Simplify => "Rewrites in plain, simple words with the text model.",
        }
    }

    /// Whether the action makes sense on dictated text; every action works on selected text. Links, emails and broken lines do not occur in speech.
    pub fn for_dictation(self) -> bool {
        !matches!(
            self,
            Action::JoinLines | Action::CleanLinks | Action::ExtractEmails | Action::ExtractUrls | Action::Summarize | Action::Simplify
        )
    }

    /// Whether the action runs the text model: slow and download-dependent, so the live test on the Rules page skips it.
    pub fn needs_model(self) -> bool {
        model::needs_model(self)
    }

    /// The transformed text, or None when there is nothing to do (no change, or nothing found), so callers can skip the write.
    pub fn apply(self, text: &str) -> Option<String> {
        if let Some(task) = model::task(self) {
            return model::run(task, text).filter(|out| out != text);
        }
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
            _ => return None, // the model actions return above
        };
        (out != text && !out.is_empty()).then_some(out)
    }
}

pub use model::{set_runner, task, Task, Unit};

#[derive(Serialize)]
pub struct ActionInfo {
    id: String,
    label: &'static str,
    hint: &'static str,
    model: bool,
    dictation: bool,
}

#[tauri::command]
pub fn list_actions() -> Vec<ActionInfo> {
    Action::ALL
        .iter()
        .map(|a| ActionInfo { id: a.id(), label: a.label(), hint: a.hint(), model: a.needs_model(), dictation: a.for_dictation() })
        .collect()
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
    fn model_actions_are_the_rewrites() {
        let model: Vec<_> = Action::ALL.iter().filter(|a| a.needs_model()).map(|a| a.id()).collect();
        assert_eq!(
            model,
            ["correct", "make_shorter", "make_formal", "make_friendly", "tidy_speech", "bullet_list", "summarize", "simplify"]
        );
    }

    #[test]
    fn dictation_skips_actions_that_need_links_or_long_text() {
        assert!(Action::TidySpeech.for_dictation() && Action::Correct.for_dictation());
        assert!(!Action::ExtractEmails.for_dictation() && !Action::Summarize.for_dictation());
    }

    #[test]
    fn nothing_to_do_is_none() {
        assert_eq!(Action::UpperCase.apply("ABC"), None);
        assert_eq!(Action::ExtractEmails.apply("no address here"), None);
        assert_eq!(Action::LowerCase.apply("ABC").as_deref(), Some("abc"));
    }
}
