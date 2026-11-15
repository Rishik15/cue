//! File: dictionary/label.rs
//! Purpose: How a dictionary window is named: its label carries the number of the lookup it is for, so a command can learn from the calling window which lookup it speaks for.
//! Contents: PREFIX; label — the label of a lookup's window; id_of — the lookup a label names.

/// What every dictionary window's label starts with (the capability file grants the same pattern).
pub const PREFIX: &str = "dictionary-";

/// The label of the window of lookup `id`. Input: the number. Return: the label.
pub fn label(id: u64) -> String {
    format!("{PREFIX}{id}")
}

/// The number of the lookup a window label names. Input: a label. Return: the number, None for any other window.
pub fn id_of(label: &str) -> Option<u64> {
    let id = label.strip_prefix(PREFIX)?.parse().ok()?;
    (self::label(id) == label).then_some(id)
}

#[cfg(test)]
mod tests {
    use super::{id_of, label};

    /// A label names its lookup and nothing else: other windows and malformed labels name none.
    #[test]
    fn a_label_names_its_lookup_only() {
        assert_eq!(id_of(&label(7)), Some(7));
        assert_eq!(id_of("dictionary"), None);
        assert_eq!(id_of("dictionary-x"), None);
        assert_eq!(id_of("main"), None);
    }

    /// Only the canonical spelling of a number names a lookup: `+3`, `03` and padded or signed forms parse to 3 but are not its label.
    #[test]
    fn other_spellings_of_a_number_name_no_lookup() {
        assert_eq!(id_of("dictionary-3"), Some(3));
        for alias in ["dictionary-+3", "dictionary-03", "dictionary-003", "dictionary-3 ", "dictionary- 3"] {
            assert_eq!(id_of(alias), None, "{alias}");
        }
    }
}
