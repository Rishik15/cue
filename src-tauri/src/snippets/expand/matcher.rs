//! Purpose: Decides, from the characters typed, when a snippet's trigger has just been typed. Pure logic with no operating system in it, so it is tested alone.
//! Contents: Matcher — the triggers with their options, indexed by their last character; Tracker — the recent characters and what each new key does to them; Hit — a trigger that was typed; follow_case — the expansion in the case the trigger was typed in.
//! Only the last `MAX_TYPED` characters are held, in memory, and they are cleared at every word break, shortcut, window change and pause; nothing typed is ever written anywhere.

use std::collections::{HashMap, HashSet, VecDeque};

use super::super::check::DELIMITERS;
use super::super::model::{Effective, Mode, When};
use super::pattern::{Captured, Pattern, PREFIX};
use crate::platform::Typed;

const MAX_TYPED: usize = 64;

struct Rule {
    id: i64,
    /// As defined (folded to lower case when case does not matter).
    trigger: Vec<char>,
    /// The trigger exactly as the user wrote it, to tell a typed `:Sig` from `:sig`.
    written: String,
    match_case: bool,
    propagate_case: bool,
    when: When,
    /// A trigger that starts with a letter must start a word (`sig` is not found inside `design`), unless the snippet says otherwise.
    needs_word_start: bool,
    /// Characters that end this trigger; empty means the usual ones.
    ending: Vec<char>,
    only_apps: Vec<String>,
    skip_apps: Vec<String>,
    /// A trigger written `re:...`; its `trigger` is empty.
    pattern: Option<Pattern>,
}

#[derive(Default)]
pub struct Matcher {
    rules: Vec<Rule>,
    /// The rules that are patterns.
    patterns: Vec<usize>,
    /// The rules whose trigger ends in this character (lower case).
    by_last: HashMap<char, Vec<usize>>,
    /// Every character that ends some trigger: a typed one of these closes the word.
    delimiters: HashSet<char>,
}

/// A trigger was typed. `erase` is how many characters the app now holds for it (the trigger, and the space or punctuation that ended it), `retype` that ending, which goes back after the text, `typed` the trigger as
/// the user typed it and `changed_case` whether that differs from how it was written.
#[derive(Debug, PartialEq, Eq)]
pub struct Hit {
    pub id: i64,
    pub rule: usize,
    pub erase: usize,
    pub retype: Option<char>,
    pub typed: String,
    pub changed_case: bool,
    /// For a pattern trigger, the groups of what it matched (`"0"` is the whole match); empty otherwise.
    pub captures: Vec<(String, String)>,
}

fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

impl Matcher {
    pub fn new(rules: &[Effective]) -> Matcher {
        let mut matcher = Matcher::default();
        for e in rules.iter().filter(|e| !e.trigger.is_empty()) {
            let pattern = Pattern::new(&e.trigger);
            if e.trigger.starts_with(PREFIX) && pattern.is_none() {
                continue;
            }
            let chars: Vec<char> = match (&pattern, e.match_case) {
                (Some(_), _) => Vec::new(),
                (None, true) => e.trigger.chars().collect(),
                (None, false) => e.trigger.chars().map(fold).collect(),
            };
            let ending: Vec<char> = e.ending.chars().collect();
            matcher.delimiters.extend(if ending.is_empty() { DELIMITERS } else { ending.as_slice() });
            match chars.last() {
                Some(&last) => matcher.by_last.entry(fold(last)).or_default().push(matcher.rules.len()),
                None => matcher.patterns.push(matcher.rules.len()),
            }
            // A pattern has no first character to tell a symbol from a letter, so "auto" means as soon as it matches.
            let when = if pattern.is_some() && e.mode == Mode::Auto { When::Immediate } else { e.mode.when(&e.trigger) };
            matcher.rules.push(Rule {
                id: e.id,
                needs_word_start: !e.inside_word && chars.first().is_some_and(|c| c.is_alphanumeric()),
                trigger: chars,
                written: e.trigger.clone(),
                match_case: e.match_case,
                propagate_case: e.propagate_case,
                when,
                ending,
                only_apps: e.only_apps.clone(),
                skip_apps: e.skip_apps.clone(),
                pattern,
            });
        }
        matcher
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Whether the trigger (rule number `rule`) may expand in `program`: not on its skip list, and on its list of allowed programs when it has one.
    pub fn allows(&self, rule: usize, program: &str) -> bool {
        self.rules.get(rule).is_some_and(|r| {
            !r.skip_apps.iter().any(|a| a == program) && (r.only_apps.is_empty() || r.only_apps.iter().any(|a| a == program))
        })
    }

    pub fn follows_case(&self, rule: usize) -> bool {
        self.rules.get(rule).is_some_and(|r| r.propagate_case)
    }

    /// The longest trigger of kind `when` that the typed text ends with, ended by `end` when it waits for one.
    fn find(&self, typed: &VecDeque<char>, when: When, end: Option<char>) -> Option<usize> {
        let candidates = self.by_last.get(&fold(*typed.back()?))?;
        candidates
            .iter()
            .copied()
            .filter(|&i| {
                let r = &self.rules[i];
                r.when == when && ends_with(typed, r) && (!r.needs_word_start || word_start(typed, r.trigger.len())) && r.ends_with(end)
            })
            .max_by_key(|&i| self.rules[i].trigger.len())
    }
}

impl Matcher {
    /// The first pattern of kind `when` that the typed text ends with, with what it matched.
    fn find_pattern(&self, typed: &VecDeque<char>, when: When, end: Option<char>) -> Option<(usize, Captured)> {
        if self.patterns.is_empty() {
            return None;
        }
        let text: String = typed.iter().collect();
        self.patterns.iter().find_map(|&i| {
            let r = &self.rules[i];
            (r.when == when && r.ends_with(end)).then(|| r.pattern.as_ref()?.find(&text).map(|c| (i, c))).flatten()
        })
    }

