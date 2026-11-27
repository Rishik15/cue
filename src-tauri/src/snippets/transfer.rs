//! File: snippets/transfer.rs
//! Purpose: Reading and writing Cue's snippet export file (JSON, version 3 only), kept apart from the database so it is testable without one, and making literal what an imported text must not run.
//! Contents: MAX_FILE_BYTES / read_limited — the largest file an import reads, and reading one within it; Parsed — what a file holds; export — the file text for some snippets and the folders they are in (name, switch, program lists); parse — the valid drafts in a file (a byte order mark skipped, `{env:...}` made literal), each with the name of its folder, the file's folders and how many entries were refused; escape_includes / is_condition_marker — an imported text made unable to read the environment through saved snippets or condition-joined placeholders.
//! The `folders` list was added to version 3 without a new version: a file without it (an older export) reads as before, with every named folder made enabled everywhere, and an older Cue ignores it.
//! Keys Cue no longer has (the per-snippet `mode`, `match_case`, `ending`, `inside_word`, `propagate_case`) are ignored; a missing or null `insert` is paste and an unknown one refuses the entry.

use std::collections::HashMap;
use std::io::Read;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use super::check::{check, clean, MAX_BODY, MAX_NAME, MAX_PROGRAM, MAX_PROGRAMS, MAX_SNIPPETS, MAX_TRIGGER, MAX_TRIGGERS};
use super::expand::template::{may_read_env, snippet_refs};
use super::model::{Draft, Folder, FolderDraft, Insert, Snippet};

/// The most characters one exported snippet and its folder hold at the limits of `check`: name, triggers, text, folder name and both program lists, of the snippet and of the folder.
const MAX_ENTRY_CHARS: usize = MAX_NAME * 2 + MAX_TRIGGERS * MAX_TRIGGER + MAX_BODY + 4 * MAX_PROGRAMS * MAX_PROGRAM;
/// The most an import file may hold: the largest export Cue can write (`MAX_SNIPPETS` entries of `MAX_ENTRY_CHARS`, each character at most 6 bytes as JSON escapes it, plus 8 KiB of keys and indentation per entry), so every export reads back, while a mistaken pick of a huge file still cannot fill memory.
pub const MAX_FILE_BYTES: u64 = (MAX_SNIPPETS * (MAX_ENTRY_CHARS * 6 + 8192)) as u64;
/// The one format read and written: several triggers, folders by name, insert mode and more options.
const VERSION: u32 = 3;

/// A whole export file: its format version, its entries (each read on its own so one bad entry is counted, not fatal) and the folders they name.
#[derive(Serialize, Deserialize)]
struct File {
    version: u32,
    snippets: Vec<serde_json::Value>,
    #[serde(default)]
    folders: Vec<FolderDraft>,
}

/// What a snippets file holds: the drafts that pass the limits, each with the name of its folder (empty for none), the file's own folders (what a missing one is made as), and how many entries were refused.
#[derive(Debug, Default)]
pub struct Parsed {
    pub entries: Vec<(Draft, String)>,
    pub folders: Vec<FolderDraft>,
    pub refused: usize,
}

/// The text of an import file, read from its one open handle and never past `limit` bytes, so a file that grows or is swapped after it was picked cannot fill memory. Inputs: the open file and the limit (`MAX_FILE_BYTES`). Returns the text, or Err when it is larger or not text.
pub fn read_limited(file: impl Read, limit: u64) -> Result<String, String> {
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1)).read_to_end(&mut bytes).map_err(|e| format!("Could not read the file: {e}"))?;
    if bytes.len() as u64 > limit {
        return Err("This file is too large to be a snippets file.".into());
    }
    String::from_utf8(bytes).map_err(|_| "Could not read the file: it is not text.".into())
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
    #[serde(default = "yes")]
    enabled: bool,
    #[serde(default)]
    insert: Option<Insert>,
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
        let draft = Draft {
            name: self.name,
            triggers: self.triggers,
            body: self.body,
            folder_id: None,
            enabled: self.enabled,
            insert: self.insert.unwrap_or_default(),
            only_apps: self.only_apps,
            skip_apps: self.skip_apps,
        };
        (draft, self.folder)
    }
}

/// The file text for `snippets`, each naming its folder from `folders`, with the folders they are in (switch and program lists) so an import makes them as they were.
pub fn export(snippets: &[Snippet], folders: &[Folder]) -> String {
    let used = folders
        .iter()
        .filter(|f| snippets.iter().any(|s| s.folder_id == Some(f.id)))
        .map(|f| FolderDraft { name: f.name.clone(), enabled: f.enabled, only_apps: f.only_apps.clone(), skip_apps: f.skip_apps.clone() })
        .collect();
    let rows = snippets
        .iter()
        .map(|s| {
            let folder = s.folder_id.and_then(|id| folders.iter().find(|f| f.id == id)).map_or("", |f| f.name.as_str());
            serde_json::json!({
                "name": s.name, "triggers": s.triggers, "body": s.body, "folder": folder, "enabled": s.enabled, "insert": s.insert,
                "only_apps": s.only_apps, "skip_apps": s.skip_apps,
            })
        })
        .collect();
    serde_json::to_string_pretty(&File { version: VERSION, snippets: rows, folders: used }).unwrap_or_default()
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

/// `body` made safe to import against a library that `find` reads (every saved text an include name may reach): each `{snippet:X}` where any saved X could read the environment (`may_read_env`) is written as literal text; if the text could still read it once conditions join their branches, its condition markers become literal too, and if even that is not enough every placeholder does. Each saved name is judged once per text in one traversal of everything it reaches (one set of names already read), its case folded in ASCII only as `find` compares names. Returns the text and how many changes were made (each fail-closed step counts one). Includes of env-free snippets and plain conditions stay live.
pub fn escape_includes<'a>(body: &str, find: &dyn Fn(&str) -> Vec<&'a str>) -> (String, usize) {
    let mut known: HashMap<String, bool> = HashMap::new();
    let unsafe_include = |p: &str| {
        let Some(name) = snippet_refs(p).into_iter().next() else { return false };
        *known.entry(name.to_ascii_lowercase()).or_insert_with(|| may_read_env(p, find))
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

/// The drafts that pass the same limits as a typed snippet (each with its folder's name, empty for none), the file's folders, and the number of entries that did not pass. A folder that cannot be read refuses the whole file, so its snippets never land in a folder without its switch and programs.
pub fn parse(text: &str) -> Result<Parsed, String> {
    let text = text.trim_start_matches('\u{feff}');
    let file: File = serde_json::from_str(text).map_err(|_| "This is not a snippets file Cue can read.".to_string())?;
    match file.version.cmp(&VERSION) {
        std::cmp::Ordering::Greater => return Err("This file comes from a newer version of Cue.".into()),
        std::cmp::Ordering::Less => return Err("This file was written by an older Cue that this version cannot read.".into()),
        std::cmp::Ordering::Equal => {}
    }
    let mut parsed = Parsed { folders: file.folders, ..Parsed::default() };
    for entry in file.snippets {
        match serde_json::from_value::<Entry>(entry).map(Entry::into_draft) {
            Ok((d, folder)) => {
                let d = Draft { body: escape_env(&d.body), ..d };
                if check(&clean(&d)).is_ok() {
                    parsed.entries.push((d, folder));
                } else {
                    parsed.refused += 1;
                }
            }
            Err(_) => parsed.refused += 1,
        }
    }
    Ok(parsed)
}
