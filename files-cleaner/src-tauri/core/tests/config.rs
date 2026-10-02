mod common;

use std::path::{Path, PathBuf};

use common::scan_root;
use files_cleaner_core::{
    load_config, ConfigError, DuplicateHandling, RuleOperation, RuleTarget, TrashDestination,
    EXAMPLE_CONFIG,
};

fn error(text: &str) -> ConfigError {
    load_config(text, Path::new("/Users/me")).unwrap_err()
}

#[test]
fn the_example_config_loads() {
    let config = load_config(EXAMPLE_CONFIG, Path::new("/Users/me")).unwrap();

    assert_eq!(config.trash_destination, TrashDestination::HoldingFolder);
    assert_eq!(config.duplicates, DuplicateHandling::Trash);
    assert_eq!(config.never_touch, [PathBuf::from("/Users/me/Downloads/Keep")]);
    let names: Vec<&str> = config.rules.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["Disk images", "Slides", "Screenshots"]);
    assert_eq!(config.rules[0].operation, RuleOperation::Trash);
    assert_eq!(config.rules[1].extensions, ["pptx", "key"]);
    assert_eq!(config.rules[2].name_glob.as_deref(), Some("Screenshot*"));
}

#[test]
fn settings_default_to_holding_folder_and_trashing_duplicates() {
    let config = load_config("{}", Path::new("/Users/me")).unwrap();

    assert_eq!(config.trash_destination, TrashDestination::HoldingFolder);
    assert_eq!(config.duplicates, DuplicateHandling::Trash);
    assert!(config.never_touch.is_empty());
    assert!(config.rules.is_empty());
}

#[test]
fn reads_every_setting_value() {
    let config = load_config(
        r#"{ "trashDestination": "systemTrash", "duplicates": "followRules" }"#,
        Path::new("/Users/me"),
    )
    .unwrap();
    assert_eq!(config.trash_destination, TrashDestination::SystemTrash);
    assert_eq!(config.duplicates, DuplicateHandling::FollowRules);

    let config = load_config(r#"{ "duplicates": "leave" }"#, Path::new("/Users/me")).unwrap();
    assert_eq!(config.duplicates, DuplicateHandling::Leave);
}

#[test]
fn invalid_json_reports_line_and_column() {
    let err = error("{\n  \"duplicates\": \"trash\"\n  \"rules\": []\n}");

    assert_eq!((err.line, err.column), (Some(3), Some(3)));
    assert!(!err.message.contains("line"), "position is not repeated in the message: {}", err.message);
}

#[test]
fn unknown_setting_values_are_rejected_with_their_line() {
    let err = error("{\n  \"trashDestination\": \"bin\"\n}");
    assert_eq!(err.line, Some(2));
    assert!(err.message.contains("bin"), "{}", err.message);

    let err = error("{\n\n  \"duplicates\": \"delete\"\n}");
    assert_eq!(err.line, Some(3));
}

#[test]
fn misspelled_keys_are_rejected() {
    let err = error(r#"{ "rules": [{ "name": "x", "match": { "extension": ["dmg"] }, "then": { "trash": true } }] }"#);

    assert!(err.message.contains("extension"), "{}", err.message);
}

#[test]
fn extensions_are_lowercased_without_a_leading_dot() {
    let config = load_config(
        r#"{ "rules": [{ "name": "x", "match": { "extensions": [".DMG", "Pkg"] }, "then": { "trash": true } }] }"#,
        Path::new("/Users/me"),
    )
    .unwrap();

    assert_eq!(config.rules[0].extensions, ["dmg", "pkg"]);
}

#[test]
fn a_rule_must_match_on_something() {
    let err = error("{ \"rules\": [\n  { \"name\": \"Everything\", \"match\": {}, \"then\": { \"trash\": true } }\n] }");

    assert_eq!(err.line, Some(2));
    assert!(err.message.contains("extensions or nameGlob"), "{}", err.message);
}

#[test]
fn a_rule_must_either_trash_or_move() {
    for then in [
        r#"{}"#,
        r#"{ "trash": false }"#,
        r#"{ "trash": true, "moveTo": "Old" }"#,
        r#"{ "trash": true, "rename": "{name}" }"#,
    ] {
        let text = format!(r#"{{ "rules": [{{ "name": "x", "match": {{ "nameGlob": "*" }}, "then": {then} }}] }}"#);
        let err = error(&text);
        assert!(err.message.contains("trash") && err.message.contains("moveTo"), "{then}: {}", err.message);
    }
}

#[test]
fn move_targets_resolve_home_absolute_and_relative_paths() {
    let config = load_config(
        r#"{ "rules": [
          { "name": "a", "match": { "extensions": ["a"] }, "then": { "moveTo": "~/Documents/Study" } },
          { "name": "b", "match": { "extensions": ["b"] }, "then": { "moveTo": "/Users/Shared/Inbox" } },
          { "name": "c", "match": { "extensions": ["c"] }, "then": { "moveTo": "Screenshots", "rename": "{modified} {name}" } }
        ] }"#,
        Path::new("/Users/me"),
    )
    .unwrap();

    let targets: Vec<(&RuleTarget, Option<&str>)> = config
        .rules
        .iter()
        .map(|r| match &r.operation {
            RuleOperation::MoveTo { folder, rename } => (folder, rename.as_deref()),
            other => panic!("expected a move, got {other:?}"),
        })
        .collect();
    assert_eq!(
        targets,
        [
            (&RuleTarget::Absolute("/Users/me/Documents/Study".into()), None),
            (&RuleTarget::Absolute("/Users/Shared/Inbox".into()), None),
            (&RuleTarget::RelativeToScanRoot("Screenshots".into()), Some("{modified} {name}")),
        ]
    );
}

#[test]
fn move_targets_inside_protected_paths_are_rejected_with_their_line() {
    let home = scan_root();
    for target in ["/System/Junk", "/usr/local/stuff", "~/Library/Stuff", "~/Downloads/Keep/more", "~/code/node_modules/x"] {
        let text = format!(
            "{{ \"neverTouch\": [\"~/Downloads/Keep\"],\n  \"rules\": [\n    {{ \"name\": \"x\", \"match\": {{ \"nameGlob\": \"*\" }},\n      \"then\": {{ \"moveTo\": \"{target}\" }} }}\n  ] }}"
        );
        let err = load_config(&text, home.path()).unwrap_err();
        assert!(err.message.contains("protected"), "{target}: {}", err.message);
        assert_eq!(err.line, Some(4), "{target}");
    }
}

#[test]
fn never_touch_paths_must_be_absolute_or_in_home() {
    let err = error(r#"{ "neverTouch": ["Downloads/Keep"] }"#);

    assert!(err.message.contains("Downloads/Keep"), "{}", err.message);
}

#[test]
fn a_rule_error_points_at_that_rule_even_when_its_text_appears_earlier() {
    let home = scan_root();
    let text = r#"{
  "neverTouch": ["~/Keep"],
  "rules": [
    { "name": "Fine", "match": { "nameGlob": "a*" }, "then": { "trash": true } },
    { "name": "Fine", "match": {}, "then": { "trash": true } },
    { "name": "Bad", "match": { "nameGlob": "b*" }, "then": { "moveTo": "~/Keep" } }
  ]
}"#;
    let err = load_config(text, home.path()).unwrap_err();
    assert_eq!(err.line, Some(5), "{}", err.message);

    let fixed = text.replace(r#""match": {}"#, r#""match": { "nameGlob": "c*" }"#);
    let err = load_config(&fixed, home.path()).unwrap_err();
    assert_eq!(err.line, Some(6), "{}", err.message);
}
