//! Purpose: The limits snippets and folders obey, the tidying of a draft before it is stored, and the one validator for triggers that cannot coexist.
//! Contents: TriggerSet — the triggers in force, to which every write adds through `try_add`; program_matches — whether a list entry names the program in front; program_entry — a list entry as it is kept; check / check_folder — the limits, with messages for the user; clean / clean_folder — trimming and normalizing;
//! DELIMITERS — the characters that end a word.

use std::collections::{BTreeMap, HashMap};

use super::expand::pattern::{self, Pattern, PREFIX};
use super::keys::MAX_ENDING;
use super::model::{Draft, FolderDraft, When};
use super::policy::Effective;

pub const MAX_SNIPPETS: usize = 2000;
/// Most enabled `re:` triggers; every typed key is tried against each one, so the typing hook's work stays bounded.
pub const MAX_PATTERNS: usize = 50;
pub const MAX_NAME: usize = 80;
const MAX_TRIGGER: usize = 40;
const MAX_TRIGGERS: usize = 8;
const MAX_PROGRAMS: usize = 40;
const MAX_PROGRAM: usize = 160;
pub const MAX_BODY: usize = 20_000;
/// Typing one of these ends a word: a trigger that starts with a letter expands when one follows it. A trigger may still contain them (`;sig`).
pub const DELIMITERS: &[char] = &[' ', '\t', '.', ',', ';', '!', '?', ')', ']', '}', '"', '\''];

/// A program entry as it is kept. Entries are `Name<TAB>exe` from the picker (`WezTerm\twezterm-gui`): the name is shown, the executable (lower case, no `.exe`) is what the focus check compares. Input: the entry text. Returns it tidied, or empty for any other shape (no tab or no executable), which is dropped.
pub fn program_entry(text: &str) -> String {
    let Some((name, exe)) = text.split_once('\t') else { return String::new() };
    let lower = exe.trim().to_lowercase();
    let exe = lower.strip_suffix(".exe").unwrap_or(&lower);
    if exe.is_empty() {
        String::new()
    } else {
        format!("{}\t{exe}", name.trim())
    }
}

/// Whether a list entry names the program in front. Inputs: the kept entry and the executable in front (lower case, no `.exe`). Returns true when the entry's executable equals it.
pub fn program_matches(entry: &str, exe: &str) -> bool {
    entry.split_once('\t').is_some_and(|(_, wanted)| wanted == exe)
}

/// A program list as it is kept. Input: the entries. Returns them tidied by `program_entry`, the dropped shapes left out and one entry per executable (two Start menu names for the same file are one program).
fn programs(list: &[String]) -> Vec<String> {
    let exe = |entry: &str| entry.split_once('\t').map(|(_, exe)| exe.to_string());
    let mut out: Vec<String> = Vec::new();
    for app in list.iter().map(|a| program_entry(a)).filter(|a| !a.is_empty()) {
        if !out.iter().any(|other| exe(other) == exe(&app)) {
            out.push(app);
        }
    }
    out
}

/// The draft as it is stored: text trimmed, triggers listed once and program lists kept as `programs` keeps them. A trigger keeps its spaces (a space at either end is part of what must be typed); one that is only spaces is dropped.
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

/// The triggers in force, for every write to ask whether new ones may join: the same text, or an immediate one that starts another, is refused, and so is a `re:` trigger past `MAX_PATTERNS`.
#[derive(Default)]
pub struct TriggerSet {
    /// Every trigger by its lower-case text, as written.
    all: HashMap<String, String>,
    /// The ones that expand as soon as typed, sorted, so the triggers that start with a new one are one range away.
    immediate: BTreeMap<String, String>,
    patterns: usize,
}

impl TriggerSet {
    /// The set of triggers already in force. Input: them, not checked (an old library may hold a clash; that is not the business of a write that does not touch it).
    pub fn new<'a>(existing: impl IntoIterator<Item = &'a Effective>) -> TriggerSet {
        let mut set = TriggerSet::default();
        existing.into_iter().for_each(|e| set.insert(e));
        set
    }

    fn insert(&mut self, e: &Effective) {
        let key = e.trigger.to_lowercase();
        self.all.insert(key.clone(), e.trigger.clone());
        if e.when == When::Immediate {
            self.immediate.insert(key, e.trigger.clone());
        }
        self.patterns += usize::from(Pattern::new(&e.trigger).is_some());
    }

    fn remove(&mut self, e: &Effective) {
        let key = e.trigger.to_lowercase();
        self.all.remove(&key);
        self.immediate.remove(&key);
        self.patterns -= usize::from(Pattern::new(&e.trigger).is_some());
    }

    /// The stored trigger that `e` clashes with, if any: the same text, or (both immediate) a prefix of it or text it is the prefix of.
    fn clashing(&self, e: &Effective) -> Option<&String> {
        let key = e.trigger.to_lowercase();
        if let Some(same) = self.all.get(&key) {
            return Some(same);
        }
        if e.when != When::Immediate {
            return None;
        }
        let shorter = key.char_indices().skip(1).find_map(|(i, _)| self.immediate.get(&key[..i]));
        shorter.or_else(|| self.immediate.range(key.clone()..).next().filter(|(k, _)| k.starts_with(&key)).map(|(_, v)| v))
    }

    /// Adds the triggers of one snippet. Input: them. Returns Err with the message for the user (the pair that clashes, or the pattern limit) and leaves the set as it was; on Ok they are in the set.
    pub fn try_add(&mut self, mine: &[Effective]) -> Result<(), String> {
        let wanted = mine.iter().filter(|e| Pattern::new(&e.trigger).is_some()).count();
        if wanted > 0 && self.patterns + wanted > MAX_PATTERNS {
            return Err(format!(
                "Cue keeps {MAX_PATTERNS} pattern triggers (re:) at most, and this would make {}. Remove some first.",
                self.patterns + wanted
            ));
        }
        for (i, e) in mine.iter().enumerate() {
            if let Some(other) = self.clashing(e) {
                let message = format!(
                    "The trigger {} clashes with {other}: the same text, or one that would expand before the other can be typed.",
                    e.trigger
                );
                mine[..i].iter().for_each(|done| self.remove(done));
                return Err(message);
            }
            self.insert(e);
        }
        Ok(())
    }
}
