//! THROWAWAY. Answers issue #5: can the System Trash keep
//! the Undo promise on this OS? See README.md for how to run it.
//!
//! It only trashes and removes items in a new folder it creates itself, and
//! never empties the Trash. Outside that folder it leaves the Trash folders an
//! OS makes on first use and, on Windows, may remove a test item's own record
//! from the Recycle Bin. The backends are drafts for ticket 02, not finished code.

use std::env;
use std::fmt;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::{self, ExitCode};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use linux as backend;
#[cfg(target_os = "macos")]
use macos as backend;
#[cfg(target_os = "windows")]
use windows as backend;

#[derive(Clone, Copy, PartialEq)]
pub enum Verdict {
    Pass,
    Fail,
    NotAnswered,
}

/// Why an item didn't go to the System Trash. Only `NoTrash` answers Q4: any
/// other failure says nothing about whether the disk has a Trash.
pub enum Refusal {
    NoTrash(String),
    Other(String),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Refusal::NoTrash(reason) | Refusal::Other(reason) => f.write_str(reason),
        }
    }
}

impl From<String> for Refusal {
    fn from(reason: String) -> Refusal {
        Refusal::Other(reason)
    }
}

impl From<&str> for Refusal {
    fn from(reason: &str) -> Refusal {
        Refusal::Other(reason.to_string())
    }
}

/// What `backend::preflight` can tell without touching the item.
pub struct Preflight {
    pub can_trash: bool,
    pub detail: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Question {
    Landing,
    MoveBack,
    Access,
    Clean,
    CleanAfterRemoval,
    SameName,
    NoTrash,
}

/// Each question in the summary, and what to print if nothing answered it.
const QUESTIONS: &[(Question, &str, &str)] = &[
    (Question::Landing, "Q1 OS reports where the item landed", "NOT RUN"),
    (Question::MoveBack, "Q2 File, folder and Package move back out", "NOT RUN"),
    (Question::Access, "Q2 Moving back works without Full Disk Access", "n/a on this OS"),
    (Question::Clean, "Q2 Clean restore, with no record left behind", "n/a on this OS"),
    (Question::CleanAfterRemoval, "Q2 Clean once the record is removed by hand", "not needed"),
    (Question::SameName, "Q3 Two items with the same name both survive", "NOT RUN"),
    (Question::NoTrash, "Q4 No-Trash volume: detected, item left in place", "NOT RUN: pass --no-trash-volume"),
];

#[derive(Default)]
struct Report {
    results: Vec<(Question, Verdict)>,
}

impl Report {
    fn check(&mut self, question: Question, verdict: Verdict, text: &str) {
        let tag = match verdict {
            Verdict::Pass => "PASS",
            Verdict::Fail => "FAIL",
            Verdict::NotAnswered => "????",
        };
        println!("[{tag}] {text}");
        self.results.push((question, verdict));
    }

    /// Prints the block to paste into the ticket. True if nothing failed.
    fn summary(&self, os_note: &str) -> bool {
        println!("\n== Summary: {} ({os_note}) ==", env::consts::OS);
        for (question, label, missing) in QUESTIONS {
            let verdicts: Vec<Verdict> = self
                .results
                .iter()
                .filter(|(q, _)| q == question)
                .map(|(_, v)| *v)
                .collect();
            let result = if verdicts.is_empty() {
                missing
            } else if verdicts.contains(&Verdict::Fail) {
                "FAIL"
            } else if verdicts.contains(&Verdict::NotAnswered) {
                "NOT ANSWERED: see the ???? lines above"
            } else {
                "PASS"
            };
            println!("{label}: {result}");
        }
        !self.results.iter().any(|(_, v)| *v == Verdict::Fail)
    }
}

/// A test item, and the files that should be inside it. A plain file is the
/// one entry with an empty relative path.
struct Item {
    label: &'static str,
    question: Question,
    path: PathBuf,
    files: Vec<(&'static str, String)>,
    landing: Option<PathBuf>,
    restored: bool,
    restore_error: Option<String>,
}

impl Item {
    fn new(label: &'static str, question: Question, path: PathBuf, files: &[&'static str]) -> Item {
        let files = files
            .iter()
            .map(|rel| (*rel, format!("{label} / {rel}\n")))
            .collect();
        Item { label, question, path, files, landing: None, restored: false, restore_error: None }
    }

