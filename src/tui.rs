use anyhow::Result;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use std::io::{Stdout, stdout};

pub type TerminalBackEnd = Terminal<CrosstermBackend<Stdout>>;

#[derive(Clone)]
pub struct Tui {
    enable_mouse_capture: bool,
}

impl Tui {
    
    pub fn new(
        enable_mouse_capture: bool,
    ) -> Self {
        Self {
            enable_mouse_capture,
        }
    }
    
    pub fn init(&self) -> Result<TerminalBackEnd> {
        enable_raw_mode()?;
        if self.enable_mouse_capture {
            execute!(stdout(), EnterAlternateScreen, EnableMouseCapture,)?;
        } else {
            execute!(stdout(), EnterAlternateScreen,)?;
        }
        let backend = CrosstermBackend::new(stdout());
        let terminal = Terminal::new(backend)?;
        set_panic_hook(self.clone());
        Ok(terminal)
    }

    pub fn restore(&self) -> Result<()> {
        if self.enable_mouse_capture {
            execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture,)?;
        } else {
            execute!(stdout(), LeaveAlternateScreen,)?;
        }
        disable_raw_mode()?;
        Ok(())
    }
}

fn set_panic_hook(tui: Tui) {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = tui.restore();
        hook(panic_info);
    }));
}
