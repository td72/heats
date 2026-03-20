use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::icon;
use heats_core::config::{pipeline_has_placeholder, ActionConfig, EvaluatorConfig, Pipeline, ProviderConfig};
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
                exec_path: dmenu_item.get_field("data").into_owned(),
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

/// Spawn a single source pipeline and parse its JSONL output.
async fn load_single_source(source: &Pipeline) -> Vec<(DmenuItem, Option<IconData>)> {
    let result = spawn_pipeline_async(source, None, Duration::from_secs(2)).await;

    match result {
        Ok(output) => {
            let dmenu_items = parse_jsonl(&output);
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
        Err(e) => {
            tracing::warn!("Source pipeline {:?} failed: {}", source, e);
            Vec::new()
        }
    }
}

/// Parse JSONL output into DmenuItems.
pub fn parse_jsonl(output: &str) -> Vec<DmenuItem> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            serde_json::from_str::<DmenuItem>(line)
                .map_err(|e| {
                    tracing::debug!("Failed to parse JSONL line: {}", e);
                    e
                })
                .ok()
        })
        .collect()
}

/// Spawn an async pipeline, return the final stdout as a String.
/// Applies a timeout to the entire pipeline. On timeout, the future is cancelled
/// and all child processes are dropped (killed via SIGKILL on Unix).
/// On pipeline errors, children are also cleaned up via drop.
pub async fn spawn_pipeline_async(
    pipeline: &Pipeline,
    input: Option<&str>,
    timeout: Duration,
) -> Result<String, String> {
    let result = tokio::time::timeout(timeout, run_pipeline(pipeline, input)).await;

    match result {
        Ok(inner) => inner,
        Err(_) => Err(format!("Pipeline {:?} timed out after {:?}", pipeline, timeout)),
    }
}

/// Internal: spawn pipeline commands, write input, read output, wait for exit.
async fn run_pipeline(
    pipeline: &Pipeline,
    input: Option<&str>,
) -> Result<String, String> {
    if pipeline.is_empty() {
        return Err("Empty pipeline".to_string());
    }

    let mut prev_stdout: Option<tokio::process::ChildStdout> = None;
    let mut children: Vec<tokio::process::Child> = Vec::new();

    for (i, cmd) in pipeline.iter().enumerate() {
        if cmd.is_empty() {
            kill_all(&mut children).await;
            return Err("Empty command in pipeline".to_string());
        }

        let program = resolve_command(&cmd[0]);
        let mut command = Command::new(&program);
        command.args(&cmd[1..]);

        // stdin: pipe from previous command, or from input, or null
        if let Some(stdout) = prev_stdout.take() {
            let std_stdout = stdout
                .into_owned_fd()
                .map_err(|e| format!("stdin conversion: {}", e))?;
            command.stdin(std_stdout);
        } else if i == 0 && input.is_some() {
            command.stdin(Stdio::piped());
        } else {
            command.stdin(Stdio::null());
        }

        command.stdout(Stdio::piped()).stderr(Stdio::null());

        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(e) => {
                kill_all(&mut children).await;
                return Err(format!("Failed to spawn '{}': {}", program, e));
            }
        };

        // Write input to first command's stdin
        if i == 0 {
            if let Some(input_data) = input {
                if let Some(mut stdin) = child.stdin.take() {
                    use tokio::io::AsyncWriteExt;
                    let _ = stdin.write_all(input_data.as_bytes()).await;
                    drop(stdin);
                }
            }
        }

        prev_stdout = child.stdout.take();
        children.push(child);
    }

    let output = read_all_output(prev_stdout).await;
    wait_all_children(children).await?;
    Ok(output)
}

/// Read all lines from a pipeline's final stdout.
async fn read_all_output(stdout: Option<tokio::process::ChildStdout>) -> String {
    let Some(stdout) = stdout else {
        return String::new();
    };
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();
    let mut output = String::new();
    while let Ok(Some(line)) = lines.next_line().await {
        output.push_str(&line);
        output.push('\n');
    }
    output
}

/// Wait for all child processes and check exit status.
async fn wait_all_children(children: Vec<tokio::process::Child>) -> Result<(), String> {
    for mut child in children {
        let status = child
            .wait()
            .await
            .map_err(|e| format!("wait failed: {}", e))?;
        if !status.success() {
            return Err(format!("Pipeline command exited with {}", status));
        }
    }
    Ok(())
}

