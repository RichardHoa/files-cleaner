mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::scan_root;
use files_cleaner_core::{
    load_config, reset_config, save_config, ConfigError, DefaultCopyRule, SettingChange,
    EXAMPLE_CONFIG,
};

fn error(text: &str) -> ConfigError {
    load_config(text, Path::new("/Users/me")).unwrap_err()
}

#[test]
fn the_example_config_loads() {
    let config = load_config(EXAMPLE_CONFIG, Path::new("/Users/me")).unwrap();

    assert_eq!(
        config.never_touch,
        [PathBuf::from("/Users/me/Downloads/Keep")]
    );
    assert!(!EXAMPLE_CONFIG.contains("\"rules\"") && !EXAMPLE_CONFIG.contains("\"duplicates\""));
}

#[test]
fn settings_default_to_nothing_set() {
    let config = load_config("{}", Path::new("/Users/me")).unwrap();

    assert!(config.never_touch.is_empty());
}

#[test]
fn the_old_trash_destination_setting_is_ignored() {
    for value in ["holdingFolder", "systemTrash", "bin"] {
        let text = format!(r#"{{ "trashDestination": "{value}" }}"#);
        load_config(&text, Path::new("/Users/me")).unwrap();
    }
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
fn invalid_json_reports_line_and_column() {
    let err = error("{\n  \"neverTouch\": []\n  \"notifications\": true\n}");

    assert_eq!((err.line, err.column), (Some(3), Some(3)));
    assert!(
        !err.message.contains("line"),
        "position is not repeated in the message: {}",
        err.message
    );
}

#[test]
fn unknown_setting_values_are_rejected_with_their_line() {
    let err = error("{\n  \"defaultCopyRule\": \"bin\"\n}");
    assert_eq!(err.line, Some(2));
    assert!(err.message.contains("bin"), "{}", err.message);
}

#[test]
fn misspelled_keys_are_rejected() {
    let err = error(r#"{ "grouping": { "folderTreshold": 0.5 } }"#);

    assert!(err.message.contains("folderTreshold"), "{}", err.message);
}

#[test]
fn never_touch_paths_must_be_absolute_or_in_home() {
    let err = error(r#"{ "neverTouch": ["Downloads/Keep"] }"#);

    assert!(err.message.contains("Downloads/Keep"), "{}", err.message);
}

#[test]
fn a_saved_config_with_old_rules_loads_and_saves() {
    let folder = scan_root();
    let path = folder.path().join("config.json");
    let rules = r#"[{ "name": "Installers", "match": { "extensions": ["dmg"] }, "then": { "trash": true } }]"#;
    fs::write(
        &path,
        format!(r#"{{ "neverTouch": ["~/Keep"], "rules": {rules} }}"#),
    )
    .unwrap();
    let home = Path::new("/Users/me");

    save_config(&path, &SettingChange::FolderThreshold(Some(0.73)), home).unwrap();

    let text = fs::read_to_string(&path).unwrap();
    let config = load_config(&text, home).unwrap();
    assert_eq!(config.grouping.folder_threshold, 0.73);
    assert_eq!(config.never_touch, [PathBuf::from("/Users/me/Keep")]);
    assert!(text.contains("\"~/Keep\""), "{text}");
}

#[test]
fn an_out_of_range_grouping_value_is_rejected_with_its_line() {
    let err = error("{\n  \"grouping\": {\n    \"smallestFolder\": 1\n  }\n}");

    assert_eq!((err.line, err.column), (Some(3), Some(5)));
    assert!(err.message.contains("2 and 10"), "{}", err.message);
}

#[test]
fn refusing_notifications_is_saved_in_the_config() {
    let root = scan_root();
    let path = root.path().join("config.json");
    fs::write(&path, EXAMPLE_CONFIG).unwrap();
    let home = Path::new("/Users/me");
    assert_eq!(
        load_config(EXAMPLE_CONFIG, home).unwrap().notifications,
        None
    );

    save_config(&path, &SettingChange::Notifications(false), home).unwrap();

    let config = load_config(&fs::read_to_string(&path).unwrap(), home).unwrap();
    assert_eq!(config.notifications, Some(false));
}

#[test]
fn the_default_copy_rule_starts_as_newest_and_a_saved_one_loads_again() {
    let root = scan_root();
    let path = root.path().join("config.json");
    fs::write(&path, EXAMPLE_CONFIG).unwrap();
    let home = Path::new("/Users/me");
    assert_eq!(
        load_config(EXAMPLE_CONFIG, home).unwrap().default_copy_rule,
        DefaultCopyRule::Newest
    );

    save_config(
        &path,
        &SettingChange::DefaultCopyRule(DefaultCopyRule::Oldest),
        home,
    )
    .unwrap();

    let config = load_config(&fs::read_to_string(&path).unwrap(), home).unwrap();
    assert_eq!(config.default_copy_rule, DefaultCopyRule::Oldest);
}

#[test]
fn an_unreadable_config_resets_to_the_example() {
    let folder = scan_root();
    let path = folder.path().join("config.json");
    fs::write(&path, "{ not json").unwrap();

    reset_config(&path, Path::new("/Users/me")).unwrap();

    assert_eq!(fs::read_to_string(&path).unwrap(), EXAMPLE_CONFIG);
}
