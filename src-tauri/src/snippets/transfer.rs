//! File: snippets/transfer.rs
//! Purpose: Reading and writing Cue's snippet export file (JSON, version 3 only), kept apart from the database so it is testable without one, and making literal what an imported text must not run.
//! Contents: export — the file text for some snippets and their folders' names; parse — the valid drafts in a file (a byte order mark skipped, `{env:...}` made literal), each with the name of its folder, and how many entries were refused; escape_includes / is_condition_marker — an imported text made unable to read the environment through saved snippets or condition-joined placeholders.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use super::check::{check, clean};
use super::expand::template::{may_read_env, snippet_refs};
use super::model::{Draft, Folder, Insert, Mode, Snippet};

/// The most an import file may hold: a mistaken pick of a huge file must not fill memory.
pub const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
/// The one format read and written: several triggers, folders by name, insert mode and more options.
const VERSION: u32 = 3;

/// A whole export file: its format version and its entries, each read on its own so one bad entry is counted, not fatal.
#[derive(Serialize, Deserialize)]
struct File {
    version: u32,
    snippets: Vec<serde_json::Value>,
}

/// One entry of a version 3 file.
#[derive(Deserialize)]
struct Entry {
    name: String,
    body: String,
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
    insert: Option<String>,
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

/// The default of a flag that is on unless the file says otherwise.
fn yes() -> bool {
    true
}

impl Entry {
    /// The entry as a draft, with the name of its folder (empty for none).
    fn into_draft(self) -> (Draft, String) {
        let insert = self.insert.as_deref().map(Insert::parse).unwrap_or_default();
        let draft = Draft {
            name: self.name,
            triggers: self.triggers,
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

/// The file text for `snippets`, each naming its folder from `folders`.
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

/// `body` with each `{...}` placeholder for which `unsafe_at` holds written with doubled braces (literal text), and how many were. A doubled brace is already literal and is left alone.
fn literal_where(body: &str, mut unsafe_at: impl FnMut(&str) -> bool) -> (String, usize) {
    static PLACEHOLDER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\{[^{}]*\}").expect("constant pattern"));
    let (mut out, mut rest, mut changed) = (String::new(), body, 0);
    while let Some(at) = rest.find('{') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        match PLACEHOLDER.find(rest).filter(|m| unsafe_at(m.as_str())) {
            Some(m) => {
                out.push_str(&format!("{{{}}}", m.as_str()));
                rest = &rest[m.end()..];
                changed += 1;
            }
            None => {
                let pair = if rest.starts_with("{{") { 2 } else { 1 };
                out.push_str(&rest[..pair]);
                rest = &rest[pair..];
            }
        }
    }
    out.push_str(rest);
    (out, changed)
}

/// `body` with each `{env:NAME}` written as literal text. A file from elsewhere must not read the environment (a token, a path) into text that is typed into another app.
fn escape_env(body: &str) -> String {
    static ENV: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\{\s*env\s*:").expect("constant pattern"));
    literal_where(body, |p| ENV.is_match(p)).0
}

/// `body` made safe to import against a library that `find` reads: each `{snippet:X}` whose saved X could read the environment (`may_read_env`) is written as literal text; if the text could still read it once conditions join their branches, its condition markers become literal too, and if even that is not enough every placeholder does. Each saved name is judged once per text, its case folded in ASCII only as `find` compares names. Returns the text and how many changes were made (each fail-closed step counts one). Includes of env-free snippets and plain conditions stay live.
pub fn escape_includes(body: &str, find: &dyn Fn(&str) -> Option<String>) -> (String, usize) {
    let mut known: HashMap<String, bool> = HashMap::new();
    let unsafe_include = |p: &str| {
        let Some(name) = snippet_refs(p).into_iter().next() else { return false };
        *known.entry(name.to_ascii_lowercase()).or_insert_with(|| find(&name).is_some_and(|found| may_read_env(&found, find)))
    };
    let (out, changed) = literal_where(body, unsafe_include);
    if !may_read_env(&out, find) {
        return (out, changed);
    }
    let (out, _) = literal_where(&out, is_condition_marker);
    if !may_read_env(&out, find) {
        return (out, changed + 1);
    }
    (literal_where(&out, |_| true).0, changed + 2)
}

/// Whether placeholder `p` (with its braces) is an `{if ...}`, `{else}` or `{/if}` marker.
fn is_condition_marker(p: &str) -> bool {
    let inner = p.trim_matches(['{', '}']).trim_start();
    inner.starts_with("if ") || inner == "else" || inner == "/if"
}

/// The drafts that pass the same limits as a typed snippet (each with its folder's name, empty for none), and the number of entries that did not.
pub fn parse(text: &str) -> Result<(Vec<(Draft, String)>, usize), String> {
    let text = text.trim_start_matches('\u{feff}');
    let file: File = serde_json::from_str(text).map_err(|_| "This is not a snippets file Cue can read.".to_string())?;
    match file.version.cmp(&VERSION) {
        std::cmp::Ordering::Greater => return Err("This file comes from a newer version of Cue.".into()),
        std::cmp::Ordering::Less => return Err("This file was written by an older Cue that this version cannot read.".into()),
        std::cmp::Ordering::Equal => {}
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
