pub mod applications;
pub mod windows;

use std::borrow::Cow;
use std::sync::Arc;

/// JSONL protocol type: the schema for source command → daemon communication
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DmenuItem {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl DmenuItem {
    /// Get a field value by dot-separated path (e.g. "title", "data.pid")
    pub fn get_field(&self, field: &str) -> Cow<'_, str> {
        match field {
            "title" => Cow::Borrowed(&self.title),
            "subtitle" => match &self.subtitle {
                Some(s) => Cow::Borrowed(s),
                None => Cow::Borrowed(""),
            },
            "icon_path" => match &self.icon_path {
                Some(s) => Cow::Borrowed(s),
                None => Cow::Borrowed(""),
            },
            _ if field.starts_with("data") => {
                let data = match &self.data {
                    Some(v) => v,
                    None => return Cow::Borrowed(&self.title),
                };
                if field == "data" {
                    value_to_cow(data)
                } else if let Some(rest) = field.strip_prefix("data.") {
                    let mut current = data;
                    for key in rest.split('.') {
                        match current.get(key) {
                            Some(v) => current = v,
                            None => return Cow::Borrowed(""),
                        }
                    }
                    value_to_cow(current)
                } else {
                    Cow::Borrowed(&self.title)
                }
            }
            _ => Cow::Borrowed(&self.title),
        }
    }
}

/// Convert a JSON value to a Cow string for action arguments
fn value_to_cow(v: &serde_json::Value) -> Cow<'_, str> {
    match v {
        serde_json::Value::String(s) => Cow::Borrowed(s),
        serde_json::Value::Number(n) => Cow::Owned(n.to_string()),
        serde_json::Value::Bool(b) => Cow::Owned(b.to_string()),
        serde_json::Value::Null => Cow::Borrowed(""),
        other => Cow::Owned(other.to_string()),
    }
}

/// Icon data for a source item (daemon internal UI type)
#[derive(Debug, Clone)]
pub enum IconData {
    /// Pre-loaded RGBA pixel data (Arc to avoid expensive clones)
    Rgba {
        width: u32,
        height: u32,
        pixels: Arc<Vec<u8>>,
    },
    /// Text/emoji fallback (e.g. "📁", "🔍")
    Text(String),
}

/// An item displayed in the fuzzy finder (daemon internal UI type)
#[derive(Debug, Clone)]
pub struct SourceItem {
    /// Unique identifier within a session (e.g. original line index for dmenu items)
    pub id: Option<usize>,
    /// Display title (e.g. app name)
    pub title: String,
    /// Optional subtitle (e.g. path)
    pub subtitle: Option<String>,
    /// Data needed to execute this item (e.g. app path)
    pub exec_path: String,
    /// Which source this item came from
    pub source_name: String,
    /// Optional icon for display
    pub icon: Option<IconData>,
}
