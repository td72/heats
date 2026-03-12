use std::collections::HashMap;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use crate::command::{resolve_command, LoadedItem};
use heats_core::config::{pipeline_has_placeholder, EvaluatorConfig, Pipeline};
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
        tracing::warn!("Empty evaluator source pipeline");
        return Vec::new();
    }

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        spawn_evaluator_pipeline(&config.source, query),
    )
    .await;

    match result {
        Ok(Ok(output)) => parse_jsonl(&output),
        Ok(Err(e)) => {
            tracing::warn!("Evaluator pipeline {:?} failed: {}", config.source, e);
            Vec::new()
        }
        Err(_) => {
            tracing::warn!("Evaluator pipeline {:?} timed out after 2s", config.source);
            Vec::new()
        }
    }
}

/// Spawn an evaluator source pipeline.
/// Query is passed via `{}` placeholder (arg mode) or stdin (if no placeholder).
async fn spawn_evaluator_pipeline(
    pipeline: &Pipeline,
    query: &str,
) -> Result<String, String> {
    if pipeline.is_empty() {
        return Err("Empty pipeline".to_string());
    }

    let has_placeholder = pipeline_has_placeholder(pipeline);

    // Expand placeholders if present
    let expanded: Pipeline = if has_placeholder {
        pipeline
            .iter()
            .map(|cmd| cmd.iter().map(|arg| arg.replace("{}", query)).collect())
            .collect()
    } else {
        pipeline.clone()
    };

    let mut prev_stdout: Option<tokio::process::ChildStdout> = None;
    let mut children: Vec<tokio::process::Child> = Vec::new();

    for (i, cmd) in expanded.iter().enumerate() {
        if cmd.is_empty() {
            return Err("Empty command in pipeline".to_string());
        }

        let program = resolve_command(&cmd[0]);
        let mut command = Command::new(&program);
        command.args(&cmd[1..]);

        if let Some(stdout) = prev_stdout.take() {
            let std_stdout = stdout.into_owned_fd().map_err(|e| format!("stdin conversion: {}", e))?;
            command.stdin(std_stdout);
        } else if i == 0 && !has_placeholder {
            command.stdin(Stdio::piped());
        } else {
            command.stdin(Stdio::null());
        }

        command.stdout(Stdio::piped()).stderr(Stdio::null());

        let mut child = command
            .spawn()
            .map_err(|e| format!("Failed to spawn '{}': {}", program, e))?;

        // Write query to first command's stdin if no placeholder
        if i == 0 && !has_placeholder {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(query.as_bytes()).await;
                let _ = stdin.write_all(b"\n").await;
                drop(stdin);
            }
        }

        prev_stdout = child.stdout.take();
        children.push(child);
    }

    // Read output from last command
    let output = if let Some(stdout) = prev_stdout {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        let mut output = String::new();
        while let Ok(Some(line)) = lines.next_line().await {
            output.push_str(&line);
            output.push('\n');
        }
        output
    } else {
        String::new()
    };

    // Wait for all children
    for mut child in children {
        let status = child.wait().await.map_err(|e| format!("wait failed: {}", e))?;
        if !status.success() {
            return Err(format!("Pipeline command exited with {}", status));
        }
    }

    Ok(output)
}

fn parse_jsonl(output: &str) -> Vec<DmenuItem> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            serde_json::from_str::<DmenuItem>(line)
                .map_err(|e| {
                    tracing::debug!("Failed to parse evaluator JSONL: {}", e);
                    e
                })
                .ok()
        })
        .collect()
}
