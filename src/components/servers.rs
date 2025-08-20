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

use crate::app::{AppEvent, Listenable, Renderable, centered_rect};
use crate::components::TableColors;
use crate::components::database_editor::Form;
use crate::components::popup::Popup;
use anyhow::{Result, anyhow};
use itertools::Itertools;
use log::info;
use ratatui::crossterm::event::{KeyEvent, KeyModifiers, MouseEvent};
use ratatui::layout::Alignment;
use ratatui::layout::Constraint::Length;
use ratatui::widgets::block::Position;
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEventKind},
    layout::{Margin, Rect},
    style::{Style, Stylize},
    symbols,
    text::{Line, Text},
    widgets::{
        Cell, HighlightSpacing, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table,
        TableState,
    },
};
use ratisui_core::bus::{Message, publish_msg};
use ratisui_core::configuration::{Database, Databases, save_database_configuration};
use ratisui_core::redis_opt::switch_client;
use ratisui_core::theme::get_color;
use std::cmp;
use std::string::ToString;
use unicode_width::UnicodeWidthStr;
use ratisui_core::marcos::KeyAsserter;
use ratisui_core::mouse::MouseEventHelper;

const ITEM_HEIGHT: usize = 4;

#[derive(Clone)]
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
        [
            &self.selected,
            &self.name,
            &self.location,
            &self.db,
            &self.username,
            &self.use_tls,
            &self.protocol,
        ]
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
    init_database_name: Option<String>,
    have_changed: bool,
    show_delete_popup: bool,
    show_create_popup: bool,
    show_edit_popup: bool,
    state: TableState,
    items: Vec<Data>,
    longest_item_lens: (u16, u16, u16, u16, u16, u16, u16),
    column_styles: [Style; 7],
    scroll_state: ScrollbarState,
    colors: TableColors,
    create_form: Form,
    edit_form: Form,
    save_on_exit: bool,

    list_rect: Rect,
}

