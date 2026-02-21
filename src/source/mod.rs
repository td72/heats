pub mod applications;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Icon data for a source item
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

/// An item returned by a Source
#[derive(Debug, Clone)]
pub struct SourceItem {
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

/// Trait for item sources (extensibility point)
pub trait Source: Send + Sync {
    /// Name of this source
    fn name(&self) -> &str;

    /// Optional prefix to activate this source (e.g. "=" for calc, ">" for shell)
    fn prefix(&self) -> Option<&str> {
        None
    }

    /// Load all items from this source
    fn load(&self) -> Pin<Box<dyn Future<Output = Vec<SourceItem>> + Send>>;

    /// Execute the given item
    fn execute(&self, item: &SourceItem) -> Result<(), Box<dyn std::error::Error>>;
}

/// Registry holding all active sources
pub struct SourceRegistry {
    sources: Vec<Box<dyn Source>>,
}

impl SourceRegistry {
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    pub fn register(&mut self, source: Box<dyn Source>) {
        self.sources.push(source);
    }

    pub fn sources(&self) -> &[Box<dyn Source>] {
        &self.sources
    }

    /// Load items from all sources
    pub async fn load_all(&self) -> Vec<SourceItem> {
        let mut all_items = Vec::new();
        for source in &self.sources {
            let items = source.load().await;
            all_items.extend(items);
        }
        all_items
    }

    /// Find the source that owns an item and execute it
    pub fn execute(&self, item: &SourceItem) -> Result<(), Box<dyn std::error::Error>> {
        for source in &self.sources {
            if source.name() == item.source_name {
                return source.execute(item);
            }
        }
        Err(format!("No source found for: {}", item.source_name).into())
    }
}
