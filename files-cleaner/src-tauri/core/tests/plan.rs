mod common;

use std::fs;
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use common::{config, empty_config, scan_root, write};
use files_cleaner_core::{build_plan, scan, OperationKind, Plan};

/// Sets `path`'s modified date to `secs` after the epoch.
fn set_modified(path: &Path, secs: u64) {
    let time = UNIX_EPOCH + Duration::from_secs(secs);
    fs::File::options().write(true).open(path).unwrap().set_modified(time).unwrap();
}

fn plan_for(root: &Path) -> Plan {
    let config = empty_config(root);
    build_plan(&scan(root, &config).unwrap(), &config)
}

/// The relative paths the Plan trashes, sorted, `/`-separated.
fn trashed(plan: &Plan) -> Vec<String> {
    let mut paths: Vec<String> = plan
        .operations
        .iter()
        .filter(|op| op.kind == OperationKind::Trash)
        .map(|op| op.relative_path.join("/"))
        .collect();
    paths.sort();
    paths
}

#[test]
fn trashes_every_copy_but_the_keeper_even_when_renamed() {
    let dir = scan_root();
    let keeper = write(dir.path(), "report.pdf", "same contents");
    let older = write(dir.path(), "old/report (1).pdf", "same contents");
    let oldest = write(dir.path(), "renamed.pdf", "same contents");
    set_modified(&keeper, 3_000);
    set_modified(&older, 2_000);
    set_modified(&oldest, 1_000);
    write(dir.path(), "unique.pdf", "different contents");

    assert_eq!(trashed(&plan_for(dir.path())), ["old/report (1).pdf", "renamed.pdf"]);
}

#[test]
fn files_of_the_same_size_with_different_contents_are_not_duplicates() {
    let dir = scan_root();
    write(dir.path(), "a.txt", "aaaa");
    write(dir.path(), "b.txt", "bbbb");
    // Same size, same start and end, different middle: only the full hash tells.
    let big = |middle: u8| {
        let mut bytes = vec![b'x'; 300_000];
        bytes[150_000] = middle;
        String::from_utf8(bytes).unwrap()
    };
    write(dir.path(), "big1.bin", &big(b'1'));
    write(dir.path(), "big2.bin", &big(b'2'));

    assert!(trashed(&plan_for(dir.path())).is_empty());
}

#[test]
fn empty_files_and_packages_are_never_duplicates() {
    let dir = scan_root();
    write(dir.path(), "empty1.txt", "");
    write(dir.path(), "empty2.txt", "");
    write(dir.path(), "One.app/Contents/x.txt", "same");
    write(dir.path(), "Two.app/Contents/x.txt", "same");

    assert!(trashed(&plan_for(dir.path())).is_empty());
}

#[test]
fn the_keeper_is_the_newest_by_modified_date() {
    let dir = scan_root();
    let newer = write(dir.path(), "z/deep/newer.txt", "copy");
    let older = write(dir.path(), "older.txt", "copy");
    set_modified(&newer, 5_000);
    set_modified(&older, 4_000);

    assert_eq!(trashed(&plan_for(dir.path())), ["older.txt"]);
}

/// The Plan for `root` after `dates` sets each named entry's
/// `(modified, created)`. Created dates can't be set on disk, so the test
/// sets them on the scanned entries instead.
fn plan_with_dates(root: &Path, dates: &[(&str, i64, Option<i64>)]) -> Plan {
    let config = empty_config(root);
    let mut result = scan(root, &config).unwrap();
    for entry in &mut result.entries {
        let relative = entry.relative_path.join("/");
        let (_, modified, created) = dates.iter().find(|(path, ..)| *path == relative).unwrap();
        entry.modified = *modified;
        entry.created = *created;
    }
    build_plan(&result, &config)
}

#[test]
fn equal_modified_dates_keep_the_latest_created() {
    let dir = scan_root();
    write(dir.path(), "a.txt", "copy");
    write(dir.path(), "folder/b.txt", "copy");

    let plan = plan_with_dates(dir.path(), &[("a.txt", 1_000, Some(1)), ("folder/b.txt", 1_000, Some(2))]);

    assert_eq!(trashed(&plan), ["a.txt"]);
}

