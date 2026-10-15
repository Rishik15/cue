//! Purpose: Gives an expansion the case the trigger was typed in (`:Sig` types a capital first letter, `:SIG` capitals).
//! Contents: follow_case — the text in that case.

/// The expansion in the case the trigger was typed in: a typed capital first letter capitalizes the text, a trigger typed in capitals gives capitals.
pub fn follow_case(text: &str, typed: &str) -> String {
    let letters: Vec<char> = typed.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() >= 2 && letters.iter().all(|c| c.is_uppercase()) {
        return text.to_uppercase();
    }
    if !letters.first().is_some_and(|c| c.is_uppercase()) {
        return text.to_string();
    }
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if first.is_alphabetic() => first.to_uppercase().chain(chars).collect(),
        _ => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_follows_the_case_of_the_trigger() {
        assert_eq!(follow_case("best regards", ":Sig"), "Best regards");
        assert_eq!(follow_case("best regards", ":SIG"), "BEST REGARDS");
        assert_eq!(follow_case("best regards", ":sig"), "best regards");
        assert_eq!(follow_case("1st item", ":Sig"), "1st item", "a text that starts with a digit is left alone");
    }
}
