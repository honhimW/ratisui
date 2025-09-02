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
use crate::components::TableColors;
use anyhow::Result;
use itertools::Itertools;
use ratatui::crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Constraint::{Length, Min};
use ratatui::{
    crossterm::event::{KeyCode, KeyEventKind},
    layout::{Margin, Rect},
    style::{Style, Stylize},
    text::{Line, Text},
    widgets::{
        Cell, HighlightSpacing, Row, Scrollbar, ScrollbarOrientation, ScrollbarState
        , Table, TableState,
    }
    , Frame,
};
use std::borrow::Cow;
use std::cmp;
use std::collections::HashMap;
use unicode_width::UnicodeWidthStr;
use ratisui_core::mouse::MouseEventHelper;

const ITEM_HEIGHT: usize = 4;

pub struct Data {
    pub index: String,
    pub key: String,
    pub value: String,
    pub origin_value: String,
}

impl Data {
    fn index(&self) -> &str {
        &self.index
    }

    fn key(&self) -> &str {
        &self.key
    }

    fn value(&self) -> &str {
        &self.value
    }

}

pub struct HashValue {
    state: TableState,
    items: Vec<Data>,
    longest_item_lens: (u16, u16, u16),
    scroll_state: ScrollbarState,
    colors: TableColors,
}

impl HashValue {
    pub fn new(data: HashMap<String, String>, offset: usize) -> Self {
        let mut vec = vec![];
        for (idx, (key, value)) in data.iter().enumerate() {
            let data = Data {
                index: idx.saturating_add(offset).to_string(),
                key: key.to_string(),
                value: value.clone().replace("\n", "\\n"),
                origin_value: value.clone(),
            };
            vec.push(data);
        }
        Self {
            state: TableState::default().with_selected(0),
            longest_item_lens: constraint_len_calculator(&vec),
            scroll_state: ScrollbarState::new((vec.len() - 1) * ITEM_HEIGHT),
            colors: TableColors::new(),
            items: vec,
        }
    }

    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i * ITEM_HEIGHT);
    }

    pub fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(0) => self.items.len() - 1,
            Some(i) => i - 1,
            None => 0,
        };
        self.state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i * ITEM_HEIGHT);
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        let header_style = Style::default()
            .bold()
            .fg(self.colors.header_fg)
            .bg(self.colors.header_bg);
        let selected_style = Style::default();

        let header = ["No.", "Key", "Value"]
            .into_iter()
            .map(|title| {
                Cell::from(Text::from(format!("\n{title}\n")))
            })
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style)
            .height(3)
            ;


        let selected_idx = self.state.selected().unwrap_or(0);
        let mut selected_height = 5;
        let rows = self.items.iter().enumerate().map(|(i, data)| {
            let color = match i % 2 {
                0 => self.colors.normal_row,
                _ => self.colors.alt_row,
            };
            let item;
            let height: u16;
            if selected_idx == i {
                item = [&data.index, &data.key, &data.origin_value];
                let lines_count = data.origin_value.lines().count();
                let max = cmp::min(lines_count, 20);
                height = cmp::max(max as u16, 5);
                selected_height = height.clone();
            } else {
                item = [&data.index, &data.key, &data.value];
                height = 3;
            }

            item.into_iter()
                .map(|content| {
                    let mut text = Text::default();
                    text.push_line(Line::default());
                    let (highlight_text, _) = raw_value_to_highlight_text(Cow::from(content), false);
                    for line in highlight_text.lines {
                        text.push_line(line);
                    }
                    text.push_line(Line::default());
                    Cell::from(text)
                })
                .collect::<Row>()
                .style(Style::new().fg(self.colors.row_fg).bg(color))
                .height(height)
        }).collect_vec();
        // let bar = " ➤ ";
        let bar = " █ ";
        let mut lines: Vec<Line> = vec![];
        lines.push("".into());
        for _ in 0..selected_height.saturating_sub(2) {
            lines.push(bar.into());
        }
        lines.push("".into());
        let highlight_symbol = Text::from(lines);
        let t = Table::new(
            rows,
            [
                // + 1 is for padding.
                Length(cmp::max(self.longest_item_lens.0, 3) + 1),
                Length(cmp::min(self.longest_item_lens.1, 20) + 1),
                // Min(self.longest_item_lens.2 + 1),
                Min(1 + 1),
            ],
        )
            .header(header)
            .row_highlight_style(selected_style)
            .highlight_symbol(highlight_symbol)
            .bg(self.colors.bg)
            .highlight_spacing(HighlightSpacing::Always);
        frame.render_stateful_widget(t, area, &mut self.state);
    }

    fn render_scrollbar(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None),
            area.inner(Margin {
                vertical: 0,
                horizontal: 0,
            }),
            &mut self.scroll_state,
        );
    }

}

impl Renderable for HashValue {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        self.render_table(frame, rect);
        self.render_scrollbar(frame, rect);
        Ok(())
    }

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        let mut elements = vec![];
        elements.push(("↓/j", "Down"));
        elements.push(("↑/k", "Up"));
        elements
    }
}

impl Listenable for HashValue {
    fn handle_key_event(&mut self, _key_event: KeyEvent) -> Result<bool> {
        if _key_event.kind == KeyEventKind::Press {
            let accepted = match _key_event.code {
                KeyCode::Esc => true,
                KeyCode::Char('j') | KeyCode::Down => {
                    self.next();
                    true
                },
                KeyCode::Char('k') | KeyCode::Up => {
                    self.previous();
                    true
                },
                _ => {false},
            };
            return Ok(accepted);
        }
        Ok(false)
    }

    fn handle_mouse_event(&mut self, mouse_event: MouseEvent) -> Result<bool> {
        if mouse_event.is_scroll_up() {
            self.previous();
            return Ok(true);
        }
        if mouse_event.is_scroll_down() {
            self.next();
            return Ok(true);
        }

        Ok(false)
    }
}

fn constraint_len_calculator(items: &[Data]) -> (u16, u16, u16) {
    let index_len = items
        .iter()
        .map(Data::index)
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);
    let key_len = items
        .iter()
        .map(Data::key)
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);
    let value_len = items
        .iter()
        .map(Data::value)
        .flat_map(str::lines)
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);

    #[allow(clippy::cast_possible_truncation)]
    (index_len as u16, key_len as u16, value_len as u16)
}
