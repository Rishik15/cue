//! File: platform/win/app_name/tests.rs
//! Purpose: Protect the picker's program list: every entry carries an executable, and an installed entry without one never hides a running program.
//! Contents: tests of `merge`.

use super::{merge, Running};
use crate::platform::win::installed_apps::Program;

/// An installed entry with no executable is dropped and the running program of the same name is listed with its executable; known ones are listed once, sorted.
#[test]
fn an_unresolved_installed_entry_does_not_hide_a_running_program() {
    let installed = vec![
        Program { name: "Slack".into(), exe: Some("slack".into()) },
        Program { name: "WezTerm".into(), exe: None },
        Program { name: "Calculator".into(), exe: None },
    ];
    let running =
        vec![Running { exe: "wezterm-gui".into(), name: "WezTerm".into() }, Running { exe: "slack".into(), name: "Slack Desktop".into() }];
    let listed = merge(installed, running);
    assert_eq!(listed, [("Slack".to_string(), "slack".to_string()), ("WezTerm".to_string(), "wezterm-gui".to_string())]);
}
