//! The Files Cleaner core: everything except the UI, with no Tauri
//! dependency (ADR-0003). The public functions here are the test seam.

mod config;
mod protection;
mod scanner;

pub use config::{
    load_config, Config, ConfigError, DuplicateHandling, Rule, RuleOperation, RuleTarget,
    TrashDestination, EXAMPLE_CONFIG,
};
pub use protection::HOLDING_FOLDER_NAME;
pub use scanner::{scan, ScanEntry, ScanError, ScanResult};
