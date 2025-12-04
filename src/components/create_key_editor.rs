use crate::app::{Listenable, Renderable};
use ratisui_core::utils::{clean_text_area, clear_frame};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Constraint::{Fill, Length, Percentage};
use ratatui::layout::{Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType};
use ratatui::Frame;
use std::cmp;
use strum::{Display, EnumCount, EnumIter, IntoEnumIterator};
use tui_textarea::TextArea;
use ratisui_core::theme::get_color;

pub struct Form {
    title: String,
    editing_style: Style,
    editing: usize,
    key_type: usize,
    key_name_text_area: TextArea<'static>,
    ttl_text_area: TextArea<'static>,
}

#[derive(Default, Eq, PartialEq, EnumCount, EnumIter, Display)]
enum Editing {
    #[default]
    #[strum(serialize = "Type(*)")]
    Type,
    #[strum(serialize = "Name(*)")]
    Name,
    #[strum(serialize = "TTL")]
    TTL,
}

#[derive(Default, Eq, PartialEq, EnumCount, EnumIter, Display)]
pub enum KeyType {
    #[default]
    String,
    Hash,
    List,
    Set,
    #[strum(serialize = "Sorted Set")]
    SortedSet,
    Stream,
}

fn cursor_style() -> Style {
    Style::default().rapid_blink().reversed()
}

impl Default for Form {
    fn default() -> Self {
        let mut form = Self {
            title: String::from(""),
            editing_style: Style::default().fg(get_color(|t| &t.editor.editing)),
            editing: 0,
            key_type: 0,
            key_name_text_area: TextArea::default(),
            ttl_text_area: TextArea::default(),
        };
        form.key_name_text_area.set_placeholder_text("must not be blank");
        form.key_name_text_area.set_placeholder_style(Style::default().fg(get_color(|t| &t.editor.warning)).dim());
        form.key_name_text_area.set_cursor_style(Style::default());
        form.ttl_text_area.set_cursor_style(Style::default());
        form
    }
}

impl Form {

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn get_type(&self) -> KeyType {
        self.current_type()
    }

    pub fn get_name(&self) -> String {
        self.key_name_text_area.lines().get(0).cloned().unwrap_or(String::new())
    }

    pub fn get_ttl(&self) -> Option<u64> {
        self.ttl_text_area.lines().get(0).and_then(|s| s.parse::<u64>().ok())
    }

    pub fn next(&mut self) {
        let tmp = self.editing + 1;
        self.editing = tmp % Editing::COUNT;
        self.change_editing();
    }

    pub fn prev(&mut self) {
        let tmp = self.editing + (Editing::COUNT - 1);
        self.editing = tmp % Editing::COUNT;
        self.change_editing();
    }

    fn next_type(&mut self) {
        let tmp = self.key_type + 1;
        self.key_type = tmp % KeyType::COUNT;
        self.change_editing();
    }

    fn prev_type(&mut self) {
        let tmp = self.key_type + (KeyType::COUNT - 1);
        self.key_type = tmp % KeyType::COUNT;
        self.change_editing();
    }

    fn current(&self) -> Editing {
        Editing::iter().get(self.editing).unwrap()
    }

    fn current_type(&self) -> KeyType {
        KeyType::iter().get(self.key_type).unwrap()
    }

    fn span(&self, editing: Editing) -> Span<'static> {
        let title = editing.to_string();
        let style = if editing == self.current() {
            self.editing_style
        } else {
            Style::default()
        };
        Span::raw(title).style(style)
    }

    fn change_editing(&mut self) {
        self.key_name_text_area.set_cursor_style(Style::default());
        self.ttl_text_area.set_cursor_style(Style::default());
        self.key_name_text_area.cancel_selection();
        self.ttl_text_area.cancel_selection();

        match self.current() {
            Editing::Name => self.key_name_text_area.set_cursor_style(cursor_style()),
            Editing::TTL => self.ttl_text_area.set_cursor_style(cursor_style()),
            _ => {}
        }
    }

    fn render_key_type(&self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(18), Fill(0)]);
        let rc = horizontal.split(rect);
        let key = self.span(Editing::Type);
        let value = Span::raw(format!("◄ {} ►", self.current_type().to_string())).style(key.style);
        frame.render_widget(key, rc[0]);
        frame.render_widget(value, rc[1]);
    }

    fn render_key_name(&mut self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(18), Fill(0)]);
        let rc = horizontal.split(rect);
        let key = self.span(Editing::Name);
        self.key_name_text_area.set_style(key.style);
        let value = &self.key_name_text_area;
        frame.render_widget(key, rc[0]);
        frame.render_widget(value, rc[1]);
    }

    fn render_ttl(&mut self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(18), Fill(0)]);
        let rc = horizontal.split(rect);
        let key = self.span(Editing::TTL);
        self.ttl_text_area.set_style(key.style);
        let value = &self.ttl_text_area;
        frame.render_widget(key, rc[0]);
        frame.render_widget(value, rc[1]);
    }

}

