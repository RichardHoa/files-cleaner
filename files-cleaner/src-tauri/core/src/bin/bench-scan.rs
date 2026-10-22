//! Times `scan` on a folder, for measuring the scanner's speed:
//!
//!     cargo run --release --bin bench-scan -- <folder> [runs]
//!
//! Run it under `/usr/bin/time -l` for peak memory and CPU time too. The
//! first run usually reads from disk and later ones from the OS's cache, so
//! it scans `runs` times (2 by default) and reports each.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use files_cleaner_core::{load_config, scan_root_warning, scan_with_progress};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(root) = args.next().map(PathBuf::from) else {
        eprintln!("usage: bench-scan <folder> [runs]");
        return ExitCode::FAILURE;
    };
    let runs: usize = args.next().and_then(|n| n.parse().ok()).unwrap_or(2);
    let home = PathBuf::from(env::var_os("HOME").unwrap_or_default());
    let config = load_config(r#"{ "rules": [] }"#, &home).expect("the empty config is valid");

    if let Some(warning) = scan_root_warning(&root, &config) {
        println!("warning: {warning} (scanning anyway)");
    }
    for run in 1..=runs {
        let mut folders = 0;
        let start = Instant::now();
        let result = match scan_with_progress(&root, &config, |progress| folders = progress.folders)
        {
            Ok(result) => result,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        };
        let seconds = start.elapsed().as_secs_f64();
        let bytes: u64 = result.entries.iter().map(|e| e.size).sum();
        let packages = result.entries.iter().filter(|e| e.is_package).count();
        println!(
            "run {run}: {seconds:.2} s · {} items ({packages} Packages, {:.1} GB) · {folders} folders · \
             {:.0} items/s · {:.0} folders/s · {} skipped · {} no extension · {} unreadable",
            result.entries.len(),
            bytes as f64 / 1e9,
            result.entries.len() as f64 / seconds,
            folders as f64 / seconds,
            result.skipped,
            result.no_extension,
            result.unreadable.len(),
        );
    }
    ExitCode::SUCCESS
}
