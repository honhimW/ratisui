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
use ratatui::layout::Constraint::{Fill, Length, Min};
use ratatui::{
    crossterm::event::{KeyCode, KeyEventKind},
    layout::{Margin, Rect},
    style::{self, Color, Style, Stylize},
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
use ratatui::layout::Layout;
use style::palette::tailwind;
use unicode_width::UnicodeWidthStr;

const ITEM_HEIGHT: usize = 4;

struct TableColors {
    buffer_bg: Color,
    header_bg: Color,
    header_fg: Color,
    row_fg: Color,
    normal_row_color: Color,
    alt_row_color: Color,
}

impl TableColors {
    fn new(color: &tailwind::Palette) -> Self {
        Self {
            buffer_bg: Color::default(),
            header_bg: color.c900,
            header_fg: color.c200,
            row_fg: color.c200,
            normal_row_color: Color::default(),
            alt_row_color: color.c950,
        }
    }
}

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
            colors: TableColors::new(&tailwind::GRAY),
        };
        Self {
            opt_for_id: true,
            items: vec,
            id_table: IdTable {
                state: TableState::default().with_selected(0),
                scroll_state: ScrollbarState::new((data.len() - 1) * ITEM_HEIGHT),
                colors: TableColors::new(&tailwind::GRAY),
            },
            entry_table: value_table,
        }
    }

    pub fn next(&mut self) {
        if self.opt_for_id {
            let i = match self.id_table.state.selected() {
                Some(i) => {
                    if i >= self.items.len() - 1 {
                        0
                    } else {
                        i + 1
                    }
                }
                None => 0,
            };
            self.id_table.state.select(Some(i));
            self.id_table.scroll_state = self.id_table.scroll_state.position(i * ITEM_HEIGHT);
        } else {
            let i = match self.entry_table.state.selected() {
                Some(i) => {
                    if i >= self.items.len() - 1 {
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

    pub fn previous(&mut self) {
        if self.opt_for_id {
            let i = match self.id_table.state.selected() {
                Some(i) => {
                    if i == 0 {
                        self.items.len() - 1
                    } else {
                        i - 1
                    }
                }
                None => 0,
            };
            self.id_table.state.select(Some(i));
            self.id_table.scroll_state = self.id_table.scroll_state.position(i * ITEM_HEIGHT);
        } else {
            let i = match self.entry_table.state.selected() {
                Some(i) => {
                    if i == 0 {
                        self.items.len() - 1
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

    fn render_id_table(&mut self, frame: &mut Frame, area: Rect) {
        let header_style = Style::default()
            .bold()
            .fg(self.id_table.colors.header_fg)
            .bg(self.id_table.colors.header_bg);
        let selected_style = Style::default()
            // .add_modifier(Modifier::REVERSED)
            // .bg(self.colors.selected_style_bg)
            ;

        let header = ["ID"]
            .into_iter()
            .map(|title| {
                Cell::from(Text::from(format!("\n{title}\n")))
            })
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style)
            .height(3)
            ;

        let rows = self.items.iter().enumerate().map(|(i, data)| {
            let color = match i % 2 {
                0 => self.id_table.colors.normal_row_color,
                _ => self.id_table.colors.alt_row_color,
            };
            let item = [&data.key];

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
                .style(Style::new().fg(self.id_table.colors.row_fg).bg(color))
                .height(1)
        }).collect_vec();
        let bar = " ➤ ";
        let highlight_symbol = Text::from(bar);
        let t = Table::new(
            rows,
            [
                Fill(1),
            ],
        )
            .header(header)
            .highlight_style(selected_style)
            .highlight_symbol(highlight_symbol)
            .bg(self.id_table.colors.buffer_bg)
            .highlight_spacing(HighlightSpacing::Always);
        frame.render_stateful_widget(t, area, &mut self.id_table.state);
    }

    fn render_scrollbar(&mut self, frame: &mut Frame, area: Rect) {
        if self.opt_for_id {
            frame.render_stateful_widget(
                Scrollbar::default()
                    .orientation(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .end_symbol(None),
                area.inner(Margin {
                    vertical: 0,
                    horizontal: 0,
                }),
                &mut self.id_table.scroll_state,
            );
        } else {
            frame.render_stateful_widget(
                Scrollbar::default()
                    .orientation(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .end_symbol(None),
                area.inner(Margin {
                    vertical: 0,
                    horizontal: 0,
                }),
                &mut self.entry_table.scroll_state,
            );
        }
    }

}

impl Renderable for SteamView {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        let horizontal = Layout::horizontal([Length(20), Fill(1)]).split(rect);
        let id_table_area = Layout::horizontal([Fill(1), Length(1)]).split(horizontal[1]);
        self.render_id_table(frame, id_table_area[0]);
        self.render_scrollbar(frame, id_table_area[1]);

        Ok(())
    }

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        let mut elements = vec![];
        elements.push(("↑/j", "Up"));
        elements.push(("↓/k", "Down"));
        elements.push(("←/h", "Close"));
        elements.push(("→/l", "Open"));
        elements
    }
}

impl Listenable for SteamView {
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
                // KeyCode::Char('l') | KeyCode::Right => self.next_color(),
                // KeyCode::Char('h') | KeyCode::Left => self.previous_color(),
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