impl ServerList {
    pub fn new(databases: &Databases) -> Self {
        let mut vec = vec![];
        for (name, database) in databases.databases.iter() {
            let mut selected = false;
            if let Some(ref default) = databases.default_database {
                if name == default {
                    selected = true
                }
            }
            let data = Data {
                selected: if selected { "*" } else { "" }.into(),
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
        vec.sort_by(|x, x1| x.name.cmp(&x1.name));
        let default_selected = vec
            .iter()
            .position(|data| data.selected == "*")
            .unwrap_or(0);
        let init_database_name = vec.get(default_selected).map(|data| data.name.clone());
        Self {
            init_database_name,
            have_changed: false,
            show_delete_popup: false,
            show_create_popup: false,
            show_edit_popup: false,
            state: TableState::default().with_selected(default_selected),
            longest_item_lens: constraint_len_calculator(&vec),
            column_styles: [
                Style::default(),
                Style::default().fg(get_color(|t| &t.server.name)),
                Style::default().fg(get_color(|t| &t.server.location)),
                Style::default().fg(get_color(|t| &t.server.db)),
                Style::default().fg(get_color(|t| &t.server.username)),
                Style::default().fg(get_color(|t| &t.server.tls)),
                Style::default().fg(get_color(|t| &t.server.protocol)),
            ],
            scroll_state: ScrollbarState::new((vec.len().saturating_sub(1)) * ITEM_HEIGHT),
            colors: TableColors::new(),
            items: vec,
            create_form: Form::default().title("New"),
            edit_form: Form::default().title("Edit"),
            save_on_exit: true,

            list_rect: Rect::default(),
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

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        let header_style = Style::default()
            .bold()
            .fg(self.colors.header_fg)
            .bg(self.colors.header_bg);
        let selected_style = Style::default().bg(get_color(|t| &t.server.highlight));

        let header = ["", "Name", "Location", "DB", "Username", "TLS", "Protocol"]
            .into_iter()
            .map(|title| Cell::from(Text::raw(title)))
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style)
            .height(1);

        let rows = self
            .items
            .iter()
            .enumerate()
            .map(|(_, data)| {
                let item = data.ref_array();
                item.into_iter()
                    .enumerate()
                    .map(|(idx, content)| {
                        Cell::from(Text::raw(content).style(self.column_styles[idx]))
                    })
                    .collect::<Row>()
                    .style(Style::new().fg(self.colors.row_fg))
                    .height(1)
            })
            .collect_vec();
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
        .row_highlight_style(selected_style)
        .highlight_symbol(Text::raw(bar).style(Style::default()))
        .bg(self.colors.bg)
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
                vertical: 0,
                horizontal: 0,
            }),
            &mut self.scroll_state,
        );
    }

    fn render_delete_popup(&mut self, frame: &mut Frame, area: Rect) {
        if let Some(selected) = self.state.selected() {
            let item = self.items.get(selected).clone();
            if let Some(data) = item {
                let popup_area = centered_rect(30, 15, area);
                let mut text = Text::default();
                text.push_line(
                    Line::raw(data.name.clone())
                        .alignment(Alignment::Center)
                        .underlined(),
                );
                text.push_line(Line::default());
                text.push_line(
                    Line::raw("Will be deleted. Are you sure?")
                        .alignment(Alignment::Center)
                        .bold(),
                );
                let paragraph = Paragraph::new(text).alignment(Alignment::Center);
                let delete_popup = Popup::new(paragraph)
                    .title(String::from(" [Enter] Confirm | [Esc] Cancel "))
                    .title_position(Position::Bottom)
                    .borders(Borders::ALL)
                    .border_set(symbols::border::DOUBLE)
                    .style(Style::default());
                frame.render_widget(delete_popup, popup_area);
            }
        }
    }

    fn render_create_popup(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        self.create_form.render_frame(frame, area)?;
        Ok(())
    }

    fn render_edit_popup(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        self.edit_form.render_frame(frame, area)?;
        Ok(())
    }

    fn handle_delete_popup_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        if key_event.kind != KeyEventKind::Press || key_event.modifiers != KeyModifiers::NONE {
            return Ok(true);
        }

        match key_event.code {
            KeyCode::Enter => {
                if let Some(selected) = self.state.selected() {
                    let item = self.items.get(selected).clone();
                    if let Some(data) = item {
                        if data.selected == "*" {
                            let _ = publish_msg(Message::warning("Cannot delete selected server"));
                        } else {
                            self.have_changed = true;
                            self.items.remove(selected);
                        }
                    }
                }
                self.show_delete_popup = false;
            }
            KeyCode::Esc => self.show_delete_popup = false,
            _ => {}
        }

        Ok(true)
    }

    fn handle_create_popup_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        if key_event.kind == KeyEventKind::Press
            && key_event.modifiers == KeyModifiers::NONE
            && key_event.code == KeyCode::Enter
        {
            let database = self.create_form.to_database();
            let data = Data {
                selected: "".to_string(),
                name: self.create_form.get_name(),
                location: format!("{}:{}", database.host, database.port),
                db: database.db.to_string(),
                username: database.username.clone().unwrap_or(String::new()),
                use_tls: database.use_tls.to_string(),
                protocol: database.protocol.to_string(),
                database,
            };
            self.valid_create(&data)?;
            self.have_changed = true;
            self.items.push(data);
            self.create_form = Form::default().title("New");
            self.show_create_popup = false;
        } else {
            if !self.create_form.handle_key_event(key_event)?
                && key_event.kind == KeyEventKind::Press
                && key_event.modifiers == KeyModifiers::NONE
                && key_event.code == KeyCode::Esc
            {
                self.show_create_popup = false;
            }
        }
        Ok(true)
    }

    fn handle_edit_popup_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        if key_event.kind == KeyEventKind::Press
            && key_event.modifiers == KeyModifiers::NONE
            && key_event.code == KeyCode::Esc
        {
            self.show_edit_popup = false;
        } else if key_event.kind == KeyEventKind::Press
            && key_event.modifiers == KeyModifiers::NONE
            && key_event.code == KeyCode::Enter
        {
            let database = self.edit_form.to_database();
            if let Some(idx) = self.state.selected() && let Some(current_data) = self.items.get(idx) {
                let data = Data {
                    selected: current_data.selected.clone(),
                    name: self.edit_form.get_name(),
                    location: format!("{}:{}", database.host, database.port),
                    db: database.db.to_string(),
                    username: database.username.clone().unwrap_or(String::new()),
                    use_tls: database.use_tls.to_string(),
                    protocol: database.protocol.to_string(),
                    database,
                };
                self.valid_edit(&data)?;
                self.have_changed = true;
                if data.selected == "*" {
                    switch_client(data.name.clone(), &data.database)?;
                }
                self.items[idx] = data;
                self.edit_form = Form::default().title("Edit");
                self.show_edit_popup = false;
            }
        } else {
            let result = self.edit_form.handle_key_event(key_event)?;
            if result {
                return Ok(true);
            }
        }
        Ok(true)
    }

    fn valid_create(&self, data: &Data) -> Result<()> {
        if data.name.is_empty() {
            return Err(anyhow!("Profile name must not be blank"));
        } else {
            for item in self.items.iter() {
                if item.name == data.name {
                    return Err(anyhow!("Profile [{}] already exists", data.name));
                }
            }
        }
        Ok(())
    }

    fn valid_edit(&self, data: &Data) -> Result<()> {
        if data.name.is_empty() {
            return Err(anyhow!("Profile name must not be blank"));
        }
        Ok(())
    }

    fn selected(&self) -> Option<Data> {
        if let Some(selected) = self.state.selected() {
            let item = self.items.get(selected).clone();
            if let Some(data) = item {
                let data = data.clone();
                return Some(data);
            }
        }
        None
    }

    fn save(&self) -> Result<()> {
        let selected_data = self.selected();
        let selected_name = selected_data.map(|data| data.name);
        let default_database_changed = selected_name != self.init_database_name;
        if self.have_changed || default_database_changed {
            let mut databases = Databases::empty();
            databases.default_database = selected_name;
            for data_ref in self.items.iter() {
                databases
                    .databases
                    .insert(data_ref.name.clone(), data_ref.database.clone());
            }
            save_database_configuration(&databases)?;
        }
        Ok(())
    }
}

