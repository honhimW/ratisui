use std::sync::Arc;
use crate::context;
use crate::input::{Input, InputEvent};
use crate::tui;
use anyhow::{Context, Result};
use async_trait::async_trait;
use ratatui::crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::palette::tailwind;
use ratatui::text::Line;
use ratatui::Frame;
use tokio::sync::{Mutex, RwLock};

pub struct App {
    pub state: AppState,
    pub context: Arc<RwLock<context::Context>>,
    pub input: Input,
    debug_message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppState {
    Preparing,
    Running,
    Closing,
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppEvent {
    Init,
    Reset,
    Destroy,
}

#[async_trait]
pub trait TabImplementation : Renderable + Listenable + Send {
    fn palette(&self) -> tailwind::Palette;
    fn title(&self) -> Line<'static>;
}

#[async_trait]
pub trait Listenable {
    async fn handle_key_event(&mut self, _key_event: KeyEvent) -> Result<bool> {
        Ok(false)
    }

    async fn on_app_event(&mut self, _app_event: AppEvent) -> Result<()> {
        Ok(())
    }
}

pub trait Renderable {
    fn render_frame(&self, frame: &mut Frame, rect: Rect) -> Result<()>;

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        vec![]
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            context: Arc::new(RwLock::new(context::Context::new())),
            state: AppState::Preparing,
            input: Input::new(),
            debug_message: None,
       }
    }

    pub fn health(&self) -> bool {
        self.state == AppState::Running || self.state == AppState::Preparing
    }

    async fn handle_events(&mut self) -> Result<()> {
        // TODO using Select::new() and Select::select() instead
        let _event = self.input.receiver().recv()?;
        if let InputEvent::Input(e) = _event {
            if let Event::Key(key_event) = e {
                if key_event.kind == KeyEventKind::Press {
                    if key_event.modifiers == event::KeyModifiers::CONTROL && key_event.code == event::KeyCode::Char('c') {
                        self.state = AppState::Closing;
                    }
                    return self.context.write().await.handle_key_event(key_event)
                        .await
                        .map(|_| ())
                        .context(format!("handling key event failed: {key_event:#?}"));
                }
            }
        }
        Ok(())
    }

}

pub fn centered_rect(percentage_x: u16, percentage_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percentage_y) / 2),
            Constraint::Percentage(percentage_y),
            Constraint::Percentage((100 - percentage_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percentage_x) / 2),
            Constraint::Percentage(percentage_x),
            Constraint::Percentage((100 - percentage_x) / 2),
        ])
        .split(popup_layout[1])[1]
}