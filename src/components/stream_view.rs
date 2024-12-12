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
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Constraint::{Fill, Length};
use ratatui::layout::Layout;
use ratatui::prelude::Line;
use ratatui::{
    crossterm::event::{KeyCode, KeyEventKind},
    layout::{Margin, Rect},
    style::{Style, Stylize},
    text::Text,
    widgets::{
        Cell, HighlightSpacing, Row, Scrollbar, ScrollbarOrientation, ScrollbarState
        , Table, TableState,
    }
    , Frame,
};
use std::borrow::Cow;
use std::cmp;
use unicode_width::UnicodeWidthStr;

const ITEM_HEIGHT: usize = 4;

pub struct Data {
    pub key: String,
    pub value: Vec<(String, String)>,
}

pub struct SteamView {
    opt_for_id: bool,
    items: Vec<Data>,
    id_table: IdTable,
    entry_table: EntryTable,
}

struct IdTable {
    state: TableState,
    scroll_state: ScrollbarState,
    colors: TableColors,
}

struct EntryTable {
    state: TableState,
    longest_item_lens: (u16, u16),
    scroll_state: ScrollbarState,
    colors: TableColors,
}

impl SteamView {
    pub fn new(data: Vec<(String, Vec<String>)>) -> Self {
        let mut vec = vec![];
        for (key, value) in data.iter() {
            let mut entries = vec![];
            let mut key_idx = 0;
            let mut value_idx = 1;
            loop {
                if value.len() <= value_idx {
                    break;
                }
                entries.push((value[key_idx].clone(), value[value_idx].clone()));
                key_idx += 2;
                value_idx += 2;
            }
            let data = Data {
                key: key.clone(),
                value: entries,
            };
            vec.push(data);
        }
        let entries = &vec[0].value;
        let value_table = EntryTable {
            state: TableState::default().with_selected(0),
            longest_item_lens: constraint_len_calculator(entries),
            scroll_state: ScrollbarState::new((entries.len() - 1) * ITEM_HEIGHT),
            colors: TableColors::new(),
        };
        Self {
            opt_for_id: true,
            items: vec,
            id_table: IdTable {
                state: TableState::default().with_selected(0),
                scroll_state: ScrollbarState::new((data.len() - 1) * ITEM_HEIGHT),
                colors: TableColors::new(),
            },
            entry_table: value_table,
        }
    }

