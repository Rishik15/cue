//! File: strip/choose.rs
//! Purpose: Which actions the selection strip shows as chips: the three that fit the selected text best. The user's saved actions lead (their own conditions already matched), then every other row is scored by cheap signals read from the text and shown best first; the last action used breaks ties, then the fixed order below. Only rows that would change the text reach the chooser (`palette::build` leaves the no-ops out), so the strip never offers a no-op, and it always fills three chips when three rows apply. The naming cases (upper, lower, title, camel, snake, kebab) apply only to a short single line, where they make sense. Pure, so it is tested without the OS.
//!
//! Signals and the scores they give (an action takes the highest of its matching signals; 0 means "applies, no evidence"):
//! - `json` (starts with `{` or `[` and has a `"key":` pair): Format JSON 90, Minify JSON 60.
//! - `tracking` (a link with `utm_`, `fbclid`, `gclid`, `mc_eid`, `si=` or `ref=`): Clean Links 90. `links` (http, https or www.): Clean Links 50, Extract Links 25.
//! - `spacing` (two spaces between words, a tab, a trailing space or three blank lines in a row): Clean Whitespace 70.
//! - `wrapped` (at least two line breaks inside a sentence: the line ends with no closing punctuation and the next starts in lower case): Join Broken Lines 80.
//! - `fillers` (um, uh, er, erm, hmm, uhm as whole words): Remove Filler Words 75.
//! - `repeats` (the same word twice in a row, ignoring case): Remove Repeated Words 85.
//! - `shouting` (eight or more letters, 90 % upper case): Sentence Case 75. `flat` (four or more words, no upper-case letter): Sentence Case 60.
//! - `quotes` (curly quotes): Use Plain Quotes 60; `mixed_quotes` (curly and straight together): 80.
//! - `listy` (a line that starts with a bullet or a number): Rich Text to Markdown 55, and 35 whenever it is offered (formatting was copied).
//! - `identifier` (a short line with no space but `_`, `-` or a lower-to-upper step): Snake Case 70, Camel Case 65, Kebab Case 60. `name` (a short line of two to six words): Title Case 55, Camel Case 50, Snake Case 45, Kebab Case 40, Uppercase 35, Lowercase 30.
//! - `typos` (a lone lower-case "i", a space before `,.;:!?`, a lower-case sentence start, or a common misspelling): Correct Text 85. `prose` (two sentences, or twelve words): Correct Text 60.
//! - `long` (eighty words): Make Shorter 70; `longer` (150 words): Summarize 55.
//!
//! Every detector reads at most the first `SCAN` bytes (a 200 000-character selection costs the same as a 4 KiB one) and uses no regular expressions.
//! Contents: CHIPS; SCAN; Candidate — what the chooser knows of a row; Signals — what the text looks like; signals — read them; score — an action's score; chips — pick the chips; the detectors.

/// The most chips the strip shows besides More.
pub const CHIPS: usize = 3;
/// The most bytes of the text any detector reads.
pub const SCAN: usize = 4096;

/// The built-ins in the order they win a tie of equal scores (after the last action used).
const PRIORITY: [&str; 11] = [
    "action:correct",
    "action:clean_whitespace",
    "action:join_lines",
    "markdown",
    "action:clean_links",
    "action:plain_quotes",
    "action:sentence_case",
    "action:remove_fillers",
    "action:json_pretty",
    "action:make_shorter",
    "action:title_case",
];

/// The naming cases, offered as chips only for a short single line.
const NAMING: [&str; 6] =
    ["action:upper_case", "action:lower_case", "action:title_case", "action:camel_case", "action:snake_case", "action:kebab_case"];

/// The longest text, in characters, that counts as a short single line.
const SHORT_LINE: usize = 80;

/// Words that are common misspellings or missing apostrophes; any of them is a typo signal.
const MISSPELT: [&str; 18] = [
    "teh",
    "recieve",
    "recieved",
    "definately",
    "seperate",
    "occured",
    "untill",
    "wich",
    "alot",
    "dont",
    "cant",
    "wont",
    "didnt",
    "im",
    "youre",
    "thier",
    "beleive",
    "tommorow",
];

/// Filler words a speaker leaves in.
const FILLERS: [&str; 6] = ["um", "uh", "er", "erm", "hmm", "uhm"];

/// Query keys that only track the reader.
const TRACKERS: [&str; 6] = ["utm_", "fbclid", "gclid", "mc_eid", "si=", "ref="];

/// What the chooser knows of a row: its id (`rule:...`, `action:...`, `markdown`) and whether it is one of the user's saved actions.
pub struct Candidate<'a> {
    pub id: &'a str,
    pub saved: bool,
}

