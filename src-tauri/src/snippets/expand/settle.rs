//! Purpose: Accounts for keys the user typed after a trigger was completed and before its expansion went out. They are already in the app, behind the trigger, so the expansion must erase them too and type them again after itself, or fast typing lands in the wrong place.
//! Contents: settle — the text typed meanwhile, or None when the expansion must not go ahead (Enter was pressed, the caret moved, or Backspace reached into the trigger).

use crate::platform::Typed;

pub fn settle(keys: &[Typed]) -> Option<String> {
    let mut typed = String::new();
    for key in keys {
        match key {
            Typed::Char('\n' | '\r') | Typed::Reset => return None,
            Typed::Char(c) => typed.push(*c),
            Typed::Backspace => {
                typed.pop()?;
            }
        }
    }
    Some(typed)
}

#[cfg(test)]
mod tests {
    use super::settle;
    use crate::platform::Typed::{Backspace, Char, Reset};

    #[test]
    fn keys_typed_meanwhile_are_kept_in_order_and_unsafe_ones_stop_the_expansion() {
        assert_eq!(settle(&[]), Some(String::new()));
        assert_eq!(settle(&[Char('a'), Char('b'), Backspace, Char('c')]), Some("ac".to_string()));
        assert_eq!(settle(&[Backspace]), None); // it ate the trigger itself
        assert_eq!(settle(&[Char('a'), Reset]), None);
        assert_eq!(settle(&[Char('a'), Char('\n')]), None);
    }
}
