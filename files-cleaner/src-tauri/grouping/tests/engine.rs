use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{Days, Local};
use files_cleaner_core::{
    load_config, scan, DateField, DateGrouping, GroupBy, MeaningFolder, NameHow, NameMatch, Rule,
    RuleMatch,
};
use files_cleaner_grouping::{
    analyse, files_to_read, group_by_rules, rule_overlaps, Basis, Card, CardStore, Content, Input,
    Models, Question, Settings, Vector,
};

struct FakeModels;

impl Models for FakeModels {
    fn card_model_id(&self) -> String {
        "fake-card".to_string()
    }

    fn embedding_model_id(&self) -> String {
        "fake-embedding".to_string()
    }

    fn card(&mut self, content: &Content) -> Result<Card, String> {
        let topic = if content.body.contains("lecture") {
            "CS101"
        } else {
            "Banh mi"
        };
        Ok(Card {
            topic: topic.to_string(),
            kind: "notes".to_string(),
            summary: content.body.clone(),
        })
    }

    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vector>, String> {
        Ok(texts
            .iter()
            .map(|t| {
                if t.starts_with("CS101") {
                    vec![1.0, 0.0]
                } else {
                    vec![0.0, 1.0]
                }
            })
            .collect())
    }

    fn place(&mut self, _: &Question) -> Result<Option<usize>, String> {
        Ok(None)
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
    let dir = tempfile::Builder::new()
        .prefix("scan-")
        .tempdir_in(&base)
        .unwrap();
    let root = dir.path();
    let text = "Week one lecture on algorithms, sorting and searching with worked examples.";
    for name in ["T1T2-2706.txt", "week 2.txt", "notes.md"] {
        fs::write(root.join(name), format!("{name}: {text}")).unwrap();
    }
    fs::write(
        root.join("recipe.txt"),
        "A recipe for bread with pickled carrots, pork and fresh herbs.",
    )
    .unwrap();
    let store_dir = tempfile::tempdir().unwrap();
    let config = load_config("{}", root).unwrap();
    let scan = scan(root, &config).unwrap();
    let store = CardStore::open(store_dir.path());

    let input = Input {
        scan: &scan,
        settings: Settings::default(),
        store: &store,
        pdfium: None,
        tessdata: None,
        rules: &[],
    };
    let analysis = analyse(
        &input,
        &mut FakeModels,
        &AtomicBool::new(false),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();

    let grouping = &analysis.grouping;
    assert_eq!(grouping.folders.len(), 1);
    let folder = &grouping.folders[0];
    assert_eq!(folder.name, "CS101");
    let mut names: Vec<String> = folder
        .files
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["T1T2-2706.txt", "notes.md", "week 2.txt"]);
    assert_eq!(folder.existing, None);
    assert_eq!(grouping.ungrouped, [scan.root.join("recipe.txt")]);
}

#[test]
fn a_file_without_an_extension_is_never_a_loose_file() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-tmp");
    fs::create_dir_all(&base).unwrap();
    let dir = tempfile::Builder::new()
        .prefix("scan-")
        .tempdir_in(&base)
        .unwrap();
    let root = dir.path();
    let text = "Week one lecture on algorithms, sorting and searching with worked examples.";
    for name in ["T1T2-2706.txt", "week 2.txt", "README"] {
        fs::write(root.join(name), format!("{name}: {text}")).unwrap();
    }
    let store_dir = tempfile::tempdir().unwrap();
    let config = load_config("{}", root).unwrap();
    let scan = scan(root, &config).unwrap();
    let store = CardStore::open(store_dir.path());

