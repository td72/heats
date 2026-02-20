use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use crate::platform;
use crate::source::{Source, SourceItem};

/// Directories to scan for macOS applications
const APP_DIRS: &[&str] = &[
    "/Applications",
    "/Applications/Utilities",
    "/System/Applications",
    "/System/Applications/Utilities",
];

/// Source that discovers macOS .app bundles
pub struct ApplicationsSource;

impl ApplicationsSource {
    pub fn new() -> Self {
        Self
    }

    fn scan_apps() -> Vec<SourceItem> {
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
                    items.push(SourceItem {
                        title: name,
                        subtitle: Some(path.to_string_lossy().to_string()),
                        exec_path: path.to_string_lossy().to_string(),
                        source_name: "applications".to_string(),
                    });
                }
            }
        }
        items.sort_by(|a, b| a.title.cmp(&b.title));
        items
    }
}

impl Source for ApplicationsSource {
    fn name(&self) -> &str {
        "applications"
    }

    fn load(&self) -> Pin<Box<dyn Future<Output = Vec<SourceItem>> + Send>> {
        Box::pin(async { tokio::task::spawn_blocking(Self::scan_apps).await.unwrap() })
    }

    fn execute(&self, item: &SourceItem) -> Result<(), Box<dyn std::error::Error>> {
        platform::macos::open_application(&item.exec_path)
    }
}
