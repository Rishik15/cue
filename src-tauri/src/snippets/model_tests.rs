//! Purpose: Tests of the snippet model: when a trigger expands and how a folder's programs and switch apply.
use super::*;

fn snippet(triggers: &[&str]) -> Snippet {
    Snippet {
        id: 1,
        name: "n".into(),
        triggers: triggers.iter().map(|t| t.to_string()).collect(),
        body: "b".into(),
        uses: 0,
        last_used: 0,
        folder_id: None,
        mode: Mode::Auto,
        match_case: false,
        enabled: true,
        insert: Insert::Auto,
        ending: String::new(),
        inside_word: false,
        propagate_case: true,
        only_apps: Vec::new(),
        skip_apps: vec!["slack".into()],
    }
}

fn folder() -> Folder {
    Folder { id: 1, name: "Work".into(), enabled: true, only_apps: vec!["code".into()], skip_apps: vec!["chrome".into()] }
}

#[test]
fn auto_expands_at_once_after_a_symbol_and_after_a_space_after_a_letter() {
    assert_eq!(Mode::Auto.when(":sig"), When::Immediate);
    assert_eq!(Mode::Auto.when("sig"), When::Delimiter);
    assert_eq!(Mode::Delimiter.when(":sig"), When::Delimiter);
    assert_eq!(Mode::Auto.when("i"), When::Delimiter);
    assert_eq!(Mode::Auto.when("i "), When::Immediate, "a trigger that ends with a space is complete once the space is typed");
    assert_eq!(Mode::Auto.when(" i "), When::Immediate);
    assert_eq!(Mode::Auto.when(" i"), When::Delimiter);
}

#[test]
fn a_folder_adds_its_programs_and_its_switch() {
    let got = effective(&snippet(&["sig", "sign"]), Some(&folder()));
    assert_eq!(got.iter().map(|e| e.trigger.as_str()).collect::<Vec<_>>(), ["sig", "sign"]);
    assert_eq!(
        (got[0].only_apps.as_slice(), got[0].skip_apps.as_slice()),
        (["code".to_string()].as_slice(), ["slack".to_string(), "chrome".to_string()].as_slice())
    );
    assert!(effective(&snippet(&["sig"]), Some(&Folder { enabled: false, ..folder() })).is_empty());
    let own = Snippet { only_apps: vec!["notepad".into()], ..snippet(&["sig"]) };
    assert_eq!(effective(&own, Some(&folder()))[0].only_apps, ["notepad"]);
}
