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
use ratatui::{crossterm::event::{KeyCode, KeyEventKind}, layout::{Margin, Rect}, style::{self, Color, Style, Stylize}, symbols, text::{Line, Text}, widgets::{
    Cell, HighlightSpacing, Row, Scrollbar, ScrollbarOrientation, ScrollbarState
    , Table, TableState,
}, Frame};
use std::borrow::Cow;
use std::cmp;
use std::string::ToString;
use log::info;
use ratatui::buffer::Buffer;
use ratatui::widgets::{Block, BorderType, Borders, Clear, Widget};
use style::palette::tailwind;
use tui_textarea::TextArea;
use unicode_width::UnicodeWidthStr;
use crate::configuration::{Database, Databases, Protocol};
use crate::redis_opt::{redis_operations, switch_client};

const PALETTES: [tailwind::Palette; 4] = [
    tailwind::BLUE,
    tailwind::EMERALD,
    tailwind::INDIGO,
    tailwind::RED,
];

const ITEM_HEIGHT: usize = 4;

struct TableColors {
    buffer_bg: Color,
    header_fg: Color,
    header_bg: Color,
    selected_bg: Color,
    row_fg: Color,
}

impl TableColors {
    fn new(color: &tailwind::Palette) -> Self {
        Self {
            buffer_bg: Color::default(),
            header_fg: color.c200,
            header_bg: color.c900,
            selected_bg: color.c950,
            row_fg: color.c200,
        }
    }
}

pub struct Servers {
    host_state: TextArea<'static>,
    port_state: TextArea<'static>,
    username_state: TextArea<'static>,
    password_state: TextArea<'static>,
    use_tls_state: TextArea<'static>,
    db_state: TextArea<'static>,
    protocol_state: TextArea<'static>,
}

pub struct Data {
    pub selected: String,
    pub name: String,
    pub location: String,
    pub db: String,
    pub username: String,
    pub use_tls: String,
    pub protocol: String,
    pub database: Database,
}

impl Data {
    const fn ref_array(&self) -> [&String; 7] {
        [&self.selected, &self.name, &self.location, &self.db, &self.username, &self.use_tls, &self.protocol]
    }

