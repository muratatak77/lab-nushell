//! Keep commands matching `$env.config.history.ignore_patterns` out of the history file.
//!
//! Reedline only knows how to exclude lines starting with a fixed prefix, so the regex based
//! filtering lives in [`IgnoringHistory`], a thin [`History`] wrapper that drops new entries
//! before they reach the real backend. The active patterns are held by a [`HistoryIgnore`]
//! handle that the REPL refreshes every prompt, so changing the config at runtime takes effect
//! on the next command without rebuilding the line editor.

use fancy_regex::Regex;
use log::warn;
use nu_protocol::engine::EngineState;
use reedline::{
    History, HistoryItem, HistoryItemId, HistorySessionId, Result as ReedlineResult, SearchQuery,
};
use std::sync::{Arc, PoisonError, RwLock};

/// The compiled form of the configured patterns, along with their sources so unchanged
/// configs can skip recompilation.
#[derive(Debug, Default)]
struct Patterns {
    sources: Vec<String>,
    regexes: Vec<Regex>,
}

/// Shared handle to the regexes deciding which commands are not saved to history.
///
/// Cloning the handle is cheap and every clone observes [`HistoryIgnore::update`].
#[derive(Clone, Debug, Default)]
pub(crate) struct HistoryIgnore {
    patterns: Arc<RwLock<Patterns>>,
}

impl HistoryIgnore {
    /// Create a handle with `sources` already compiled.
    pub(crate) fn new(engine_state: &EngineState, sources: &[String]) -> Self {
        let ignore = Self::default();
        ignore.update(engine_state, sources);
        ignore
    }

    /// Replace the active patterns with `sources`, unless they are unchanged.
    ///
    /// The config layer rejects invalid regexes, so a compile failure here is unexpected; such a
    /// pattern is logged and skipped rather than stopping the REPL.
    pub(crate) fn update(&self, engine_state: &EngineState, sources: &[String]) {
        if self
            .patterns
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .sources
            == sources
        {
            return;
        }

        let regexes = sources
            .iter()
            .filter_map(|pattern| match engine_state.get_cached_regex(pattern) {
                Ok(regex) => Some(regex),
                Err(err) => {
                    warn!("Ignoring invalid history ignore pattern '{pattern}': {err}");
                    None
                }
            })
            .collect();

        *self
            .patterns
            .write()
            .unwrap_or_else(PoisonError::into_inner) = Patterns {
            sources: sources.to_vec(),
            regexes,
        };
    }

    /// Whether `command_line` matches any of the active patterns.
    ///
    /// A pattern that fails while matching (e.g. by hitting the backtracking limit) is treated
    /// as not matching, so the command is kept rather than lost.
    pub(crate) fn is_ignored(&self, command_line: &str) -> bool {
        self.patterns
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .regexes
            .iter()
            .any(|regex| regex.is_match(command_line).unwrap_or(false))
    }

    /// Wrap `history` so new entries matching the active patterns are not saved.
    pub(crate) fn wrap(&self, history: Box<dyn History>) -> Box<dyn History> {
        Box::new(IgnoringHistory {
            inner: history,
            ignore: self.clone(),
        })
    }
}

/// A [`History`] backend that silently drops new entries matched by a [`HistoryIgnore`].
///
/// Dropped entries are returned without an id, which is how reedline's own backends report
/// "not stored" (e.g. for duplicate lines). Reedline then has no last command to attach
/// metadata such as the exit status to, so nothing about the command reaches the backend.
struct IgnoringHistory {
    inner: Box<dyn History>,
    ignore: HistoryIgnore,
}

impl History for IgnoringHistory {
    fn save(&mut self, h: HistoryItem) -> ReedlineResult<HistoryItem> {
        // Items with an id are updates of entries that were already accepted.
        if h.id.is_none() && self.ignore.is_ignored(&h.command_line) {
            return Ok(h);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use reedline::{FileBackedHistory, SearchDirection};

    fn patterns(sources: &[&str]) -> Vec<String> {
        sources.iter().map(|s| s.to_string()).collect()
    }

    fn saved_commands(history: &dyn History) -> Vec<String> {
        history
            .search(SearchQuery::everything(SearchDirection::Forward, None))
            .unwrap()
            .into_iter()
            .map(|item| item.command_line)
            .collect()
    }

    #[test]
    fn empty_patterns_ignore_nothing() {
        let ignore = HistoryIgnore::default();
        assert!(!ignore.is_ignored("ls"));
        assert!(!ignore.is_ignored(""));
    }

    #[test]
    fn matches_anywhere_unless_anchored() {
        let engine_state = EngineState::new();
        let ignore = HistoryIgnore::new(&engine_state, &patterns(&["^ls$", "secret"]));

        assert!(ignore.is_ignored("ls"));
        assert!(!ignore.is_ignored("ls -la"));
        assert!(ignore.is_ignored("let x = 'my secret'"));
        assert!(!ignore.is_ignored("cd /tmp"));
    }

    #[test]
    fn update_replaces_patterns_for_all_clones() {
        let engine_state = EngineState::new();
        let ignore = HistoryIgnore::new(&engine_state, &patterns(&["^ls"]));
        let clone = ignore.clone();

        ignore.update(&engine_state, &patterns(&["^cd"]));

        assert!(!clone.is_ignored("ls"));
        assert!(clone.is_ignored("cd /tmp"));
    }

    #[test]
    fn invalid_patterns_are_skipped() {
        let engine_state = EngineState::new();
        let ignore = HistoryIgnore::new(&engine_state, &patterns(&["(unclosed", "^exit$"]));

        assert!(ignore.is_ignored("exit"));
        assert!(!ignore.is_ignored("(unclosed"));
    }

    #[test]
    fn wrapped_history_drops_ignored_items() {
        let engine_state = EngineState::new();
        let ignore = HistoryIgnore::new(&engine_state, &patterns(&["(?i)password"]));
        let mut history = ignore.wrap(Box::new(FileBackedHistory::new(100).unwrap()));

        let kept = history.save(HistoryItem::from_command_line("ls")).unwrap();
        let dropped = history
            .save(HistoryItem::from_command_line("set PASSWORD hunter2"))
            .unwrap();

        assert!(kept.id.is_some());
        assert!(dropped.id.is_none());
        assert_eq!(saved_commands(history.as_ref()), vec!["ls".to_string()]);
    }

    #[test]
    fn wrapped_history_follows_runtime_updates() {
        let engine_state = EngineState::new();
        let ignore = HistoryIgnore::default();
        let mut history = ignore.wrap(Box::new(FileBackedHistory::new(100).unwrap()));

        history.save(HistoryItem::from_command_line("ls")).unwrap();
        ignore.update(&engine_state, &patterns(&["^ls$"]));
        history.save(HistoryItem::from_command_line("ls")).unwrap();
        history.save(HistoryItem::from_command_line("pwd")).unwrap();

        assert_eq!(
            saved_commands(history.as_ref()),
            vec!["ls".to_string(), "pwd".to_string()]
        );
    }
}