    /// A literal trigger if one fits, otherwise a pattern.
    fn lookup(&self, typed: &VecDeque<char>, when: When, end: Option<char>) -> Option<(usize, Option<Captured>)> {
        self.find(typed, when, end).map(|i| (i, None)).or_else(|| self.find_pattern(typed, when, end).map(|(i, c)| (i, Some(c))))
    }
}

impl Rule {
    fn ends_with(&self, end: Option<char>) -> bool {
        let Some(c) = end else { return true };
        if self.ending.is_empty() {
            DELIMITERS.contains(&c)
        } else {
            self.ending.contains(&c)
        }
    }
}

fn ends_with(typed: &VecDeque<char>, rule: &Rule) -> bool {
    typed.len() >= rule.trigger.len()
        && typed.iter().rev().zip(rule.trigger.iter().rev()).all(|(&t, &r)| if rule.match_case { t == r } else { fold(t) == r })
}

fn word_start(typed: &VecDeque<char>, trigger_len: usize) -> bool {
    typed.len() == trigger_len || typed.get(typed.len() - trigger_len - 1).is_some_and(|c| !c.is_alphanumeric())
}

#[derive(Default)]
pub struct Tracker {
    typed: VecDeque<char>,
}

impl Tracker {
    pub fn clear(&mut self) {
        self.typed.clear();
    }

    /// What one key did. A trigger that expands at once is a hit when its last character is typed; one that waits is a hit when a space or punctuation follows it. Enter never ends a trigger:
    /// in a chat box it already sent the message.
    pub fn feed(&mut self, key: Typed, matcher: &Matcher) -> Option<Hit> {
        match key {
            Typed::Reset | Typed::Char('\n') => self.typed.clear(),
            Typed::Backspace => drop(self.typed.pop_back()),
            Typed::Char(c) if matcher.delimiters.contains(&c) && self.ends_word(matcher, c) => {
                let hit = matcher.lookup(&self.typed, When::Delimiter, Some(c)).map(|found| self.hit(matcher, found, Some(c)));
                self.typed.clear();
                return hit;
            }
            Typed::Char(c) if c.is_whitespace() => self.typed.clear(),
            Typed::Char(c) => {
                if self.typed.len() == MAX_TYPED {
                    self.typed.pop_front();
                }
                self.typed.push_back(c);
                let hit = matcher.lookup(&self.typed, When::Immediate, None).map(|found| self.hit(matcher, found, None));
                if hit.is_some() {
                    self.typed.clear();
                }
                return hit;
            }
        }
        None
    }

    /// Whether this delimiter closes a trigger that waits for one. Punctuation that closes nothing is an ordinary character, so a trigger may start with one (`;sig`).
    fn ends_word(&self, matcher: &Matcher, c: char) -> bool {
        c.is_whitespace() || matcher.lookup(&self.typed, When::Delimiter, Some(c)).is_some()
    }

    fn hit(&self, matcher: &Matcher, (rule, found): (usize, Option<Captured>), retype: Option<char>) -> Hit {
        let r = &matcher.rules[rule];
        let (typed, captures) = match found {
            Some(c) => (c.text, c.groups),
            None => (self.typed.iter().skip(self.typed.len() - r.trigger.len()).collect(), Vec::new()),
        };
        let erase = typed.chars().count() + usize::from(retype.is_some());
        Hit { id: r.id, rule, erase, retype, changed_case: typed != r.written && r.pattern.is_none(), typed, captures }
    }
}

#[cfg(test)]
#[path = "matcher_tests.rs"]
mod tests;
