use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::picker::item::Row;
use crate::picker::state::{Mode, State};
use crate::picker::theme;
use crate::repo::diff::PatchLine;

/// Below this width the preview would be too narrow to read, so the list gets
/// the whole viewport instead.
const MIN_WIDTH_FOR_PREVIEW: u16 = 80;
const LIST_SHARE: u16 = 40;

pub struct View<'a> {
    pub rows: &'a [Row],
    pub preview: Option<&'a [PatchLine]>,
    pub prompt: &'a str,
}

pub fn draw(frame: &mut Frame, state: &mut State, view: &View<'_>) {
    let [body, status, prompt] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    let show_preview =
        state.preview_open() && view.preview.is_some() && body.width >= MIN_WIDTH_FOR_PREVIEW;
    let (list_area, preview_area) = if show_preview {
        let [left, right] =
            Layout::horizontal([Constraint::Percentage(LIST_SHARE), Constraint::Min(1)])
                .areas(body);
        (left, Some(right))
    } else {
        (body, None)
    };

    state.set_viewport(list_area.height as usize);
    draw_list(frame, state, view.rows, list_area, show_preview);
    if let (Some(area), Some(lines)) = (preview_area, view.preview) {
        draw_preview(frame, lines, state.preview_scroll(), area);
    }
    draw_status(frame, state, status);
    draw_prompt(frame, state, view.prompt, prompt);
}

fn draw_list(frame: &mut Frame, state: &State, rows: &[Row], area: Rect, bordered: bool) {
    let inner = if bordered {
        let block = Block::new()
            .borders(ratatui::widgets::Borders::RIGHT)
            .border_style(theme::dim());
        let inner = block.inner(area);
        frame.render_widget(block, area);
        inner
    } else {
        area
    };

    let visible = state
        .matches()
        .iter()
        .enumerate()
        .skip(state.offset())
        .take(inner.height as usize);

    let lines: Vec<Line> = visible
        .map(|(position, matched)| {
            let row = &rows[matched.index];
            let on_cursor = position == state.cursor();
            let mut spans = vec![
                Span::styled(if on_cursor { "▌" } else { " " }, theme::accent()),
                Span::styled(
                    if state.is_marked(matched.index) {
                        "●"
                    } else {
                        " "
                    },
                    theme::marked(),
                ),
                Span::raw(" "),
            ];
            spans.extend(row.prefix.iter().map(|c| Span::styled(&c.text, c.style)));
            spans.extend(highlighted(&row.label, &matched.highlights, on_cursor));
            spans.extend(row.suffix.iter().map(|c| Span::styled(&c.text, c.style)));
            Line::from(spans)
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
}

/// Split a label so the characters the query matched can be picked out.
fn highlighted<'a>(label: &'a str, matches: &[u32], on_cursor: bool) -> Vec<Span<'a>> {
    let base = if on_cursor {
        theme::cursor_row()
    } else {
        Style::default()
    };
    if matches.is_empty() {
        return vec![Span::styled(label, base)];
    }

    let mut spans = Vec::new();
    let mut run = String::new();
    let mut run_is_match = false;
    for (index, ch) in label.chars().enumerate() {
        let is_match = matches.binary_search(&(index as u32)).is_ok();
        if is_match != run_is_match && !run.is_empty() {
            spans.push(styled_run(std::mem::take(&mut run), run_is_match, base));
        }
        run_is_match = is_match;
        run.push(ch);
    }
    if !run.is_empty() {
        spans.push(styled_run(run, run_is_match, base));
    }
    spans
}

fn styled_run<'a>(text: String, is_match: bool, base: Style) -> Span<'a> {
    if is_match {
        Span::styled(text, base.patch(theme::highlight()))
    } else {
        Span::styled(text, base)
    }
}

fn draw_preview(frame: &mut Frame, lines: &[PatchLine], scroll: u16, area: Rect) {
    let rendered: Vec<Line> = lines
        .iter()
        .skip(scroll as usize)
        .take(area.height as usize)
        .map(|line| Line::styled(line.text.clone(), theme::patch_line(line.kind)))
        .collect();
    frame.render_widget(Paragraph::new(rendered), area);
}

