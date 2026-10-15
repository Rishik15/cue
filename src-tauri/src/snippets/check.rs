//! Purpose: The limits snippets and folders obey, the tidying of a draft before it is stored, and the test for two triggers that cannot coexist.
//! Contents: program_matches — whether a list entry names the program in front; check / check_folder — the limits, with messages for the user; clean / clean_folder — trimming and normalizing; program_name — a program as the foreground check names it; clash — two triggers that cannot both work;
//! DELIMITERS — the characters that end a word.

use super::expand::pattern::{self, PREFIX};
use super::model::{Draft, FolderDraft, Mode, When};

pub const MAX_SNIPPETS: usize = 2000;
pub const MAX_NAME: usize = 80;
const MAX_TRIGGER: usize = 40;
const MAX_TRIGGERS: usize = 8;
const MAX_ENDING: usize = 20;
const MAX_PREFIX: usize = 10;
const MAX_PROGRAMS: usize = 40;
const MAX_PROGRAM: usize = 80;
pub const MAX_BODY: usize = 20_000;
/// Typing one of these ends a word: a trigger that starts with a letter expands when one follows it. A trigger may still contain them (`;sig`).
pub const DELIMITERS: &[char] = &[' ', '\t', '.', ',', ';', '!', '?', ')', ']', '}', '"', '\''];

/// A program name as the foreground check reports it: lower case, without `.exe`.
pub fn program_name(text: &str) -> String {
    let lower = text.trim().to_lowercase();
    lower.strip_suffix(".exe").unwrap_or(&lower).to_string()
}

/// The words of a program name: lower case, `.exe` dropped, split at anything that is not a letter or a digit (`WezTerm-GUI.exe` is `wezterm`, `gui`).
fn words(name: &str) -> Vec<String> {
    let lower = name.to_lowercase();
    let bare = lower.strip_suffix(".exe").unwrap_or(&lower);
    bare.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(String::from).collect()
}

/// Whether a list entry names the program in front. `names` are what the program is called: its executable and its own name (`wezterm-gui`, `wezterm`). An entry matches a name when its words are the name's words or a run of
/// them, so `wezterm` finds `wezterm-gui`, `code` finds `Visual Studio Code`, `edge` finds `Microsoft Edge`, and `Visual Studio Code` finds itself; `code` does not find `codex`, and spaces, dashes and case do not matter.
pub fn program_matches(entry: &str, names: &[String]) -> bool {
    let wanted = words(entry);
    !wanted.is_empty() && names.iter().any(|name| words(name).windows(wanted.len()).any(|run| run == wanted.as_slice()))
}

fn programs(list: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for app in list.iter().map(|a| program_name(a)).filter(|a| !a.is_empty()) {
        if !out.contains(&app) {
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
    FolderDraft {
        name: d.name.trim().into(),
        prefix: d.prefix.trim().into(),
        only_apps: programs(&d.only_apps),
        skip_apps: programs(&d.skip_apps),
        ..d.clone()
    }
}

fn check_trigger(t: &str) -> Result<(), String> {
    if let Some(pattern) = t.strip_prefix(PREFIX) {
        pattern::compile(pattern)?;
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
    if d.prefix.chars().count() > MAX_PREFIX || d.prefix.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(format!("A prefix has {MAX_PREFIX} characters at most, with no spaces."));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triggers_that_would_fire_early_or_twice_clash() {
        assert!(clash((":addr", Mode::Auto), (":ADDR", Mode::Auto)));
        assert!(clash((":addr", Mode::Auto), (":addr2", Mode::Auto)));
        assert!(!clash(("addr", Mode::Auto), ("addr2", Mode::Auto)), "letter triggers wait for a space");
        assert!(!clash((":a1", Mode::Auto), (":b1", Mode::Auto)));
    }

    #[test]
    fn programs_and_triggers_are_normalized_and_listed_once() {
        let d = Draft {
            name: " n ".into(),
            body: "b".into(),
            triggers: vec![" :a1 ".into(), ":a1".into(), " ".into(), " :a1 ".into()],
            skip_apps: vec!["Chrome.EXE".into(), "chrome".into()],
            ..Draft::default()
        };
        let c = clean(&d);
        assert_eq!(
            (c.triggers, c.skip_apps),
            (vec![" :a1 ".to_string(), ":a1".to_string()], vec!["chrome".to_string()]),
            "spaces stay, a space-only trigger and a repeat go"
        );
    }

    #[test]
    fn a_program_entry_finds_the_program_by_either_of_its_names() {
        let wez = ["wezterm-gui".to_string(), "wezterm".to_string()];
        let code = ["code".to_string(), "visual studio code".to_string()];
        assert!(program_matches("wezterm", &wez) && program_matches("WezTerm", &wez) && program_matches("wezterm-gui.exe", &wez));
        assert!(program_matches("code", &code) && program_matches("Visual Studio Code", &code) && program_matches("studio code", &code));
        assert!(
            !program_matches("wez", &wez)
                && !program_matches("code", &["codex".to_string()])
                && !program_matches("", &code)
                && !program_matches("studio", &wez)
        );
    }

    #[test]
    fn triggers_and_prefixes_need_a_valid_shape() {
        let draft = |t: &str| Draft { name: "n".into(), body: "b".into(), triggers: vec![t.into()], ..Draft::default() };
        assert!(check(&draft(":a")).is_ok() && check(&Draft { name: "n".into(), body: "b".into(), ..Draft::default() }).is_ok());
        assert!(check(&draft(r"re:\d+px")).is_ok() && check(&draft("re:a*")).is_err() && check(&draft("re:(")).is_err());
        assert!(
            check(&draft(":")).is_ok()
                && check(&draft("i")).is_ok()
                && check(&draft(" i ")).is_ok()
                && check(&draft("good morning")).is_ok()
        );
        assert!(
            check(&draft("")).is_err() && check(&draft("   ")).is_err() && check(&draft("a\tb")).is_err() && check(&draft(";a.b")).is_ok()
        );
        assert!(check(&draft(&"x".repeat(41))).is_err());
        let folder = |p: &str| FolderDraft { name: "f".into(), enabled: true, prefix: p.into(), only_apps: vec![], skip_apps: vec![] };
        assert!(check_folder(&folder(";")).is_ok() && check_folder(&folder("; ")).is_err());
    }
}
