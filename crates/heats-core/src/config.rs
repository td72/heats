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

/// A mode: hotkey → providers mapping
#[derive(Debug, Clone, Deserialize)]
pub struct ModeConfig {
    pub name: String,
    pub hotkey: String,
    pub providers: Vec<String>,
    #[serde(default)]
    pub evaluators: Vec<String>,
}

/// How to pass input to a source/action command
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum InputMode {
    Stdin,
    Arg,
}

impl Default for InputMode {
    fn default() -> Self {
        Self::Stdin
    }
}

/// An evaluator: query-driven source + action
#[derive(Debug, Clone, Deserialize)]
pub struct EvaluatorConfig {
    /// Source command (receives query, outputs JSONL)
    pub source: Vec<String>,
    /// How to pass the query to the source command
    #[serde(default)]
    pub input: InputMode,
    /// Action command (executed on selection)
    pub action: Vec<String>,
    /// How to pass the field value to the action command
    #[serde(default)]
    pub action_input: InputMode,
    /// DmenuItem field to pass to the action
    #[serde(default = "default_field")]
    pub field: String,
}

/// A provider: source command + action command bundled together
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    /// Source command + arguments (stdout に JSONL を出力)
    pub source: Vec<String>,
    /// Action command + arguments (選択時に field 値を末尾に付与して実行)
    pub action: Vec<String>,
    /// DmenuItem field to pass to the action (e.g. "data.path", "title"). Default: "data"
    #[serde(default = "default_field")]
    pub field: String,
    /// Background cache refresh interval in seconds. None = no caching (load on demand).
    pub cache_interval: Option<u64>,
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
                    hotkey: "Cmd+Semicolon".to_string(),
                    providers: vec!["open-apps".to_string(), "focus-window".to_string()],
                    evaluators: vec!["calculator".to_string()],
                },
                ModeConfig {
                    name: "windows".to_string(),
                    hotkey: "Cmd+Quote".to_string(),
                    providers: vec!["focus-window".to_string()],
                    evaluators: Vec::new(),
                },
            ],
            provider: HashMap::from([
                (
                    "open-apps".to_string(),
                    ProviderConfig {
                        source: vec!["heats-list-apps".to_string()],
                        action: vec!["open".to_string(), "-a".to_string()],
                        field: "data.path".to_string(),
                        cache_interval: None,
                    },
                ),
                (
                    "focus-window".to_string(),
                    ProviderConfig {
                        source: vec!["heats-list-windows".to_string()],
                        action: vec!["heats-focus-window".to_string()],
                        field: "data.pid".to_string(),
                        cache_interval: None,
                    },
                ),
            ]),
            evaluator: HashMap::from([(
                "calculator".to_string(),
                EvaluatorConfig {
                    source: vec!["heats-eval-calc".to_string()],
                    input: InputMode::default(),
                    action: vec!["pbcopy".to_string()],
                    action_input: InputMode::default(),
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
    match std::fs::read_to_string(&path) {
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
