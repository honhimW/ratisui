use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Alignment, Layout, Rect};
use ratatui::layout::Constraint::{Fill, Length, Percentage};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::style::palette::tailwind;
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Clear};
use strum::{EnumCount, EnumIter, EnumString, IntoEnumIterator, ToString};
use tui_textarea::TextArea;
use crate::app::{centered_rect, Listenable, Renderable};
use crate::components::servers::Data;
use crate::configuration::{Database, Protocol};
use crate::tabs::explorer::FilterMod;

pub struct Form {
    title: String,
    editing_style: Style,
    editing: usize,
    name_text_area: TextArea<'static>,
    host_text_area: TextArea<'static>,
    port_text_area: TextArea<'static>,
    enabled_authentication: bool,
    username_text_area: TextArea<'static>,
    password_text_area: TextArea<'static>,
    use_tls: bool,
    db_text_area: TextArea<'static>,
    protocol: Protocol,
}

#[derive(Default, Eq, PartialEq, EnumCount, EnumIter, ToString)]
enum Editing {
    #[default]
    #[strum(serialize = "Name")]
    Name,
    #[strum(serialize = "Host")]
    Host,
    #[strum(serialize = "Port")]
    Port,
    #[strum(serialize = "Authentication")]
    EnabledAuthentication,
    #[strum(serialize = "Username")]
    Username,
    #[strum(serialize = "Password")]
    Password,
    #[strum(serialize = "Use TLS")]
    UseTls,
    #[strum(serialize = "Database")]
    Db,
    #[strum(serialize = "Protocol")]
    Protocol,
}

fn cursor_style() -> Style {
    Style::default().rapid_blink().underlined()
}

impl Default for Form {
    fn default() -> Self {
        let mut form = Self {
            title: String::from(""),
            editing_style: Style::default().fg(tailwind::SKY.c700),
            editing: 0,
            name_text_area: TextArea::default(),
            host_text_area: TextArea::default(),
            port_text_area: TextArea::default(),
            enabled_authentication: true,
            username_text_area: TextArea::default(),
            password_text_area: TextArea::default(),
            use_tls: false,
            db_text_area: TextArea::default(),
            protocol: Protocol::RESP3,
        };
        form.host_text_area.set_placeholder_text("127.0.0.1");
        form.port_text_area.set_placeholder_text("6379");
        form.username_text_area.set_placeholder_text("");
        form.password_text_area.set_placeholder_text("");
        form.db_text_area.set_placeholder_text("0");
        form.name_text_area.set_cursor_style(cursor_style());
        form.host_text_area.set_cursor_style(Style::default());
        form.port_text_area.set_cursor_style(Style::default());
        form.username_text_area.set_cursor_style(Style::default());
        form.password_text_area.set_cursor_style(Style::default());
        form.db_text_area.set_cursor_style(Style::default());
        form
    }
}

impl Form {

    pub fn from_data(data: &Data) -> Self {
        let mut form = Self::default();
        form.name_text_area.insert_str(data.name.clone());
        form.host_text_area.insert_str(data.database.host.clone());
        form.port_text_area.insert_str(data.database.port.to_string());
        form.enabled_authentication = data.database.password.is_some();
        form.username_text_area.insert_str(data.database.username.clone().unwrap_or_default());
        form.password_text_area.insert_str(data.database.password.clone().unwrap_or_default());
        form.use_tls = data.database.use_tls;
        form.db_text_area.insert_str(data.db.clone());
        form.protocol = data.database.protocol.clone();
        form
    }

    pub fn title(mut self, title: String) -> Self {
        self.title = title;
        self
    }

    pub fn get_name(&self) -> String {
        self.name_text_area.lines().get(0).cloned().unwrap_or(self.name_text_area.placeholder_text().to_string())
    }

    pub fn to_database(&self) -> Database {
        let host = self.host_text_area.lines().get(0).cloned().filter(|x| !x.is_empty()).unwrap_or(self.host_text_area.placeholder_text().to_string());
        let port = self.port_text_area.lines().get(0).cloned().filter(|x| !x.is_empty()).unwrap_or(self.port_text_area.placeholder_text().to_string()).parse::<u16>().unwrap_or(6379);
        let username = self.username_text_area.lines().get(0).cloned().filter(|x| !x.is_empty());
        let password = self.password_text_area.lines().get(0).cloned().filter(|x| !x.is_empty());
        let use_tls = self.use_tls;
        let db = self.db_text_area.lines().get(0).cloned().filter(|x| !x.is_empty()).unwrap_or(self.db_text_area.placeholder_text().to_string()).parse::<u32>().unwrap_or(0);
        let protocol = self.protocol.clone();
        Database {
            host,
            port,
            username,
            password,
            use_tls,
            use_ssh_tunnel: false,
            db,
            protocol,
        }
    }

