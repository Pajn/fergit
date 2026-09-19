use std::fs::File;
use std::io::{self, IsTerminal};
use std::os::fd::AsFd;

use ratatui::Terminal;
use ratatui::backend::{Backend, ClearType, CrosstermBackend};
use ratatui::crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::layout::Position;
use ratatui::{TerminalOptions, Viewport};

use crate::error::{Error, Result};

/// The share of the terminal the picker is allowed to occupy.
const HEIGHT_RATIO: (u16, u16) = (4, 5);
const MIN_HEIGHT: u16 = 8;

/// The picker's hold on the terminal.
///
/// Drawing goes to the terminal device rather than to stdout, so the picker
/// still works when stdout is redirected, and an inline viewport is used rather
/// than the alternate screen so that scrollback survives.
///
/// Restoring is done on drop, which is what makes an early return or a panic
/// safe: there is no path out of the picker that leaves the terminal in raw
/// mode.
pub struct Screen {
    terminal: Terminal<CrosstermBackend<File>>,
}

impl Screen {
    pub fn open() -> Result<Self> {
        let tty = Self::terminal_device()?;
        let (_, rows) = ratatui::crossterm::terminal::size()?;
        let height = Self::height(rows);
        enable_raw_mode()?;
        let terminal = Terminal::with_options(
            CrosstermBackend::new(tty),
            TerminalOptions {
                viewport: Viewport::Inline(height),
            },
        )
        // Raw mode is on at this point, so it has to come back off by hand:
        // the guard that would normally do it does not exist yet.
        .inspect_err(|_| {
            let _ = disable_raw_mode();
        })?;
        Ok(Screen { terminal })
    }

    /// Where to draw.
    ///
    /// The terminal device is preferred, so the picker still works when stdout
    /// is redirected. Where there is no controlling terminal to open — inside
    /// some sandboxes and test harnesses — an inherited terminal on stderr or
    /// stdout will do.
    fn terminal_device() -> Result<File> {
        if let Ok(tty) = File::options().read(true).write(true).open("/dev/tty") {
            return Ok(tty);
        }
        for stream in [io::stderr().as_fd(), io::stdout().as_fd()] {
            if stream.is_terminal() {
                return Ok(File::from(stream.try_clone_to_owned()?));
            }
        }
        Err(Error::usage("fergit needs a terminal to show a picker"))
    }

    /// How tall to make the picker: most of the terminal, but always leaving
    /// the line the command was typed on, and never taller than the terminal
    /// however small that is.
    fn height(rows: u16) -> u16 {
        let most = rows.saturating_sub(1).max(1);
        let wanted = rows.saturating_mul(HEIGHT_RATIO.0) / HEIGHT_RATIO.1;
        wanted.max(MIN_HEIGHT).min(most)
    }

    pub fn terminal(&mut self) -> &mut Terminal<CrosstermBackend<File>> {
        &mut self.terminal
    }

    /// Wipe the viewport and leave the cursor where the picker began, so that
    /// whatever the command prints next takes its place.
    fn restore(&mut self) -> io::Result<()> {
        let top = self.terminal.get_frame().area().y;
        self.terminal.set_cursor_position(Position::new(0, top))?;
        self.terminal
            .backend_mut()
            .clear_region(ClearType::AfterCursor)?;
        self.terminal.show_cursor()?;
        disable_raw_mode()
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        // Nothing useful can be done if putting the terminal back fails, and
        // the command's own result is the more interesting news.
        let _ = self.restore();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_room_for_the_command_line() {
        assert_eq!(Screen::height(40), 32);
        assert_eq!(Screen::height(20), 16);
    }

    #[test]
    fn never_outgrows_a_small_terminal() {
        assert_eq!(Screen::height(10), 8);
        assert_eq!(Screen::height(4), 3);
        assert_eq!(Screen::height(1), 1);
    }
}