    let input = Input {
        scan: &scan,
        settings: Settings::default(),
        store: &store,
        pdfium: None,
        tessdata: None,
        rules: &[],
    };
    let analysis = analyse(
        &input,
        &mut FakeModels,
        &AtomicBool::new(false),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();

    let readme = scan.root.join("README");
    let grouping = &analysis.grouping;
    assert_eq!(grouping.folders.len(), 1);
    assert!(!grouping.folders[0].files.contains(&readme));
    assert!(!grouping.ungrouped.contains(&readme));
}

/// Every vector points its own way, so only topics can group files.
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
        Ok(Card {
            topic: topic.to_string(),
            kind: "notes".to_string(),
            summary: String::new(),
        })
    }

    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vector>, String> {
        Ok(texts
            .iter()
            .map(|_| {
                self.embedded += 1;
                (0..64)
                    .map(|i| if i == self.embedded { 1.0 } else { 0.0 })
                    .collect()
            })
            .collect())
    }

    fn place(&mut self, question: &Question) -> Result<Option<usize>, String> {
        let words: Vec<String> = question
            .card
            .topic
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let shares = |t: &String| {
            t.split_whitespace()
                .any(|w| words.contains(&w.to_lowercase()))
        };
        Ok(question
            .folders
            .iter()
            .position(|f| f.topics.iter().any(shares)))
    }

    fn device(&self) -> Option<String> {
        None
    }
}

#[test]
fn topics_sharing_a_mark_group_and_a_named_archive_joins_them() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-tmp");
    fs::create_dir_all(&base).unwrap();
    let dir = tempfile::Builder::new()
        .prefix("scan-")
        .tempdir_in(&base)
        .unwrap();
    let root = dir.path();
    fs::write(root.join("writing.txt"), "IELTS Writing | band descriptors").unwrap();
    fs::write(
        root.join("speaking.txt"),
        "IELTS Speaking | band descriptors",
    )
    .unwrap();
    fs::write(
        root.join("Lab5_Report.txt"),
        "IELTS Reading | matching headings",
    )
    .unwrap();
    fs::write(root.join("Lab5_StudentID.zip"), "not a real archive").unwrap();
    fs::write(root.join("recipe.txt"), "Banh mi | recipe").unwrap();
    let store_dir = tempfile::tempdir().unwrap();
    let config = load_config("{}", root).unwrap();
    let scan = scan(root, &config).unwrap();
    let store = CardStore::open(store_dir.path());

    let input = Input {
        scan: &scan,
        settings: Settings::default(),
        store: &store,
        pdfium: None,
        tessdata: None,
        rules: &[],
    };
    let analysis = analyse(
        &input,
        &mut TopicModels { embedded: 0 },
        &AtomicBool::new(false),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();

    let grouping = &analysis.grouping;
    assert_eq!(grouping.folders.len(), 1);
    let folder = &grouping.folders[0];
    assert_eq!(folder.name, "IELTS");
    let mut names: Vec<String> = folder
        .files
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "Lab5_Report.txt",
            "Lab5_StudentID.zip",
            "speaking.txt",
            "writing.txt"
        ]
    );
    assert_eq!(grouping.ungrouped, [scan.root.join("recipe.txt")]);
}

#[test]
fn a_cleared_store_reads_every_file_again() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-tmp");
    fs::create_dir_all(&base).unwrap();
    let dir = tempfile::Builder::new()
        .prefix("scan-")
        .tempdir_in(&base)
        .unwrap();
    let root = dir.path();
    fs::write(root.join("writing.txt"), "IELTS Writing | band descriptors").unwrap();
    fs::write(
        root.join("speaking.txt"),
        "IELTS Speaking | band descriptors",
    )
    .unwrap();
    let store_dir = tempfile::tempdir().unwrap();
    let config = load_config("{}", root).unwrap();
    let scan = scan(root, &config).unwrap();
    let store = CardStore::open(store_dir.path());
    let input = Input {
        scan: &scan,
        settings: Settings::default(),
        store: &store,
        pdfium: None,
        tessdata: None,
        rules: &[],
    };
    let embedded = |models: &mut TopicModels| {
        analyse(
            &input,
            models,
            &AtomicBool::new(false),
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
        std::mem::take(&mut models.embedded)
    };
    let mut models = TopicModels { embedded: 0 };

    assert_eq!(embedded(&mut models), 2);
    assert_eq!(embedded(&mut models), 0);
    store.clear().unwrap();
    assert_eq!(embedded(&mut models), 2);
}