    pub fn next(&mut self) {
        let tmp = self.editing + 1;
        self.editing = tmp % Editing::COUNT;
        if !self.enabled_authentication {
            let editing = self.current();
            if editing == Editing::Username || editing == Editing::Password {
                self.next();
            }
        }
        self.change_editing();
    }

    pub fn prev(&mut self) {
        let tmp = self.editing + (Editing::COUNT - 1);
        self.editing = tmp % Editing::COUNT;
        if !self.enabled_authentication {
            let editing = self.current();
            if editing == Editing::Username || editing == Editing::Password {
                self.prev();
            }
        }
        self.change_editing();
    }

    fn current(&self) -> Editing {
        Editing::iter().get(self.editing).unwrap()
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
        self.name_text_area.set_cursor_style(Style::default());
        self.host_text_area.set_cursor_style(Style::default());
        self.port_text_area.set_cursor_style(Style::default());
        self.username_text_area.set_cursor_style(Style::default());
        self.password_text_area.set_cursor_style(Style::default());
        self.db_text_area.set_cursor_style(Style::default());
        match self.current() {
            Editing::Name => self.name_text_area.set_cursor_style(cursor_style()),
            Editing::Host => self.host_text_area.set_cursor_style(cursor_style()),
            Editing::Port => self.port_text_area.set_cursor_style(cursor_style()),
            Editing::Username => self.username_text_area.set_cursor_style(cursor_style()),
            Editing::Password => self.password_text_area.set_cursor_style(cursor_style()),
            Editing::Db => self.db_text_area.set_cursor_style(cursor_style()),
            _ => {}
        }
    }

    fn render_name(&self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(18), Fill(0)]);
        let rc = horizontal.split(rect);
        frame.render_widget(self.span(Editing::Name), rc[0]);
        frame.render_widget(&self.name_text_area, rc[1]);
    }

    fn render_host_port(&self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Percentage(65), Percentage(35)]);
        let rc = horizontal.split(rect);
        let host_area = Layout::horizontal([Length(18), Fill(0)]).split(rc[0]);
        let port_area = Layout::horizontal([Length(5), Fill(0)]).split(rc[1]);
        frame.render_widget(self.span(Editing::Host), host_area[0]);
        frame.render_widget(&self.host_text_area, host_area[1]);
        frame.render_widget(self.span(Editing::Port), port_area[0]);
        frame.render_widget(&self.port_text_area, port_area[1]);
    }

    fn render_enabled_auth(&self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(18), Fill(0)]);
        let rc = horizontal.split(rect);
        frame.render_widget(self.span(Editing::EnabledAuthentication), rc[0]);
        frame.render_widget(Span::raw(if self.enabled_authentication { "◄ Username & Password ►" } else { "◄ None ►" }), rc[1]);
    }

    fn render_username(&self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(18), Fill(0)]);
        let rc = horizontal.split(rect);
        frame.render_widget(self.span(Editing::Username), rc[0]);
        frame.render_widget(&self.username_text_area, rc[1]);
    }

    fn render_password(&self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(18), Fill(0)]);
        let rc = horizontal.split(rect);
        frame.render_widget(self.span(Editing::Password), rc[0]);
        frame.render_widget(&self.password_text_area, rc[1]);
    }

    fn render_use_tls(&self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(18), Fill(0)]);
        let rc = horizontal.split(rect);
        frame.render_widget(self.span(Editing::UseTls), rc[0]);
        frame.render_widget(Span::raw(if self.use_tls { "◄ Yes ►" } else { "◄ No ►" }), rc[1]);
    }

    fn render_db(&self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(18), Fill(0)]);
        let rc = horizontal.split(rect);
        frame.render_widget(self.span(Editing::Db), rc[0]);
        frame.render_widget(&self.db_text_area, rc[1]);
    }

    fn render_protocol(&self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(18), Fill(0)]);
        let rc = horizontal.split(rect);
        frame.render_widget(self.span(Editing::Protocol), rc[0]);
        frame.render_widget(Span::raw(format!("◄ {} ►", self.protocol.to_string())), rc[1]);
    }

}

