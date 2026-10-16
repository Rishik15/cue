//! Purpose: Decides, from the characters typed, when a snippet's trigger has just been typed. Pure logic with no operating system in it, so it is tested alone.
//! Contents: Matcher — the triggers with their options, indexed by their last character; Tracker — the recent characters and what each new key does to them; Hit — a trigger that was typed.
//! A trigger is any text of 1 to 40 characters and may contain spaces. Spaces at its edges are part of what must be typed but are kept: `i ` and ` i ` both fire for the word `i`, and the surrounding spaces stay where they were typed;
//! spaces inside (`good morning`) are erased with the rest. A trigger fires either as soon as its last character is typed (`:sig`, `i `) or, when it starts with a letter and does not end with a space or punctuation (`sig`),
//! when the space or punctuation that ends the word follows it. When two triggers fit one key, the one that erases more wins, and on a tie the one written with its own ending.
//! Only the last `MAX_TYPED` characters are held, in memory, and they are cleared at every Enter, shortcut, navigation key, window change and pause; they are also cleared at every space unless some trigger contains a space.
//! Nothing typed is ever written anywhere.

use std::collections::{HashMap, HashSet, VecDeque};

use super::super::check::{program_matches, DELIMITERS};
use super::super::model::{Effective, Mode, When};
use super::pattern::{Captured, Pattern, PREFIX};
use crate::platform::Typed;

const MAX_TYPED: usize = 64;
/// Most `re:` triggers kept; every keystroke is tried against each one.
const MAX_PATTERNS: usize = 50;

struct Rule {
    id: i64,
    /// As defined, spaces included (folded to lower case when case does not matter); empty for a pattern.
    trigger: Vec<char>,
    /// Spaces at the start and at the end of the trigger: required, never erased.
    lead: usize,
    trail: String,
    /// The trigger without those spaces, exactly as written, to tell a typed `:Sig` from `:sig`.
    written: String,
    match_case: bool,
    propagate_case: bool,
    when: When,
    /// A trigger that starts with a letter must start a word (`sig` is not found inside `design`), unless the snippet says otherwise or the trigger starts with a space.
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
    /// Some trigger contains a space, so the typed text must survive a space.
    spans_words: bool,
}

