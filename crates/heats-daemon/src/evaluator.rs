use std::collections::HashMap;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use crate::command::{resolve_command, LoadedItem};
use heats_core::config::{EvaluatorConfig, InputMode};
use heats_core::source::{DmenuItem, SourceItem};

/// Run all evaluators for the given query and return results.
pub async fn run_evaluators(
    query: &str,
    evaluator_names: &[String],
    configs: &HashMap<String, EvaluatorConfig>,
) -> Vec<LoadedItem> {
    tracing::debug!(
        "run_evaluators: query='{}', evaluators={:?}, configs_keys={:?}",
        query, evaluator_names, configs.keys().collect::<Vec<_>>()
    );

    let mut set = tokio::task::JoinSet::new();

    for name in evaluator_names {
        let name = name.clone();
        let config = match configs.get(&name) {
            Some(c) => c.clone(),
            None => {
                tracing::warn!("Evaluator '{}' not found in config", name);
                continue;
            }
        };
        let query = query.to_string();
        set.spawn(async move {
            let items = run_single_evaluator(&query, &config).await;
            (name, items)
        });
    }

    let mut all_items = Vec::new();
    while let Some(result) = set.join_next().await {
        let (eval_name, dmenu_items) = match result {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("Evaluator task panicked: {}", e);
                continue;
            }
        };
        tracing::debug!("Evaluator '{}' returned {} items", eval_name, dmenu_items.len());
        for dmenu_item in dmenu_items {
            let source_item = SourceItem {
                id: None,
                title: dmenu_item.title.clone(),
                subtitle: dmenu_item.subtitle.clone(),
                exec_path: dmenu_item.get_field("data"),
                source_name: format!("eval:{eval_name}"),
                icon: None,
            };
            all_items.push(LoadedItem {
                item: source_item,
                provider_name: eval_name.clone(),
                dmenu_item,
            });
        }
    }

    all_items
}

async fn run_single_evaluator(query: &str, config: &EvaluatorConfig) -> Vec<DmenuItem> {
    if config.source.is_empty() {
        tracing::warn!("Empty evaluator source command");
        return Vec::new();
    }

    let program = resolve_command(&config.source[0]);
    let mut cmd = Command::new(&program);
    cmd.args(&config.source[1..]);

    match config.input {
        InputMode::Stdin => {
            cmd.stdin(Stdio::piped());
        }
        InputMode::Arg => {
            cmd.args([query]);
            cmd.stdin(Stdio::null());
        }
    }

    cmd.stdout(Stdio::piped()).stderr(Stdio::null());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to spawn evaluator {:?}: {}", config.source, e);
            return Vec::new();
        }
    };

    // Write query to stdin if stdin mode
    if config.input == InputMode::Stdin {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(query.as_bytes()).await;
            let _ = stdin.write_all(b"\n").await;
            drop(stdin);
        }
    }

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        read_output(&mut child),
    )
    .await;

    match result {
        Ok(items) => {
            let _ = child.wait().await;
            items
        }
        Err(_) => {
            tracing::warn!("Evaluator command {:?} timed out after 2s", config.source);
            let _ = child.kill().await;
            Vec::new()
        }
    }
}

async fn read_output(child: &mut tokio::process::Child) -> Vec<DmenuItem> {
    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => return Vec::new(),
    };

    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();
    let mut items = Vec::new();

    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<DmenuItem>(&line) {
            Ok(item) => items.push(item),
            Err(e) => {
                tracing::debug!("Failed to parse evaluator JSONL: {}", e);
            }
        }
    }

    items
}
