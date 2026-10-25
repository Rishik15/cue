//! Purpose: Whitespace, line-break and letter-case transforms.
//! Contents: clean_whitespace; join_lines (PDF line breaks); split_bullets (one bullet per line or sentence, no model); title_case / sentence_case.

/// Trims every line, collapses runs of spaces and tabs, and keeps at most one blank line between paragraphs.
pub fn clean_whitespace(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = line.split([' ', '\t', '\u{a0}']).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ");
        if line.is_empty() && out.last().is_none_or(|l| l.is_empty()) {
            continue;
        }
        out.push(line);
    }
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out.join("\n")
}

/// Text copied from a PDF or e-mail is wrapped at a fixed width. Lines of one paragraph are joined with a space; a word split by a
/// hyphen at the line end is rejoined; blank lines and list items keep their own line.
pub fn join_lines(text: &str) -> String {
    let mut out = String::new();
    let mut prev = "";
    for line in text.lines().map(str::trim) {
        if line.is_empty() {
            out.push_str(if prev.is_empty() { "" } else { "\n\n" });
        } else if prev.is_empty() {
            out.push_str(line);
        } else if is_list_item(line) {
            out.push('\n');
            out.push_str(line);
        } else if split_word(prev, line) {
            out.pop(); // the hyphen
            out.push_str(line);
        } else {
            out.push(' ');
            out.push_str(line);
        }
        prev = line;
    }
    out.trim_end().to_string()
}

/// A bullet or a number marker followed by a space. Slicing by bytes would panic on a multi-byte bullet such as `•`.
fn is_list_item(line: &str) -> bool {
    let spaced = |rest: Option<&str>| rest.is_some_and(|r| r.starts_with(' '));
    let digits = line.trim_start_matches(|c: char| c.is_ascii_digit());
    spaced(line.strip_prefix(['-', '*', '•'])) || (digits.len() < line.len() && spaced(digits.strip_prefix(['.', ')'])))
}

/// `prev` ends in `letter-` and `next` continues in lower case: the word was broken by the wrap.
fn split_word(prev: &str, next: &str) -> bool {
    prev.strip_suffix('-').is_some_and(|stem| stem.ends_with(char::is_alphabetic)) && next.starts_with(char::is_lowercase)
}

/// One bullet per line, or per sentence when the text is a single line. Existing markers are replaced so none is doubled. shortcut: a sentence ends at . ! or ? followed by a space, so "e.g. this" splits; real sentence detection needs a language model.
pub fn split_bullets(text: &str) -> String {
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let items = match lines.as_slice() {
        [one] => sentences(one),
        many => many.iter().map(|l| l.to_string()).collect(),
    };
    items.iter().map(|item| format!("- {}", strip_marker(item))).collect::<Vec<_>>().join("\n")
}

fn sentences(line: &str) -> Vec<String> {
    let (mut out, mut current, mut chars) = (Vec::new(), String::new(), line.chars().peekable());
    while let Some(c) = chars.next() {
        current.push(c);
        if matches!(c, '.' | '!' | '?') && chars.peek().is_none_or(|n| n.is_whitespace()) {
            out.push(current.trim().to_string());
            current.clear();
        }
    }
    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }
    out
}

fn strip_marker(item: &str) -> &str {
    if !is_list_item(item) {
        return item;
    }
    item.trim_start_matches(|c: char| c.is_ascii_digit()).trim_start_matches(['-', '*', '•', '.', ')']).trim_start()
}

/// `text` with the first character of each whitespace-separated word upper case and its remaining characters lower case, so a word that starts with a quote is not capitalized (`"HELLO"` gives `"hello"`); returns the new text. shortcut: capitalizes every word, including "of" and "and"; style-guide rules (AP, Chicago) need a per-language stop list.
pub fn title_case(text: &str) -> String {
    map_words(text, |word| {
        let mut chars = word.chars();
        chars.next().map(|c| c.to_uppercase().chain(chars.flat_map(char::to_lowercase)).collect()).unwrap_or_default()
    })
}

fn map_words(text: &str, f: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    for c in text.chars() {
        if c.is_whitespace() {
            out.push_str(&f(&std::mem::take(&mut word)));
            out.push(c);
        } else {
            word.push(c);
        }
    }
    out.push_str(&f(&word));
    out
}

/// `text` lower-cased, except the first letter met at the start or after `.`, `!`, `?` or a newline, which is upper case (a number first ends the wait, so `123 HELLO` gives `123 hello`); returns the new text. shortcut: lowercases everything first, so "NASA" becomes "Nasa"; keeping acronyms needs a dictionary.
pub fn sentence_case(text: &str) -> String {
    let mut start = true;
    text.chars()
        .flat_map(|c| {
            let out: Vec<char> = if start && c.is_alphabetic() { c.to_uppercase().collect() } else { c.to_lowercase().collect() };
            if c.is_alphabetic() || c.is_numeric() {
                start = false;
            } else if matches!(c, '.' | '!' | '?' | '\n') {
                start = true;
            }
            out
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_is_collapsed_per_line() {
        assert_eq!(clean_whitespace("  a   b \t c \n\n\n\n d  \n"), "a b c\n\nd");
    }

    #[test]
    fn pdf_lines_join_and_hyphenated_words_heal() {
        let text = "The quick brown fox jumps over the lazy dog and keeps run-\nning for a long time.\n\nNext para\ngraph here.\n- item one\n- item two";
        assert_eq!(join_lines(text), "The quick brown fox jumps over the lazy dog and keeps running for a long time.\n\nNext para graph here.\n- item one\n- item two");
    }

    #[test]
    fn list_markers_are_found_without_slicing_inside_a_character() {
        assert!(is_list_item("• item") && is_list_item("- item") && is_list_item("12. item") && is_list_item("3) item"));
        assert!(!is_list_item("-item") && !is_list_item(". item") && !is_list_item("1.5 liters") && !is_list_item("é item"));
        assert_eq!(join_lines("Intro text\n• one\n• two"), "Intro text\n• one\n• two");
    }

    #[test]
    fn bullets_split_by_line_or_sentence_without_doubling_markers() {
        assert_eq!(split_bullets("One. Two! Three?"), "- One.\n- Two!\n- Three?");
        assert_eq!(split_bullets("• a\n\n2) b\nplain"), "- a\n- b\n- plain");
        assert_eq!(split_bullets("Add 1.5 liters. Stir"), "- Add 1.5 liters.\n- Stir");
    }

    #[test]
    fn cases() {
        assert_eq!(title_case("hello wORLD  now"), "Hello World  Now");
        assert_eq!(sentence_case("HELLO there. how ARE you? fine"), "Hello there. How are you? Fine");
    }
}
