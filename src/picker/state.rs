use std::collections::BTreeSet;

use crate::picker::matcher::{Ranked, Ranker};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Enter takes the item under the cursor.
    Single,
    /// Items can be marked, and Enter takes all of them.
    Multi,
}

/// Everything the picker knows, with no terminal attached.
///
/// The event loop turns key presses into calls on this type and the renderer
/// reads it back, which keeps the interesting behaviour — filtering, movement,
/// selection — testable on its own.
pub struct State {
    labels: Vec<String>,
    ranker: Ranker,
    ranked: Vec<Ranked>,
    query: String,
    /// Position in `ranked`, not in the original item list.
    cursor: usize,
    /// First visible row, adjusted to keep the cursor on screen.
    offset: usize,
    /// Original item indices the user marked.
    marked: BTreeSet<usize>,
    mode: Mode,
    viewport: usize,
    preview_open: bool,
    preview_scroll: u16,
}

impl State {
    pub fn new(labels: Vec<String>, mode: Mode) -> Self {
        let mut ranker = Ranker::new();
        let ranked = ranker.rank("", &labels);
        State {
            labels,
            ranker,
            ranked,
            query: String::new(),
            cursor: 0,
            offset: 0,
            marked: BTreeSet::new(),
            mode,
            viewport: 1,
            preview_open: true,
            preview_scroll: 0,
        }
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn matches(&self) -> &[Ranked] {
        &self.ranked
    }

    pub fn total(&self) -> usize {
        self.labels.len()
    }

    pub fn marked_count(&self) -> usize {
        self.marked.len()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn is_marked(&self, item: usize) -> bool {
        self.marked.contains(&item)
    }

    pub fn preview_open(&self) -> bool {
        self.preview_open
    }

    pub fn preview_scroll(&self) -> u16 {
        self.preview_scroll
    }

    /// The item the cursor is on, as an index into the original list.
    pub fn current(&self) -> Option<usize> {
        self.ranked.get(self.cursor).map(|r| r.index)
    }

    pub fn push_query(&mut self, c: char) {
        self.query.push(c);
        self.refilter();
    }

    pub fn pop_query(&mut self) {
        if self.query.pop().is_some() {
            self.refilter();
        }
    }

    pub fn clear_query(&mut self) {
        if !self.query.is_empty() {
            self.query.clear();
            self.refilter();
        }
    }

    /// Drop the last word, for the usual ctrl-w. A trailing separator belongs
    /// to the word being deleted, so that pressing it twice on `src/main`
    /// clears the query rather than stalling on `src/`.
    pub fn pop_query_word(&mut self) {
        let separator = |c: char| c.is_whitespace() || c == '/';
        let trimmed = self.query.trim_end_matches(separator);
        let cut = trimmed.rfind(separator).map_or(0, |i| i + 1);
        if cut < self.query.len() {
            self.query.truncate(cut);
            self.refilter();
        }
    }

    fn refilter(&mut self) {
        self.ranked = self.ranker.rank(&self.query, &self.labels);
        // Marks survive a change of query: they are held against the item, not
        // against its position in the filtered list.
        self.cursor = 0;
        self.offset = 0;
        self.preview_scroll = 0;
    }

    pub fn move_by(&mut self, delta: isize) {
        if self.ranked.is_empty() {
            return;
        }
        let last = self.ranked.len() - 1;
        let next = self.cursor as isize + delta;
        let next = next.clamp(0, last as isize) as usize;
        if next != self.cursor {
            self.cursor = next;
            self.preview_scroll = 0;
        }
        self.scroll_into_view();
    }

    pub fn move_to(&mut self, index: usize) {
        if self.ranked.is_empty() {
            return;
        }
        self.cursor = index.min(self.ranked.len() - 1);
        self.preview_scroll = 0;
        self.scroll_into_view();
    }

    pub fn page(&mut self, pages: isize) {
        self.move_by(pages * self.viewport.max(1) as isize);
    }

    /// Tell the state how many rows the list has, so scrolling can follow the
    /// cursor. Called by the renderer, which is the only part that knows.
    pub fn set_viewport(&mut self, rows: usize) {
        self.viewport = rows.max(1);
        self.scroll_into_view();
    }

    fn scroll_into_view(&mut self) {
        if self.cursor < self.offset {
            self.offset = self.cursor;
        } else if self.cursor >= self.offset + self.viewport {
            self.offset = self.cursor + 1 - self.viewport;
        }
    }

    pub fn toggle_mark(&mut self) {
        if self.mode == Mode::Single {
            return;
        }
        if let Some(item) = self.current()
            && !self.marked.insert(item)
        {
            self.marked.remove(&item);
        }
    }

    /// Mark everything currently matching, or clear the marks if they are all
    /// marked already.
    pub fn toggle_all(&mut self) {
        if self.mode == Mode::Single {
            return;
        }
        let visible: Vec<usize> = self.ranked.iter().map(|r| r.index).collect();
        if visible.iter().all(|i| self.marked.contains(i)) {
            for i in visible {
                self.marked.remove(&i);
            }
        } else {
            self.marked.extend(visible);
        }
    }

    pub fn scroll_preview(&mut self, delta: i16) {
        self.preview_scroll = self.preview_scroll.saturating_add_signed(delta);
    }

    pub fn toggle_preview(&mut self) {
        self.preview_open = !self.preview_open;
    }

    /// What Enter should act on: the marked items, or the one under the cursor
    /// when nothing is marked.
    pub fn resolve_selection(&self) -> Vec<usize> {
        if self.marked.is_empty() {
            return self.current().into_iter().collect();
        }
        self.marked.iter().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(items: &[&str], mode: Mode) -> State {
        let mut state = State::new(items.iter().map(|s| s.to_string()).collect(), mode);
        state.set_viewport(3);
        state
    }

    #[test]
    fn starts_on_the_first_item() {
        let state = state(&["a", "b", "c"], Mode::Multi);
        assert_eq!(state.current(), Some(0));
        assert_eq!(state.matches().len(), 3);
    }

    #[test]
    fn movement_stops_at_the_ends() {
        let mut state = state(&["a", "b", "c"], Mode::Multi);
        state.move_by(-1);
        assert_eq!(state.current(), Some(0));
        state.move_by(99);
        assert_eq!(state.current(), Some(2));
    }

    #[test]
    fn scrolls_to_keep_the_cursor_visible() {
        let mut state = state(&["a", "b", "c", "d", "e"], Mode::Multi);
        state.set_viewport(2);
        state.move_by(3);
        assert_eq!(state.cursor(), 3);
        assert_eq!(state.offset(), 2);
        state.move_by(-3);
        assert_eq!(state.offset(), 0);
    }

    #[test]
    fn enter_takes_the_cursor_when_nothing_is_marked() {
        let mut state = state(&["a", "b", "c"], Mode::Multi);
        state.move_by(1);
        assert_eq!(state.resolve_selection(), vec![1]);
    }

    #[test]
    fn enter_takes_the_marks_when_there_are_any() {
        let mut state = state(&["a", "b", "c"], Mode::Multi);
        state.move_by(2);
        state.toggle_mark();
        state.move_to(0);
        state.toggle_mark();
        assert_eq!(state.resolve_selection(), vec![0, 2]);
    }

    #[test]
    fn marking_twice_unmarks() {
        let mut state = state(&["a", "b"], Mode::Multi);
        state.toggle_mark();
        state.toggle_mark();
        assert_eq!(state.marked_count(), 0);
    }

    #[test]
    fn single_mode_ignores_marking() {
        let mut state = state(&["a", "b"], Mode::Single);
        state.toggle_mark();
        state.toggle_all();
        assert_eq!(state.marked_count(), 0);
        assert_eq!(state.resolve_selection(), vec![0]);
    }

    #[test]
    fn toggle_all_covers_only_what_matches() {
        let mut state = state(&["alpha", "beta", "alps"], Mode::Multi);
        for c in "alp".chars() {
            state.push_query(c);
        }
        state.toggle_all();
        assert_eq!(state.marked_count(), 2);
        assert!(state.is_marked(0));
        assert!(state.is_marked(2));
        assert!(!state.is_marked(1));
    }

    #[test]
    fn marks_survive_a_change_of_query() {
        let mut state = state(&["alpha", "beta"], Mode::Multi);
        state.toggle_mark();
        for c in "bet".chars() {
            state.push_query(c);
        }
        assert_eq!(state.matches().len(), 1);
        assert!(state.is_marked(0));
        assert_eq!(state.resolve_selection(), vec![0]);
    }

    #[test]
    fn filtering_moves_the_cursor_back_to_the_top() {
        let mut state = state(&["alpha", "beta", "gamma"], Mode::Multi);
        state.move_by(2);
        state.push_query('a');
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn nothing_matches_means_nothing_to_return() {
        let mut state = state(&["alpha"], Mode::Multi);
        for c in "zzz".chars() {
            state.push_query(c);
        }
        assert!(state.matches().is_empty());
        assert_eq!(state.current(), None);
        assert!(state.resolve_selection().is_empty());
    }

    #[test]
    fn ctrl_w_drops_a_path_segment() {
        let mut state = state(&["a"], Mode::Multi);
        for c in "src/main".chars() {
            state.push_query(c);
        }
        state.pop_query_word();
        assert_eq!(state.query(), "src/");
        state.pop_query_word();
        assert_eq!(state.query(), "");
    }
}
