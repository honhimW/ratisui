use std::borrow::Cow;
use ratatui::layout::{Position, Rect};
use ratatui::prelude::Text;
use ratatui::style::{Style, Stylize};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui_macros::{line, span};
use std::cmp;
use strum::Display;
use OutputKind::{ERR, STD, CMD, Else, Raw};
use crate::components::raw_value::raw_value_to_highlight_text;
use ratisui_core::theme::get_color;

pub struct ConsoleData<'a> {
    pub lines: Vec<(OutputKind, String)>,
    pub paragraph: Paragraph<'a>,
    pub position: Position,
    pub height: u16,
    pub weight: u16,
    pub total_lines: usize,
    is_bottom: bool,
    max_offset: u16,
}

#[derive(Debug, Display)]
pub enum OutputKind {
    CMD,
    STD,
    ERR,
    Else(Style),
    Raw,
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
            max_offset: 0,
        }
    }

    pub fn build_paragraph(&mut self) {
        let mut text = Text::default();
        for (kind, l) in self.lines.iter() {
            let new_line = match kind {
                CMD => line![span!(Style::default().fg(get_color(|t| &t.tab.cli.console.cmd)); l.clone())],
                STD => line![span!(Style::default().fg(get_color(|t| &t.tab.cli.console.out)); l.clone())],
                ERR => line![span!(Style::default().fg(get_color(|t| &t.tab.cli.console.err)); l.clone())],
                Else(style) => line![span!(*style; l.clone())],
                Raw => line![],
            };
            text.push_line(new_line);
            if matches!(kind, Raw) {
                let (highlight_text, content_type) = raw_value_to_highlight_text(Cow::from(l.clone()), true);
                let ct = content_type.map(|ct| ct.to_string()).unwrap_or_default();
                text.push_line(line![span!(Style::default().dim(); format!("```{ct}"))]);
                for x in highlight_text.lines {
                    text.push_line(x)
                }
                text.push_line(line![span!(Style::default().dim(); "```")])
            }

        }
        let paragraph = Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .scroll((self.position.y, self.position.x));
        self.paragraph = paragraph;
    }

    pub fn update(&mut self, area: &Rect) {
        let Rect { height, width, .. } = area;
        self.height = height.clone();
        self.weight = width.clone();
        self.max_offset = (self.paragraph.line_count(self.weight) as u16).saturating_sub(self.height);
        if self.is_bottom {
            self.scroll_end();
        }
    }

    pub fn push(&mut self, kind: OutputKind, line: impl Into<String>) {
        self.lines.push((kind, line.into()));
        self.total_lines = self.lines.len();
    }

    pub fn push_std(&mut self, line: impl Into<String>) {
        self.push(STD, line.into());
    }

    pub fn push_err(&mut self, line: impl Into<String>) {
        self.push(ERR, line.into());
    }

    pub fn extend(&mut self, lines: Vec<(OutputKind, String)>) {
        for (kind, line) in lines {
            self.push(kind, line);
        }
    }

    pub fn scroll_start(&mut self) {
        let mut position = self.position.clone();
        position.y = 0;
        self.position = position;
        self.paragraph = self.paragraph.clone().scroll((position.y, position.x));
        self.is_bottom = false;
    }

    pub fn scroll_end(&mut self) {
        let mut position = self.position.clone();
        position.y = self.max_offset;
        self.position = position;
        self.paragraph = self.paragraph.clone().scroll((position.y, position.x));
        self.is_bottom = self.position.y >= self.max_offset;
    }

    pub fn scroll_up(&mut self) {
        let mut position = self.position.clone();
        position.y = position.y.saturating_sub(3);
        self.position = position;
        self.paragraph = self.paragraph.clone().scroll((position.y, position.x));
        self.is_bottom = false;
    }

    pub fn scroll_down(&mut self) {
        let mut position = self.position.clone();
        position.y = cmp::min(position.y.saturating_add(3), self.max_offset);
        self.position = position;
        self.paragraph = self.paragraph.clone().scroll((position.y, position.x));
        self.is_bottom = self.position.y >= self.max_offset;
    }

    pub fn scroll_page_up(&mut self) {
        let mut position = self.position.clone();
        position.y = position.y.saturating_sub(self.height);
        self.position = position;
        self.paragraph = self.paragraph.clone().scroll((position.y, position.x));
        self.is_bottom = self.position.y >= self.max_offset;
    }

    pub fn scroll_page_down(&mut self) {
        let mut position = self.position.clone();
        position.y = cmp::min(position.y.saturating_add(self.height), self.max_offset);
        self.position = position;
        self.paragraph = self.paragraph.clone().scroll((position.y, position.x));
        self.is_bottom = false;
    }
}