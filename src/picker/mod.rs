pub mod item;
pub mod matcher;
mod render;
pub mod state;
mod terminal;
pub mod theme;

use std::collections::HashMap;

pub use item::{Item, Row};
pub use state::Mode;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};

use crate::error::Result;
use crate::picker::render::View;
use crate::picker::state::State;
use crate::picker::terminal::Screen;
use crate::repo::diff::PatchLine;

/// What the user did with the picker.
pub enum Selection<T> {
    Picked(Vec<T>),
    Cancelled,
}

impl<T> Selection<T> {
    pub fn into_picked(self) -> Option<Vec<T>> {
        match self {
            Selection::Picked(items) => Some(items),
            Selection::Cancelled => None,
        }
    }
}

/// Renders the pane shown beside the list for one item.
type PreviewFn<'a, T> = Box<dyn Fn(&T) -> Vec<PatchLine> + 'a>;

/// A picker, built up and then run.
///
/// ```ignore
/// let chosen = Prompt::new(files)
///     .prompt("add")
///     .multi()
///     .preview(|file| repo.file_patch(file).unwrap_or_default())
///     .run()?;
/// ```
pub struct Prompt<'a, T> {
    items: Vec<T>,
    prompt: String,
    mode: Mode,
    preview: Option<PreviewFn<'a, T>>,
}

impl<'a, T: Item> Prompt<'a, T> {
    pub fn new(items: Vec<T>) -> Self {
        Prompt {
            items,
            prompt: ">".into(),
            mode: Mode::Single,
            preview: None,
        }
    }

