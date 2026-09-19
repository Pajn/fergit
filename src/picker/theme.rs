use ratatui::style::{Color, Modifier, Style};

use crate::repo::diff::LineKind;

pub fn dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

pub fn accent() -> Style {
    Style::default().fg(Color::Cyan)
}

pub fn marked() -> Style {
    Style::default().fg(Color::Yellow)
}

pub fn cursor_row() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

pub fn highlight() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

/// Status codes read as a traffic light: green for what is already staged,
/// red for what is not.
pub fn status_code(index: char, worktree: char) -> (Style, Style) {
    let staged = Style::default().fg(Color::Green);
    let unstaged = Style::default().fg(Color::Red);
    (
        if index == ' ' { dim() } else { staged },
        if worktree == ' ' { dim() } else { unstaged },
    )
}

pub fn patch_line(kind: LineKind) -> Style {
    match kind {
        LineKind::Addition => Style::default().fg(Color::Green),
        LineKind::Deletion => Style::default().fg(Color::Red),
        LineKind::Hunk => Style::default().fg(Color::Cyan),
        LineKind::FileHeader => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        LineKind::Meta => dim(),
        LineKind::Context => Style::default(),
    }
}