/// Kill all child processes in a pipeline.
async fn kill_all(children: &mut [tokio::process::Child]) {
    for child in children.iter_mut() {
        let _ = child.kill().await;
    }
}

/// Spawn a sync pipeline for action execution.
/// Returns after the pipeline completes.
fn spawn_pipeline_sync(pipeline: &Pipeline, input: Option<&str>) -> Result<(), String> {
    if pipeline.is_empty() {
        return Err("Empty pipeline".to_string());
    }

    let len = pipeline.len();
    let mut prev_stdout: Option<std::process::ChildStdout> = None;
    let mut children: Vec<std::process::Child> = Vec::new();

    for (i, cmd) in pipeline.iter().enumerate() {
        if cmd.is_empty() {
            return Err("Empty command in pipeline".to_string());
        }

        let program = resolve_command(&cmd[0]);
        let mut command = std::process::Command::new(&program);
        command.args(&cmd[1..]);

        // stdin
        if let Some(stdout) = prev_stdout.take() {
            command.stdin(stdout);
        } else if i == 0 && input.is_some() {
            command.stdin(Stdio::piped());
        } else {
            command.stdin(Stdio::null());
        }

        // stdout: pipe between commands, null for last
        if i < len - 1 {
            command.stdout(Stdio::piped());
        } else {
            command.stdout(Stdio::null());
        }

        command.stderr(Stdio::null());

        let mut child = command
            .spawn()
            .map_err(|e| format!("Failed to spawn '{}': {}", program, e))?;

        // Write input to first command's stdin
        if i == 0 {
            if let Some(input_data) = input {
                if let Some(mut stdin) = child.stdin.take() {
                    use std::io::Write;
                    let _ = stdin.write_all(input_data.as_bytes());
                    drop(stdin);
                }
            }
        }

        prev_stdout = child.stdout.take();
        children.push(child);
    }

    // Wait for all children (always reap all to avoid zombies)
    let mut first_error: Option<String> = None;
    for mut child in children {
        match child.wait() {
            Ok(status) if !status.success() && first_error.is_none() => {
                first_error = Some(format!("Pipeline command exited with {}", status));
            }
            Err(e) if first_error.is_none() => {
                first_error = Some(format!("wait failed: {}", e));
            }
            _ => {}
        }
    }

    match first_error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Expand `{}` placeholders in a pipeline with the given value.
pub fn expand_placeholder(pipeline: &Pipeline, value: &str) -> Pipeline {
    pipeline
        .iter()
        .map(|cmd| {
            cmd.iter()
                .map(|arg| arg.replace("{}", value))
                .collect()
        })
        .collect()
}

/// Execute a pipeline with a field value: auto-detect arg (placeholder) vs stdin mode.
fn run_pipeline_with_field(pipeline: &Pipeline, field_value: &str) {
    if pipeline_has_placeholder(pipeline) {
        // Arg mode: expand {} placeholders
        let expanded = expand_placeholder(pipeline, field_value);
        tracing::info!("Executing pipeline (arg): {:?}", expanded);
        if let Err(e) = spawn_pipeline_sync(&expanded, None) {
            tracing::error!("Pipeline execution failed: {}", e);
        }
    } else {
        // Stdin mode: pass field value to first command's stdin
        tracing::info!("Executing pipeline (stdin): {:?}", pipeline);
        if let Err(e) = spawn_pipeline_sync(pipeline, Some(field_value)) {
            tracing::error!("Pipeline execution failed: {}", e);
        }
    }
}

/// Execute an action by running the provider's action pipeline with the field value from the DmenuItem.
pub fn execute_action(provider: &ProviderConfig, dmenu_item: &DmenuItem) {
    let field_value = dmenu_item.get_field(&provider.field);
    run_pipeline_with_field(&provider.action, &field_value);
}

/// Execute an evaluator action pipeline with the field value from the DmenuItem.
pub fn run_action(config: &EvaluatorConfig, dmenu_item: &DmenuItem) {
    let field_value = dmenu_item.get_field(&config.field);
    run_pipeline_with_field(&config.action, &field_value);
}

/// Execute a named alternative action on a DmenuItem.
pub fn execute_named_action(action: &ActionConfig, field: &str, dmenu_item: &DmenuItem) {
    let field_value = dmenu_item.get_field(field);
    run_pipeline_with_field(&action.command, &field_value);
}

/// Resolve a command name: if it's not an absolute path, check the directory
/// of our own executable first, then fall back to PATH lookup.
pub fn resolve_command(name: &str) -> String {
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
