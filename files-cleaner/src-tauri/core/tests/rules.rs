mod common;

use std::fs;
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use common::{config, scan_root, write};
use files_cleaner_core::{build_plan, scan, OperationKind, Plan, Reason};

fn set_modified(path: &Path, secs: u64) {
    let time = UNIX_EPOCH + Duration::from_secs(secs);
    fs::File::options().write(true).open(path).unwrap().set_modified(time).unwrap();
}

/// The Plan for `root` with the config `text`, `~` being `home`.
fn plan_with(root: &Path, home: &Path, text: &str) -> Plan {
    let config = config(text, home);
    build_plan(&scan(root, &config).unwrap(), &config)
}

/// Each Operation as `(kind, source relative path, destination relative path)`.
fn summary(plan: &Plan) -> Vec<(OperationKind, String, String)> {
    plan.operations.iter().map(|op| (op.kind, op.relative_path.join("/"), op.destination.join("/"))).collect()
}

#[test]
fn a_trash_rule_trashes_matching_files_into_the_holding_folder() {
    let dir = scan_root();
    write(dir.path(), "setup.dmg", "installer");
    write(dir.path(), "old/Other.DMG", "another installer");
    write(dir.path(), "notes.txt", "keep me");

    let plan = plan_with(
        dir.path(),
        dir.path(),
        r#"{ "rules": [{ "name": "Disk images", "match": { "extensions": ["dmg"] }, "then": { "trash": true } }] }"#,
    );

    assert_eq!(
        summary(&plan),
        [
            (OperationKind::Trash, "old/Other.DMG".into(), "Files Cleaner Trash/This Session/old/Other.DMG".into()),
            (OperationKind::Trash, "setup.dmg".into(), "Files Cleaner Trash/This Session/setup.dmg".into()),
        ]
    );
    assert!(plan.operations.iter().all(|op| op.reason == Reason::Rule { rule: "Disk images".into() }));
}

/// The Rule each Operation was proposed by, by relative path; "duplicate"
/// for a duplicate.
fn reasons(plan: &Plan) -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = plan
        .operations
        .iter()
        .map(|op| {
            let why = match &op.reason {
                Reason::Duplicate { .. } => "duplicate".to_string(),
                Reason::Rule { rule } => rule.clone(),
            };
            (op.relative_path.join("/"), why)
        })
        .collect();
    found.sort();
    found
}

fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
    expected.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

#[test]
fn the_first_matching_rule_wins_and_both_matchers_must_match() {
    let dir = scan_root();
    write(dir.path(), "Screenshot 1.png", "a");
    write(dir.path(), "screenshot-2.PNG", "b");
    write(dir.path(), "Screenshot notes.txt", "c");
    write(dir.path(), "holiday.png", "d");
    write(dir.path(), "a1b.jpg", "e");
    write(dir.path(), "a12b.jpg", "f");

    let plan = plan_with(
        dir.path(),
        dir.path(),
        r#"{ "rules": [
          { "name": "Screenshots", "match": { "extensions": ["png"], "nameGlob": "screenshot*" }, "then": { "trash": true } },
          { "name": "Images", "match": { "extensions": ["png"] }, "then": { "trash": true } },
          { "name": "Never reached", "match": { "nameGlob": "*.png" }, "then": { "trash": true } },
          { "name": "One character", "match": { "nameGlob": "a?b.*" }, "then": { "trash": true } }
        ] }"#,
    );

    assert_eq!(
        reasons(&plan),
        pairs(&[
            ("Screenshot 1.png", "Screenshots"),
            ("a1b.jpg", "One character"),
            ("holiday.png", "Images"),
            ("screenshot-2.PNG", "Screenshots"),
        ])
    );
}

#[test]
fn rules_match_packages_but_never_reach_inside_them_or_match_folders() {
    let dir = scan_root();
    write(dir.path(), "Old.app/Contents/Info.plist", "app");
    write(dir.path(), "Photos/pic.jpg", "pic");

    let plan = plan_with(
        dir.path(),
        dir.path(),
        r#"{ "rules": [
          { "name": "Apps", "match": { "extensions": ["app"] }, "then": { "trash": true } },
          { "name": "Everything", "match": { "nameGlob": "*" }, "then": { "moveTo": "Sorted" } }
        ] }"#,
    );

    let found = reasons(&plan);
    assert!(found.contains(&("Old.app".into(), "Apps".into())), "{found:?}");
    assert!(found.iter().all(|(path, _)| path != "Photos" && !path.starts_with("Old.app/")), "{found:?}");
}

