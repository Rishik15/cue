//! Purpose: Keep the three lists of commands in step: `COMMANDS` in build.rs (what the app manifest knows), `generate_handler!` in lib.rs (what can be called) and the `allow-*` grants in capabilities/.
//! A command missing from `COMMANDS` is callable by every window; one that is not in the handler or granted to no window is dead.

use std::collections::BTreeSet;
use std::path::Path;

fn read(path: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The text between the first `[` after `marker` and the next `]`.
fn list_after<'a>(source: &'a str, marker: &str) -> &'a str {
    let at = source.find(marker).unwrap_or_else(|| panic!("no {marker}")) + marker.len();
    let rest = &source[at..];
    let open = rest.find('[').expect("list start") + 1;
    &rest[open..open + rest[open..].find(']').expect("list end")]
}

fn manifest_commands() -> BTreeSet<String> {
    let build = read("build.rs");
    list_after(&build, "const COMMANDS: &[&str] =").split('"').skip(1).step_by(2).map(String::from).collect()
}

fn handler_commands() -> BTreeSet<String> {
    let lib = read("src/lib.rs");
    list_after(&lib, "generate_handler!")
        .split(',')
        .map(|item| item.trim().rsplit("::").next().unwrap_or_default().to_string())
        .filter(|name| !name.is_empty())
        .collect()
}

/// Every `"allow-some-command"` in any capability file, as `some_command`.
fn granted_commands() -> BTreeSet<String> {
    let mut grants = BTreeSet::new();
    for entry in std::fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities")).expect("capabilities folder") {
        let text = std::fs::read_to_string(entry.expect("entry").path()).expect("capability file");
        grants.extend(text.split('"').filter_map(|s| s.strip_prefix("allow-")).map(|name| name.replace('-', "_")));
    }
    grants
}

#[test]
fn manifest_handler_and_grants_name_the_same_commands() {
    let (manifest, handler, grants) = (manifest_commands(), handler_commands(), granted_commands());
    assert!(manifest.len() > 20, "the lists were not parsed");
    assert_eq!(manifest, handler, "build.rs COMMANDS differs from generate_handler! (left: COMMANDS)");
    assert_eq!(manifest, grants, "capabilities/ grants differ from COMMANDS (left: COMMANDS)");
}
