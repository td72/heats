use std::path::PathBuf;

/// Directories to scan for macOS applications
const APP_DIRS: &[&str] = &[
    "/Applications",
    "/Applications/Utilities",
    "/System/Applications",
    "/System/Applications/Utilities",
];

/// A discovered application entry (no icon loading)
pub struct AppEntry {
    pub name: String,
    pub path: String,
}

/// Scan standard macOS directories for .app bundles.
/// Returns a sorted list of AppEntry without loading icons.
pub fn scan_apps() -> Vec<AppEntry> {
    let mut items = Vec::new();
    for dir in APP_DIRS {
        let path = PathBuf::from(dir);
        if !path.exists() {
            continue;
        }
        let entries = match std::fs::read_dir(&path) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "app") {
                let name = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                items.push(AppEntry {
                    name,
                    path: path.to_string_lossy().to_string(),
                });
            }
        }
    }
    items.sort_by(|a, b| a.name.cmp(&b.name));
    items
}
