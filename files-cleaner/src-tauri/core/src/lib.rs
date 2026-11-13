//! The Files Cleaner core: everything except the UI, with no Tauri
//! dependency (ADR-0003). The public functions here are the test seam.

mod clash;
mod config;
mod download_hosts;
mod duplicates;
mod executor;
mod folder_name;
mod journal;
mod model_link;
mod os_trash;
mod planner;
mod protection;
mod recent;
mod scanner;
mod session_id;
mod sessions;
mod suggested_folders;
mod volume;

pub use config::{
    load_config, reset_config, save_config, Config, ConfigError, DefaultCopyRule, GroupingSettings,
    SettingChange, EXAMPLE_CONFIG,
};
pub use download_hosts::{check_download, follow_redirect, Hop};
pub use duplicates::{
    duplicate_groups, DuplicateCopy, DuplicateGroup, DuplicatePhase, DuplicateProgress,
};
pub use executor::{
    redo_operations, retry_operations, run, run_with_progress, undo, undo_operations, NoteKind,
    Outcome, RunProgress, Summary,
};
pub use folder_name::safe_folder_name;
pub use journal::{lock_sessions, SessionError, SessionLock};
pub use model_link::{describing_model_link, ModelLink};
pub use os_trash::{OsTrash, SystemTrash};
pub use planner::{
    build_plan, build_plan_with_progress, Operation, OperationKind, Plan, Reason, SourceState,
};
pub use recent::{
    recent_scan_roots, record_recent_scan_root, remove_recent_scan_root, RecentScanRoot,
};
pub use scanner::{
    scan, scan_root_warning, scan_with_progress, ScanEntry, ScanError, ScanProgress, ScanResult,
};
pub use sessions::{
    delete_all_sessions, list_sessions, session_details, OperationState, SessionDetails,
    SessionInfo, SessionOperation, SessionPage, SessionStatus,
};
pub use suggested_folders::{
    build_review_plan, build_suggested_folders_plan, DuplicateChoice, FileChoice, FolderChoice,
    SuggestedFolderChoice, SuggestedFolderError,
};
pub use volume::different_disks;
