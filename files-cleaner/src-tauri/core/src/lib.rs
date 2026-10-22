//! The Files Cleaner core: everything except the UI, with no Tauri
//! dependency (ADR-0003). The public functions here are the test seam.

mod clash;
mod config;
mod duplicates;
mod executor;
mod folder_name;
mod journal;
mod planner;
mod protection;
mod recent;
mod rules;
mod scanner;
mod session_id;
mod sessions;
mod suggested_folders;
mod volume;

pub use config::{
    load_config, Config, ConfigError, Rule, RuleOperation, RuleTarget,
    TrashDestination, EXAMPLE_CONFIG,
};
pub use folder_name::safe_folder_name;
pub use executor::{redo_operations, run, run_with_progress, undo, undo_operations, Outcome, RunProgress, Summary};
pub use journal::{lock_sessions, SessionError, SessionLock};
pub use planner::{build_plan, build_rule_plan, Operation, OperationKind, Plan, Reason, SourceState, THIS_SESSION_FOLDER};
pub use protection::HOLDING_FOLDER_NAME;
pub use recent::{recent_scan_roots, record_recent_scan_root, remove_recent_scan_root, RecentScanRoot};
pub use scanner::{scan, scan_root_warning, scan_with_progress, ScanEntry, ScanError, ScanProgress, ScanResult};
pub use sessions::{
    list_sessions, session_details, OperationState, SessionDetails, SessionInfo, SessionOperation, SessionPage,
    SessionStatus,
};
pub use suggested_folders::{
    build_suggested_folders_plan, FileChoice, FolderChoice, SuggestedFolderChoice, SuggestedFolderError,
};