#[test]
fn files_already_read_are_not_counted_as_files_to_read() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-tmp");
    fs::create_dir_all(&base).unwrap();
    let dir = tempfile::Builder::new()
        .prefix("scan-")
        .tempdir_in(&base)
        .unwrap();
    let root = dir.path();
    fs::write(root.join("writing.txt"), "IELTS Writing | band descriptors").unwrap();
    fs::write(
        root.join("speaking.txt"),
        "IELTS Speaking | band descriptors",
    )
    .unwrap();
    fs::write(root.join("photo.jpg"), "never read").unwrap();
    let store_dir = tempfile::tempdir().unwrap();
    let config = load_config("{}", root).unwrap();
    let scan = scan(root, &config).unwrap();
    let store = CardStore::open(store_dir.path());
    let input = Input {
        scan: &scan,
        settings: Settings::default(),
        store: &store,
        pdfium: None,
        tessdata: None,
        rules: &[],
    };
    let mut models = TopicModels { embedded: 0 };
    let never = AtomicBool::new(false);

    assert_eq!(files_to_read(&input, &models, &never).unwrap(), 2);
    analyse(&input, &mut models, &never, &never, |_| {}).unwrap();
    assert_eq!(files_to_read(&input, &models, &never).unwrap(), 0);
}

#[test]
fn a_file_left_out_goes_where_the_model_places_it_and_stays_there_on_regroup() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-tmp");
    fs::create_dir_all(&base).unwrap();
    let dir = tempfile::Builder::new()
        .prefix("scan-")
        .tempdir_in(&base)
        .unwrap();
    let root = dir.path();
    fs::write(root.join("writing.txt"), "IELTS Writing | band descriptors").unwrap();
    fs::write(
        root.join("speaking.txt"),
        "IELTS Speaking | band descriptors",
    )
    .unwrap();
    fs::write(
        root.join("collocations.txt"),
        "English Writing | collocations",
    )
    .unwrap();
    fs::write(root.join("recipe.txt"), "Banh mi | recipe").unwrap();
    let store_dir = tempfile::tempdir().unwrap();
    let config = load_config("{}", root).unwrap();
    let scan = scan(root, &config).unwrap();
    let store = CardStore::open(store_dir.path());

    let input = Input {
        scan: &scan,
        settings: Settings::default(),
        store: &store,
        pdfium: None,
        tessdata: None,
        rules: &[],
    };
    let analysis = analyse(
        &input,
        &mut TopicModels { embedded: 0 },
        &AtomicBool::new(false),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();

    for grouping in [
        analysis.grouping.clone(),
        analysis.regroup(&Settings::default()),
    ] {
        assert_eq!(grouping.folders.len(), 1);
        let folder = &grouping.folders[0];
        assert_eq!(folder.name, "IELTS");
        let mut names: Vec<String> = folder
            .files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["collocations.txt", "speaking.txt", "writing.txt"]);
        assert_eq!(grouping.ungrouped, [scan.root.join("recipe.txt")]);
    }
}

struct StopAfter<'a> {
    stop: &'a AtomicBool,
    left: usize,
}

impl Models for StopAfter<'_> {
    fn card_model_id(&self) -> String {
        FakeModels.card_model_id()
    }

    fn embedding_model_id(&self) -> String {
        FakeModels.embedding_model_id()
    }

    fn card(&mut self, content: &Content) -> Result<Card, String> {
        self.left -= 1;
        if self.left == 0 {
            self.stop.store(true, Ordering::Relaxed);
        }
        FakeModels.card(content)
    }

    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vector>, String> {
        FakeModels.embed(texts)
    }

    fn place(&mut self, question: &Question) -> Result<Option<usize>, String> {
        FakeModels.place(question)
    }

    fn device(&self) -> Option<String> {
        FakeModels.device()
    }
}

