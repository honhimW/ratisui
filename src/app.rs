use crate::context;
use ratisui_core::input::Input;
use anyhow::{Result};
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::Line;
use ratatui::Frame;
use ratatui::style::Color;
use ratisui_core::bus::GlobalEvent;
use ratisui_core::configuration::{Configuration, Databases};

pub struct App {
    pub state: AppState,
    pub context: context::Context,
    pub input: Input,
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
    InitConfig(Configuration),
    Reset,
    Destroy,
    Bus(GlobalEvent),
}

pub trait TabImplementation: Renderable + Listenable {
    fn highlight(&self) -> Color;
    fn title(&self) -> Line<'static>;
}

pub trait Listenable {
    fn handle_key_event(&mut self, _key_event: KeyEvent) -> Result<bool> {
        Ok(false)
    }

    fn on_app_event(&mut self, _app_event: AppEvent) -> Result<()> {
        Ok(())
    }
}

pub trait Renderable {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> Result<()>;

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        vec![]
    }
}

impl App {
    pub fn new(databases: Databases) -> Self {
        Self {
            context: context::Context::new(databases),
            state: AppState::Preparing,
            input: Input::new(),
        }
    }

    pub fn health(&self) -> bool {
        self.state == AppState::Running || self.state == AppState::Preparing
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