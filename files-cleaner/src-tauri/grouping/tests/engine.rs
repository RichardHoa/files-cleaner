use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use files_cleaner_core::{load_config, scan};
use files_cleaner_grouping::{analyse, Card, CardStore, Content, Input, Models, Settings, Vector};

/// Cards and vectors fixed by what each file says.
struct FakeModels;

impl Models for FakeModels {
    fn card_model_id(&self) -> String {
        "fake-card".to_string()
    }

    fn embedding_model_id(&self) -> String {
        "fake-embedding".to_string()
    }

    fn card(&mut self, content: &Content) -> Result<Card, String> {
        let topic = if content.body.contains("lecture") { "CS101" } else { "Banh mi" };
        Ok(Card { topic: topic.to_string(), kind: "notes".to_string(), summary: content.body.clone() })
    }

    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vector>, String> {
        Ok(texts.iter().map(|t| if t.starts_with("CS101") { vec![1.0, 0.0] } else { vec![0.0, 1.0] }).collect())
    }

    fn device(&self) -> Option<String> {
        Some("fake".to_string())
    }
}

#[test]
fn three_files_on_one_topic_make_one_suggested_folder() {
    // Not the system temp folder: on macOS it is under the Protected `/private`.
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-tmp");
    fs::create_dir_all(&base).unwrap();
    let dir = tempfile::Builder::new().prefix("scan-").tempdir_in(&base).unwrap();
    let root = dir.path();
    let text = "Week one lecture on algorithms, sorting and searching with worked examples.";
    for name in ["T1T2-2706.txt", "week 2.txt", "notes.md"] {
        fs::write(root.join(name), format!("{name}: {text}")).unwrap();
    }
    fs::write(root.join("recipe.txt"), "A recipe for bread with pickled carrots, pork and fresh herbs.").unwrap();
    let store_dir = tempfile::tempdir().unwrap();
    let config = load_config(r#"{ "rules": [] }"#, root).unwrap();
    let scan = scan(root, &config).unwrap();
    let store = CardStore::open(store_dir.path());

    let input = Input { scan: &scan, settings: Settings::default(), store: &store, pdfium: None };
    let analysis = analyse(&input, &mut FakeModels, &AtomicBool::new(false), |_| {}).unwrap();

    let grouping = &analysis.grouping;
    assert_eq!(grouping.folders.len(), 1);
    let folder = &grouping.folders[0];
    assert_eq!(folder.name, "CS101");
    let mut names: Vec<String> =
        folder.files.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
    names.sort();
    assert_eq!(names, ["T1T2-2706.txt", "notes.md", "week 2.txt"]);
    assert_eq!(folder.existing, None);
    assert_eq!(grouping.ungrouped, [scan.root.join("recipe.txt")]);
}

/// Each file's topic is what it says before `|`; every vector points its own
/// way, so only topics can group files.
struct TopicModels {
    embedded: usize,
}

impl Models for TopicModels {
    fn card_model_id(&self) -> String {
        "topic-card".to_string()
    }

    fn embedding_model_id(&self) -> String {
        "topic-embedding".to_string()
    }

    fn card(&mut self, content: &Content) -> Result<Card, String> {
        let topic = content.body.split('|').next().unwrap_or("").trim();
        Ok(Card { topic: topic.to_string(), kind: "notes".to_string(), summary: String::new() })
    }

    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vector>, String> {
        Ok(texts
            .iter()
            .map(|_| {
                self.embedded += 1;
                (0..64).map(|i| if i == self.embedded { 1.0 } else { 0.0 }).collect()
            })
            .collect())
    }

    fn device(&self) -> Option<String> {
        None
    }
}

#[test]
fn topics_sharing_a_mark_group_and_a_named_archive_joins_them() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-tmp");
    fs::create_dir_all(&base).unwrap();
    let dir = tempfile::Builder::new().prefix("scan-").tempdir_in(&base).unwrap();
    let root = dir.path();
    fs::write(root.join("writing.txt"), "IELTS Writing | band descriptors").unwrap();
    fs::write(root.join("speaking.txt"), "IELTS Speaking | band descriptors").unwrap();
    fs::write(root.join("Lab5_Report.txt"), "IELTS Reading | matching headings").unwrap();
    fs::write(root.join("Lab5_StudentID.zip"), "not a real archive").unwrap();
    fs::write(root.join("recipe.txt"), "Banh mi | recipe").unwrap();
    let store_dir = tempfile::tempdir().unwrap();
    let config = load_config(r#"{ "rules": [] }"#, root).unwrap();
    let scan = scan(root, &config).unwrap();
    let store = CardStore::open(store_dir.path());

    let input = Input { scan: &scan, settings: Settings::default(), store: &store, pdfium: None };
    let analysis = analyse(&input, &mut TopicModels { embedded: 0 }, &AtomicBool::new(false), |_| {}).unwrap();

    let grouping = &analysis.grouping;
    assert_eq!(grouping.folders.len(), 1);
    let folder = &grouping.folders[0];
    assert_eq!(folder.name, "IELTS");
    let mut names: Vec<String> =
        folder.files.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
    names.sort();
    assert_eq!(names, ["Lab5_Report.txt", "Lab5_StudentID.zip", "speaking.txt", "writing.txt"]);
    assert_eq!(grouping.ungrouped, [scan.root.join("recipe.txt")]);
}
