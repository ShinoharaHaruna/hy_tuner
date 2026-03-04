use anyhow::Result;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io;

use crate::ui::app::App;
use crate::ui::layout::render_ui;

pub fn run_tui() -> Result<()> {
    let mut app = App::new();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    loop {
        terminal.draw(|f| {
            render_ui(
                f,
                &app.state,
                &app.fields,
                app.selected,
                app.focus,
                &app.logs,
                app.log_scroll,
            );
        })?;

        if !app.run_event_loop()? {
            break;
        }
    }

    disable_raw_mode()?;
    crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen)?;
    Ok(())
}