/// What the selected text looks like; each field is one signal of the module header.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Signals {
    pub short: bool,
    pub json: bool,
    pub links: bool,
    pub tracking: bool,
    pub spacing: bool,
    pub wrapped: bool,
    pub fillers: bool,
    pub repeats: bool,
    pub shouting: bool,
    pub flat: bool,
    pub quotes: bool,
    pub mixed_quotes: bool,
    pub listy: bool,
    pub identifier: bool,
    pub name: bool,
    pub typos: bool,
    pub prose: bool,
    pub long: bool,
    pub longer: bool,
}

/// The first `SCAN` bytes of `text`, cut at a character boundary. Input: the text. Return: the sample every detector reads.
pub fn sample(text: &str) -> &str {
    let mut end = text.len().min(SCAN);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Reads every signal from the text. Input: the selected text. Return: the signals.
pub fn signals(text: &str) -> Signals {
    let s = sample(text);
    let short = !s.contains('\n') && text.chars().take(SHORT_LINE + 1).count() <= SHORT_LINE;
    let words = s.split_whitespace().count();
    Signals {
        short,
        json: json(s),
        links: links(s),
        tracking: tracking(s),
        spacing: spacing(s),
        wrapped: wrapped(s),
        fillers: any_word(s, |w| FILLERS.contains(&w)),
        repeats: repeats(s),
        shouting: shouting(s),
        flat: words >= 4 && !s.chars().any(char::is_uppercase),
        quotes: s.contains(['\u{201C}', '\u{201D}', '\u{2018}', '\u{2019}']),
        mixed_quotes: s.contains(['\u{201C}', '\u{201D}', '\u{2018}', '\u{2019}']) && s.contains(['"', '\'']),
        listy: listy(s),
        identifier: short && identifier(s),
        name: short && (2..=6).contains(&words) && !identifier(s),
        typos: typos(s),
        prose: words >= 12 || sentences(s) >= 2,
        long: words >= 80,
        longer: words >= 150,
    }
}

/// The score `id` earns from the signals: the highest of its matching signals, 0 when none match. Input: the row id and the signals. Return: the score.
pub fn score(id: &str, g: &Signals) -> u32 {
    let table: &[(bool, u32)] = match id {
        "action:json_pretty" => &[(g.json, 90)],
        "action:json_minify" => &[(g.json, 60)],
        "action:clean_links" => &[(g.tracking, 90), (g.links, 50)],
        "action:extract_urls" => &[(g.links, 25)],
        "action:clean_whitespace" => &[(g.spacing, 70)],
        "action:join_lines" => &[(g.wrapped, 80)],
        "action:remove_fillers" => &[(g.fillers, 75)],
        "action:remove_repeats" => &[(g.repeats, 85)],
        "action:sentence_case" => &[(g.shouting, 75), (g.flat, 60)],
        "action:plain_quotes" => &[(g.mixed_quotes, 80), (g.quotes, 60)],
        "markdown" => &[(g.listy, 55), (true, 35)],
        "action:snake_case" => &[(g.identifier, 70), (g.name, 45)],
        "action:camel_case" => &[(g.identifier, 65), (g.name, 50)],
        "action:kebab_case" => &[(g.identifier, 60), (g.name, 40)],
        "action:title_case" => &[(g.name, 55)],
        "action:upper_case" => &[(g.name, 35)],
        "action:lower_case" => &[(g.name, 30)],
        "action:correct" => &[(g.typos, 85), (g.prose, 60)],
        "action:make_shorter" => &[(g.long, 70)],
        "action:summarize" => &[(g.longer, 55)],
        _ => &[],
    };
    table.iter().filter(|(on, _)| *on).map(|(_, s)| *s).max().unwrap_or(0)
}

/// The rows to show as chips, in order. Input: the rows, the id of the last action used (if any), the selected text. Return: at most `CHIPS` row indexes, without repeats: saved actions in list order, then the rest by score, the last action used, `PRIORITY` and list order.
pub fn chips(rows: &[Candidate], last: Option<&str>, text: &str) -> Vec<usize> {
    let g = signals(text);
    let fits = |i: &usize| rows[*i].saved || g.short || !NAMING.contains(&rows[*i].id);
    let rank = |i: usize| {
        let id = rows[i].id;
        let tie = PRIORITY.iter().position(|p| *p == id).unwrap_or(PRIORITY.len());
        (std::cmp::Reverse(score(id, &g)), Some(id) != last, tie, i)
    };
    let mut others: Vec<usize> = (0..rows.len()).filter(|i| !rows[*i].saved).filter(fits).collect();
    others.sort_by_key(|i| rank(*i));
    (0..rows.len()).filter(|i| rows[*i].saved).chain(others).take(CHIPS).collect()
}

/// Whether the text looks like JSON: it opens with `{` or `[` and holds a `"key":` pair. Input: the sample. Return: the answer.
fn json(s: &str) -> bool {
    let t = s.trim_start();
    (t.starts_with('{') || t.starts_with('[')) && t.contains("\":")
}

/// Whether the text holds a link. Input: the sample. Return: the answer.
fn links(s: &str) -> bool {
    s.contains("http://") || s.contains("https://") || s.contains("www.")
}

/// Whether a link carries a tracking parameter. Input: the sample. Return: the answer.
fn tracking(s: &str) -> bool {
    links(s) && s.split_whitespace().any(|w| w.contains('?') && TRACKERS.iter().any(|t| w.contains(t)))
}

/// Whether the spacing needs cleaning: two spaces between words, a tab, a space before a line break or at the end, or three blank lines in a row. Input: the sample. Return: the answer.
fn spacing(s: &str) -> bool {
    let doubled = s.split('\n').any(|line| line.trim().contains("  "));
    let trailing = s.split('\n').any(|line| line.ends_with(' '));
    doubled || trailing || s.contains('\t') || s.replace('\r', "").contains("\n\n\n\n")
}

/// Whether the text was hard-wrapped: at least two line breaks fall inside a sentence (the line has no closing punctuation and the next line starts in lower case). Input: the sample. Return: the answer.
fn wrapped(s: &str) -> bool {
    let lines: Vec<&str> = s.lines().map(str::trim).collect();
    let inside = lines.windows(2).filter(|p| {
        let open = p[0].chars().last().is_some_and(|c| c.is_alphanumeric() || c == ',');
        open && p[1].chars().next().is_some_and(char::is_lowercase)
    });
    inside.count() >= 2
}

/// The words of the text, lower-cased and stripped of the punctuation round them. Input: the sample. Return: the words.
fn words(s: &str) -> impl Iterator<Item = String> + '_ {
    s.split_whitespace().map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()).filter(|w| !w.is_empty())
}

