mod common;

use common::{empty_config, real_path, scan_root, write};
use files_cleaner_core::{
    build_suggested_folders_plan, scan, FileChoice, FolderChoice, OperationKind, Reason,
    SuggestedFolderChoice,
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
