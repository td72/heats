use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::icon;
use heats_core::config::ProviderConfig;
use heats_core::source::{DmenuItem, IconData, SourceItem};

/// A loaded item with metadata for action resolution
#[derive(Debug, Clone)]
pub struct LoadedItem {
    /// The SourceItem for display in the fuzzy finder
    pub item: SourceItem,
    /// Provider name this item belongs to (key for action lookup)
    pub provider_name: String,
    /// The original DmenuItem (for field value extraction at action time)
    pub dmenu_item: DmenuItem,
}

/// Spawn source commands for the given providers in parallel and collect their JSONL output.
/// Each source command is expected to print DmenuItem JSON objects, one per line.
pub async fn load_from_providers(
    provider_names: &[String],
    providers: &HashMap<String, ProviderConfig>,
) -> Vec<LoadedItem> {
    let mut set = tokio::task::JoinSet::new();

    for name in provider_names {
        let name = name.clone();
        let source = match providers.get(&name) {
            Some(p) => p.source.clone(),
            None => {
                tracing::warn!("Provider '{}' not found in config", name);
                continue;
            }
        };
        set.spawn(async move {
            let items = load_single_source(&source).await;
            (name, items)
        });
    }

    let mut all_items = Vec::new();
    while let Some(Ok((provider_name, items))) = set.join_next().await {
        for (dmenu_item, icon) in items {
            let source_item = SourceItem {
                id: None,
                title: dmenu_item.title.clone(),
                subtitle: dmenu_item.subtitle.clone(),
                exec_path: dmenu_item.get_field("data"),
                source_name: provider_name.clone(),
                icon,
            };
            all_items.push(LoadedItem {
                item: source_item,
                provider_name: provider_name.clone(),
                dmenu_item,
            });
        }
    }

    all_items
}

/// Spawn a single source command and parse its JSONL output.
async fn load_single_source(source: &[String]) -> Vec<(DmenuItem, Option<IconData>)> {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        spawn_and_read(source),
    )
    .await;

    match result {
        Ok(items) => items,
        Err(_) => {
            tracing::warn!("Source command {:?} timed out after 2s", source);
            Vec::new()
        }
    }
}

async fn spawn_and_read(source: &[String]) -> Vec<(DmenuItem, Option<IconData>)> {
    if source.is_empty() {
        tracing::warn!("Empty source command");
        return Vec::new();
    }

    // Resolve command: if not an absolute path, look next to our own executable first
    let program = resolve_command(&source[0]);

    let child = Command::new(&program)
        .args(&source[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();

    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to spawn source command {:?}: {}", source, e);
            return Vec::new();
        }
    };

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => return Vec::new(),
    };

    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();
    let mut dmenu_items = Vec::new();

    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<DmenuItem>(&line) {
            Ok(dmenu_item) => {
                dmenu_items.push(dmenu_item);
            }
            Err(e) => {
                tracing::debug!("Failed to parse JSONL line from {:?}: {}", source, e);
            }
        }
    }

    // Wait for the process to exit
    let _ = child.wait().await;

    // Load icons in a blocking thread to avoid blocking the async runtime
    tokio::task::spawn_blocking(move || {
        dmenu_items
            .into_iter()
            .map(|dmenu_item| {
                let icon = dmenu_item
                    .icon_path
                    .as_ref()
                    .and_then(|p| icon::load_app_icon(&PathBuf::from(p)));
                (dmenu_item, icon)
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// Execute an action by running the provider's action command with the field value from the DmenuItem.
pub fn execute_action(provider: &ProviderConfig, dmenu_item: &DmenuItem) {
    let field_value = dmenu_item.get_field(&provider.field);

    if provider.action.is_empty() {
        tracing::error!("Provider action command is empty");
        return;
    }

    let program = resolve_command(&provider.action[0]);
    let mut args: Vec<&str> = provider.action[1..].iter().map(|s| s.as_str()).collect();
    args.push(&field_value);

    tracing::info!("Executing action: {} {:?}", program, args);

    match std::process::Command::new(&program)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(_) => {}
        Err(e) => {
            tracing::error!("Failed to execute action '{}': {}", &program, e);
        }
    }
}

/// Resolve a command name: if it's not an absolute path, check the directory
/// of our own executable first, then fall back to PATH lookup.
fn resolve_command(name: &str) -> String {
    let path = std::path::Path::new(name);
    if path.is_absolute() {
        return name.to_string();
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(name);
            if candidate.exists() {
                return candidate.to_string_lossy().to_string();
            }
        }
    }
    // Fall back to PATH
    name.to_string()
}
