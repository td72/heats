use std::sync::Arc;

use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config, Nucleo};

use crate::source::SourceItem;

/// Wrapper around nucleo for fuzzy matching
pub struct Matcher {
    nucleo: Nucleo<SourceItem>,
    last_query: String,
}

impl Matcher {
    pub fn new() -> Self {
        let nucleo = Nucleo::new(
            Config::DEFAULT,
            Arc::new(|| {}), // notify callback (unused; we tick on a timer)
            None,            // auto thread count
            1,               // single column
        );
        Self {
            nucleo,
            last_query: String::new(),
        }
    }

    /// Inject items into the matcher. Clears existing items first.
    pub fn set_items(&mut self, items: Vec<SourceItem>) {
        self.nucleo.restart(true);
        let injector = self.nucleo.injector();
        for item in items {
            let title = item.title.clone();
            injector.push(item, |_item, cols| {
                cols[0] = title.as_str().into();
            });
        }
    }

    /// Update the search query
    pub fn update_query(&mut self, query: &str) {
        if query == self.last_query {
            return;
        }
        let is_append = query.starts_with(&self.last_query) && !self.last_query.is_empty();
        self.nucleo.pattern.reparse(
            0,
            query,
            CaseMatching::Smart,
            Normalization::Smart,
            is_append,
        );
        self.last_query = query.to_string();
    }

    /// Tick the matcher, returning whether results changed
    pub fn tick(&mut self) -> bool {
        let status = self.nucleo.tick(10);
        status.changed
    }

    /// Get the current matched results (sorted by score, best first)
    pub fn results(&self, max: usize) -> Vec<SourceItem> {
        let snapshot = self.nucleo.snapshot();
        let count = (snapshot.matched_item_count() as usize).min(max);
        snapshot
            .matched_items(0..count as u32)
            .map(|item| item.data.clone())
            .collect()
    }

    /// Check if the query is empty (meaning all items should be shown)
    pub fn query_is_empty(&self) -> bool {
        self.last_query.is_empty()
    }
}