    fn file_at(root: &Path, rel: &str) -> PathBuf {
        if rel.is_empty() {
            root.to_path_buf()
        } else {
            root.join(rel)
        }
    }

    fn create(&self) -> io::Result<()> {
        for (rel, contents) in &self.files {
            let file = Item::file_at(&self.path, rel);
            fs::create_dir_all(file.parent().expect("test items are inside the work folder"))?;
            fs::write(file, contents)?;
        }
        Ok(())
    }

    fn intact_at(&self, root: &Path) -> bool {
        self.files.iter().all(|(rel, contents)| {
            fs::read_to_string(Item::file_at(root, rel)).is_ok_and(|read| read == *contents)
        })
    }
}

fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

/// The check `core`'s executor makes before an Undo move, then a same-disk
/// rename that never copies. Something appearing at `to` between the check and
/// the rename would be replaced; this test doesn't exercise that.
pub fn move_without_overwriting(from: &Path, to: &Path) -> Result<(), String> {
    if exists(to) {
        return Err(format!("{} already exists", to.display()));
    }
    fs::rename(from, to).map_err(|e| format!("couldn't move it to {}: {e}", to.display()))
}

fn trash_items(items: &mut [Item], report: &mut Report) {
    for item in items.iter_mut() {
        println!("\n-- {}: {}", item.label, item.path.display());
        match backend::trash(&item.path, false) {
            Err(error) => report.check(
                Question::Landing,
                Verdict::Fail,
                &format!("{}: didn't go to the {}: {error}", item.label, backend::PLACE),
            ),
            Ok(landing) => {
                println!("       landed at {}", landing.display());
                // Only a definite "not there" counts against it: on macOS,
                // looking inside the Trash may need Full Disk Access.
                let looked = fs::symlink_metadata(&landing);
                let (verdict, note) = match &looked {
                    _ if exists(&item.path) => (Verdict::Fail, "but it's still at its old path".to_string()),
                    Err(e) if e.kind() == ErrorKind::NotFound => {
                        (Verdict::Fail, "but nothing is there".to_string())
                    }
                    Err(e) => (Verdict::Pass, format!("the OS won't let this program look there: {e}")),
                    Ok(_) => (Verdict::Pass, "and it's there".to_string()),
                };
                report.check(
                    Question::Landing,
                    verdict,
                    &format!("{}: OS reported the landing, {note}", item.label),
                );
                item.landing = Some(landing);
            }
        }
    }
}

fn restore_items(items: &mut [Item], report: &mut Report) {
    println!();
    for item in items.iter_mut() {
        let Some(landing) = &item.landing else {
            report.check(
                item.question,
                Verdict::NotAnswered,
                &format!("{}: never reached the {}, so moving it back wasn't tested", item.label, backend::PLACE),
            );
            continue;
        };
        match backend::restore(landing, &item.path) {
            Err(error) => {
                report.check(
                    item.question,
                    Verdict::Fail,
                    &format!("{}: couldn't be moved back: {error}", item.label),
                );
                item.restore_error = Some(error);
            }
            Ok(()) => {
                let intact = item.intact_at(&item.path);
                let verdict = if intact && !exists(landing) { Verdict::Pass } else { Verdict::Fail };
                item.restored = intact;
                report.check(
                    item.question,
                    verdict,
                    &format!(
                        "{}: moved back; contents intact: {intact}, gone from the {}: {}",
                        item.label,
                        backend::PLACE,
                        !exists(landing),
                    ),
                );
            }
        }
    }
}

/// Did moving items out leave anything in the Trash's own books?
fn check_clean(items: &[Item], work: &Path, baseline: Option<i64>, report: &mut Report) {
    if !backend::KEEPS_RECORD || !items.iter().any(|i| i.restored) {
        return;
    }
    let records: Vec<PathBuf> = items
        .iter()
        .filter(|i| i.restored)
        .filter_map(|i| i.landing.as_deref().and_then(backend::leftover))
        .collect();
    // Items that didn't come back still count, and aren't this question's fault.
    let still_there = items.iter().filter(|i| i.landing.is_some() && !i.restored).count() as i64;
    let describe = |count: Option<i64>| match (baseline, count) {
        (Some(before), Some(now)) => format!(
            "items in the {}: {before} before, {now} now, {still_there} of ours still there",
            backend::PLACE,
        ),
        _ => format!("couldn't read the {}'s item count", backend::PLACE),
    };
    let verdict = |records_left: bool, count: Option<i64>| match (baseline, count) {
        _ if records_left => Verdict::Fail,
        (Some(before), Some(now)) if now == before + still_there => Verdict::Pass,
        (Some(_), Some(_)) => Verdict::Fail,
        _ => Verdict::NotAnswered,
    };
    let count = backend::bin_count(work);
    report.check(
        Question::Clean,
        verdict(!records.is_empty(), count),
        &format!("{} record(s) left behind; {}", records.len(), describe(count)),
    );
    if records.is_empty() {
        return;
    }
    for record in &records {
        println!("       record left behind: {}", record.display());
        if let Err(e) = fs::remove_file(record) {
            println!("       couldn't remove it: {e}");
        }
    }
    let count = backend::bin_count(work);
    let left = records.iter().any(|r| exists(r));
    report.check(
        Question::CleanAfterRemoval,
        verdict(left, count),
        &format!("after removing the record(s) by hand; {}", describe(count)),
    );
}

/// Trashes a file on a volume the developer says has no Trash. Passing means
/// it was refused because there's no Trash, and is still there, untouched.
fn check_no_trash_volume(volume: &Path, report: &mut Report) -> io::Result<()> {
    println!("\n-- no-Trash volume: {}", volume.display());
    let work = volume.join(work_folder_name());
    fs::create_dir(&work)?;
    let item = Item::new("no-trash file", Question::NoTrash, work.join("no-trash.txt"), &[""]);
    item.create()?;
    let name = volume.display();

    let preflight = backend::preflight(&item.path);
    println!(
        "       preflight says it {} be trashed: {}",
        if preflight.can_trash { "can" } else { "can't" },
        preflight.detail,
    );
    let mut keep_work = false;
    match backend::trash(&item.path, true) {
        Err(refusal) if !item.intact_at(&item.path) => {
            keep_work = true;
            report.check(
                Question::NoTrash,
                Verdict::Fail,
                &format!("{name}: refused, but the file is NOT in place. It was destroyed or damaged: {refusal}"),
            );
        }
        Err(Refusal::NoTrash(reason)) => {
            let when = if preflight.can_trash { "only when it was tried" } else { "by the preflight too" };
            report.check(
                Question::NoTrash,
                Verdict::Pass,
                &format!("{name}: detected {when}, file left in place: {reason}"),
            );
        }
        Err(Refusal::Other(reason)) => report.check(
            Question::NoTrash,
            Verdict::NotAnswered,
            &format!("{name}: file left in place, but this failure doesn't show there's no {}: {reason}", backend::PLACE),
        ),
        Ok(landing) => {
            println!("       landed at {}", landing.display());
            report.check(
                Question::NoTrash,
                Verdict::NotAnswered,
                &format!("{name}: this volume has a {}, so this says nothing: try another", backend::PLACE),
            );
            if let Err(error) = backend::restore(&landing, &item.path) {
                keep_work = true;
                println!("       couldn't move it back: {error}");
            }
        }
    }
    if keep_work {
        println!("       left in place for you to look at: {}", work.display());
        Ok(())
    } else {
        fs::remove_dir_all(&work)
    }
}

fn work_folder_name() -> String {
    format!("files-cleaner-trash-test-{}", process::id())
}

fn pause() {
    println!("\nLook in the {} now: the test items should be listed there.", backend::PLACE);
    println!("Don't restore or delete them. Press Enter to carry on.");
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
}

fn run(volumes: &[PathBuf], pausing: bool, report: &mut Report) -> io::Result<()> {
    // The home folder, not the temp folder: /tmp is often its own disk on
    // Linux, which wouldn't test the home Trash a real Scan Root would use.
    let home = env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("no home folder set"))?;
    let work = home.join(work_folder_name());
    fs::create_dir(&work)?;
    println!("Test items are in {}", work.display());

