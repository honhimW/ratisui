use crate::components::console_output::OutputKind::{ERR, STD};
use log::{info, warn};
use ratatui::layout::{Position, Rect};
use ratatui::prelude::Text;
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui_macros::{line, span};
use std::cmp;
use ratatui::text::Line;
use strum::Display;

pub struct ConsoleData<'a> {
    pub lines: Vec<(OutputKind, String)>,
    pub paragraph: Paragraph<'a>,
    pub position: Position,
    pub height: u16,
    pub weight: u16,
    pub total_lines: usize,
    is_bottom: bool,
}

#[derive(Debug, Display)]
pub enum OutputKind {
    STD,
    ERR,
}

impl ConsoleData<'_> {
    pub fn default() -> Self {
        Self {
            lines: vec![],
            paragraph: Paragraph::default(),
            position: Position::new(0, 0),
            height: 1,
            weight: 0,
            total_lines: 0,
            is_bottom: true,
        }
    }

    pub fn build_paragraph(&mut self) {
        info!("y: {}, max: {}, height: {}", self.position.y, self.max_offset(), self.height);
        self.is_bottom = self.position.y >= self.max_offset();
        let mut text = Text::default();
        for (kind, l) in self.lines.iter() {
            warn!("{}", kind);
            let new_line = match kind {
                STD => line![l.clone()],
                ERR => line![l.clone().red()],
            };
            warn!("{:?}", &new_line);
            text.push_line(new_line);
        }
        let mut paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
        paragraph = paragraph.scroll((self.position.y, self.position.x));
        self.paragraph = paragraph;
    }

    pub fn update(&mut self, area: &Rect) {
        let Rect { height, width, .. } = area;
        self.height = height.clone();
        self.weight = width.clone();
        if self.is_bottom {
            self.scroll_end();
        }
    }

    pub fn push(&mut self, line: impl Into<String>) {
        self.lines.push((STD, line.into()));
        self.total_lines = self.lines.len();
    }

    pub fn push_err(&mut self, line: impl Into<String>) {
        self.lines.push((ERR, line.into()));
        self.total_lines = self.lines.len();
    }

    pub fn extend(&mut self, lines: Vec<(OutputKind, String)>) {
        for (kind, line) in lines {
            match kind {
                STD => self.push(line),
                ERR => self.push_err(line),
            }
        }
    }

    fn max_offset(&self) -> u16 {
        (self.paragraph.line_count(self.weight) as u16).saturating_sub(self.height)
    }

    pub fn scroll_start(&mut self) {
        let mut position = self.position.clone();
        position.y = 0;
        self.position = position;
        let current = std::mem::replace(&mut self.paragraph, Paragraph::default());
        self.paragraph = current.scroll((
            position.y,
            position.x,
        ));
        self.is_bottom = self.position.y >= self.max_offset();
    }

    pub fn scroll_end(&mut self) {
        let mut position = self.position.clone();
        position.y = self.max_offset();
        self.position = position;
        let current = std::mem::replace(&mut self.paragraph, Paragraph::default());
        self.paragraph = current.scroll((
            position.y,
            position.x,
        ));
        self.is_bottom = self.position.y >= self.max_offset();
    }

    pub fn scroll_up(&mut self) {
        let mut position = self.position.clone();
        position.y = position.y.saturating_sub(1);
        self.position = position;
        let current = std::mem::replace(&mut self.paragraph, Paragraph::default());
        self.paragraph = current.scroll((
            position.y,
            position.x,
        ));
        self.is_bottom = self.position.y >= self.max_offset();
    }

    pub fn scroll_down(&mut self) {
        let mut position = self.position.clone();
        position.y = cmp::min(position.y.saturating_add(1), self.max_offset());
        self.position = position;
        let current = std::mem::replace(&mut self.paragraph, Paragraph::default());
        self.paragraph = current.scroll((
            position.y,
            position.x,
        ));
        self.is_bottom = self.position.y >= self.max_offset();
    }

    pub fn scroll_page_up(&mut self) {
        let mut position = self.position.clone();
        position.y = position.y.saturating_sub(self.height);
        self.position = position;
        let current = std::mem::replace(&mut self.paragraph, Paragraph::default());

        self.paragraph = current.scroll((
            position.y,
            position.x,
        ));
        self.is_bottom = self.position.y >= self.max_offset();
    }

    pub fn scroll_page_down(&mut self) {
        let mut position = self.position.clone();
        position.y = cmp::min(position.y.saturating_add(self.height), self.max_offset());
        self.position = position;
        let current = std::mem::replace(&mut self.paragraph, Paragraph::default());

        self.paragraph = current.scroll((
            position.y,
            position.x,
        ));
        self.is_bottom = self.position.y >= self.max_offset();
    }
}