impl Renderable for Form {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> anyhow::Result<()> {
        let blank_length = (rect.height - 10) / 2;
        let area = Layout::vertical([Length(blank_length), Length(10), Length(blank_length)]).split(rect)[1];
        let area = Layout::horizontal([Percentage(20), Percentage(60), Percentage(20)]).split(area)[1];
        // let area = centered_rect(50, 70, rect);
        frame.render_widget(Clear::default(), area);
        let block = Block::bordered()
            .title(self.title.clone())
            .border_type(BorderType::Rounded);
        let block_inner_area = block
            .inner(area);
        let block_inner_area = Layout::horizontal([Length(1), Fill(0), Length(1)]).split(block_inner_area)[1];
        if !self.enabled_authentication {
            let vertical = Layout::vertical([
                Length(1), // name
                Length(1), // host + port
                Length(1), // enabled auth
                Length(1), // tls
                Length(1), // db
                Length(1), // protocol
            ]);
            let rc = vertical.split(block_inner_area);
            self.render_name(frame, rc[0]);
            self.render_host_port(frame, rc[1]);
            self.render_enabled_auth(frame, rc[2]);
            self.render_use_tls(frame, rc[3]);
            self.render_db(frame, rc[4]);
            self.render_protocol(frame, rc[5]);
        } else {
            let vertical = Layout::vertical([
                Length(1), // name
                Length(1), // host + port
                Length(1), // enabled auth
                Length(1), // username
                Length(1), // password
                Length(1), // tls
                Length(1), // db
                Length(1), // protocol
            ]);
            let rc = vertical.split(block_inner_area);
            self.render_name(frame, rc[0]);
            self.render_host_port(frame, rc[1]);
            self.render_enabled_auth(frame, rc[2]);
            self.render_username(frame, rc[3]);
            self.render_password(frame, rc[4]);
            self.render_use_tls(frame, rc[5]);
            self.render_db(frame, rc[6]);
            self.render_protocol(frame, rc[7]);
        }
        frame.render_widget(block, area);
        Ok(())
    }

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        todo!()
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
            Editing::Name => Some(&mut self.name_text_area),
            Editing::Host => Some(&mut self.host_text_area),
            Editing::Port => Some(&mut self.port_text_area),
            Editing::Username => Some(&mut self.username_text_area),
            Editing::Password => Some(&mut self.password_text_area),
            Editing::Db => Some(&mut self.db_text_area),
            _ => None,
        };
        if let Some(text_area) = editor {
            match key_event {
                KeyEvent { code: KeyCode::Esc, .. } => {
                    if text_area.is_selecting() {
                        text_area.cancel_selection();
                    }
                }
                KeyEvent { code: KeyCode::Enter, .. } => {}
                KeyEvent { code: KeyCode::Char('m'), modifiers: KeyModifiers::CONTROL, .. } => {}
                KeyEvent { code: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, .. } => {
                    text_area.select_all();
                }
                KeyEvent { code: KeyCode::Char('z'), modifiers: KeyModifiers::CONTROL, .. } => {
                    text_area.undo();
                }
                KeyEvent { code: KeyCode::Char('y'), modifiers: KeyModifiers::CONTROL, .. } => {
                    text_area.redo();
                }
                input => {
                    if editing == Editing::Port || editing == Editing::Db {
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
                                            let port = first_line.parse::<u32>().unwrap_or(0);
                                            let new_port = if port > u16::MAX as u32 {
                                                u16::MAX
                                            } else {
                                                port as u16
                                            }.to_string();
                                            text_area.delete_line_by_head();
                                            text_area.insert_str(new_port);
                                        }
                                    }
                                } else {
                                    let port = first_line.parse::<u16>().unwrap_or(6379);
                                    let new_port = match input.code {
                                        KeyCode::Char('h') | KeyCode::Left => port.saturating_sub(1).to_string(),
                                        KeyCode::Char('l') | KeyCode::Right => port.saturating_add(1).to_string(),
                                        KeyCode::Char('j') | KeyCode::Down => port.saturating_sub(10).to_string(),
                                        KeyCode::Char('k') | KeyCode::Up => port.saturating_add(10).to_string(),
                                        _ => first_line
                                    };
                                    text_area.delete_line_by_head();
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
                    match editing {
                        Editing::EnabledAuthentication => self.enabled_authentication = !self.enabled_authentication,
                        Editing::UseTls => self.use_tls = !self.use_tls,
                        Editing::Protocol => self.protocol = match self.protocol {
                            Protocol::RESP2 => Protocol::RESP3,
                            Protocol::RESP3 => Protocol::RESP2,
                        },
                        _ => {}
                    }
                }
                KeyCode::Char('l') | KeyCode::Right => {
                    match editing {
                        Editing::EnabledAuthentication => self.enabled_authentication = !self.enabled_authentication,
                        Editing::UseTls => self.use_tls = !self.use_tls,
                        Editing::Protocol => self.protocol = match self.protocol {
                            Protocol::RESP2 => Protocol::RESP3,
                            Protocol::RESP3 => Protocol::RESP2,
                        },
                        _ => {}
                    }
                }
                _ => {}
            }
            Ok(true)
        }
    }
}