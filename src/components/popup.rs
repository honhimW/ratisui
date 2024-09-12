use crate::app::Renderable;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::{Alignment, Widget};
use ratatui::style::Style;
use ratatui::symbols::border::Set;
use ratatui::text::Text;
use ratatui::widgets::block::Position;
use ratatui::widgets::{Block, Borders, Clear};
use ratatui::{symbols, Frame};
use std::fmt::{Display, Formatter};

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
        Clear::default().render(area, buf);
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
//         frame.render_widget(self.inner_widget.(), inner_area);
//         // self.inner_widget.render(inner_area, frame.buffer_mut());
//         frame.render_widget(popup_block, rect);
//         Ok(())
//     }
// }

pub struct RenderAblePopup<W: Renderable> {
    pub title_position: Position,
    pub title: Option<String>,
    pub inner_widget: Box<W>,
    pub style: Style,
    pub borders: Borders,
    pub border_set: Set,
    pub border_style: Style,
}

impl<W: Renderable> Display for RenderAblePopup<W> {
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

impl<W: Renderable> RenderAblePopup<W> {

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

impl<W: Renderable> Renderable for RenderAblePopup<W> {

    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> anyhow::Result<()> {
        frame.render_widget(Clear::default(), rect);
        let popup_block = Block::default()
            .title_position(self.title_position)
            .title(self.title.clone().unwrap_or_else(String::new))
            .title_alignment(Alignment::Center)
            .borders(self.borders)
            .border_style(self.border_style)
            .border_set(self.border_set)
            .style(self.style);
        let inner_area = popup_block.inner(rect);
        self.inner_widget.render_frame(frame, inner_area)?;
        frame.render_widget(popup_block, rect);
        Ok(())
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
//         frame.render_widget(self.inner_widget.(), inner_area);
//         // self.inner_widget.render(inner_area, frame.buffer_mut());
//         frame.render_widget(popup_block, rect);
//         Ok(())
//     }
// }

#[cfg(test)]
mod test {
    use crate::components::popup::Popup;
    use ratatui::symbols;
    use ratatui::widgets::block::Position;
    use tui_textarea::TextArea;

    #[test]
    fn test() {
        let mut text_area = TextArea::default();
        text_area.set_placeholder_text("");
        let popup = Popup::new(&text_area)
            .title_position(Position::Bottom)
            .title(String::from("String"))
            .border_set(symbols::border::DOUBLE);
        assert_eq!(popup.title, Some(String::from("String")));
        assert_eq!(popup.title_position, Position::Bottom);
        println!("{}", popup);
    }
}