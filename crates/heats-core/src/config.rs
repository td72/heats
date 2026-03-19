use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub window: WindowConfig,
    pub mode: Vec<ModeConfig>,
    pub provider: HashMap<String, ProviderConfig>,
    pub evaluator: HashMap<String, EvaluatorConfig>,
}

/// A mode: optional hotkey + providers/evaluators mapping
#[derive(Debug, Clone, Deserialize)]
pub struct ModeConfig {
    pub name: String,
    pub hotkey: Option<String>,
    pub providers: Vec<String>,
    #[serde(default)]
    pub evaluators: Vec<String>,
    /// Keybinding → action name mapping (e.g. "Alt+Enter" → "reveal")
    #[serde(default)]
    pub keybindings: HashMap<String, String>,
}

/// A pipeline of commands: each inner Vec is [program, arg1, arg2, ...].
/// Commands are piped together: cmd1 | cmd2 | cmd3.
///
/// Use `{}` placeholder in arguments to insert the field value (arg mode).
/// If no `{}` is found, the field value is passed via stdin to the first command.
pub type Pipeline = Vec<Vec<String>>;

/// Check if a pipeline contains a `{}` placeholder in any command's arguments.
pub fn pipeline_has_placeholder(pipeline: &Pipeline) -> bool {
    pipeline.iter().any(|cmd| cmd.iter().any(|arg| arg.contains("{}")))
}

/// An evaluator: query-driven source + action
#[derive(Debug, Clone, Deserialize)]
pub struct EvaluatorConfig {
    /// Source command pipeline (receives query, outputs JSONL)
    pub source: Pipeline,
    /// Action command pipeline (executed on selection)
    pub action: Pipeline,
    /// DmenuItem field to pass to the action
    #[serde(default = "default_field")]
    pub field: String,
}

/// A named alternative action for a provider
#[derive(Debug, Clone, Deserialize)]
pub struct ActionConfig {
    /// Action command pipeline
    pub command: Pipeline,
    /// DmenuItem field to pass to the action (overrides provider default)
    #[serde(default)]
    pub field: Option<String>,
}

/// A provider: source command + action command bundled together
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    /// Source command pipeline (stdout に JSONL を出力)
    pub source: Pipeline,
    /// Action command pipeline (選択時に field 値を渡して実行)
    pub action: Pipeline,
    /// DmenuItem field to pass to the action (e.g. "data.path", "title"). Default: "data"
    #[serde(default = "default_field")]
    pub field: String,
    /// Background cache refresh interval in seconds. None = no caching (load on demand).
    pub cache_interval: Option<u64>,
    /// Named alternative actions (e.g. "reveal" → open -R, "copy-path" → pbcopy)
    #[serde(default)]
    pub actions: HashMap<String, ActionConfig>,
}

fn default_field() -> String {
    "data".to_string()
}

/// Window management mode
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WindowMode {
    /// Normal mode: window appears on the display with keyboard focus
    Normal,
    /// Fixed mode: window always appears on a specific display (for tiling WMs like AeroSpace)
    Fixed,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    pub width: f32,
    pub height: f32,
    /// "normal" = follow mouse cursor, "fixed" = pin to a specific display
    pub mode: WindowMode,
    /// Display name for fixed mode (substring match, e.g. "LG" or "Built-in")
    pub display: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            window: WindowConfig::default(),
            mode: vec![
                ModeConfig {
                    name: "launcher".to_string(),
                    hotkey: Some("Cmd+Semicolon".to_string()),
                    providers: vec!["open-apps".to_string(), "focus-window".to_string()],
                    evaluators: vec!["calculator".to_string()],
                    keybindings: HashMap::new(),
                },
                ModeConfig {
                    name: "windows".to_string(),
                    hotkey: Some("Cmd+Quote".to_string()),
                    providers: vec!["focus-window".to_string()],
                    evaluators: Vec::new(),
                    keybindings: HashMap::new(),
                },
            ],
            provider: HashMap::from([
                (
                    "open-apps".to_string(),
                    ProviderConfig {
                        source: vec![vec!["heats-list-apps".to_string()]],
                        action: vec![vec!["open".to_string(), "-a".to_string(), "{}".to_string()]],
                        field: "data.path".to_string(),
                        cache_interval: None,
                        actions: HashMap::new(),
                    },
                ),
                (
                    "focus-window".to_string(),
                    ProviderConfig {
                        source: vec![vec!["heats-list-windows".to_string()]],
                        action: vec![vec!["heats-focus-window".to_string(), "{}".to_string()]],
                        field: "data.pid".to_string(),
                        cache_interval: None,
                        actions: HashMap::new(),
                    },
                ),
            ]),
            evaluator: HashMap::from([(
                "calculator".to_string(),
                EvaluatorConfig {
                    source: vec![vec!["heats-eval-calc".to_string()]],
                    action: vec![vec!["pbcopy".to_string()]],
                    field: "data".to_string(),
                },
            )]),
        }
    }
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 600.0,
            height: 400.0,
            mode: WindowMode::Normal,
            display: String::new(),
        }
    }
}

pub fn load_from(path: &std::path::Path) -> Config {
    let path = path.to_path_buf();
    load_path(&path)
}

pub fn load() -> Config {
    let path = config_path();
    load_path(&path)
}

fn load_path(path: &PathBuf) -> Config {
    if !path.exists() {
        tracing::info!("No config file found at {:?}, using defaults", path);
        return Config::default();
    }
    match std::fs::read_to_string(path) {
        Ok(contents) => match toml::from_str(&contents) {
            Ok(config) => {
                tracing::info!("Loaded config from {:?}", path);
                config
            }
            Err(e) => {
                tracing::warn!("Failed to parse config: {}, using defaults", e);
                Config::default()
            }
        },
        Err(e) => {
            tracing::warn!("Failed to read config file: {}, using defaults", e);
            Config::default()
        }
    }
}

fn config_path() -> PathBuf {
    // Use ~/.config/ (XDG convention) instead of ~/Library/Application Support/ (macOS default)
    dirs::home_dir()
        .expect("Could not determine home directory")
        .join(".config")
        .join("heats")
        .join("config.toml")
}