fn draw_status(frame: &mut Frame, state: &State, area: Rect) {
    let mut spans = vec![Span::styled(
        format!("  {}/{}", state.matches().len(), state.total()),
        theme::dim(),
    )];
    if state.mode() == Mode::Multi && state.marked_count() > 0 {
        spans.push(Span::styled(
            format!("  {} selected", state.marked_count()),
            theme::marked(),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_prompt(frame: &mut Frame, state: &State, prompt: &str, area: Rect) {
    let line = Line::from(vec![
        Span::styled(format!("{prompt} "), theme::accent()),
        Span::raw(state.query()),
    ]);
    frame.render_widget(Paragraph::new(line), area);

    let cursor_x = area.x + (prompt.chars().count() + 1 + state.query().chars().count()) as u16;
    if cursor_x < area.x + area.width {
        frame.set_cursor_position((cursor_x, area.y));
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::repo::diff::LineKind;

    fn rows(labels: &[&str]) -> Vec<Row> {
        labels
            .iter()
            .map(|label| Row::new(*label).prefix("M ", Style::default()))
            .collect()
    }

    fn screen(width: u16, height: u16, state: &mut State, view: &View<'_>) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, state, view)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol().to_string())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    fn state_for(labels: &[&str]) -> State {
        State::new(labels.iter().map(|s| s.to_string()).collect(), Mode::Multi)
    }

    #[test]
    fn puts_the_list_above_the_counts_above_the_prompt() {
        let labels = ["alpha.txt", "beta.txt"];
        let mut state = state_for(&labels);
        let rows = rows(&labels);
        let lines = screen(
            40,
            6,
            &mut state,
            &View {
                rows: &rows,
                preview: None,
                prompt: "add",
            },
        );

        assert!(lines[0].contains("alpha.txt"), "{lines:?}");
        assert!(lines[1].contains("beta.txt"), "{lines:?}");
        // Two rows of list, two blank, then the counts and the prompt.
        assert_eq!(lines[4].trim(), "2/2");
        assert_eq!(lines[5], "add");
    }

    #[test]
    fn marks_the_cursor_and_the_marked_rows() {
        let labels = ["alpha.txt", "beta.txt"];
        let mut state = state_for(&labels);
        state.set_viewport(2);
        state.toggle_mark();
        state.move_by(1);
        let rows = rows(&labels);
        let lines = screen(
            40,
            4,
            &mut state,
            &View {
                rows: &rows,
                preview: None,
                prompt: "add",
            },
        );

        assert!(lines[0].starts_with(" ●"), "marked row: {:?}", lines[0]);
        assert!(lines[1].starts_with("▌ "), "cursor row: {:?}", lines[1]);
        assert!(lines[2].contains("1 selected"), "{lines:?}");
    }

    #[test]
    fn shows_the_preview_beside_the_list_when_there_is_room() {
        let labels = ["alpha.txt"];
        let mut state = state_for(&labels);
        let rows = rows(&labels);
        let preview = vec![PatchLine {
            kind: LineKind::Addition,
            text: "+a new line".into(),
        }];
        let lines = screen(
            100,
            4,
            &mut state,
            &View {
                rows: &rows,
                preview: Some(&preview),
                prompt: "add",
            },
        );
        assert!(lines[0].contains("alpha.txt"));
        assert!(lines[0].contains("+a new line"), "{:?}", lines[0]);
    }

    #[test]
    fn a_narrow_terminal_gives_the_list_the_whole_width() {
        let labels = ["alpha.txt"];
        let mut state = state_for(&labels);
        let rows = rows(&labels);
        let preview = vec![PatchLine {
            kind: LineKind::Addition,
            text: "+a new line".into(),
        }];
        let lines = screen(
            50,
            4,
            &mut state,
            &View {
                rows: &rows,
                preview: Some(&preview),
                prompt: "add",
            },
        );
        assert!(!lines[0].contains("+a new line"), "{:?}", lines[0]);
    }

    #[test]
    fn typing_shows_the_query_after_the_prompt() {
        let labels = ["alpha.txt"];
        let mut state = state_for(&labels);
        state.push_query('a');
        let rows = rows(&labels);
        let lines = screen(
            40,
            4,
            &mut state,
            &View {
                rows: &rows,
                preview: None,
                prompt: "add",
            },
        );
        assert_eq!(lines[3], "add a");
    }
}
