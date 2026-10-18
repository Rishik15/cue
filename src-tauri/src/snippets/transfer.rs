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

#[cfg(test)]
mod tests {
    use super::*;

    fn snippet(name: &str, body: &str) -> Snippet {
        Snippet {
            id: 1,
            name: name.into(),
            triggers: vec![":k".into(), ":kk".into()],
            body: body.into(),
            uses: 9,
            last_used: 5,
            folder_id: Some(7),
            mode: Mode::Auto,
            match_case: false,
            enabled: true,
            insert: Insert::Slow,
            ending: String::new(),
            inside_word: false,
            propagate_case: true,
            only_apps: Vec::new(),
            skip_apps: vec!["slack".into()],
        }
    }

    fn work() -> Folder {
        Folder { id: 7, name: "Work".into(), enabled: true, prefix: String::new(), only_apps: vec![], skip_apps: vec![] }
    }

    #[test]
    fn an_export_reads_back_with_its_options_and_folder_name_without_the_use_counts() {
        let text = export(&[snippet("Address", "1 Main St\nTown")], &[work()]);
        let (entries, refused) = parse(&text).unwrap();
        let (d, folder) = &entries[0];
        assert_eq!((entries.len(), refused, folder.as_str()), (1, 0, "Work"));
        assert_eq!(
            (d.name.as_str(), d.triggers.len(), d.insert, d.skip_apps.as_slice()),
            ("Address", 2, Insert::Slow, ["slack".to_string()].as_slice())
        );
    }

    #[test]
    fn files_from_earlier_versions_still_read() {
        let one = r#"{"version":1,"snippets":[{"name":"a","keyword":"sig1","body":"x"}]}"#;
        let two = r#"{"version":2,"snippets":[{"name":"a","trigger":":sig","body":"x","paste":true,"folder":"Old"}]}"#;
        let (v1, _) = parse(one).unwrap();
        let (v2, _) = parse(two).unwrap();
        assert_eq!(v1[0].0.triggers, ["sig1"]);
        assert_eq!(
            (v2[0].0.triggers.as_slice(), v2[0].0.insert, v2[0].1.as_str()),
            ([":sig".to_string()].as_slice(), Insert::Paste, "Old")
        );
    }

    #[test]
    fn bad_entries_are_counted_and_other_files_are_refused() {
        let text = r#"{"version":3,"snippets":[{"name":"ok","body":"x"},{"name":"","body":"x"},{"body":"no name"},42,{"name":"t","body":"x","triggers":["a\tb"]}]}"#;
        let (drafts, refused) = parse(text).unwrap();
        assert_eq!((drafts.len(), refused), (1, 4));
        assert!(parse("not json").is_err());
        assert!(parse(r#"{"version":4,"snippets":[]}"#).is_err());
    }

    #[test]
    fn a_raycast_list_reads_with_text_and_keyword() {
        let (good, refused) = parse(r#"[{"name":"Sig","text":"Best, me","keyword":"sig"}]"#).unwrap();
        assert_eq!((good.len(), refused), (1, 0));
        assert_eq!((good[0].0.triggers.clone(), good[0].0.body.as_str()), (vec!["sig".to_string()], "Best, me"));
    }

    #[test]
    fn env_placeholders_in_an_imported_file_become_literal_text() {
        let text = r#"{"version":3,"snippets":[{"name":"a","body":"k {env:TOKEN} {{env:X}} { ENV : Y | upper } {date} {env:open"}]}"#;
        let (good, _) = parse(text).unwrap();
        assert_eq!(good[0].0.body, "k {{env:TOKEN}} {{env:X}} {{ ENV : Y | upper }} {date} {env:open");
        assert_eq!(escape_env("{{{env:A}}"), "{{{{env:A}}}");
    }

    #[test]
    fn a_byte_order_mark_does_not_stop_a_file_from_reading() {
        let (good, refused) = parse("\u{feff}{\"version\":3,\"snippets\":[{\"name\":\"a\",\"body\":\"x\"}]}").unwrap();
        assert_eq!((good.len(), refused), (1, 0));
    }

    #[test]
    fn the_starter_snippets_are_all_valid() {
        let (good, refused) = parse(include_str!("starter.json")).unwrap();
        assert_eq!((good.len(), refused), (11, 0));
        assert!(good.iter().all(|(_, folder)| folder == "Starter"));
    }
}
