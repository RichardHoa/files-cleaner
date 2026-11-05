mod common;

use common::{empty_config, real_path, scan_root, write};
use files_cleaner_core::{
    build_review_plan, build_suggested_folders_plan, duplicate_groups, scan, DuplicateChoice,
    FileChoice, FolderChoice, OperationKind, Reason, SuggestedFolderChoice,
};

#[test]
fn create_here_plans_the_folder_then_a_move_per_file() {
    let dir = scan_root();
    let root = dir.path();
    let lecture = write(root, "lecture 1.pdf", "one");
    let slides = write(root, "slides.pptx", "two");
    write(root, "unrelated.txt", "three");
    let config = empty_config(root);
    let scan = scan(root, &config).unwrap();
    let root = &scan.root;

    let folder = SuggestedFolderChoice {
        name: "CS101".to_string(),
        choice: FolderChoice::CreateHere,
        files: vec![
            (real_path(&lecture), FileChoice::InFolder),
            (real_path(&slides), FileChoice::InFolder),
        ],
    };
    let plan = build_suggested_folders_plan(&scan, &config, &[folder]).unwrap();

    let ops: Vec<(OperationKind, Vec<String>)> = plan
        .operations
        .iter()
        .map(|op| (op.kind, op.destination.clone()))
        .collect();
    let names = |path: &[&str]| path.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(
        ops,
        [
            (OperationKind::CreateFolder, names(&["CS101"])),
            (OperationKind::Move, names(&["CS101", "lecture 1.pdf"])),
            (OperationKind::Move, names(&["CS101", "slides.pptx"])),
        ]
    );
    assert_eq!(
        plan.operations[1].destination_path,
        root.join("CS101").join("lecture 1.pdf")
    );
    assert!(plan.operations.iter().all(|op| op.reason
        == Reason::SuggestedFolder {
            folder: "CS101".to_string()
        }
        && op.ticked));
}

#[test]
fn one_plan_trashes_copies_moves_folder_files_and_offers_the_folder_both_empty() {
    let dir = scan_root();
    let root = dir.path();
    // Older and deeper than the Kept Copy, so it is the copy to trash.
    let copy = write(root, "Old/report.pdf", "same");
    let notes = write(root, "Old/notes.txt", "notes");
    let kept = write(root, "report.pdf", "same");
    let config = empty_config(root);
    let scan = scan(root, &config).unwrap();

    let groups = duplicate_groups(&scan, config.default_copy_rule, |_| {});
    assert_eq!(groups.len(), 1);
    let found: Vec<_> = groups[0].copies.iter().map(|c| c.path.clone()).collect();
    assert_eq!(
        found,
        [real_path(&kept), real_path(&copy)],
        "the copy kept by default comes first"
    );
    let duplicates = [DuplicateChoice {
        kept: Some(found[0].clone()),
        trash: vec![found[1].clone()],
    }];
    let folder = SuggestedFolderChoice {
        name: "Notes".to_string(),
        choice: FolderChoice::CreateHere,
        files: vec![(real_path(&notes), FileChoice::InFolder)],
    };
    let plan = build_review_plan(&scan, &config, &duplicates, &[folder]).unwrap();

    let ops: Vec<(OperationKind, Vec<String>, &Reason, bool)> = plan
        .operations
        .iter()
        .map(|op| (op.kind, op.relative_path.clone(), &op.reason, op.ticked))
        .collect();
    let names = |path: &[&str]| path.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let notes_folder = Reason::SuggestedFolder {
        folder: "Notes".to_string(),
    };
    let duplicate = Reason::Duplicate {
        kept: Some(names(&["report.pdf"])),
    };
    assert_eq!(
        ops,
        [
            (
                OperationKind::CreateFolder,
                names(&["Notes"]),
                &notes_folder,
                true
            ),
            (
                OperationKind::Move,
                names(&["Old", "notes.txt"]),
                &notes_folder,
                true
            ),
            (
                OperationKind::Trash,
                names(&["Old", "report.pdf"]),
                &duplicate,
                true
            ),
            (
                OperationKind::Trash,
                names(&["Old"]),
                &Reason::EmptiedFolder,
                false
            ),
        ]
    );
}

#[test]
fn a_group_with_no_kept_copy_trashes_every_copy_and_its_reason_names_none() {
    let dir = scan_root();
    let root = dir.path();
    let first = write(root, "report.pdf", "same");
    let second = write(root, "Old/report.pdf", "same");
    let config = empty_config(root);
    let scan = scan(root, &config).unwrap();

    let duplicates = [DuplicateChoice {
        kept: None,
        trash: vec![real_path(&first), real_path(&second)],
    }];
    let plan = build_review_plan(&scan, &config, &duplicates, &[]).unwrap();

    let trashed: Vec<_> = plan
        .operations
        .iter()
        .filter(|op| op.reason == Reason::Duplicate { kept: None })
        .map(|op| (op.kind, op.ticked))
        .collect();
    assert_eq!(
        trashed,
        [(OperationKind::Trash, true), (OperationKind::Trash, true)]
    );
}