#[test]
fn stopping_groups_the_files_read_so_far_and_leaves_the_rest_where_they_are() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-tmp");
    fs::create_dir_all(&base).unwrap();
    let dir = tempfile::Builder::new()
        .prefix("scan-")
        .tempdir_in(&base)
        .unwrap();
    let root = dir.path();
    for name in ["a.txt", "b.txt", "c.txt", "d.txt"] {
        fs::write(root.join(name), format!("{name}: a lecture on sorting")).unwrap();
    }
    let store_dir = tempfile::tempdir().unwrap();
    let config = load_config("{}", root).unwrap();
    let scan = scan(root, &config).unwrap();
    let store = CardStore::open(store_dir.path());
    let input = Input {
        scan: &scan,
        settings: Settings::default(),
        store: &store,
        pdfium: None,
        tessdata: None,
        rules: &[],
    };
    let stop = AtomicBool::new(false);

    let analysis = analyse(
        &input,
        &mut StopAfter {
            stop: &stop,
            left: 2,
        },
        &AtomicBool::new(false),
        &stop,
        |_| {},
    )
    .unwrap();

    let grouping = &analysis.grouping;
    let read: Vec<_> = grouping.summaries.keys().collect();
    assert_eq!(read.len(), 2);
    let mut unread: Vec<_> = scan
        .entries
        .iter()
        .map(|e| &e.path)
        .filter(|p| !read.contains(p))
        .collect();
    unread.sort();
    let left: Vec<_> = grouping
        .ungrouped
        .iter()
        .filter(|p| !read.contains(p))
        .collect();
    assert_eq!(left, unread);
    assert!(grouping
        .folders
        .iter()
        .flat_map(|f| &f.files)
        .all(|p| read.contains(&p)));
}

#[test]
fn clearing_a_damaged_store_starts_it_again() {
    let store_dir = tempfile::tempdir().unwrap();
    fs::write(store_dir.path().join("cards.sqlite"), "not a database").unwrap();

    CardStore::open(store_dir.path()).clear().unwrap();

    assert!(!store_dir.path().join("cards.sqlite").exists());
}

/// As when the LLM finds no topic.
struct TopiclessModels;

impl Models for TopiclessModels {
    fn card_model_id(&self) -> String {
        "topicless-card".to_string()
    }

    fn embedding_model_id(&self) -> String {
        "topicless-embedding".to_string()
    }

    fn card(&mut self, _: &Content) -> Result<Card, String> {
        Ok(Card {
            topic: String::new(),
            kind: "document".to_string(),
            summary: String::new(),
        })
    }

    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vector>, String> {
        Ok(texts.iter().map(|_| vec![1.0, 0.0]).collect())
    }

    fn place(&mut self, _: &Question) -> Result<Option<usize>, String> {
        Ok(None)
    }

    fn device(&self) -> Option<String> {
        None
    }
}

#[test]
fn read_files_sharing_no_topic_stay_where_they_are() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-tmp");
    fs::create_dir_all(&base).unwrap();
    let dir = tempfile::Builder::new()
        .prefix("scan-")
        .tempdir_in(&base)
        .unwrap();
    let root = dir.path();
    for name in ["a.txt", "b.txt", "c.txt"] {
        fs::write(root.join(name), "Some text about nothing in particular.").unwrap();
    }
    let store_dir = tempfile::tempdir().unwrap();
    let config = load_config("{}", root).unwrap();
    let scan = scan(root, &config).unwrap();
    let store = CardStore::open(store_dir.path());

    let input = Input {
        scan: &scan,
        settings: Settings::default(),
        store: &store,
        pdfium: None,
        tessdata: None,
        rules: &[],
    };
    let analysis = analyse(
        &input,
        &mut TopiclessModels,
        &AtomicBool::new(false),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();

    let grouping = &analysis.grouping;
    assert!(grouping.folders.is_empty());
    assert_eq!(grouping.ungrouped.len(), 3);
}

