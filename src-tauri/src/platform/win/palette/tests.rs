//! File: platform/win/palette/tests.rs
//! Purpose: Protect how a saved theme family picks the native windows' colours: only an exact id names a family.
//! Contents: tests of `resolve`.

use super::resolve;
use crate::platform::Theme;

/// Builds a dark theme for a family. Input: the saved family value. Returns the theme.
fn dark(family: &str) -> Theme {
    Theme { family: family.into(), mode: "Dark".into() }
}

/// An exact id picks its family; a display name is not an id and reads as the default family.
#[test]
fn only_an_exact_id_names_a_family() {
    let default = resolve(&dark("default"), false);
    let tokyo = resolve(&dark("tokyo-night"), false);
    assert_ne!(tokyo, default);
    assert_eq!(resolve(&dark("Tokyo Night"), false), default);
}