#[test]
fn equal_dates_keep_the_shortest_path_then_the_first_alphabetically() {
    let dir = scan_root();
    write(dir.path(), "folder/a.txt", "copy");
    write(dir.path(), "c.txt", "copy");
    write(dir.path(), "b.txt", "copy");

    let plan = plan_with_dates(
        dir.path(),
        &[("folder/a.txt", 1_000, Some(5)), ("c.txt", 1_000, Some(5)), ("b.txt", 1_000, Some(5))],
    );

    assert_eq!(trashed(&plan), ["c.txt", "folder/a.txt"]);
}

#[test]
fn a_missing_created_date_skips_to_the_next_tie_break() {
    let dir = scan_root();
    write(dir.path(), "folder/a.txt", "copy");
    write(dir.path(), "b.txt", "copy");

    let plan = plan_with_dates(dir.path(), &[("folder/a.txt", 1_000, Some(9)), ("b.txt", 1_000, None)]);

    assert_eq!(trashed(&plan), ["folder/a.txt"]);
}

#[test]
fn leave_and_follow_rules_propose_nothing_for_duplicates() {
    let dir = scan_root();
    write(dir.path(), "a.txt", "copy");
    write(dir.path(), "b.txt", "copy");

    for handling in ["leave", "followRules"] {
        let config = config(&format!(r#"{{ "duplicates": "{handling}" }}"#), dir.path());
        let plan = build_plan(&scan(dir.path(), &config).unwrap(), &config);
        assert!(trashed(&plan).is_empty(), "{handling}");
    }
}

#[test]
fn operation_ids_are_stable_between_plans() {
    let dir = scan_root();
    write(dir.path(), "a.txt", "copy");
    write(dir.path(), "b.txt", "copy");
    write(dir.path(), "c.txt", "copy");

    let ids = |plan: Plan| plan.operations.into_iter().map(|op| op.id).collect::<Vec<_>>();
    let first = ids(plan_for(dir.path()));
    assert_eq!(first.len(), 2);
    assert_eq!(first, ids(plan_for(dir.path())));
}

#[test]
fn each_trashed_copy_names_where_it_goes_and_its_keeper() {
    let dir = scan_root();
    let keeper = write(dir.path(), "keep/report.pdf", "copy");
    let copy = write(dir.path(), "sub/copy.pdf", "copy");
    let top = write(dir.path(), "top.pdf", "copy");
    set_modified(&keeper, 3_000);
    set_modified(&copy, 2_000);
    set_modified(&top, 1_000);
    let older = write(dir.path(), "a.txt", "other");
    let newer = write(dir.path(), "b.txt", "other");
    set_modified(&older, 1_000);
    set_modified(&newer, 2_000);

    let plan = plan_for(dir.path());
    let summary: Vec<(String, String, Option<String>)> = plan
        .operations
        .iter()
        .map(|op| (op.relative_path.join("/"), op.destination.join("/"), op.keeper.as_ref().map(|k| k.join("/"))))
        .collect();

    let trashed = |path: &str, keeper: &str| {
        (path.to_string(), format!("Files Cleaner Trash/This Session/{path}"), Some(keeper.to_string()))
    };
    assert_eq!(
        summary,
        [trashed("a.txt", "b.txt"), trashed("sub/copy.pdf", "keep/report.pdf"), trashed("top.pdf", "keep/report.pdf")]
    );
}

#[test]
fn one_copy_missing_its_created_date_skips_that_tie_break_for_the_whole_group() {
    let dir = scan_root();
    write(dir.path(), "a/x/long.txt", "copy");
    write(dir.path(), "b/mid.txt", "copy");
    write(dir.path(), "c.txt", "copy");

    // Comparing created dates only where both are known would make
    // long < c < mid < long, and the Keeper would depend on scan order.
    let plan = plan_with_dates(
        dir.path(),
        &[("a/x/long.txt", 1_000, Some(2)), ("b/mid.txt", 1_000, None), ("c.txt", 1_000, Some(1))],
    );

    assert_eq!(trashed(&plan), ["a/x/long.txt", "b/mid.txt"], "the shortest path is kept");
}
