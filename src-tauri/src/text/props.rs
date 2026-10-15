//! Purpose: Property tests for the pieces that handle arbitrary text around the text model: cutting a text into units and joining it back, and judging a model answer.
use proptest::prelude::*;

use super::chunk::{join, split, Seg};
use super::reply::{accept, changed_chars};
use crate::actions::Unit;

fn rebuilt(text: &str, unit: Unit) -> String {
    let segs = split(text, unit);
    let same: Vec<String> = segs.iter().filter_map(|s| if let Seg::Unit(u) = s { Some(u.to_string()) } else { None }).collect();
    join(&segs, &same)
}

proptest! {
    #[test]
    fn splitting_then_joining_gives_back_the_exact_text(s in ".{0,300}") {
        prop_assert_eq!(rebuilt(&s, Unit::Sentence), s.clone());
        prop_assert_eq!(rebuilt(&s, Unit::Paragraph), s);
    }

    #[test]
    fn punctuation_and_line_breaks_split_and_join_exactly(s in "[a-zA-Z .!?\"')\n\r\t\u{201d}]{0,200}") {
        prop_assert_eq!(rebuilt(&s, Unit::Sentence), s.clone());
        prop_assert_eq!(rebuilt(&s, Unit::Paragraph), s);
    }

    #[test]
    fn a_model_answer_of_any_shape_is_judged_without_panicking(input in ".{0,200}", answer in ".{0,400}") {
        let _ = accept(&input, &answer, Some(0.5));
        let _ = accept(&input, &answer, None);
    }

    #[test]
    fn edit_distance_is_symmetric_and_bounded(a in ".{0,60}", b in ".{0,60}") {
        let d = changed_chars(&a, &b);
        prop_assert_eq!(d, changed_chars(&b, &a));
        prop_assert!(d <= a.to_lowercase().chars().count().max(b.to_lowercase().chars().count()));
    }
}
