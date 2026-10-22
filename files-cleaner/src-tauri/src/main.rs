// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // The grouping engine starts the app itself to read one PDF.
    if let Some(code) = files_cleaner_grouping::pdf_child_main() {
        std::process::exit(code);
    }
    files_cleaner_lib::run()
}