#[test]
fn a_date_rule_sorts_its_files_and_the_rest_group_automatically() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-tmp");
    fs::create_dir_all(&base).unwrap();
    let dir = tempfile::Builder::new()
        .prefix("scan-")
        .tempdir_in(&base)
        .unwrap();
    let root = dir.path();
    let text = "Week one lecture on algorithms, sorting and searching with worked examples.";
    for name in ["T1T2-2706.txt", "week 2.txt", "notes.md"] {
        fs::write(root.join(name), format!("{name}: {text}")).unwrap();
    }
    fs::write(root.join("today.png"), "").unwrap();
    fs::write(root.join("yesterday.png"), "").unwrap();
    let yesterday_noon = Local::now()
        .date_naive()
        .checked_sub_days(Days::new(1))
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap()
        .and_local_timezone(Local)
        .unwrap();
    fs::File::options()
        .write(true)
        .open(root.join("yesterday.png"))
        .unwrap()
        .set_modified(yesterday_noon.into())
        .unwrap();
    let store_dir = tempfile::tempdir().unwrap();
    let config = load_config("{}", root).unwrap();
    let scan = scan(root, &config).unwrap();
    let store = CardStore::open(store_dir.path());
    let rules = [Rule {
        enabled: true,
        words: None,
        matches: RuleMatch {
            extensions: vec!["png".into()],
            ..RuleMatch::default()
        },
        group_by: GroupBy::Date {
            date: DateField::Changed,
            by: DateGrouping::Buckets,
        },
    }];

    let input = Input {
        scan: &scan,
        settings: Settings::default(),
        store: &store,
        pdfium: None,
        tessdata: None,
        rules: &rules,
    };
    let analysis = analyse(
        &input,
        &mut FakeModels,
        &AtomicBool::new(false),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();

    let folder = |name: &str| {
        analysis
            .grouping
            .folders
            .iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("no folder {name}"))
    };
    assert_eq!(folder("Today").files, [scan.root.join("today.png")]);
    assert_eq!(folder("Yesterday").files, [scan.root.join("yesterday.png")]);
    assert_eq!(folder("Today").basis, Basis::Rule);
    assert_eq!(folder("Today").rule.as_ref(), Some(&rules[0]));
    assert_eq!(folder("CS101").files.len(), 3);
    assert_eq!(analysis.grouping.folders.len(), 3);
}

#[test]
fn a_name_rule_finds_its_word_anywhere_in_any_case_and_accent() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-tmp");
    fs::create_dir_all(&base).unwrap();
    let dir = tempfile::Builder::new()
        .prefix("scan-")
        .tempdir_in(&base)
        .unwrap();
    let root = dir.path();
    for name in [
        "Trip to BARAKA.jpg",
        "baraka_2024-final.pdf",
        "Báraka notes.txt",
        "baraka draft.txt",
        "other.txt",
    ] {
        fs::write(root.join(name), "").unwrap();
    }
    let config = load_config("{}", root).unwrap();
    let scan = scan(root, &config).unwrap();
    let rules = [Rule {
        enabled: true,
        words: None,
        matches: RuleMatch {
            name: Some(NameMatch {
                how: NameHow::Contains,
                words: vec!["baraka".into()],
            }),
            except_names: vec!["Draft".into()],
            ..RuleMatch::default()
        },
        group_by: GroupBy::Folder {
            name: "Baraka".into(),
            date: DateField::Changed,
        },
    }];

    let grouping = group_by_rules(&scan, &rules, false);

    let mut files = grouping.folders[0].files.clone();
    files.sort();
    let mut expected: Vec<_> = [
        "Báraka notes.txt",
        "Trip to BARAKA.jpg",
        "baraka_2024-final.pdf",
    ]
    .iter()
    .map(|n| scan.root.join(n))
    .collect();
    expected.sort();
    assert_eq!(grouping.folders.len(), 1);
    assert_eq!(files, expected);
}

struct PlacingInTrip;

impl Models for PlacingInTrip {
    fn card_model_id(&self) -> String {
        FakeModels.card_model_id()
    }

    fn embedding_model_id(&self) -> String {
        FakeModels.embedding_model_id()
    }

    fn card(&mut self, content: &Content) -> Result<Card, String> {
        FakeModels.card(content)
    }

    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vector>, String> {
        FakeModels.embed(texts)
    }