    let mut items = vec![
        Item::new("file", Question::MoveBack, work.join("a file.txt"), &[""]),
        Item::new("folder", Question::MoveBack, work.join("a folder"), &["inner.txt", "sub/deep.txt"]),
        Item::new("Package", Question::MoveBack, work.join("Test.app"), &["Contents/Info.plist", "Contents/MacOS/test"]),
        Item::new("same name A", Question::SameName, work.join("a/same name.txt"), &[""]),
        Item::new("same name B", Question::SameName, work.join("b/same name.txt"), &[""]),
    ];
    for item in &items {
        item.create()?;
    }

    let baseline = backend::bin_count(&work);
    trash_items(&mut items, report);
    println!("\nitems in the {}: {baseline:?} before, {:?} now", backend::PLACE, backend::bin_count(&work));
    let same_name: Vec<&PathBuf> = items
        .iter()
        .filter(|i| i.question == Question::SameName)
        .filter_map(|i| i.landing.as_ref())
        .collect();
    if let [a, b] = same_name[..] {
        report.check(
            Question::SameName,
            if a == b { Verdict::Fail } else { Verdict::Pass },
            &format!("two landings: {} and {}", a.display(), b.display()),
        );
    }

    if pausing {
        pause();
    }
    restore_items(&mut items, report);
    check_clean(&items, &work, baseline, report);