    pub fn prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = prompt.into();
        self
    }

    /// Allow marking several items.
    pub fn multi(mut self) -> Self {
        self.mode = Mode::Multi;
        self
    }

    /// Show a patch beside the list for whichever item the cursor is on.
    /// Results are kept, so moving back to an item does not recompute it.
    pub fn preview(mut self, render: impl Fn(&T) -> Vec<PatchLine> + 'a) -> Self {
        self.preview = Some(Box::new(render));
        self
    }

    pub fn run(self) -> Result<Selection<T>> {
        let Prompt {
            items,
            prompt,
            mode,
            preview,
        } = self;
        if items.is_empty() {
            return Ok(Selection::Cancelled);
        }

        let rows: Vec<Row> = items.iter().map(Item::row).collect();
        let labels = rows.iter().map(|row| row.label.clone()).collect();
        let mut state = State::new(labels, mode);
        let mut patches: HashMap<usize, Vec<PatchLine>> = HashMap::new();

        let mut screen = Screen::open()?;
        let flow = loop {
            let current = state.current();
            if let (Some(render), Some(index)) = (&preview, current) {
                patches
                    .entry(index)
                    .or_insert_with(|| render(&items[index]));
            }
            let patch = current.and_then(|i| patches.get(&i)).map(Vec::as_slice);

            let view = View {
                rows: &rows,
                preview: patch,
                prompt: &prompt,
            };
            screen
                .terminal()
                .draw(|frame| render::draw(frame, &mut state, &view))?;

            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    match handle_key(key, &mut state) {
                        Flow::Continue => {}
                        done => break done,
                    }
                }
                _ => {}
            }
        };

        let chosen = state.resolve_selection();
        // Give the terminal back before the caller prints anything.
        drop(screen);

        match flow {
            Flow::Accept if !chosen.is_empty() => {
                let mut pool: Vec<Option<T>> = items.into_iter().map(Some).collect();
                let picked = chosen.iter().filter_map(|i| pool[*i].take()).collect();
                Ok(Selection::Picked(picked))
            }
            _ => Ok(Selection::Cancelled),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Flow {
    Continue,
    Accept,
    Cancel,
}

/// Translate a key press into a change of state.
///
/// The bindings follow readline and fzf where they agree, so that muscle
/// memory carries over.
fn handle_key(key: KeyEvent, state: &mut State) -> Flow {
    let ctrl = key.modifiers.contains(event::KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(event::KeyModifiers::ALT);

    match key.code {
        KeyCode::Esc => return Flow::Cancel,
        KeyCode::Char('c' | 'g' | 'q') if ctrl => return Flow::Cancel,
        KeyCode::Enter => return Flow::Accept,

        KeyCode::Up => state.move_by(-1),
        KeyCode::Down => state.move_by(1),
        KeyCode::Char('p' | 'k') if ctrl => state.move_by(-1),
        KeyCode::Char('n' | 'j') if ctrl => state.move_by(1),
        KeyCode::PageUp => state.page(-1),
        KeyCode::PageDown => state.page(1),
        KeyCode::Home => state.move_to(0),
        KeyCode::End => state.move_to(usize::MAX),

        KeyCode::Tab => {
            state.toggle_mark();
            state.move_by(1);
        }
        KeyCode::BackTab => {
            state.toggle_mark();
            state.move_by(-1);
        }
        KeyCode::Char('a') if ctrl => state.toggle_all(),

        // Preview controls sit on alt, leaving ctrl free for movement.
        KeyCode::Char('k') if alt => state.scroll_preview(-1),
        KeyCode::Char('j') if alt => state.scroll_preview(1),
        KeyCode::Char('o') if ctrl => state.toggle_preview(),

        KeyCode::Backspace => state.pop_query(),
        KeyCode::Char('u') if ctrl => state.clear_query(),
        KeyCode::Char('w') if ctrl => state.pop_query_word(),
        KeyCode::Char(c) if !ctrl && !alt => state.push_query(c),
        _ => {}
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    fn state() -> State {
        let labels = ["alpha", "beta", "gamma"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut state = State::new(labels, Mode::Multi);
        state.set_viewport(3);
        state
    }

    fn press(code: KeyCode, modifiers: KeyModifiers, state: &mut State) -> Flow {
        handle_key(
            KeyEvent::new_with_kind(code, modifiers, KeyEventKind::Press),
            state,
        )
    }

    #[test]
    fn escape_and_ctrl_c_cancel() {
        let mut state = state();
        assert_eq!(
            press(KeyCode::Esc, KeyModifiers::NONE, &mut state),
            Flow::Cancel
        );
        assert_eq!(
            press(KeyCode::Char('c'), KeyModifiers::CONTROL, &mut state),
            Flow::Cancel
        );
    }

    #[test]
    fn typing_filters_and_ctrl_u_undoes_it() {
        let mut state = state();
        press(KeyCode::Char('b'), KeyModifiers::NONE, &mut state);
        assert_eq!(state.query(), "b");
        assert_eq!(state.matches().len(), 1);
        press(KeyCode::Char('u'), KeyModifiers::CONTROL, &mut state);
        assert_eq!(state.matches().len(), 3);
    }

    #[test]
    fn ctrl_letters_move_rather_than_type() {
        let mut state = state();
        press(KeyCode::Char('n'), KeyModifiers::CONTROL, &mut state);
        assert_eq!(state.current(), Some(1));
        assert_eq!(state.query(), "");
        press(KeyCode::Char('k'), KeyModifiers::CONTROL, &mut state);
        assert_eq!(state.current(), Some(0));
    }

    #[test]
    fn tab_marks_and_steps_down() {
        let mut state = state();
        press(KeyCode::Tab, KeyModifiers::NONE, &mut state);
        assert!(state.is_marked(0));
        assert_eq!(state.current(), Some(1));
    }

    #[test]
    fn end_goes_to_the_last_match() {
        let mut state = state();
        press(KeyCode::End, KeyModifiers::NONE, &mut state);
        assert_eq!(state.current(), Some(2));
    }

    #[test]
    fn alt_keys_scroll_the_preview_without_typing() {
        let mut state = state();
        press(KeyCode::Char('j'), KeyModifiers::ALT, &mut state);
        assert_eq!(state.preview_scroll(), 1);
        assert_eq!(state.query(), "");
    }
}
