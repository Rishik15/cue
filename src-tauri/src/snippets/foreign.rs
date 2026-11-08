//! Purpose: Reads snippets from other tools' export files (Espanso YAML, TextExpander CSV, AutoHotkey hotstrings, Raycast JSON) into drafts Cue can store. Pure text in, drafts out.
//! Contents: parse — pick the reader by file extension and keep the drafts that pass the limits; espanso / csv / ahk — the three readers; unquote / unescape — a YAML scalar to text.
//! shortcut: Espanso reading covers the common shape (`- trigger:` / `triggers:` and `replace:` as a plain, quoted or block scalar), not all of YAML; TextExpander reads the first two columns.
//! Placeholders of the other tools are not translated: their text arrives as written.

use super::check::{check, clean};
use super::model::{Draft, Mode};
use super::transfer;

/// The drafts in a file (each with an empty folder name) and how many entries were refused. `extension` is the file's, lower case.
pub fn parse(extension: &str, text: &str) -> Result<(Vec<(Draft, String)>, usize), String> {
    let text = text.trim_start_matches('\u{feff}');
    let drafts = match extension {
        "yml" | "yaml" => espanso(text),
        "csv" => csv(text),
        "ahk" => ahk(text),
        _ => return transfer::parse(text),
    };
    let total = drafts.len();
    // Cue's own placeholders must not run in a pack written for another tool ({env:TOKEN} would type a secret), so braces become literal.
    let literal = |mut d: Draft| {
        d.body = d.body.replace('{', "{{").replace('}', "}}");
        d
    };
    let good: Vec<_> = drafts.into_iter().map(literal).filter(|d| check(&clean(d)).is_ok()).map(|d| (d, String::new())).collect();
    let refused = total - good.len();
    Ok((good, refused))
}

fn draft(triggers: Vec<String>, body: String) -> Draft {
    let name = triggers.first().cloned().unwrap_or_default();
    Draft { name, triggers, body, ..Draft::default() }
}

/// A YAML scalar: quotes removed, escapes in double quotes read.
fn unquote(value: &str) -> String {
    let v = value.trim();
    if let Some(inner) = v.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        return unescape(inner);
    }
    match v.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
        Some(inner) => inner.replace("''", "'"),
        None => v.to_string(),
    }
}

/// The escapes of a double-quoted YAML scalar, read left to right in one pass: `\\n` is a backslash and an n, not a backslash and a new line.
fn unescape(inner: &str) -> String {
    let (mut out, mut chars) = (String::new(), inner.chars());
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(other @ ('"' | '\\')) => out.push(other),
            Some(other) => out.extend(['\\', other]),
            None => out.push('\\'),
        }
    }
    out
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// A line that starts a new match: `- key: value`.
fn item_start(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix("- ")?;
    let key_end = rest.find(':')?;
    rest[..key_end].chars().all(|c| c.is_ascii_alphanumeric() || c == '_').then_some(rest)
}

fn espanso(text: &str) -> Vec<Draft> {
    let lines: Vec<&str> = text.lines().collect();
    let (mut out, mut triggers, mut body, mut i) = (Vec::new(), Vec::<String>::new(), None::<String>, 0);
    let mut flush = |triggers: &mut Vec<String>, body: &mut Option<String>| {
        if let Some(b) = body.take().filter(|_| !triggers.is_empty()) {
            out.push(draft(std::mem::take(triggers), b));
        }
        triggers.clear();
    };
    while i < lines.len() {
        let line = lines[i];
        i += 1;
        let fresh = item_start(line);
        if fresh.is_some() {
            flush(&mut triggers, &mut body);
        }
        let field = fresh.unwrap_or(line.trim());
        if let Some(item) = field.strip_prefix("- ").filter(|_| fresh.is_none()) {
            triggers.push(unquote(item));
            continue;
        }
        let Some((key, value)) = field.split_once(':') else { continue };
        match (key.trim(), value.trim()) {
            ("trigger", v) => triggers.push(unquote(v)),
            ("triggers", v) => triggers.extend(v.trim_matches(['[', ']']).split(',').map(unquote).filter(|t| !t.is_empty())),
            ("replace", v) if v.starts_with(['|', '>']) => {
                let start = i;
                while i < lines.len() && (lines[i].trim().is_empty() || indent(lines[i]) > indent(line)) {
                    i += 1;
                }
                let block: Vec<&str> = lines[start..i].iter().map(|l| l.trim()).collect();
                body = Some(block.join("\n").trim_end().to_string());
            }
            ("replace", v) => body = Some(unquote(v)),
            _ => {}
        }
    }
    flush(&mut triggers, &mut body);
    out
}

/// Rows of a CSV file: quoted cells may hold commas, quotes (doubled) and new lines.
fn csv_rows(text: &str) -> Vec<Vec<String>> {
    let (mut rows, mut row, mut cell, mut quoted) = (Vec::new(), Vec::new(), String::new(), false);
    let mut chars = text.trim_start_matches('\u{feff}').chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                cell.push('"');
                chars.next();
            }
            ('"', _) => quoted = !quoted,
            (',', false) => row.push(std::mem::take(&mut cell)),
            ('\n' | '\r', false) => {
                if !(c == '\r' && chars.peek() == Some(&'\n')) {
                    row.push(std::mem::take(&mut cell));
                    rows.push(std::mem::take(&mut row));
                }
            }
            _ => cell.push(c),
        }
    }
    row.push(cell);
    rows.push(row);
    rows
}

fn csv(text: &str) -> Vec<Draft> {
    let header = ["abbreviation", "trigger", "keyword", "shortcut"];
    csv_rows(text)
        .into_iter()
        .filter(|r| r.len() >= 2 && !header.contains(&r[0].trim().to_lowercase().as_str()))
        .map(|r| draft(vec![r[0].trim().to_string()], r[1].clone()))
        .collect()
}

/// `::abbr::text`, `:*:abbr::text`: options `*` (no ending character needed), `?` (inside words) and `C` (match case) are kept; backtick escapes are read.
fn ahk(text: &str) -> Vec<Draft> {
    text.lines()
        .filter_map(|line| {
            let (options, rest) = line.strip_prefix(':')?.split_once(':')?;
            let (abbr, body) = rest.split_once("::")?;
            let body = body.replace("`n", "\n").replace("`t", "\t").replace("``", "`");
            let mode = if options.contains('*') { Mode::Immediate } else { Mode::Auto };
            Some(Draft {
                mode,
                inside_word: options.contains('?'),
                match_case: options.contains('C'),
                ..draft(vec![abbr.to_string()], body)
            })
        })
        .collect()
}
