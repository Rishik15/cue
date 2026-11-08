//! Purpose: Reading and writing the snippet export file (JSON), kept apart from the database so it is testable without one.
//! Contents: export — the file text for some snippets and their folders' names; parse — the valid drafts in a file (a byte order mark skipped, `{env:...}` made literal), each with the name of its folder, and how many entries were refused. Files from earlier versions
//! (a single `keyword` or `trigger`, `paste`) still read.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use super::check::{check, clean};
use super::model::{Draft, Folder, Insert, Mode, Snippet};

/// The most an import file may hold: a mistaken pick of a huge file must not fill memory.
pub const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
/// 3: several triggers, folders by name, insert mode and more options (2 had one trigger, 1 a keyword).
const VERSION: u32 = 3;

#[derive(Serialize, Deserialize)]
struct File {
    version: u32,
    snippets: Vec<serde_json::Value>,
}

/// One entry as any version wrote it.
#[derive(Deserialize)]
struct Entry {
    name: String,
    #[serde(alias = "text")]
    body: String,
    #[serde(default, alias = "keyword")]
    trigger: String,
    #[serde(default)]
    triggers: Vec<String>,
    #[serde(default)]
    folder: String,
    #[serde(default)]
    mode: Mode,
    #[serde(default)]
    match_case: bool,
    #[serde(default = "yes")]
    enabled: bool,
    #[serde(default)]
    insert: Option<Insert>,
    #[serde(default)]
    paste: bool,
    #[serde(default)]
    ending: String,
    #[serde(default)]
    inside_word: bool,
    #[serde(default = "yes")]
    propagate_case: bool,
    #[serde(default)]
    only_apps: Vec<String>,
    #[serde(default)]
    skip_apps: Vec<String>,
}

fn yes() -> bool {
    true
}

impl Entry {
    fn into_draft(self) -> (Draft, String) {
        let triggers = if self.triggers.is_empty() && !self.trigger.is_empty() { vec![self.trigger] } else { self.triggers };
        let insert = self.insert.unwrap_or(if self.paste { Insert::Paste } else { Insert::Auto });
        let draft = Draft {
            name: self.name,
            triggers,
            body: self.body,
            folder_id: None,
            mode: self.mode,
            match_case: self.match_case,
            enabled: self.enabled,
            insert,
            ending: self.ending,
            inside_word: self.inside_word,
            propagate_case: self.propagate_case,
            only_apps: self.only_apps,
            skip_apps: self.skip_apps,
        };
        (draft, self.folder)
    }
}

pub fn export(snippets: &[Snippet], folders: &[Folder]) -> String {
    let rows = snippets
        .iter()
        .map(|s| {
            let folder = s.folder_id.and_then(|id| folders.iter().find(|f| f.id == id)).map_or("", |f| f.name.as_str());
            serde_json::json!({
                "name": s.name, "triggers": s.triggers, "body": s.body, "folder": folder, "mode": s.mode, "match_case": s.match_case,
                "enabled": s.enabled, "insert": s.insert, "ending": s.ending, "inside_word": s.inside_word, "propagate_case": s.propagate_case,
                "only_apps": s.only_apps, "skip_apps": s.skip_apps,
            })
        })
        .collect();
    serde_json::to_string_pretty(&File { version: VERSION, snippets: rows }).unwrap_or_default()
}

/// An `{env:NAME}` placeholder written as literal text. A file from elsewhere must not read the environment (a token, a path) into text that is typed into another app.
fn escape_env(body: &str) -> String {
    static ENV: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\{\s*env\s*:[^{}]*\}").expect("constant pattern"));
    let (mut out, mut rest) = (String::new(), body);
    while let Some(at) = rest.find('{') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        if let Some(m) = ENV.find(rest) {
            out.push_str(&format!("{{{}}}", m.as_str()));
            rest = &rest[m.end()..];
        } else {
            let pair = if rest.starts_with("{{") { 2 } else { 1 }; // a doubled brace is already literal
            out.push_str(&rest[..pair]);
            rest = &rest[pair..];
        }
    }
    out.push_str(rest);
    out
}

/// The drafts that pass the same limits as a typed snippet (each with its folder's name, empty for none), and the number of entries that did not.
pub fn parse(text: &str) -> Result<(Vec<(Draft, String)>, usize), String> {
    // Raycast exports a bare list of {name, text, keyword}; it reads as a version 1 file.
    let text = text.trim_start_matches('\u{feff}'); // a byte order mark, as Windows editors write
    let file: File = serde_json::from_str(text)
        .or_else(|_| serde_json::from_str(text).map(|snippets| File { version: 1, snippets }))
        .map_err(|_| "This is not a snippets file Cue can read.".to_string())?;
    if file.version > VERSION {
        return Err("This file comes from a newer version of Cue.".into());
    }
    let (mut good, mut refused) = (Vec::new(), 0);
    for entry in file.snippets {
        match serde_json::from_value::<Entry>(entry).map(Entry::into_draft) {
            Ok((d, folder)) => {
                let d = Draft { body: escape_env(&d.body), ..d };
                if check(&clean(&d)).is_ok() {
                    good.push((d, folder));
                } else {
                    refused += 1;
                }
            }
            Err(_) => refused += 1,
        }
    }
    Ok((good, refused))
}