/// A trigger was typed. `erase` is how many characters the app now holds for it (the trigger without its leading spaces, and the space or punctuation that ended it), `retype` what goes back after the text (the trigger's trailing
/// spaces and that ending), `typed` the trigger as the user typed it and `changed_case` whether that differs from how it was written.
#[derive(Debug, PartialEq, Eq)]
pub struct Hit {
    pub id: i64,
    pub rule: usize,
    pub erase: usize,
    pub retype: String,
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
        for e in rules.iter().filter(|e| !e.trigger.trim().is_empty()) {
            let pattern = Pattern::new(&e.trigger);
            if e.trigger.starts_with(PREFIX) && (pattern.is_none() || matcher.patterns.len() >= MAX_PATTERNS) {
                continue;
            }
            matcher.add(e, pattern);
        }
        matcher
    }

    fn add(&mut self, e: &Effective, pattern: Option<Pattern>) {
        let chars: Vec<char> = match (&pattern, e.match_case) {
            (Some(_), _) => Vec::new(),
            (None, true) => e.trigger.chars().collect(),
            (None, false) => e.trigger.chars().map(fold).collect(),
        };
        let lead = chars.iter().take_while(|c| c.is_whitespace()).count();
        let trail: String = e.trigger.chars().rev().take_while(|c| c.is_whitespace()).collect();
        let ending: Vec<char> = e.ending.chars().collect();
        self.delimiters.extend(if ending.is_empty() { DELIMITERS } else { ending.as_slice() });
        self.spans_words |= pattern.is_none() && e.trigger.contains(char::is_whitespace);
        match chars.last() {
            Some(&last) => self.by_last.entry(fold(last)).or_default().push(self.rules.len()),
            None => self.patterns.push(self.rules.len()),
        }
        // A pattern has no first character to tell a symbol from a letter, so "auto" means as soon as it matches.
        let when = if pattern.is_some() && e.mode == Mode::Auto { When::Immediate } else { e.mode.when(&e.trigger) };
        self.rules.push(Rule {
            id: e.id,
            needs_word_start: !e.inside_word && lead == 0 && chars.first().is_some_and(|c| c.is_alphanumeric()),
            trigger: chars,
            lead,
            written: if pattern.is_some() { e.trigger.clone() } else { e.trigger.trim().to_string() },
            trail,
            match_case: e.match_case,
            propagate_case: e.propagate_case,
            when,
            ending,
            only_apps: e.only_apps.clone(),
            skip_apps: e.skip_apps.clone(),
            pattern,
        });
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Whether the trigger (rule number `rule`) may expand in the program in front (`exe` is its executable): not on its skip list, and on its list of allowed programs when it has one.
    pub fn allows(&self, rule: usize, exe: &str) -> bool {
        self.rules.get(rule).is_some_and(|r| {
            !r.skip_apps.iter().any(|a| program_matches(a, exe))
                && (r.only_apps.is_empty() || r.only_apps.iter().any(|a| program_matches(a, exe)))
        })
    }

    pub fn follows_case(&self, rule: usize) -> bool {
        self.rules.get(rule).is_some_and(|r| r.propagate_case)
    }

    /// The longest literal trigger of kind `when` that the typed text ends with, ended by `end` when it waits for one.
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

    /// What one key did. Enter never ends a trigger (in a chat box it already sent the message) and starts afresh; Backspace takes the last character back; any other character may complete a trigger.
    pub fn feed(&mut self, key: Typed, matcher: &Matcher) -> Option<Hit> {
        match key {
            Typed::Reset | Typed::Char('\n') => self.typed.clear(),
            Typed::Backspace => drop(self.typed.pop_back()),
            Typed::Char(c) => return self.on_char(c, matcher),
        }
        None
    }

    fn on_char(&mut self, c: char, matcher: &Matcher) -> Option<Hit> {
        // A trigger that waits for its ending is complete when `c` ends it; one that fires on its own last character is complete once `c` is added. Both can fit: the one that erases more wins, the written ending on a tie.
        let waiting = matcher.delimiters.contains(&c).then(|| matcher.lookup(&self.typed, When::Delimiter, Some(c))).flatten();
        let waiting = waiting.map(|found| self.hit(matcher, found, Some(c)));
        self.push(c);
        let now = matcher.lookup(&self.typed, When::Immediate, None).map(|found| self.hit(matcher, found, None));
        let chosen = match (waiting, now) {
            (Some(w), Some(n)) => Some(if w.erase > n.erase { w } else { n }),
            (w, n) => w.or(n),
        };
        if chosen.is_some() || (c.is_whitespace() && !matcher.spans_words) {
            self.typed.clear();
        }
        chosen
    }

    fn push(&mut self, c: char) {
        if self.typed.len() == MAX_TYPED {
            self.typed.pop_front();
        }
        self.typed.push_back(c);
    }

    /// The hit for a rule found in the typed text; for a trigger that waited, `ending` is the character that ended it (typed already, not yet in the text).
    fn hit(&self, matcher: &Matcher, (rule, found): (usize, Option<Captured>), ending: Option<char>) -> Hit {
        let r = &matcher.rules[rule];
        let (typed, captures, kept) = match found {
            Some(c) => (c.text, c.groups, String::new()),
            None => {
                let whole: String = self.typed.iter().skip(self.typed.len() - r.trigger.len()).collect();
                let core: String = whole.chars().skip(r.lead).take(r.trigger.len() - r.lead - r.trail.chars().count()).collect();
                (core, Vec::new(), r.trail.clone())
            }
        };
        let retype = format!("{kept}{}", ending.map(String::from).unwrap_or_default());
        let erase = typed.chars().count() + retype.chars().count();
        let changed_case = r.pattern.is_none() && typed != r.written;
        Hit { id: r.id, rule, erase, retype, typed, changed_case, captures }
    }
}

#[cfg(test)]
#[path = "matcher_tests.rs"]
mod tests;