/// A Scan Root with `keep.pdf` (the Keeper) and an older copy `copy.pdf`.
fn with_duplicates() -> tempfile::TempDir {
    let dir = scan_root();
    set_modified(&write(dir.path(), "keep.pdf", "same"), 2_000);
    set_modified(&write(dir.path(), "copy.pdf", "same"), 1_000);
    dir
}

const PDF_RULE: &str = r#"{ "name": "PDFs", "match": { "extensions": ["pdf"] }, "then": { "trash": true } }"#;

fn duplicates_plan(dir: &Path, handling: Option<&str>) -> Plan {
    let setting = handling.map_or(String::new(), |h| format!(r#""duplicates": "{h}","#));
    plan_with(dir, dir, &format!(r#"{{ {setting} "rules": [{PDF_RULE}] }}"#))
}

#[test]
fn duplicate_handling_trash_is_the_default_and_beats_rules() {
    let dir = with_duplicates();

    for handling in [None, Some("trash")] {
        let plan = duplicates_plan(dir.path(), handling);
        assert_eq!(reasons(&plan), pairs(&[("copy.pdf", "duplicate"), ("keep.pdf", "PDFs")]), "{handling:?}");
    }
}

#[test]
fn duplicate_handling_follow_rules_treats_copies_like_any_other_file() {
    let dir = with_duplicates();

    let plan = duplicates_plan(dir.path(), Some("followRules"));

    assert_eq!(reasons(&plan), pairs(&[("copy.pdf", "PDFs"), ("keep.pdf", "PDFs")]));
}

#[test]
fn duplicate_handling_leave_leaves_copies_out_of_the_plan() {
    let dir = with_duplicates();

    let plan = duplicates_plan(dir.path(), Some("leave"));

    assert_eq!(reasons(&plan), pairs(&[("keep.pdf", "PDFs")]));
}

fn move_rule(target: &str, rename: Option<&str>) -> String {
    let target = serde_json::to_string(target).unwrap();
    let rename = rename.map_or(String::new(), |r| format!(r#", "rename": {}"#, serde_json::to_string(r).unwrap()));
    format!(
        r#"{{ "rules": [{{ "name": "Slides", "match": {{ "extensions": ["pptx"] }}, "then": {{ "moveTo": {target}{rename} }} }}] }}"#
    )
}

#[test]
fn a_relative_target_is_inside_the_scan_root_and_missing_folders_are_created_first() {
    let dir = scan_root();
    write(dir.path(), "talk.pptx", "slides");
    write(dir.path(), "old/lecture.pptx", "more slides");

    let plan = plan_with(dir.path(), dir.path(), &move_rule("Study/Slides", None));

    assert_eq!(
        summary(&plan),
        [
            (OperationKind::CreateFolder, "Study".into(), "Study".into()),
            (OperationKind::CreateFolder, "Study/Slides".into(), "Study/Slides".into()),
            (OperationKind::Move, "old/lecture.pptx".into(), "Study/Slides/lecture.pptx".into()),
            (OperationKind::Move, "talk.pptx".into(), "Study/Slides/talk.pptx".into()),
        ]
    );
    let create = &plan.operations[0];
    assert_eq!(create.reason, Reason::Rule { rule: "Slides".into() });
    assert_eq!(create.destination_path, plan.root.join("Study"));
    assert_eq!(plan.operations[3].destination_path, plan.root.join("Study/Slides/talk.pptx"));
}

#[test]
fn an_existing_target_folder_is_not_created_again_and_files_already_in_it_stay() {
    let dir = scan_root();
    write(dir.path(), "talk.pptx", "slides");
    write(dir.path(), "Slides/already.pptx", "sorted");

    let plan = plan_with(dir.path(), dir.path(), &move_rule("Slides", None));

    assert_eq!(summary(&plan), [(OperationKind::Move, "talk.pptx".into(), "Slides/talk.pptx".into())]);
}

#[test]
fn home_and_absolute_targets_can_be_outside_the_scan_root() {
    let dir = scan_root();
    let home = scan_root();
    let elsewhere = scan_root();
    fs::create_dir(home.path().join("Study")).unwrap();
    write(dir.path(), "talk.pptx", "slides");
    let destinations = |plan: Plan| plan.operations.into_iter().map(|op| (op.kind, op.destination_path)).collect::<Vec<_>>();

    let plan = plan_with(dir.path(), home.path(), &move_rule("~/Study", None));
    assert_eq!(destinations(plan), [(OperationKind::Move, home.path().join("Study/talk.pptx"))]);

    let plan = plan_with(dir.path(), home.path(), &move_rule(elsewhere.path().to_str().unwrap(), None));
    assert_eq!(destinations(plan), [(OperationKind::Move, elsewhere.path().join("talk.pptx"))]);
}

/// The destinations of the Plan's Move Operations, sorted.
fn moved_to(plan: &Plan) -> Vec<String> {
    let mut found: Vec<String> =
        plan.operations.iter().filter(|op| op.kind == OperationKind::Move).map(|op| op.destination.join("/")).collect();
    found.sort();
    found
}

#[test]
fn rename_fills_in_name_ext_and_modified_and_keeps_the_extension() {
    let dir = scan_root();
    // 2001-09-09 01:46:40 UTC, and 1999-12-31 23:59:59 UTC.
    set_modified(&write(dir.path(), "talk.pptx", "a"), 1_000_000_000);
    set_modified(&write(dir.path(), "old.final.pptx", "b"), 946_684_799);

    let plan = plan_with(dir.path(), dir.path(), &move_rule("Slides", Some("{modified} {name} ({ext})")));

    assert_eq!(moved_to(&plan), ["Slides/1999-12-31 old.final (pptx).pptx", "Slides/2001-09-09 talk (pptx).pptx"]);
}

#[test]
fn rename_dates_before_1970_and_in_leap_years_are_right() {
    let dir = scan_root();
    write(dir.path(), "a.pptx", "a");
    write(dir.path(), "b.pptx", "b");
    let config = config(&move_rule("Slides", Some("{modified}")), dir.path());
    let mut result = scan(dir.path(), &config).unwrap();
    // 1969-12-31 23:59:59.999 UTC, and 2024-02-29 12:00 UTC.
    result.entries[0].modified = -1;
    result.entries[1].modified = 1_709_208_000_000;

    assert_eq!(moved_to(&build_plan(&result, &config)), ["Slides/1969-12-31.pptx", "Slides/2024-02-29.pptx"]);
}

#[test]
fn a_name_taken_on_disk_gets_a_clash_suffix_even_in_another_letter_case() {
    let dir = scan_root();
    write(dir.path(), "Slides/NOTES.pptx", "an old note");
    write(dir.path(), "Slides/talk (1).pptx", "an old talk");
    write(dir.path(), "new/notes.pptx", "a new note");
    write(dir.path(), "new/talk.pptx", "a new talk");
    write(dir.path(), "talk.pptx", "another talk");

    let plan = plan_with(dir.path(), dir.path(), &move_rule("Slides", None));

    assert_eq!(moved_to(&plan), ["Slides/notes (1).pptx", "Slides/talk (2).pptx", "Slides/talk.pptx"]);
}

#[test]
fn names_clashing_within_the_plan_get_clash_suffixes() {
    let dir = scan_root();
    write(dir.path(), "a/Notes.pptx", "one");
    write(dir.path(), "b/notes.pptx", "two");
    write(dir.path(), "c/notes (1).pptx", "three");

    let plan = plan_with(dir.path(), dir.path(), &move_rule("Slides", None));

    assert_eq!(moved_to(&plan), ["Slides/Notes.pptx", "Slides/notes (1).pptx", "Slides/notes (2).pptx"]);
}

#[test]
fn a_relative_target_that_is_protected_proposes_nothing() {
    let dir = scan_root();
    write(dir.path(), "talk.pptx", "slides");

    for target in ["Files Cleaner Trash", "project/node_modules", "Keynote.app/Slides"] {
        let plan = plan_with(dir.path(), dir.path(), &move_rule(target, None));
        assert!(plan.operations.is_empty(), "{target}: {:?}", summary(&plan));
    }
}
