use crate::app::{Listenable, Renderable};
use crate::components::raw_value::raw_value_to_highlight_text_with_content_type;
use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Position, Rect};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;
use ratisui_core::marcos::KeyAsserter;
use ratisui_core::utils::ContentType;
use std::borrow::Cow;
use std::cmp;
use ratisui_core::bus::{publish_msg, Message};

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
        let (text, _) = raw_value_to_highlight_text_with_content_type(
            Cow::from(raw.clone()),
            content_type.clone(),
            format,
        );
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

    fn update(&mut self, area: &Rect) {
        let Rect { height, width, .. } = area;
        self.height = height.clone();
        self.weight = width.clone();
    }

    pub fn scroll_start(&mut self) {
        let mut position = self.position.clone();
        position.y = 0;
        self.position = position;
        let current = std::mem::take(&mut self.paragraph);
        self.paragraph = current.scroll((position.y, position.x));
    }

    pub fn scroll_end(&mut self) {
        let mut position = self.position.clone();
        position.y = self.max_offset();
        self.position = position;
        let current = std::mem::take(&mut self.paragraph);
        self.paragraph = current.scroll((position.y, position.x));
    }

    pub fn scroll_up(&mut self) {
        let mut position = self.position.clone();
        position.y = position.y.saturating_sub(3);
        self.position = position;
        let current = std::mem::take(&mut self.paragraph);
        self.paragraph = current.scroll((position.y, position.x));
    }

    pub fn scroll_down(&mut self) {
        let mut position = self.position.clone();
        position.y = cmp::min(position.y.saturating_add(3), self.max_offset());
        self.position = position;
        let current = std::mem::take(&mut self.paragraph);
        self.paragraph = current.scroll((position.y, position.x));
    }

    pub fn scroll_page_up(&mut self) {
        let mut position = self.position.clone();
        position.y = position.y.saturating_sub(self.height);
        self.position = position;
        let current = std::mem::take(&mut self.paragraph);

        self.paragraph = current.scroll((position.y, position.x));
    }

    pub fn scroll_page_down(&mut self) {
        let mut position = self.position.clone();
        position.y = cmp::min(position.y.saturating_add(self.height), self.max_offset());
        self.position = position;
        let current = std::mem::take(&mut self.paragraph);

        self.paragraph = current.scroll((position.y, position.x));
    }

    fn max_offset(&self) -> u16 {
        (self.paragraph.line_count(self.weight) as u16).saturating_sub(self.height)
    }
}

impl<'a> Listenable for RawParagraph<'a> {
    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        if key_event.is_c_y() {
            arboard::Clipboard::new()?.set_text(self.raw.clone())?;
            let _ = publish_msg(Message::info("Yanked to clipboard."));
            return Ok(true);
        }
        if key_event.modifiers == KeyModifiers::NONE {
            match key_event.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.scroll_down();
                    return Ok(true);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.scroll_up();
                    return Ok(true);
                }
                KeyCode::PageDown => {
                    self.scroll_page_down();
                    return Ok(true);
                }
                KeyCode::PageUp => {
                    self.scroll_page_up();
                    return Ok(true);
                }
                KeyCode::End => {
                    self.scroll_end();
                    return Ok(true);
                }
                KeyCode::Home => {
                    self.scroll_start();
                    return Ok(true);
                }
                _ => {}
            }
        }
        Ok(false)
    }
}

impl<'a> Renderable for RawParagraph<'a> {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        self.update(&rect);
        frame.render_widget(&self.paragraph, rect);
        Ok(())
    }

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        let mut elements = vec![];
        elements.push(("^y", "Yank"));
        elements.push(("↓/j", "Down"));
        elements.push(("↑/k", "Up"));
        elements
    }
}
