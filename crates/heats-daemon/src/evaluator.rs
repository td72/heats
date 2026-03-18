use std::collections::HashMap;
use std::time::Duration;

use crate::command::{expand_placeholder, parse_jsonl, spawn_pipeline_async, LoadedItem};
use heats_core::config::{pipeline_has_placeholder, EvaluatorConfig};
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

    let has_placeholder = pipeline_has_placeholder(&config.source);

    // Determine pipeline and input based on placeholder presence
    let (pipeline, input) = if has_placeholder {
        (expand_placeholder(&config.source, query), None)
    } else {
        // Pass query + newline via stdin
        (config.source.clone(), Some(format!("{}\n", query)))
    };

    let result = spawn_pipeline_async(
        &pipeline,
        input.as_deref(),
        Duration::from_secs(2),
    )
    .await;

    match result {
        Ok(output) => parse_jsonl(&output),
        Err(e) => {
            tracing::warn!("Evaluator pipeline {:?} failed: {}", config.source, e);
            Vec::new()
        }
    }
}
