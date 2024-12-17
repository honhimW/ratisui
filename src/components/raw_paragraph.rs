use crate::components::raw_value::raw_value_to_highlight_text_with_content_type;
use anyhow::Result;
use ratatui::layout::{Position, Rect};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;
use std::borrow::Cow;
use std::cmp;
use crate::utils::ContentType;

pub struct RawParagraph<'a> {
    #[allow(unused)]
    pub raw: String,
    #[allow(unused)]
    pub content_type: Option<ContentType>,
    pub paragraph: Paragraph<'a>,
    pub position: Position,
    pub height: u16,
    pub weight: u16,
}

impl<'a> RawParagraph<'a> {
    pub fn new(raw: String, content_type: Option<ContentType>, format: bool) -> Self {
        let (text, _) = raw_value_to_highlight_text_with_content_type(Cow::from(raw.clone()), content_type.clone(), format);
        let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
        Self {
            raw,
            content_type,
            paragraph,
            position: Position::new(0, 0),
            height: 1,
            weight: 0,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        self.update(&area);
        frame.render_widget(&self.paragraph, area);
        Ok(())
    }

    fn update(&mut self, area: &Rect) {
        let Rect { height, width, .. } = area;
        self.height = height.clone();
        self.weight = width.clone();
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
    }

    pub fn scroll_up(&mut self) {
        let mut position = self.position.clone();
        position.y = position.y.saturating_sub(3);
        self.position = position;
        let current = std::mem::replace(&mut self.paragraph, Paragraph::default());
        self.paragraph = current.scroll((
            position.y,
            position.x,
        ));
    }

    pub fn scroll_down(&mut self) {
        let mut position = self.position.clone();
        position.y = cmp::min(position.y.saturating_add(3), self.max_offset());
        self.position = position;
        let current = std::mem::replace(&mut self.paragraph, Paragraph::default());
        self.paragraph = current.scroll((
            position.y,
            position.x,
        ));
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
    }

    fn max_offset(&self) -> u16 {
        (self.paragraph.line_count(self.weight) as u16).saturating_sub(self.height)
    }

}