impl Renderable for ServerList {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> Result<()> {
        self.list_rect = rect;
        frame.render_widget(Clear::default(), rect);
        let block = Block::bordered()
            .title("Servers")
            .border_set(symbols::border::DOUBLE)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded);
        let inner_block = block.inner(rect);
        frame.render_widget(block, rect);
        self.render_table(frame, inner_block);
        self.render_scrollbar(frame, inner_block);

        if self.show_delete_popup {
            self.render_delete_popup(frame, frame.area());
        }

        if self.show_create_popup {
            self.render_create_popup(frame, frame.area())?;
        }

        if self.show_edit_popup {
            self.render_edit_popup(frame, frame.area())?;
        }

        Ok(())
    }

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        let mut elements = vec![];
        if self.show_delete_popup {
            elements.push(("Enter", "Confirm"));
            elements.push(("Esc", "Close"));
            return elements;
        } else if self.show_create_popup {
            elements = self.create_form.footer_elements();
            elements.push(("Enter", "Create"));
            elements.push(("Esc", "Close"));
            return elements;
        } else if self.show_edit_popup {
            elements = self.edit_form.footer_elements();
            elements.push(("Enter", "Update"));
            elements.push(("Esc", "Close"));
            return elements;
        }

        elements.push(("↑/j", "Up"));
        elements.push(("↓/k", "Down"));
        elements.push(("Enter", "Choose"));
        elements.push(("c", "Create"));
        elements.push(("d", "Delete"));
        elements.push(("e", "Edit"));
        elements.push(("Esc", "Close"));
        elements
    }
}

impl Listenable for ServerList {
    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        if key_event.is_c_c() {
            return Ok(false);
        }
        if key_event.kind == KeyEventKind::Press {
            if self.show_delete_popup {
                return self.handle_delete_popup_key_event(key_event);
            }
            if self.show_create_popup {
                return self.handle_create_popup_key_event(key_event);
            }
            if self.show_edit_popup {
                return self.handle_edit_popup_key_event(key_event);
            }

            if key_event.modifiers.is_empty() {
                let accepted = match key_event.code {
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
                    }
                    KeyCode::Char('d') => {
                        self.show_delete_popup = true;
                        true
                    }
                    KeyCode::Char('c') => {
                        self.show_create_popup = true;
                        true
                    }
                    KeyCode::Char('e') => {
                        self.show_edit_popup = true;
                        if let Some(idx) = self.state.selected() {
                            if let Some(data) = self.items.get(idx) {
                                self.edit_form = Form::from_data(data).title("Edit");
                            }
                        }
                        true
                    }
                    _ => false,
                };
                return Ok(accepted);
            }
        }
        Ok(false)
    }

    fn handle_mouse_event(&mut self, mouse_event: MouseEvent) -> Result<bool> {
        if self.show_create_popup {
            if !self.create_form.handle_mouse_event(mouse_event)? && mouse_event.is_left_up() {
                self.show_create_popup = false;
            }
            return Ok(true);
        }
        if self.show_edit_popup {
            if !self.edit_form.handle_mouse_event(mouse_event)? && mouse_event.is_left_up() {
                self.show_edit_popup = false;
            }
            return Ok(true);
        }
        if mouse_event.within(&self.list_rect) {
            return Ok(true);
        }
        Ok(false)
    }


    fn on_app_event(&mut self, app_event: AppEvent) -> Result<()> {
        match app_event {
            AppEvent::InitConfig(_, args) => self.save_on_exit = !args.once,
            AppEvent::Destroy => {
                if self.save_on_exit {
                    self.save()?;
                }
            }
            _ => {}
        }
        Ok(())
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
        name_len as u16,
        location_len as u16,
        db_len as u16,
        username_len as u16,
        use_tls_len as u16,
        protocol_len as u16,
    )
}
