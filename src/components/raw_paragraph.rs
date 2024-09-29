use std::cmp;
use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::text::Text;
use anyhow::Result;
use ratatui::widgets::Paragraph;
use crate::components::highlight_value::ContentType;

pub struct RawParagraph<'a> {
    pub raw: String,
    pub content_type: Option<ContentType>,
    pub paragraph: Paragraph<'a>,
    pub position: Position,
}

impl<'a> RawParagraph<'a> {
    pub fn default() -> Self {
        Self {
            raw: String::new(),
            content_type: None,
            paragraph: Paragraph::default(),
            position: Position::new(0, 0),
        }
    }

    // pub fn render(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
    //     frame.render_widget(&self.paragraph, area);
    //     Ok(())
    // }
    //
    // pub fn scroll_up(&mut self) {
    //     let mut position = self.position.clone();
    //     position.y = position.y.saturating_sub(3);
    //     self.position = position;
    //     let current = std::mem::replace(&mut self.paragraph, Paragraph::default());
    //     self.paragraph = current.scroll((
    //         position.y,
    //         position.x,
    //     ));
    // }
    //
    // pub fn scroll_down(&mut self) {
    //     let mut position = self.position.clone();
    //     position.y = cmp::min(position.y.saturating_add(3), self.max_offset());
    //     self.position = position;
    //     let current = std::mem::replace(&mut self.paragraph, Paragraph::default());
    //     self.paragraph = current.scroll((
    //         position.y,
    //         position.x,
    //     ));
    // }
    //
    // // fn max_offset(&self) -> u16 {
    // //     (self.paragraph.line_count(self.weight) as u16).saturating_sub(self.height)
    // // }

}