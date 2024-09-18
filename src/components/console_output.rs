//! # [Ratatui] Table example
//!
//! The latest version of this example is available in the [examples] folder in the repository.
//!
//! Please note that the examples are designed to be run against the `main` branch of the Github
//! repository. This means that you may not be able to compile with the latest release version on
//! crates.io, or the one that you have installed locally.
//!
//! See the [examples readme] for more information on finding examples that match the version of the
//! library you are using.
//!
//! [Ratatui]: https://github.com/ratatui/ratatui
//! [examples]: https://github.com/ratatui/ratatui/blob/main/examples
//! [examples readme]: https://github.com/ratatui/ratatui/blob/main/examples/README.md

use crate::app::{Listenable, Renderable};
use crate::components::raw_value::raw_value_to_highlight_text;
use anyhow::Result;
use itertools::Itertools;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Constraint::{Length, Min};
use ratatui::{
    crossterm::event::{KeyCode, KeyEventKind},
    layout::Rect,
    style::{self, Style},
    text::{Line, Text},
    widgets::{
        Cell, Row, ScrollbarState
        , Table, TableState,
    }
    , Frame,
};
use std::borrow::Cow;
use std::cmp;
use log::info;
use ratatui::style::Color;
use unicode_width::UnicodeWidthStr;

const ITEM_HEIGHT: usize = 4;

pub struct Data {
    pub index: String,
    pub value: String,
    pub origin_value: String,
}

impl Data {
    const fn ref_array(&self) -> [&String; 1] {
        [&self.value]
    }

    fn index(&self) -> &str {
        &self.index
    }

    fn value(&self) -> &str {
        &self.value
    }

}

pub struct ConsoleData {
    item_values: Vec<String>,
    state: TableState,
    items: Vec<Data>,
    longest_item_lens: (u16),
    color_index: usize,
}

impl ConsoleData {
    pub fn new(data: Vec<String>) -> Self {
        let mut vec = vec![];
        for (idx, string) in data.iter().enumerate() {
            let data = Data {
                index: idx.to_string(),
                value: string.clone().replace("\n", "\\n"),
                origin_value: string.clone(),
            };
            vec.push(data);
        }
        Self {
            item_values: data,
            state: TableState::default().with_selected(0),
            longest_item_lens: constraint_len_calculator(&vec),
            color_index: 3,
            items: vec,
        }
    }

    pub fn next(&mut self, step: u16) {
        let i = match self.state.selected() {
            Some(i) => i.saturating_add(step as usize),
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self, step: u16) {
        let i = match self.state.selected() {
            Some(i) => i.saturating_sub(step as usize),
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn push_data(&mut self, data: Data) {
        self.items.push(data);
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        let rows = self.items.iter().enumerate().map(|(i, data)| {
            let height = data.origin_value.lines().count();
            let mut text = Text::default();
            let highlight_text = raw_value_to_highlight_text(Cow::from(&data.value), false);
            for line in highlight_text.lines {
                text.push_line(line);
            }
            let cell = Cell::from(text);
            let row = Row::new(vec![cell])
                .style(Style::default().bg(Color::Red))
                .height(height as u16);
            row
        }).collect_vec();
        let t = Table::new(rows, [Min(2)]);
        frame.render_stateful_widget(t, area, &mut self.state);
    }

}

impl Renderable for ConsoleData {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        self.render_table(frame, rect);
        Ok(())
    }

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        let mut elements = vec![];
        elements.push(("↑/j", "Up"));
        elements.push(("↓/k", "Down"));
        // elements.push(("←/h", "Close"));
        // elements.push(("→/l", "Open"));
        elements
    }
}

impl Listenable for ConsoleData {
    fn handle_key_event(&mut self, _key_event: KeyEvent) -> Result<bool> {
        if _key_event.kind == KeyEventKind::Press {
            let accepted = match _key_event.code {
                KeyCode::Esc => true,
                KeyCode::Char('j') | KeyCode::Down => {
                    self.next(1);
                    true
                },
                KeyCode::Char('k') | KeyCode::Up => {
                    self.previous(1);
                    true
                },
                KeyCode::Home => {
                    true
                }
                KeyCode::End => {
                    true
                }
                KeyCode::PageUp => {
                    true
                }
                KeyCode::PageDown => {
                    true
                }
                // KeyCode::Char('l') | KeyCode::Right => self.next_color(),
                // KeyCode::Char('h') | KeyCode::Left => self.previous_color(),
                _ => {false},
            };
            return Ok(accepted);
        }
        Ok(false)
    }
}

fn constraint_len_calculator(items: &[Data]) -> (u16) {
    let value_len = items
        .iter()
        .map(Data::value)
        .flat_map(str::lines)
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);

    #[allow(clippy::cast_possible_truncation)]
    (value_len as u16)
}