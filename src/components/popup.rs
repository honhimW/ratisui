use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Style;
use ratatui::symbols::border::Set;
use ratatui::widgets::{Block, Borders, Clear, WidgetRef};
use ratatui::{symbols, Frame};
use ratatui::text::Text;
use crate::app::Renderable;

pub struct Popup<W: Widget> {
    pub title: Option<String>,
    pub inner_widget: Box<W>,
    pub style: Style,
    pub borders: Borders,
    pub border_set: Set,
    pub border_style: Style,
}

impl Default for Popup<Text<'_>> {
    fn default() -> Self {
        Self {
            title: None,
            inner_widget: Box::new(Text::default()),
            style: Style::default(),
            borders: Borders::ALL,
            border_set: symbols::border::ROUNDED,
            border_style: Style::default(),
        }
    }
}

impl<W: Widget> Popup<W> {

    pub fn new(widget: W) -> Self {
        Self {
            title: None,
            inner_widget: Box::new(widget),
            style: Style::default(),
            borders: Borders::ALL,
            border_set: symbols::border::ROUNDED,
            border_style: Style::default(),
        }
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
        Clear::default().render(area, buf);
        let popup_block = Block::default()
            .title(self.title.clone().unwrap_or_else(String::new))
            .borders(self.borders.clone())
            .border_style(self.border_style)
            .border_set(self.border_set)
            .style(self.style);
        let inner_area = popup_block.inner(area);
        self.inner_widget.render(inner_area, buf);
        popup_block.render(area, buf);
    }
}

// impl<W: Widget> WidgetRef for Popup<W> {
//     fn render_ref(&self, area: Rect, buf: &mut Buffer) {
//         Clear::default().render(area, buf);
//         let popup_block = Block::default()
//             .title(self.title.clone().unwrap_or_else(String::new))
//             .borders(self.borders.clone())
//             .border_style(self.border_style)
//             .border_set(self.border_set)
//             .style(self.style);
//         let inner_area = popup_block.inner(area);
//         self.inner_widget.render(inner_area, buf);
//         popup_block.render(area, buf);
//     }
// }
//
// impl<W: Widget> Renderable for Popup<W> {
//     fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> anyhow::Result<()> {
//         frame.render_widget(Clear::default(), rect);
//         let popup_block = Block::default()
//             .title(self.title.clone().unwrap_or_else(String::new))
//             .borders(self.borders.clone())
//             .border_style(self.border_style)
//             .border_set(self.border_set)
//             .style(self.style);
//         let inner_area = popup_block.inner(rect);
//         self.inner_widget.render(inner_area, frame.buffer_mut());
//         frame.render_widget(popup_block, rect);
//         Ok(())
//     }
// }