    fn place(&mut self, question: &Question) -> Result<Option<usize>, String> {
        Ok(question.folders.iter().position(|f| f.name == "Trip"))
    }

    fn device(&self) -> Option<String> {
        FakeModels.device()
    }
}

#[test]
fn a_file_left_out_can_be_placed_in_a_rules_folder_without_reading_its_files() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-tmp");
    fs::create_dir_all(&base).unwrap();
    let dir = tempfile::Builder::new()
        .prefix("scan-")
        .tempdir_in(&base)
        .unwrap();
    let root = dir.path();
    fs::write(root.join("trip plan.txt"), "Where we stay in Da Lat.").unwrap();
    fs::write(root.join("recipe.txt"), "A recipe for bread with pork.").unwrap();
    let store_dir = tempfile::tempdir().unwrap();
    let config = load_config("{}", root).unwrap();
    let scan = scan(root, &config).unwrap();
    let store = CardStore::open(store_dir.path());
    let rules = [Rule {
        enabled: true,
        words: None,
        matches: RuleMatch {
            name: Some(NameMatch {
                how: NameHow::Contains,
                words: vec!["trip".into()],
            }),
            ..RuleMatch::default()
        },
        group_by: GroupBy::Folder {
            name: "Trip".into(),
            date: DateField::Changed,
        },
    }];
    let input = Input {
        scan: &scan,
        settings: Settings::default(),
        store: &store,
        pdfium: None,
        tessdata: None,
        rules: &rules,
    };

    let analysis = analyse(
        &input,
        &mut PlacingInTrip,
        &AtomicBool::new(false),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();

    let trip = &analysis.grouping.folders[0];
    assert_eq!(trip.name, "Trip");
    assert_eq!(
        trip.files,
        [
            scan.root.join("trip plan.txt"),
            scan.root.join("recipe.txt")
        ]
    );
    // The Rule's file was never read, so it has no summary.
    assert!(!analysis
        .grouping
        .summaries
        .contains_key(&scan.root.join("trip plan.txt")));
}

#[test]
fn a_rule_matching_files_an_earlier_one_matches_is_found_to_overlap_it() {
    let rule = |matches: RuleMatch, name: &str| Rule {
        enabled: true,
        words: None,
        matches,
        group_by: GroupBy::Folder {
            name: name.into(),
            date: DateField::Changed,
        },
    };
    let rules = [
        rule(
            RuleMatch {
                extensions: vec!["pdf".into()],
                ..RuleMatch::default()
            },
            "Documents",
        ),
        rule(
            RuleMatch {
                extensions: vec!["pdf".into()],
                name: Some(NameMatch {
                    how: NameHow::Contains,
                    words: vec!["invoice".into()],
                }),
                ..RuleMatch::default()
            },
            "Invoices",
        ),
        rule(
            RuleMatch {
                kind: Some("Images".into()),
                ..RuleMatch::default()
            },
            "Pictures",
        ),
    ];

    // "invoice.pdf" fits the first two; no image is a PDF.
    assert_eq!(rule_overlaps(&rules), [None, Some(0), None]);
}

#[test]
fn a_disabled_rules_files_group_automatically() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-tmp");
    fs::create_dir_all(&base).unwrap();
    let dir = tempfile::Builder::new()
        .prefix("scan-")
        .tempdir_in(&base)
        .unwrap();
    let root = dir.path();
    let text = "Week one lecture on algorithms, sorting and searching with worked examples.";
    for name in ["T1T2-2706.txt", "week 2.txt", "notes.txt"] {
        fs::write(root.join(name), format!("{name}: {text}")).unwrap();
    }
    let store_dir = tempfile::tempdir().unwrap();
    let config = load_config("{}", root).unwrap();
    let scan = scan(root, &config).unwrap();
    let store = CardStore::open(store_dir.path());
    let rules = [Rule {
        enabled: false,
        words: None,
        matches: RuleMatch {
            extensions: vec!["txt".into()],
            ..RuleMatch::default()
        },
        group_by: GroupBy::Folder {
            name: "Notes".into(),
            date: DateField::Changed,
        },
    }];
    let input = Input {
        scan: &scan,
        settings: Settings::default(),
        store: &store,
        pdfium: None,
        tessdata: None,
        rules: &rules,
    };

    let analysis = analyse(
        &input,
        &mut FakeModels,
        &AtomicBool::new(false),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();

    let folders = &analysis.grouping.folders;
    assert_eq!(folders.len(), 1);
    assert_eq!(folders[0].name, "CS101");
    assert_eq!(folders[0].files.len(), 3);
    assert_eq!(folders[0].basis, Basis::Topic);
}

