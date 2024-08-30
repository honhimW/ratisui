use anyhow::Result;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use std::io::{stderr, Stderr, Write};

pub type TerminalBackEnd = Terminal<CrosstermBackend<Stderr>>;

pub fn init() -> Result<TerminalBackEnd> {
    enable_raw_mode()?;
    execute!(stderr(), EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stderr());
    let terminal = Terminal::new(backend)?;
    set_panic_hook();
    Ok(terminal)
}

pub fn restore() -> Result<()> {
    execute!(stderr(), LeaveAlternateScreen, DisableMouseCapture)?;
    disable_raw_mode()?;
    Ok(())
}

fn set_panic_hook() {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = restore();
        hook(panic_info);
    }));
}