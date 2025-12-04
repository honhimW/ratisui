use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::{Alignment, Widget};
use ratatui::style::Style;
use ratatui::symbols::border::Set;
use ratatui::text::Text;
use ratatui::widgets::block::Position;
use ratatui::widgets::{Block, Borders};
use ratatui::{symbols};
use std::fmt::{Display, Formatter};
use ratisui_core::utils::clear_buffer;

pub struct Popup<W: Widget> {
    pub title_position: Position,
    pub title: Option<String>,
    pub inner_widget: Box<W>,
    pub style: Style,
    pub borders: Borders,
    pub border_set: Set,
    pub border_style: Style,
}

impl<W: Widget> Display for Popup<W> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Popup")
            .field("title_position", &self.title_position)
            .field("title", &self.title)
            .field("style", &self.style)
            .field("borders", &self.borders)
            .field("border_set", &self.border_set)
            .field("border_style", &self.border_style)
            .finish()
    }
}

impl Default for Popup<Text<'_>> {
    fn default() -> Self {
        Self {
            title_position: Position::Top,
            title: None,
            inner_widget: Box::new(Text::default()),
            style: Style::default(),
            borders: Borders::ALL,
            border_set: symbols::border::ROUNDED,
            border_style: Style::default(),
        }
    }
}

#[allow(unused)]
impl<W: Widget> Popup<W> {

    pub fn new(widget: W) -> Self {
        Self {
            title_position: Position::Top,
            title: None,
            inner_widget: Box::new(widget),
            style: Style::default(),
            borders: Borders::ALL,
            border_set: symbols::border::ROUNDED,
            border_style: Style::default(),
        }
    }

    pub fn title_position(mut self, title_position: Position) -> Self {
        self.title_position = title_position;
        self
    }

    pub fn title(mut self, title: String) -> Self {
        self.title = Some(title);
        self
    }

    pub fn inner_widget(mut self, inner_widget: Box<W>) -> Self {
        self.inner_widget = inner_widget;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn borders(mut self, borders: Borders) -> Self {
        self.borders = borders;
        self
    }

    pub fn border_set(mut self, border_set: Set) -> Self {
        self.border_set = border_set;
        self
    }

    pub fn border_style(mut self, style: Style) -> Self {
        self.border_style = style;
        self
    }
}

impl<W: Widget> Widget for Popup<W> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        clear_buffer(buf, area);
        let popup_block = Block::default()
            .title_position(self.title_position)
            .title(self.title.unwrap_or_else(String::new))
            .title_alignment(Alignment::Center)
            .borders(self.borders)
            .border_style(self.border_style)
            .border_set(self.border_set)
            .style(self.style);
        let inner_area = popup_block.inner(area);
        self.inner_widget.render(inner_area, buf);
        popup_block.render(area, buf);
    }
}
