use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub hotkey: HotkeyConfig,
    pub window: WindowConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct HotkeyConfig {
    pub modifiers: String,
    pub key: String,
}

/// Window management mode
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WindowMode {
    /// Normal mode: window appears on the display with the mouse cursor
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
            hotkey: HotkeyConfig::default(),
            window: WindowConfig::default(),
        }
    }
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            modifiers: "Cmd".to_string(),
            key: "Semicolon".to_string(),
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

pub fn load() -> Config {
    let path = config_path();
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
