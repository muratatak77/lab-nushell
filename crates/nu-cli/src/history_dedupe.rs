//! Keep a command out of the history file when it repeats the one saved right before it.

use reedline::{
    History, HistoryItem, HistoryItemId, HistorySessionId, Result as ReedlineResult, SearchQuery,
};

/// A [`History`] backend that drops a new entry equal to the last one it saved.
pub struct DedupingHistory {
    inner: Box<dyn History>,
    last: Option<String>,
}

impl DedupingHistory {
    /// Wrap `inner`, returning a backend that skips consecutive duplicates.
    pub fn new(inner: Box<dyn History>) -> Self {
        Self { inner, last: None }
    }
}

impl History for DedupingHistory {
    fn save(&mut self, h: HistoryItem) -> ReedlineResult<HistoryItem> {
        if h.id.is_none() && self.last.as_deref() == Some(h.command_line.as_str()) {
            return Ok(h);
        }
        self.last = Some(h.command_line.clone());
        self.inner.save(h)
    }

    fn load(&self, id: HistoryItemId) -> ReedlineResult<HistoryItem> {
        self.inner.load(id)
    }

    fn count(&self, query: SearchQuery) -> ReedlineResult<i64> {
        self.inner.count(query)
    }

    fn search(&self, query: SearchQuery) -> ReedlineResult<Vec<HistoryItem>> {
        self.inner.search(query)
    }

    fn update(
        &mut self,
        id: HistoryItemId,
        updater: &dyn Fn(HistoryItem) -> HistoryItem,
    ) -> ReedlineResult<()> {
        self.inner.update(id, updater)
    }

    fn clear(&mut self) -> ReedlineResult<()> {
        self.last = None;
        self.inner.clear()
    }

    fn delete(&mut self, h: HistoryItemId) -> ReedlineResult<()> {
        self.inner.delete(h)
    }

    fn sync(&mut self) -> std::io::Result<()> {
        self.inner.sync()
    }

    fn session(&self) -> Option<HistorySessionId> {
        self.inner.session()
    }
}
