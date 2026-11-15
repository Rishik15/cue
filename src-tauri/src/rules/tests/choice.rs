//! File: rules/tests/choice.rs
//! Purpose: Protect the saved-action choice: an action offered in a palette is one fixed rule of one saved list, so editing, reordering or replacing the list while the palette is open refuses the old row id instead of running, noting or routing to another rule.
//! Contents: one test over `publish`, `matching` and `choose`.

use super::helpers::rule;
use crate::actions::Action;
use crate::rules::engine::compile;
use crate::rules::model::{Output, Trigger};
use crate::rules::{choose, matching, publish};

/// A saved action for the selection shortcut with its output.
fn saved(name: &str, action: Action, output: Output) -> crate::rules::engine::CompiledRule {
    let mut r = rule(name, &[action]);
    r.trigger = Trigger::Selection;
    r.output = output;
    compile(r).expect("compiles")
}

/// An offer keeps meaning its own rule and output after the list changes, and its row id is refused once the list moved on, even when a rule with the same name sits at the same place.
#[test]
fn an_offered_saved_action_is_one_fixed_rule_and_a_stale_id_is_refused() {
    publish(vec![saved("Shout", Action::UpperCase, Output::Copy), saved("Whisper", Action::LowerCase, Output::Insert)]);
    let offered = matching(Trigger::Selection, "Hello");
    assert_eq!(offered.iter().map(|c| c.name().to_string()).collect::<Vec<_>>(), ["Shout", "Whisper"]);
    let shout = offered.into_iter().next().expect("offered");
    let tail = shout.tail();
    assert!(choose(&tail).is_some_and(|c| c.output() == Output::Copy));

    publish(vec![saved("Shout", Action::UpperCase, Output::Insert), saved("Whisper", Action::LowerCase, Output::Insert)]);
    assert!(choose(&tail).is_none(), "the same name at the same place on a newer list is refused");
    assert_eq!(shout.output(), Output::Copy, "the offer still routes as it was offered");
    assert_eq!(shout.run("Hello", "notepad").text, "HELLO");

    publish(vec![saved("Whisper", Action::LowerCase, Output::Insert)]);
    assert!(choose(&tail).is_none());
    assert!(choose("not a tail").is_none());
}
