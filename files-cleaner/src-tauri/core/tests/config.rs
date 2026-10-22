mod common;

use std::path::{Path, PathBuf};

use common::scan_root;
use files_cleaner_core::{
    load_config, ConfigError, RuleOperation, RuleTarget, TrashDestination, EXAMPLE_CONFIG,
};

fn error(text: &str) -> ConfigError {
    load_config(text, Path::new("/Users/me")).unwrap_err()
}

#[test]
fn the_example_config_loads() {
    let config = load_config(EXAMPLE_CONFIG, Path::new("/Users/me")).unwrap();

    assert_eq!(config.trash_destination, TrashDestination::HoldingFolder);
    assert_eq!(
        config.never_touch,
        [PathBuf::from("/Users/me/Downloads/Keep")]
    );
    assert!(config.rules.is_empty());
    assert!(!EXAMPLE_CONFIG.contains("\"rules\"") && !EXAMPLE_CONFIG.contains("\"duplicates\""));
}

#[test]
fn settings_default_to_the_holding_folder() {
    let config = load_config("{}", Path::new("/Users/me")).unwrap();

    assert_eq!(config.trash_destination, TrashDestination::HoldingFolder);
    assert!(config.never_touch.is_empty());
    assert!(config.rules.is_empty());
}

#[test]
fn reads_every_setting_value() {
    let config = load_config(
        r#"{ "trashDestination": "holdingFolder" }"#,
        Path::new("/Users/me"),
    )
    .unwrap();
    assert_eq!(config.trash_destination, TrashDestination::HoldingFolder);
}

#[test]
fn the_old_duplicate_handling_setting_is_ignored() {
    for value in [
        r#""trash""#,
        r#""followRules""#,
        r#""leave""#,
        r#""delete""#,
        "3",
    ] {
        let text = format!(r#"{{ "duplicates": {value}, "neverTouch": ["~/Keep"] }}"#);
        let config = load_config(&text, Path::new("/Users/me")).unwrap();
        assert_eq!(
            config.never_touch,
            [PathBuf::from("/Users/me/Keep")],
            "{value}"
        );
    }
}

#[test]
fn the_system_trash_is_rejected_as_not_supported_yet_with_its_line() {
    let err = error("{\n  \"trashDestination\": \"systemTrash\"\n}");

    assert!(err.message.contains("not supported yet"), "{}", err.message);
    assert_eq!(err.line, Some(2));
}

#[test]
fn invalid_json_reports_line_and_column() {
    let err = error("{\n  \"trashDestination\": \"holdingFolder\"\n  \"rules\": []\n}");

    assert_eq!((err.line, err.column), (Some(3), Some(3)));
    assert!(
        !err.message.contains("line"),
        "position is not repeated in the message: {}",
        err.message
    );
}

#[test]
fn unknown_setting_values_are_rejected_with_their_line() {
    let err = error("{\n  \"trashDestination\": \"bin\"\n}");
    assert_eq!(err.line, Some(2));
    assert!(err.message.contains("bin"), "{}", err.message);
}

#[test]
fn misspelled_keys_are_rejected() {
    let err = error(
        r#"{ "rules": [{ "name": "x", "match": { "extension": ["dmg"] }, "then": { "trash": true } }] }"#,
    );

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
    assert!(
        err.message.contains("extensions or nameGlob"),
        "{}",
        err.message
    );
}

#[test]
fn a_rule_must_either_trash_or_move() {
    for then in [
        r#"{}"#,
        r#"{ "trash": false }"#,
        r#"{ "trash": true, "moveTo": "Old" }"#,
        r#"{ "trash": true, "rename": "{name}" }"#,
    ] {
        let text = format!(
            r#"{{ "rules": [{{ "name": "x", "match": {{ "nameGlob": "*" }}, "then": {then} }}] }}"#
        );
        let err = error(&text);
        assert!(
            err.message.contains("trash") && err.message.contains("moveTo"),
            "{then}: {}",
            err.message
        );
    }
}

#[test]
fn move_targets_resolve_home_absolute_and_relative_paths() {
    #[cfg(unix)]
    let absolute = "/Users/Shared/Inbox";
    #[cfg(windows)]
    let absolute = "C:/Users/Shared/Inbox";
    let text = format!(
        r#"{{ "rules": [
          {{ "name": "a", "match": {{ "extensions": ["a"] }}, "then": {{ "moveTo": "~/Documents/Study" }} }},
          {{ "name": "b", "match": {{ "extensions": ["b"] }}, "then": {{ "moveTo": "{absolute}" }} }},
          {{ "name": "c", "match": {{ "extensions": ["c"] }}, "then": {{ "moveTo": "Screenshots", "rename": "{{modified}} {{name}}" }} }}
        ] }}"#
    );
    let config = load_config(&text, Path::new("/Users/me")).unwrap();

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
            (
                &RuleTarget::Absolute("/Users/me/Documents/Study".into()),
                None
            ),
            (&RuleTarget::Absolute(absolute.into()), None),
            (
                &RuleTarget::RelativeToScanRoot("Screenshots".into()),
                Some("{modified} {name}")
            ),
        ]
    );
}

#[test]
fn move_targets_inside_protected_paths_are_rejected_with_their_line() {
    let home = scan_root();
    #[cfg(target_os = "macos")]
    let system = ["/System/Junk", "/usr/local/stuff", "~/Library/Stuff"];
    #[cfg(target_os = "linux")]
    let system = ["/usr/local/stuff", "/etc/stuff", "~/.config/stuff"];
    #[cfg(target_os = "windows")]
    let system = [
        r"C:\\Windows\\Junk",
        r"C:\\Program Files\\Stuff",
        r"~\\AppData\\Stuff",
    ];
    for target in system
        .into_iter()
        .chain(["~/Downloads/Keep/more", "~/code/node_modules/x"])
    {
        let text = format!(
            "{{ \"neverTouch\": [\"~/Downloads/Keep\"],\n  \"rules\": [\n    {{ \"name\": \"x\", \"match\": {{ \"nameGlob\": \"*\" }},\n      \"then\": {{ \"moveTo\": \"{target}\" }} }}\n  ] }}"
        );
        let err = load_config(&text, home.path()).unwrap_err();
        assert!(
            err.message.contains("protected"),
            "{target}: {}",
            err.message
        );
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

#[test]
fn a_rename_must_be_a_plain_name_using_known_placeholders() {
    for (rename, problem) in [
        ("{date} {name}", "{date}"),
        ("sub/{name}", "/"),
        ("sub\\\\{name}", "\\"),
        ("", "empty"),
        ("  ", "empty"),
    ] {
        let text = format!(
            "{{ \"rules\": [\n  {{ \"name\": \"x\", \"match\": {{ \"nameGlob\": \"*\" }},\n    \"then\": {{ \"moveTo\": \"Old\", \"rename\": \"{rename}\" }} }}\n] }}"
        );
        let err = error(&text);
        assert!(err.message.contains(problem), "{rename:?}: {}", err.message);
        assert_eq!(err.line, Some(3), "{rename:?}");
    }
}