    fn selected(&self) -> &str {
        &self.selected
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn location(&self) -> &str {
        &self.location
    }

    fn db(&self) -> &str {
        &self.db
    }

    fn username(&self) -> &str {
        &self.username
    }

    fn use_tls(&self) -> &str {
        &self.use_tls
    }

    fn protocol(&self) -> &str {
        &self.protocol
    }
}

pub struct ServerList {
    state: TableState,
    items: Vec<Data>,
    longest_item_lens: (u16, u16, u16, u16, u16, u16, u16),
    column_styles: [Style; 7],
    scroll_state: ScrollbarState,
    colors: TableColors,
    color_index: usize,
    servers: Servers,
}

impl ServerList {
    pub fn new(databases: &Databases) -> Self {
        let mut vec = vec![];
        let server_name = redis_operations().map(|ops| {ops.name}).unwrap_or("".to_string());
        for (name, database) in databases.databases.iter() {

            let data = Data {
                selected: if &server_name == name {
                    "*".to_string()
                } else {
                    "".to_string()
                },
                name: name.to_string(),
                location: format!("{}:{}", database.host, database.port),
                username: database.clone().username.unwrap_or(String::new()),
                use_tls: database.use_tls.to_string(),
                db: database.db.to_string(),
                protocol: database.protocol.to_string(),
                database: database.clone(),
            };
            vec.push(data);
        }
        vec.sort_by(|x, x1| {
            x.name.cmp(&x1.name)
        });
        Self {
            state: TableState::default().with_selected(0),
            longest_item_lens: constraint_len_calculator(&vec),
            column_styles: [
                Style::default(),
                Style::default().fg(tailwind::AMBER.c400),
                Style::default().fg(tailwind::CYAN.c500),
                Style::default().fg(tailwind::BLUE.c600),
                Style::default().fg(tailwind::AMBER.c400),
                Style::default().fg(tailwind::ROSE.c600),
                Style::default().fg(tailwind::EMERALD.c600),
            ],
            scroll_state: ScrollbarState::new((vec.len() - 1) * ITEM_HEIGHT),
            colors: TableColors::new(&tailwind::GRAY),
            color_index: 3,
            items: vec,
            servers: Servers {
                host_state: TextArea::default(),
                port_state: TextArea::default(),
                username_state: TextArea::default(),
                password_state: TextArea::default(),
                use_tls_state: TextArea::default(),
                db_state: TextArea::default(),
                protocol_state: TextArea::default(),
            },
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

    pub fn switch(&mut self) -> Result<()> {
        if let Some(selected) = self.state.selected() {
            let item = self.items.get(selected).clone();
            let mut selected_name: Option<String> = None;
            if let Some(data) = item {
                switch_client(data.name.clone(), &data.database)?;
                selected_name = Some(data.name.clone());
            }
            if let Some(name) = selected_name {
                info!("name: {}", name);
                self.items.iter_mut().for_each(|x| {
                    if x.name == name {
                        x.selected = "*".to_string();
                    } else {
                        x.selected = "".to_string();
                    }
                });
            }
        }
        Ok(())
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
            .bg(self.colors.selected_bg)
            ;

        let header = ["", "Name", "Location", "DB", "Username", "TLS", "Protocol"]
            .into_iter()
            .map(|title| {
                Cell::from(Text::raw(title))
            })
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style)
            .height(1)
            ;

        let rows = self.items.iter().enumerate().map(|(i, data)| {
            let item = data.ref_array();
            item.into_iter().enumerate()
                .map(|(idx, content)| {
                    Cell::from(Text::raw(content).style(self.column_styles[idx]))
                })
                .collect::<Row>()
                .style(Style::new().fg(self.colors.row_fg))
                .height(1)
        }).collect_vec();
        let bar = "➤ ";
        let t = Table::new(
            rows,
            [
                // + 1 is for padding.
                Length(self.longest_item_lens.0),
                Length(cmp::max(self.longest_item_lens.1, 4) + 1),
                Length(cmp::max(self.longest_item_lens.2, 8) + 1),
                Length(cmp::max(self.longest_item_lens.3, 2) + 1),
                Length(cmp::max(self.longest_item_lens.4, 8) + 1),
                Length(cmp::max(self.longest_item_lens.5, 3) + 1),
                Length(cmp::max(self.longest_item_lens.6, 8) + 1),
            ],
        )
            .header(header)
            .highlight_style(selected_style)
            .highlight_symbol(Text::raw(bar))
            .bg(self.colors.buffer_bg)
            .column_spacing(1)
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
}

impl Renderable for ServerList {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        frame.render_widget(Clear::default(), rect);
        let block = Block::bordered()
            .title("Servers")
            .border_set(symbols::border::DOUBLE)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            ;
        let inner_block = block.inner(rect);
        frame.render_widget(block, rect);
        self.render_table(frame, inner_block);
        self.render_scrollbar(frame, inner_block);
        Ok(())
    }

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        let mut elements = vec![];
        elements.push(("↑/j", "Up"));
        elements.push(("↓/k", "Down"));
        elements.push(("Enter", "Choose"));
        elements.push(("c", "Create"));
        elements.push(("e", "Edit"));
        elements.push(("Esc", "Close"));
        elements
    }
}

impl Listenable for ServerList {
    fn handle_key_event(&mut self, _key_event: KeyEvent) -> Result<bool> {
        if _key_event.kind == KeyEventKind::Press {
            let accepted = match _key_event.code {
                KeyCode::Esc => true,
                KeyCode::Char('j') | KeyCode::Down => {
                    self.next();
                    true
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.previous();
                    true
                }
                KeyCode::Enter => {
                    self.switch()?;
                    true
                },
                _ => { false }
            };
            return Ok(accepted);
        }
        Ok(false)
    }
}

fn constraint_len_calculator(items: &[Data]) -> (u16, u16, u16, u16, u16, u16, u16) {
    let name_len = items
        .iter()
        .map(Data::name)
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);
    let location_len = items
        .iter()
        .map(Data::location)
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);
    let db_len = items
        .iter()
        .map(Data::db)
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);
    let username_len = items
        .iter()
        .map(Data::username)
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);
    let use_tls_len = items
        .iter()
        .map(Data::use_tls)
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);
    let protocol_len = items
        .iter()
        .map(Data::protocol)
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);

    #[allow(clippy::cast_possible_truncation)]
    (
        1,
        name_len as u16, location_len as u16, db_len as u16,
        username_len as u16, use_tls_len as u16, protocol_len as u16,
    )
}