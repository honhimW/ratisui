use std::borrow::Cow;
use ratatui::layout::{ Margin, Position, Rect};
use ratatui::prelude::Text;
use ratatui::style::{Style, Stylize};
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, WidgetRef, Wrap};
use ratatui_macros::{line, span};
use std::cmp;
use std::collections::VecDeque;
use ratatui::buffer::Buffer;
use ratatui::symbols::scrollbar::Set;
use strum::Display;
use OutputKind::{ERR, STD, CMD, Else, Raw};
use crate::components::raw_value::raw_value_to_highlight_text_with_content_type;
use ratisui_core::theme::get_color;
use ratisui_core::utils::ContentType;

pub struct ConsoleData<'a> {
    lines: VecDeque<(OutputKind, String)>,
    paragraph: Paragraph<'a>,
    position: Position,
    height: u16,
    weight: u16,
    paragraph_line_count: u16,
    total_lines: usize,
    is_bottom: bool,
    max_offset: u16,
    capacity: usize,
    scroll_state: ScrollbarState,
}

#[derive(Debug, Display)]
pub enum OutputKind {
    CMD,
    STD,
    ERR,
    Else(Style),
    Raw(Option<ContentType>),
}

impl WidgetRef for ConsoleData<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        self.paragraph.render_ref(area, buf);
        if area.height < self.paragraph_line_count {
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .symbols(Set {
                    track: " ",
                    thumb: "█",
                    begin: "↑",
                    end: "↓",
                })
                .begin_symbol(None)
                .end_symbol(None);
            scrollbar.render(area.inner(Margin {
                vertical: 0,
                horizontal: 0,
            }), buf, &mut self.scroll_state.clone());
        }
    }
}

impl ConsoleData<'_> {
    pub fn new(capacity: usize) -> Self {
        Self {
            lines: VecDeque::with_capacity(capacity),
            paragraph: Paragraph::default(),
            position: Position::new(0, 0),
            height: 1,
            weight: 0,
            paragraph_line_count: 1,
            total_lines: 0,
            is_bottom: true,
            max_offset: 0,
            capacity,
            scroll_state: ScrollbarState::default(),
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
                Raw(_) => line![],
            };
            text.push_line(new_line);
            if let Raw(content_type) = kind {
                let (highlight_text, content_type) = raw_value_to_highlight_text_with_content_type(Cow::from(l.clone()), content_type.clone(), true);
                let ct = content_type.map(|ct| match ct {
                    ContentType::JavaSerialized => format!("{} {}", ContentType::Ron, ContentType::JavaSerialized),
                    ContentType::Protobuf  => format!("{} {}", ContentType::Ron, ContentType::Protobuf),
                    _ => ct.to_string(),
                }).unwrap_or_default();
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
        self.paragraph_line_count = self.paragraph.line_count(self.weight) as u16;
        self.max_offset = self.paragraph_line_count.saturating_sub(self.height);
        self.scroll_state = self.scroll_state.content_length(self.max_offset as usize);
        if self.is_bottom {
            self.scroll_end();
        }
    }

    pub fn push(&mut self, kind: OutputKind, line: impl Into<String>) {
        if self.lines.len() == self.capacity {
            self.lines.pop_front();
        }
        self.lines.push_back((kind, line.into()));
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
        self.update_state(position);
        self.is_bottom = false;
    }

    pub fn scroll_end(&mut self) {
        let mut position = self.position.clone();
        position.y = self.max_offset;
        self.update_state(position);
        self.is_bottom = self.position.y >= self.max_offset;
    }

    pub fn scroll_up(&mut self) {
        let mut position = self.position.clone();
        position.y = position.y.saturating_sub(3);
        self.update_state(position);
        self.is_bottom = false;
    }

    pub fn scroll_down(&mut self) {
        let mut position = self.position.clone();
        position.y = cmp::min(position.y.saturating_add(3), self.max_offset);
        self.update_state(position);
        self.is_bottom = self.position.y >= self.max_offset;
    }

    pub fn scroll_page_up(&mut self) {
        let mut position = self.position.clone();
        position.y = position.y.saturating_sub(self.height);
        self.update_state(position);
        self.is_bottom = self.position.y >= self.max_offset;
    }

    pub fn scroll_page_down(&mut self) {
        let mut position = self.position.clone();
        position.y = cmp::min(position.y.saturating_add(self.height), self.max_offset);
        self.update_state(position);
        self.is_bottom = false;
    }
    
    pub fn line_count(&self, width: u16) -> usize {
        self.paragraph.line_count(width)
    }

    fn update_state(&mut self, position: Position) {
        self.position = position;
        self.paragraph = self.paragraph.clone().scroll((position.y, position.x));
        self.scroll_state = self.scroll_state.position(position.y as usize);
    }
}