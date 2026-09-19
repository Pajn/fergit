use ratatui::style::Style;

/// A run of text in a row, styled as a whole.
#[derive(Debug, Clone)]
pub struct Cell {
    pub text: String,
    pub style: Style,
}

impl Cell {
    pub fn new(text: impl Into<String>, style: Style) -> Self {
        Cell {
            text: text.into(),
            style,
        }
    }
}

/// One line in the list.
///
/// Only `label` is matched against the query and highlighted; the cells around
/// it are decoration, so a search for "main" cannot be satisfied by a status
/// code or a timestamp that happens to contain it.
#[derive(Debug, Clone)]
pub struct Row {
    pub prefix: Vec<Cell>,
    pub label: String,
    pub suffix: Vec<Cell>,
}

impl Row {
    pub fn new(label: impl Into<String>) -> Self {
        Row {
            prefix: Vec::new(),
            label: label.into(),
            suffix: Vec::new(),
        }
    }

    pub fn prefix(mut self, text: impl Into<String>, style: Style) -> Self {
        self.prefix.push(Cell::new(text, style));
        self
    }

    pub fn suffix(mut self, text: impl Into<String>, style: Style) -> Self {
        self.suffix.push(Cell::new(text, style));
        self
    }
}

/// Anything that can be offered in a picker.
pub trait Item {
    fn row(&self) -> Row;
}