impl Renderable for Form {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> anyhow::Result<()> {
        let height = 10;
        let blank_length = (rect.height - height) / 2;
        let area = Layout::vertical([Length(blank_length), Length(height), Length(blank_length)]).split(rect)[1];
        let area = Layout::horizontal([Percentage(20), Percentage(60), Percentage(20)]).split(area)[1];
        // let area = centered_rect(50, 70, rect);
        clear_frame(frame, area);
        let block = Block::bordered()
            .title(self.title.clone())
            .border_type(BorderType::Rounded);
        let block_inner_area = block
            .inner(area);
        let block_inner_area = Layout::horizontal([Length(1), Fill(0), Length(1)]).split(block_inner_area)[1];
        let vertical = Layout::vertical([
            Length(1), // type
            Length(1), // name
            Length(1), // ttl
        ]);
        let rc = vertical.split(block_inner_area);
        self.render_key_type(frame, rc[0]);
        self.render_key_name(frame, rc[1]);
        self.render_ttl(frame, rc[2]);
        frame.render_widget(block, area);
        Ok(())
    }

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        let mut elements = vec![];
        elements.push(("Tab", "Next"));
        elements.push(("BackTab", "Prev"));

        let editing = self.current();
        if editing == Editing::TTL {
            elements.push(("↑", "+1M"));
            elements.push(("↓", "-1M"));
            elements.push(("→", "+1S"));
            elements.push(("←", "-1S"));
        } else if editing == Editing::Type {
            elements.push(("1", "String"));
            elements.push(("2", "Hash"));
            elements.push(("3", "List"));
            elements.push(("4", "Set"));
            elements.push(("5", "ZSet"));
            elements.push(("6", "Stream"));
        }
        elements
    }
}

impl Listenable for Form {
    fn handle_key_event(&mut self, key_event: KeyEvent) -> anyhow::Result<bool> {
        let editing = self.current();
        match key_event.code {
            KeyCode::Tab => {
                self.next();
                return Ok(true);
            }
            KeyCode::BackTab => {
                self.prev();
                return Ok(true);
            }
            _ => {}
        }
        if key_event.kind != KeyEventKind::Press {
            return Ok(true);
        }
        let editor = match editing {
            Editing::Name => Some(&mut self.key_name_text_area),
            Editing::TTL => Some(&mut self.ttl_text_area),
            _ => None,
        };
        if let Some(text_area) = editor {
            match key_event {
                KeyEvent { code: KeyCode::Esc, .. } => {
                    return if text_area.is_selecting() {
                        text_area.cancel_selection();
                        Ok(true)
                    } else {
                        Ok(false)
                    }
                }
                KeyEvent { code: KeyCode::Enter, .. } => {}
                KeyEvent { code: KeyCode::Char('m'), modifiers: KeyModifiers::CONTROL, .. } => {}
                KeyEvent { code: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, .. } => text_area.select_all(),
                KeyEvent { code: KeyCode::Char('z'), modifiers: KeyModifiers::CONTROL, .. } => {
                    text_area.undo();
                }
                KeyEvent { code: KeyCode::Char('y'), modifiers: KeyModifiers::CONTROL, .. } => {
                    text_area.redo();
                }
                input => {
                    if editing == Editing::TTL {
                        if input.code == KeyCode::Backspace {
                            text_area.input(input);
                        } else {
                            let first_line = text_area.lines().get(0).clone().cloned();
                            if let Some(first_line) = first_line {
                                if let KeyCode::Char(c) = input.code {
                                    if c.is_numeric() {
                                        text_area.input(input);
                                        let first_line = text_area.lines().get(0).clone().cloned();
                                        if let Some(first_line) = first_line {
                                            if let Ok(ttl_second) = first_line.parse::<u64>() {
                                                let new_value = cmp::min(ttl_second, u64::MAX).to_string();
                                                clean_text_area(text_area);
                                                text_area.insert_str(new_value);
                                            }
                                        }
                                    }
                                } else {
                                    let ttl_second = first_line.parse::<u64>().unwrap_or(0);
                                    let new_port = match input.code {
                                        KeyCode::Char('h') | KeyCode::Left => ttl_second.saturating_sub(1).to_string(),
                                        KeyCode::Char('l') | KeyCode::Right => ttl_second.saturating_add(1).to_string(),
                                        KeyCode::Char('j') | KeyCode::Down => ttl_second.saturating_sub(60).to_string(),
                                        KeyCode::Char('k') | KeyCode::Up => ttl_second.saturating_add(60).to_string(),
                                        _ => first_line
                                    };
                                    clean_text_area(text_area);
                                    text_area.insert_str(new_port);
                                }
                            }
                        }
                    } else {
                        text_area.input(input);
                    }
                }
            }
            Ok(true)
        } else {
            match key_event.code {
                KeyCode::Char('h') | KeyCode::Left => {
                    if editing == Editing::Type {
                        self.prev_type();
                    }
                }
                KeyCode::Char('l') | KeyCode::Right => {
                    if editing == Editing::Type {
                        self.next_type();
                    }
                }
                KeyCode::Char('1') => self.key_type = 0,
                KeyCode::Char('2') => self.key_type = 1,
                KeyCode::Char('3') => self.key_type = 2,
                KeyCode::Char('4') => self.key_type = 3,
                KeyCode::Char('5') => self.key_type = 4,
                KeyCode::Char('6') => self.key_type = 5,
                _ => {}
            }
            Ok(true)
        }
    }
}
