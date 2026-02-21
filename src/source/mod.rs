pub mod applications;
pub mod windows;

use std::sync::Arc;

use applications::ApplicationsSource;
use windows::WindowsSource;

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
    fn load(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<SourceItem>> + Send>>;

    /// Execute the given item
    fn execute(&self, item: &SourceItem) -> Result<(), Box<dyn std::error::Error>>;
}

type SourceFactory = fn() -> Box<dyn Source>;

/// Registry holding all active sources (factory-based)
pub struct SourceRegistry {
    factories: Vec<(&'static str, SourceFactory)>,
}

impl Default for SourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceRegistry {
    /// Create a registry with all sources registered
    pub fn new() -> Self {
        let mut reg = Self {
            factories: Vec::new(),
        };
        reg.register("applications", || Box::new(ApplicationsSource::new()));
        reg.register("windows", || Box::new(WindowsSource::new()));
        reg
    }

    fn register(&mut self, name: &'static str, factory: SourceFactory) {
        self.factories.push((name, factory));
    }

    /// Load items from sources. Pass `None` for all sources, or `Some(names)` to filter.
    pub async fn load_items(filter: Option<Vec<String>>) -> Vec<SourceItem> {
        let registry = Self::new();
        let sources: Vec<Box<dyn Source>> = match &filter {
            Some(names) => registry
                .factories
                .iter()
                .filter(|(n, _)| names.iter().any(|f| f == n))
                .map(|(_, f)| f())
                .collect(),
            None => registry.factories.iter().map(|(_, f)| f()).collect(),
        };

        let mut set = tokio::task::JoinSet::new();
        for source in sources {
            set.spawn(async move { source.load().await });
        }

        let mut items = Vec::new();
        while let Some(Ok(result)) = set.join_next().await {
            items.extend(result);
        }
        items
    }

    /// Find the source that owns an item and execute it
    pub fn execute(&self, item: &SourceItem) -> Result<(), Box<dyn std::error::Error>> {
        for (name, factory) in &self.factories {
            if *name == item.source_name {
                return factory().execute(item);
            }
        }
        Err(format!("No source found for: {}", item.source_name).into())
    }
}