/// Whether any word passes `test`. Input: the sample and the test. Return: the answer.
fn any_word(s: &str, test: impl Fn(&str) -> bool) -> bool {
    words(s).any(|w| test(&w))
}

/// Whether a word is said twice in a row, ignoring case (numbers do not count). Input: the sample. Return: the answer.
fn repeats(s: &str) -> bool {
    let list: Vec<String> = words(s).collect();
    list.windows(2).any(|p| p[0] == p[1] && p[0].chars().any(char::is_alphabetic))
}

/// Whether the text is shouted: at least eight letters, nine in ten of them upper case. Input: the sample. Return: the answer.
fn shouting(s: &str) -> bool {
    let (mut upper, mut letters) = (0usize, 0usize);
    for c in s.chars().filter(|c| c.is_alphabetic()) {
        letters += 1;
        upper += usize::from(c.is_uppercase());
    }
    letters >= 8 && upper * 10 >= letters * 9
}

/// Whether a line starts with a bullet or a number, as a list does. Input: the sample. Return: the answer.
fn listy(s: &str) -> bool {
    s.lines().map(str::trim_start).any(|l| {
        let bullet = ["• ", "◦ ", "▪ ", "- ", "* "].iter().any(|b| l.starts_with(b));
        let digits = l.chars().take_while(char::is_ascii_digit).count();
        bullet || (digits > 0 && (l[digits..].starts_with(". ") || l[digits..].starts_with(") ")))
    })
}

/// Whether one word is a code name: no space, and an `_`, a `-` between letters or a lower-to-upper step. Input: the sample. Return: the answer.
fn identifier(s: &str) -> bool {
    let t = s.trim();
    let hump = t.chars().zip(t.chars().skip(1)).any(|(a, b)| a.is_lowercase() && b.is_uppercase());
    !t.is_empty() && !t.contains(char::is_whitespace) && (t.contains('_') || t.contains('-') || hump)
}

/// Whether the text has a typo signal: a lone lower-case "i", a space before punctuation, a sentence that starts in lower case, or a common misspelling. Input: the sample. Return: the answer.
fn typos(s: &str) -> bool {
    let lone_i = s.split_whitespace().any(|w| w == "i" || w.starts_with("i'"));
    let spaced = [" ,", " .", " ;", " :", " !", " ?"].iter().any(|p| s.contains(p));
    let low_start = [". ", "! ", "? "].iter().any(|p| s.split(p).skip(1).any(|r| r.chars().next().is_some_and(char::is_lowercase)));
    lone_i || spaced || low_start || any_word(s, |w| MISSPELT.contains(&w))
}

/// How many sentences end in the text. Input: the sample. Return: the count.
fn sentences(s: &str) -> usize {
    s.split_whitespace().filter(|w| w.len() > 1 && w.ends_with(['.', '!', '?'])).count()
}
