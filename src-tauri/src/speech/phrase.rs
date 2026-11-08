//! Purpose: Turns the growing hypothesis of a streaming recognizer into clauses that are safe to type. A streaming transducer never revises a
//! word once it is emitted; only the last word can still grow ("four" becoming "fourteen"). So everything but the last word is settled, and
//! settled words are handed out in clauses (the unit vocabulary, fillers and rules work on), not one word at a time.
//! Contents: Phrases — what has been handed out so far; next — the clauses that became ready; reset — a new utterance starts (after a pause).

/// A clause ends at . ? ! at once, at , ; : once it has this many words, and never grows past `MAX_WORDS`.
const COMMA_AFTER: usize = 4;
const MAX_WORDS: usize = 10;

#[derive(Default)]
pub struct Phrases {
    /// Bytes of the hypothesis already handed out.
    done: usize,
}

impl Phrases {
    pub fn reset(&mut self) {
        self.done = 0;
    }

    /// The clauses of `hypothesis` (the whole text of the current utterance so far) not handed out before. `last`: nothing more is coming
    /// (a pause, or Stop), so the open last word counts too.
    pub fn next(&mut self, hypothesis: &str, last: bool) -> Vec<String> {
        let Some(rest) = hypothesis.get(self.done..) else { return Vec::new() };
        let words: Vec<(usize, &str)> = rest.split_whitespace().map(|w| (w.as_ptr() as usize - rest.as_ptr() as usize, w)).collect();
        let closed = words.len().saturating_sub(usize::from(!last));
        let (mut clauses, mut from) = (Vec::new(), 0);
        for (i, (_, word)) in words[..closed].iter().enumerate() {
            let count = i + 1 - from;
            let ends =
                word.ends_with(['.', '?', '!', '…']) || (word.ends_with([',', ';', ':']) && count >= COMMA_AFTER) || count >= MAX_WORDS;
            if ends || (last && i + 1 == closed) {
                clauses.push(words[from..=i].iter().map(|(_, w)| *w).collect::<Vec<_>>().join(" "));
                from = i + 1;
            }
        }
        if from > 0 {
            let (start, word) = words[from - 1];
            self.done += start + word.len();
        }
        clauses
    }
}