    let all_restored = items.iter().all(|i| i.restored);
    let restored = items.iter().filter(|i| i.restored).count();
    let errors: Vec<&str> = items.iter().filter_map(|i| i.restore_error.as_deref()).collect();
    if let Some((verdict, text)) = backend::access_check(restored, &errors) {
        report.check(Question::Access, verdict, &text);
    }

    if all_restored {
        fs::remove_dir_all(&work)?;
    } else {
        println!("\nSome items didn't come back, so these are left for you to clear by hand:");
        println!("  {}", work.display());
        for item in items.iter().filter(|i| !i.restored) {
            if let Some(landing) = &item.landing {
                println!("  {}", landing.display());
            }
        }
    }

    for volume in volumes {
        if let Err(error) = check_no_trash_volume(volume, report) {
            report.check(
                Question::NoTrash,
                Verdict::NotAnswered,
                &format!("{}: couldn't set up the test there: {error}", volume.display()),
            );
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let mut volumes = Vec::new();
    let mut pausing = false;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--pause" => pausing = true,
            "--no-trash-volume" => match args.next().map(std::path::absolute) {
                Some(Ok(dir)) if dir.is_dir() => volumes.push(dir),
                _ => {
                    eprintln!("--no-trash-volume needs a folder that exists");
                    return ExitCode::FAILURE;
                }
            },
            _ => {
                eprintln!("usage: os-trash-test [--pause] [--no-trash-volume <folder>]...");
                return ExitCode::FAILURE;
            }
        }
    }

    let mut report = Report::default();
    let setup = run(&volumes, pausing, &mut report);
    if let Err(error) = &setup {
        println!("\nStopped early: {error}");
    }
    let passed = report.summary(backend::OS_NOTE);
    if setup.is_ok() && passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