struct SubjectModels;

impl Models for SubjectModels {
    fn card_model_id(&self) -> String {
        "fake-card".to_string()
    }

    fn embedding_model_id(&self) -> String {
        "fake-embedding".to_string()
    }

    fn card(&mut self, content: &Content) -> Result<Card, String> {
        Ok(Card {
            topic: String::new(),
            kind: "notes".to_string(),
            summary: content.body.clone(),
        })
    }

    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vector>, String> {
        Ok(texts
            .iter()
            .map(|t| {
                let t = t.to_lowercase();
                if t.contains("thesis") {
                    vec![1.0, 0.0, 0.0]
                } else if t.contains("tax") {
                    vec![0.0, 1.0, 0.0]
                } else {
                    vec![0.0, 0.0, 1.0]
                }
            })
            .collect())
    }

    fn place(&mut self, _: &Question) -> Result<Option<usize>, String> {
        Ok(None)
    }

    fn device(&self) -> Option<String> {
        None
    }
}

#[test]
fn a_meaning_rule_sends_each_file_to_the_most_like_named_folder_and_a_far_one_stays() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-tmp");
    fs::create_dir_all(&base).unwrap();
    let dir = tempfile::Builder::new()
        .prefix("scan-")
        .tempdir_in(&base)
        .unwrap();
    let root = dir.path();
    fs::write(
        root.join("ch1.txt"),
        "Chapter one of my thesis on river ecology.",
    )
    .unwrap();
    fs::write(
        root.join("ch2.txt"),
        "Thesis chapter two: methods and field sites.",
    )
    .unwrap();
    fs::write(
        root.join("return.txt"),
        "Income tax return for 2025 with receipts.",
    )
    .unwrap();
    fs::write(
        root.join("bread.txt"),
        "A recipe for bread with fresh herbs.",
    )
    .unwrap();
    let store_dir = tempfile::tempdir().unwrap();
    let config = load_config("{}", root).unwrap();
    let scan = scan(root, &config).unwrap();
    let store = CardStore::open(store_dir.path());
    let rules = [Rule {
        enabled: true,
        words: None,
        matches: RuleMatch::default(),
        group_by: GroupBy::Meaning {
            folders: vec![
                MeaningFolder {
                    name: "Thesis".into(),
                    description: None,
                },
                MeaningFolder {
                    name: "Money".into(),
                    description: Some("tax papers".into()),
                },
            ],
        },
    }];
    let input = Input {
        scan: &scan,
        settings: Settings::default(),
        store: &store,
        pdfium: None,
        tessdata: None,
        rules: &rules,
    };

    let analysis = analyse(
        &input,
        &mut SubjectModels,
        &AtomicBool::new(false),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();

    let grouping = &analysis.grouping;
    let folders: Vec<(&str, Vec<String>, Basis)> = grouping
        .folders
        .iter()
        .map(|f| {
            let mut names: Vec<String> = f
                .files
                .iter()
                .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
                .collect();
            names.sort();
            (f.name.as_str(), names, f.basis)
        })
        .collect();
    assert_eq!(
        folders,
        [
            (
                "Thesis",
                vec!["ch1.txt".into(), "ch2.txt".into()],
                Basis::Rule
            ),
            ("Money", vec!["return.txt".into()], Basis::Rule),
        ]
    );
    assert_eq!(grouping.ungrouped, [scan.root.join("bread.txt")]);
}