    pub fn next(&mut self) {
        if self.opt_for_id {
            let i = self.id_table.state.selected().unwrap_or(0);
            let new_selected = if i >= self.items.len() - 1 {
                0
            } else {
                i + 1
            };
            if i != new_selected {
                self.id_table.state.select(Some(new_selected));
                self.id_table.scroll_state = self.id_table.scroll_state.position(new_selected * ITEM_HEIGHT);
                self.build_entry_table();
            }
        } else {
            let i = self.id_table.state.selected().unwrap_or(0);
            if let Some(data) = self.items.get(i) {
                let i = match self.entry_table.state.selected() {
                    Some(i) => {
                        if i >= data.value.len() - 1 {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                self.entry_table.state.select(Some(i));
                self.entry_table.scroll_state = self.entry_table.scroll_state.position(i * ITEM_HEIGHT);
            }
        }
    }

    pub fn previous(&mut self) {
        if self.opt_for_id {
            let i = self.id_table.state.selected().unwrap_or(0);
            let new_selected = if i == 0 {
                self.items.len() - 1
            } else {
                i - 1
            };
            if i != new_selected {
                self.id_table.state.select(Some(new_selected));
                self.id_table.scroll_state = self.id_table.scroll_state.position(new_selected * ITEM_HEIGHT);
                self.build_entry_table();
            }
        } else {
            let i = self.id_table.state.selected().unwrap_or(0);
            if let Some(data) = self.items.get(i) {
                let i = match self.entry_table.state.selected() {
                    Some(i) => {
                        if i == 0 {
                            data.value.len() - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.entry_table.state.select(Some(i));
                self.entry_table.scroll_state = self.entry_table.scroll_state.position(i * ITEM_HEIGHT);
            }
        }
    }

    fn build_entry_table(&mut self) {
        let i = self.id_table.state.selected().unwrap_or_default();
        if let Some(data) = self.items.get(i) {
            self.entry_table = EntryTable {
                state: TableState::default().with_selected(0),
                longest_item_lens: constraint_len_calculator(&data.value),
                scroll_state: ScrollbarState::new((data.value.len() - 1) * ITEM_HEIGHT),
                colors: TableColors::new(),
            }
        }
    }

    fn render_id_table(&mut self, frame: &mut Frame, area: Rect) {
        let header_style = Style::default()
            .bold()
            .fg(self.id_table.colors.header_fg)
            .bg(self.id_table.colors.header_bg);
        let selected_style = Style::default();

        let header = ["ID"]
            .into_iter()
            .map(|title| {
                Cell::from(Text::from(format!("{title}")))
            })
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style)
            .height(1)
            ;

        let rows = self.items.iter().enumerate().map(|(i, data)| {
            let color = match i % 2 {
                0 => self.id_table.colors.normal_row_color,
                _ => self.id_table.colors.alt_row_color,
            };
            let item = [&data.key];

            item.into_iter()
                .map(|content| Cell::from(Text::raw(content)))
                .collect::<Row>()
                .style(Style::new().fg(self.id_table.colors.row_fg).bg(color))
                .height(1)
        }).collect_vec();
        let bar = "➤ ";
        let highlight_symbol = Text::from(bar);
        let t = Table::new(
            rows,
            [
                Fill(1),
            ],
        )
            .header(header)
            .row_highlight_style(selected_style)
            .highlight_symbol(highlight_symbol)
            .bg(self.id_table.colors.buffer_bg)
            .highlight_spacing(HighlightSpacing::Always);
        frame.render_stateful_widget(t, area, &mut self.id_table.state);
    }

    fn render_id_scrollbar(&mut self, frame: &mut Frame, area: Rect) {
        let style = if self.opt_for_id {
            Style::default()
        } else {
            Style::default().dim()
        };
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .style(style),
            area.inner(Margin {
                vertical: 0,
                horizontal: 0,
            }),
            &mut self.id_table.scroll_state,
        );
    }

    fn render_entry_table(&mut self, frame: &mut Frame, area: Rect) {
        let header_style = Style::default()
            .bold()
            .fg(self.entry_table.colors.header_fg)
            .bg(self.entry_table.colors.header_bg);
        let selected_style = Style::default()
            ;

        let header = ["Field", "Value"]
            .into_iter()
            .map(|title| {
                Cell::from(Text::from(format!("{title}")))
            })
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style)
            .height(1)
            ;

        let selected_idx = self.entry_table.state.selected().unwrap_or(0);
        let mut selected_height = 5;

        let i = self.id_table.state.selected().unwrap_or_default();
        if let Some(data) = self.items.get(i) {
            let rows = data.value.iter().enumerate().map(|(i, (field, value))| {
                let color = match i % 2 {
                    0 => self.entry_table.colors.normal_row_color,
                    _ => self.entry_table.colors.alt_row_color,
                };
                let item;
                let height: u16;
                if selected_idx == i {
                    item = [field, value];
                    let lines_count = cmp::max(field.lines().count(), value.lines().count());
                    let max = cmp::min(lines_count, 20);
                    height = cmp::max(max as u16, 5);
                    selected_height = height.clone();
                } else {
                    item = [field, value];
                    height = 3;
                }

                item.into_iter()
                    .map(|content| {
                        let mut text = Text::default();
                        text.push_line(Line::default());
                        let highlight_text = raw_value_to_highlight_text(Cow::from(content), false);
                        for line in highlight_text.lines {
                            text.push_line(line);
                        }
                        text.push_line(Line::default());
                        Cell::from(text)
                    })
                    .collect::<Row>()
                    .style(Style::new().fg(self.entry_table.colors.row_fg).bg(color))
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
                    Length(cmp::max(self.entry_table.longest_item_lens.0, 5) + 1),
                    // Length(cmp::min(self.entry_table.longest_item_lens.1, 20) + 1),
                    Fill(1),
                ],
            )
                .header(header)
                .row_highlight_style(selected_style)
                .highlight_symbol(highlight_symbol)
                .bg(self.entry_table.colors.buffer_bg)
                .highlight_spacing(HighlightSpacing::Always);
            frame.render_stateful_widget(t, area, &mut self.entry_table.state);
        }

    }

    fn render_entry_scrollbar(&mut self, frame: &mut Frame, area: Rect) {
        let style = if !self.opt_for_id {
            Style::default()
        } else {
            Style::default().dim()
        };
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .style(style),
            area.inner(Margin {
                vertical: 0,
                horizontal: 0,
            }),
            &mut self.entry_table.scroll_state,
        );
    }
}

impl Renderable for SteamView {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        let id_length = if self.opt_for_id {
            29
        } else {
            19
        };
        let horizontal = Layout::horizontal([Length(id_length), Length(1), Fill(1)]).split(rect);
        self.render_id_table(frame, horizontal[0]);
        self.render_id_scrollbar(frame, horizontal[0]);
        self.render_entry_table(frame, horizontal[2]);
        self.render_entry_scrollbar(frame, horizontal[2]);
        Ok(())
    }

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        let mut elements = vec![];
        if self.opt_for_id {
            elements.push(("→/l", "Entry"));
        } else {
            elements.push(("←/h", "Id"));
        }
        elements.push(("↑/j", "Up"));
        elements.push(("↓/k", "Down"));
        elements
    }
}

impl Listenable for SteamView {
    fn handle_key_event(&mut self, _key_event: KeyEvent) -> Result<bool> {
        if _key_event.kind == KeyEventKind::Press {
            let accepted = match _key_event.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.next();
                    true
                },
                KeyCode::Char('k') | KeyCode::Up => {
                    self.previous();
                    true
                },
                KeyCode::Char('l') | KeyCode::Right => {
                    if self.opt_for_id {
                        self.opt_for_id = false;
                        true
                    } else {
                        false
                    }
                },
                KeyCode::Char('h') | KeyCode::Left => {
                    if !self.opt_for_id {
                        self.opt_for_id = true;
                        true
                    } else {
                        false
                    }
                },
                _ => {false},
            };
            return Ok(accepted);
        }
        Ok(false)
    }
}

fn constraint_len_calculator(items: &Vec<(String, String)>) -> (u16, u16) {
    let key_len = items
        .iter()
        .map(|(x, _)| x.lines().map(UnicodeWidthStr::width).max().unwrap_or(0))
        .max()
        .unwrap_or(0);
    let value_len = items
        .iter()
        .map(|(_, x)| x.lines().map(UnicodeWidthStr::width).max().unwrap_or(0))
        .max()
        .unwrap_or(0);

    #[allow(clippy::cast_possible_truncation)]
    (key_len as u16, value_len as u16)
}