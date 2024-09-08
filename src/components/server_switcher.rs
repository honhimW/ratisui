use ratatui::crossterm::event::KeyEvent;
use ratatui::{layout, Frame};
use ratatui::layout::{Layout, Rect};
use crate::configuration::Databases;
use tui_textarea::TextArea;
use crate::app::{AppEvent, Listenable, Renderable};
use anyhow::Result;
use ratatui::layout::Constraint::{Length, Min};
use crate::components::popup::Popup;
use crate::redis_opt::redis_operations;

pub struct Servers {
    databases: Databases,
    host_state: TextArea<'static>,
    port_state: TextArea<'static>,
    username_state: TextArea<'static>,
    password_state: TextArea<'static>,
    use_tls_state: TextArea<'static>,
    db_state: TextArea<'static>,
    protocol_state: TextArea<'static>,
}

impl Servers {
    pub fn new(databases: Databases) -> Self {
        Self {
            databases,
            host_state: Default::default(),
            port_state: Default::default(),
            username_state: Default::default(),
            password_state: Default::default(),
            use_tls_state: Default::default(),
            db_state: Default::default(),
            protocol_state: Default::default(),
        }
    }
}

impl Renderable for Servers {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        let layout = Layout::vertical([
            Length(1),
            Length(1),
            Length(1),
            Length(1),
            Length(1),
            Length(1),
        ]);
        Ok(())
    }

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        todo!()
    }
}

impl Listenable for Servers {
    fn handle_key_event(&mut self, _key_event: KeyEvent) -> Result<bool> {
        todo!()
    }
}