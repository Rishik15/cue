//! File: text/mask.rs
//! Purpose: Keep links, paths, commands, amounts and code safe by construction: lift them out before the model sees the text, leave a placeholder in their place, and put them back after, refusing any answer that does not carry every placeholder exactly once.
//! Contents: Masked — the text with placeholders and the spans taken out (`lift`, `text`, `restore`).
//! The placeholder style is chosen so that its opening mark does not occur in the text, so a placeholder can never be confused with the user's own words; when no style is free, nothing is lifted and the text goes as it is. Off for every action until the bench proves it (see `Task::mask`).
//! The scan is one regex pass (linear) and keeps no model memory.

use super::spans::MASKABLE;

/// Placeholder styles as (open, close); the first whose open mark is absent from the text is used.
const STYLES: [(&str, &str); 4] = [("\u{27e6}", "\u{27e7}"), ("\u{ab}", "\u{bb}"), ("[[", "]]"), ("{{", "}}")];

/// A text with its spans replaced by numbered placeholders, and the spans in order.
pub struct Masked {
    text: String,
    style: (&'static str, &'static str),
    spans: Vec<String>,
}

/// Lifts the maskable spans out of `text`. Returns the masked text and what was taken; when no placeholder style is free, or nothing matches, the text is returned as it is with no spans.
pub fn lift(text: &str) -> Masked {
    let free = STYLES.iter().find(|(open, _)| !text.contains(open));
    let Some(&style) = free else { return Masked { text: text.to_string(), style: STYLES[0], spans: Vec::new() } };
    let (mut out, mut spans, mut last) = (String::new(), Vec::new(), 0);
    for found in MASKABLE.find_iter(text) {
        out.push_str(&text[last..found.start()]);
        out.push_str(&format!("{}{}{}", style.0, spans.len() + 1, style.1));
        spans.push(found.as_str().to_string());
        last = found.end();
    }
    out.push_str(&text[last..]);
    Masked { text: out, style, spans }
}

impl Masked {
    /// The text to send to the model.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Puts the spans back into the model's `answer`. Returns the restored text, or None when any placeholder is missing or repeated, or an unknown one appears.
    pub fn restore(&self, answer: &str) -> Option<String> {
        let (open, close) = self.style;
        if self.spans.is_empty() {
            return Some(answer.to_string());
        }
        let mut restored = answer.to_string();
        for (i, span) in self.spans.iter().enumerate() {
            let mark = format!("{open}{}{close}", i + 1);
            if answer.matches(&mark).count() != 1 {
                return None;
            }
            restored = restored.replacen(&mark, span, 1);
        }
        let leftovers = answer.matches(open).count();
        (leftovers == self.spans.len()).then_some(restored)
    }
}
