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

use std::cmp;
use anyhow::Result;
use itertools::Itertools;
use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    layout::{Constraint, Layout, Margin, Rect},
    style::{self, Color, Modifier, Style, Stylize},
    text::{Line, Text},
    widgets::{
        StatefulWidget,
        Block, BorderType, Cell, HighlightSpacing, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, TableState,
    },
    DefaultTerminal, Frame,
};
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Alignment;
use ratatui::widgets::{List, ListItem};
use style::palette::tailwind;
use tui_widget_list::{ListBuilder, ListState, ListView};
use unicode_width::UnicodeWidthStr;
use crate::app::{Listenable, Renderable};
use crate::components::list_row::ListRow;
use crate::components::raw_value::raw_value_to_highlight_text;

const PALETTES: [tailwind::Palette; 4] = [
    tailwind::BLUE,
    tailwind::EMERALD,
    tailwind::INDIGO,
    tailwind::RED,
];
const INFO_TEXT: &str =
    "(Esc) quit | (↑) move up | (↓) move down | (→) next color | (←) previous color";

const ITEM_HEIGHT: usize = 4;

struct TableColors {
    buffer_bg: Color,
    header_bg: Color,
    header_fg: Color,
    row_fg: Color,
    selected_style_bg: Color,
    normal_row_color: Color,
    alt_row_color: Color,
    footer_border_color: Color,
}

impl TableColors {
    fn new(color: &tailwind::Palette) -> Self {
        Self {
            buffer_bg: Color::default(),
            header_bg: color.c900,
            header_fg: color.c200,
            row_fg: color.c200,
            selected_style_bg: color.c900,
            normal_row_color: Color::default(),
            alt_row_color: color.c950,
            footer_border_color: color.c400,
        }
    }
}

pub struct Data {
    pub index: String,
    pub value: String,
}

impl Data {
    const fn ref_array(&self) -> [&String; 2] {
        [&self.index, &self.value]
    }

    fn index(&self) -> &str {
        &self.index
    }

    fn value(&self) -> &str {
        &self.value
    }

}

pub struct ListValue {
    item_values: Vec<String>,
    state: TableState,
    items: Vec<Data>,
    longest_item_lens: (u16, u16),
    scroll_state: ScrollbarState,
    colors: TableColors,
    color_index: usize,
}

impl ListValue {
    pub fn new(data: Vec<String>) -> Self {
        let mut vec = vec![];
        for (idx, string) in data.iter().enumerate() {
            let data = Data {
                index: idx.to_string(),
                value: string.clone().replace("\n", "\\n"),
            };
            vec.push(data);
        }
        Self {
            item_values: data,
            state: TableState::default().with_selected(0),
            longest_item_lens: constraint_len_calculator(&vec),
            scroll_state: ScrollbarState::new((vec.len() - 1) * ITEM_HEIGHT),
            colors: TableColors::new(&tailwind::GRAY),
            color_index: 3,
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
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i * ITEM_HEIGHT);
    }

    pub fn next_color(&mut self) {
        self.color_index = (self.color_index + 1) % PALETTES.len();
    }

    pub fn previous_color(&mut self) {
        let count = PALETTES.len();
        self.color_index = (self.color_index + count - 1) % count;
    }

    pub fn set_colors(&mut self) {
        self.colors = TableColors::new(&PALETTES[self.color_index]);
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        let header_style = Style::default()
            .bold()
            .fg(self.colors.header_fg)
            .bg(self.colors.header_bg);
        let selected_style = Style::default()
            // .add_modifier(Modifier::REVERSED)
            // .bg(self.colors.selected_style_bg)
            ;

        let header = ["Index", "Value"]
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
                0 => self.colors.normal_row_color,
                _ => self.colors.alt_row_color,
            };
            let item = data.ref_array();
            item.into_iter()
                .map(|content| Cell::from(raw_value_to_highlight_text(&content, false)))
                .collect::<Row>()
                .style(Style::new().fg(self.colors.row_fg).bg(color))
                .height(1)
        });
        let bar = " ➤ ";
        let t = Table::new(
            rows,
            [
                // + 1 is for padding.
                Constraint::Length(cmp::max(self.longest_item_lens.0, 5) + 1),
                Constraint::Min(self.longest_item_lens.1 + 1),
            ],
        )
            .header(header)
            .highlight_style(selected_style)
            .highlight_symbol(Text::from(vec![
                bar.into(),
                // bar.into(),
                // bar.into(),
                // "".into(),
            ]))
            .bg(self.colors.buffer_bg)
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
                vertical: 1,
                horizontal: 1,
            }),
            &mut self.scroll_state,
        );
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let info_footer = Paragraph::new(Line::from(INFO_TEXT))
            .style(
                Style::new()
                    .fg(self.colors.row_fg)
                    .bg(self.colors.buffer_bg),
            )
            .centered()
            .block(
                Block::bordered()
                    .border_type(BorderType::Double)
                    .border_style(Style::new().fg(self.colors.footer_border_color)),
            );
        frame.render_widget(info_footer, area);
    }

    fn render_list(&mut self, frame: &mut Frame, area: Rect) {
        ListRow
        let builder = ListBuilder::new(move |context| {
            let mut main_axis_size = 2;

            let mut container = containers[context.index].clone();

            if context.index % 2 == 0 {
                container.style = Style::default().bg(Color::Rgb(28, 28, 32));
            } else {
                container.style = Style::default().bg(Color::Rgb(0, 0, 0));
            }

            if context.is_selected {
                container.style = Style::default()
                    .bg(tailwind::ROSE.c300)
                    .fg(Color::Rgb(28, 28, 32));
                container.expand = true;
                main_axis_size = 3 + container.content.len() as u16;
            }

            (container, main_axis_size)
        });

        let item_count = 2;
        let list = ListView::new(builder, item_count);
        let state = &mut self.state;
        let mut l_state = ListState::default();

        list.render(area, frame.buffer_mut(), &mut l_state);
    }
}

impl Renderable for ListValue {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        self.render_list(frame, rect);

        // let vertical = &Layout::vertical([Constraint::Min(5), Constraint::Length(3)]);
        // let rects = vertical.split(rect);
        //
        // self.render_table(frame, rects[0]);
        // self.render_scrollbar(frame, rects[0]);
        // self.render_footer(frame, rects[1]);

        Ok(())
    }

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        let mut elements = vec![];
        elements.push(("Esc", "Escape"));
        elements.push(("↑/j", "Up"));
        elements.push(("↓/k", "Down"));
        // elements.push(("←/h", "Close"));
        // elements.push(("→/l", "Open"));
        elements
    }
}

impl Listenable for ListValue {
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

fn constraint_len_calculator(items: &[Data]) -> (u16, u16) {
    let index_len = items
        .iter()
        .map(Data::index)
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
    (index_len as u16, value_len as u16)
}

#[cfg(test)]
mod tests {
    use super::Data;

    #[test]
    fn constraint_len_calculator() {
        let test_data = vec![
            Data {
                index: "Emirhan Tala".to_string(),
                value: "Cambridgelaan 6XX\n3584 XX Utrecht".to_string(),
            },
            Data {
                index: "thistextis26characterslong".to_string(),
                value: "this line is 31 characters long\nbottom line is 33 characters long"
                    .to_string(),
            },
        ];
        let (longest_name_len, longest_address_len) =
            super::constraint_len_calculator(&test_data);

        assert_eq!(26, longest_name_len);
        assert_eq!(33, longest_address_len);
    }
}