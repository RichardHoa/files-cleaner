//! Valid on every OS, since disks move between machines.

const FORBIDDEN: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// Names Windows keeps for devices, with or without an extension.
const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

const MAX_CHARS: usize = 100;

/// `None` if nothing is left.
pub fn safe_folder_name(name: &str) -> Option<String> {
    let kept: String = name
        .chars()
        .filter(|c| !FORBIDDEN.contains(c) && !c.is_control())
        .collect();
    let mut safe = trimmed(&kept);
    let stem = safe.split('.').next().unwrap_or("");
    if WINDOWS_RESERVED
        .iter()
        .any(|reserved| reserved.eq_ignore_ascii_case(stem))
    {
        safe.insert(stem.len(), '_');
    }
    let safe = trimmed(&safe.chars().take(MAX_CHARS).collect::<String>());
    (!safe.is_empty()).then_some(safe)
}

fn trimmed(name: &str) -> String {
    name.trim()
        .trim_end_matches(|c: char| c == '.' || c.is_whitespace())
        .to_string()
}
