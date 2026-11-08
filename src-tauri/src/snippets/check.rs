//! Purpose: The limits snippets and folders obey, the tidying of a draft before it is stored, and the test for two triggers that cannot coexist.
//! Contents: program_matches — whether a list entry names the program in front; program_entry — a list entry as it is kept; check / check_folder — the limits, with messages for the user; clean / clean_folder — trimming and normalizing; clash — two triggers that cannot both work;
//! DELIMITERS — the characters that end a word.

use super::expand::pattern::{self, PREFIX};
use super::model::{Draft, FolderDraft, Mode, When};

pub const MAX_SNIPPETS: usize = 2000;
pub const MAX_NAME: usize = 80;
const MAX_TRIGGER: usize = 40;
const MAX_TRIGGERS: usize = 8;
const MAX_ENDING: usize = 20;
const MAX_PROGRAMS: usize = 40;
const MAX_PROGRAM: usize = 160;
pub const MAX_BODY: usize = 20_000;
/// Typing one of these ends a word: a trigger that starts with a letter expands when one follows it. A trigger may still contain them (`;sig`).
pub const DELIMITERS: &[char] = &[' ', '\t', '.', ',', ';', '!', '?', ')', ']', '}', '"', '\''];

/// A program entry as it is kept. Entries from the picker are `Name<TAB>exe` (`WezTerm\twezterm-gui`): the name is shown, the executable is what the focus check compares. An entry without a tab (an older list, an import) is a name, kept in lower case without `.exe`.
pub fn program_entry(text: &str) -> String {
    let bare = |t: &str| {
        let lower = t.trim().to_lowercase();
        lower.strip_suffix(".exe").unwrap_or(&lower).to_string()
    };
    match text.split_once('\t') {
        Some((name, exe)) if !bare(exe).is_empty() => format!("{}\t{}", name.trim(), bare(exe)),
        Some((name, _)) => bare(name),
        None => bare(text),
    }
}

/// The words of a program name: lower case, `.exe` dropped, split at anything that is not a letter or a digit (`WezTerm-GUI.exe` is `wezterm`, `gui`).
fn words(name: &str) -> Vec<String> {
    let lower = name.to_lowercase();
    let bare = lower.strip_suffix(".exe").unwrap_or(&lower);
    bare.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(String::from).collect()
}

/// Whether a list entry names the program in front, whose executable is `exe` (lower case, no `.exe`). A picked entry carries its executable and must equal it. A plain name matches when its words are the executable's words or a run of
/// them, so `wezterm` finds `wezterm-gui`; `code` does not find `codex`, and spaces, dashes and case do not matter.
pub fn program_matches(entry: &str, exe: &str) -> bool {
    if let Some((_, wanted)) = entry.split_once('\t') {
        return wanted == exe;
    }
    let wanted = words(entry);
    !wanted.is_empty() && words(exe).windows(wanted.len()).any(|run| run == wanted.as_slice())
}

fn programs(list: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for app in list.iter().map(|a| program_entry(a)).filter(|a| !a.is_empty()) {
        // One entry per program: the same executable twice (two Start menu names) is one program.
        let same = |other: &String| {
            other == &app || matches!((other.split_once('\t'), app.split_once('\t')), (Some((_, a)), Some((_, b))) if a == b)
        };
        if !out.iter().any(same) {
            out.push(app);
        }
    }
    out
}

/// The draft as it is stored: text trimmed, triggers and program names listed once. A trigger keeps its spaces (a space at either end is part of what must be typed); one that is only spaces is dropped.
pub fn clean(d: &Draft) -> Draft {
    let mut triggers: Vec<String> = Vec::new();
    for t in d.triggers.iter().filter(|t| !t.trim().is_empty()) {
        if !triggers.contains(t) {
            triggers.push(t.clone());
        }
    }
    Draft { name: d.name.trim().into(), triggers, only_apps: programs(&d.only_apps), skip_apps: programs(&d.skip_apps), ..d.clone() }
}

pub fn clean_folder(d: &FolderDraft) -> FolderDraft {
    FolderDraft { name: d.name.trim().into(), only_apps: programs(&d.only_apps), skip_apps: programs(&d.skip_apps), ..d.clone() }
}

/// Combining marks, joiners and variation selectors: they join the character before them into one the user sees, so the trigger's characters and what Backspace erases would differ.
fn is_joining(c: char) -> bool {
    matches!(c, '\u{200C}' | '\u{200D}' | '\u{FE00}'..='\u{FE0F}' | '\u{0300}'..='\u{036F}' | '\u{1AB0}'..='\u{1AFF}' | '\u{1DC0}'..='\u{1DFF}' | '\u{20D0}'..='\u{20FF}' | '\u{FE20}'..='\u{FE2F}')
}

fn check_trigger(t: &str) -> Result<(), String> {
    if let Some(pattern) = t.strip_prefix(PREFIX) {
        pattern::compile(pattern)?;
    }
    if t.chars().any(is_joining) {
        return Err("A trigger cannot hold combining marks or joined emoji: use plain characters.".into());
    }
    let length = t.chars().count();
    if length > MAX_TRIGGER || t.trim().is_empty() || t.chars().any(|c| c.is_control() || (c.is_whitespace() && c != ' ')) {
        return Err(format!("A trigger has 1 to {MAX_TRIGGER} characters, for example :sig, and may contain spaces"));
    }
    Ok(())
}

fn check_programs(lists: &[&Vec<String>]) -> Result<(), String> {
    if lists.iter().any(|l| l.len() > MAX_PROGRAMS || l.iter().any(|a| a.chars().count() > MAX_PROGRAM)) {
        return Err(format!("A list holds {MAX_PROGRAMS} programs at most, each with a name up to {MAX_PROGRAM} characters."));
    }
    Ok(())
}

/// The limits for one snippet; the message is shown to the user.
pub fn check(d: &Draft) -> Result<(), String> {
    if d.name.trim().is_empty() || d.body.trim().is_empty() {
        return Err("A snippet needs a name and some text.".into());
    }
    if d.name.chars().count() > MAX_NAME
        || d.body.chars().count() > MAX_BODY
        || d.triggers.len() > MAX_TRIGGERS
        || d.ending.chars().count() > MAX_ENDING
    {
        return Err(format!("A name can have {MAX_NAME} characters, the text {MAX_BODY}, there can be {MAX_TRIGGERS} triggers and {MAX_ENDING} ending characters."));
    }
    d.triggers.iter().try_for_each(|t| check_trigger(t))?;
    check_programs(&[&d.only_apps, &d.skip_apps])
}

pub fn check_folder(d: &FolderDraft) -> Result<(), String> {
    if d.name.trim().is_empty() || d.name.chars().count() > MAX_NAME {
        return Err(format!("A folder needs a name of up to {MAX_NAME} characters."));
    }
    check_programs(&[&d.only_apps, &d.skip_apps])
}

/// Two triggers that cannot both work: the same text, or (when both expand as soon as typed) one that is the start of the other, which would fire before the longer one is typed.
pub fn clash(mine: (&str, Mode), other: (&str, Mode)) -> bool {
    let (a, b) = (mine.0.to_lowercase(), other.0.to_lowercase());
    if a == b {
        return true;
    }
    let both_immediate = mine.1.when(mine.0) == When::Immediate && other.1.when(other.0) == When::Immediate;
    both_immediate && (a.starts_with(&b) || b.starts_with(&a))
}
