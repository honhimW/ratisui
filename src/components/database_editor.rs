use crate::app::{Listenable, Renderable};
use crate::components::servers::Data;
use crate::configuration::{Database, Protocol, SshTunnel};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Constraint::{Fill, Length, Percentage};
use ratatui::layout::{Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Clear};
use ratatui::Frame;
use strum::{Display, EnumCount, EnumIter, IntoEnumIterator};
use tui_textarea::TextArea;
use uuid::Uuid;
use crate::theme::get_color;

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
    use_ssh_tunnel: bool,
    ssh_host_text_area: TextArea<'static>,
    ssh_port_text_area: TextArea<'static>,
    ssh_username_text_area: TextArea<'static>,
    ssh_password_text_area: TextArea<'static>,
}

#[derive(Default, Eq, PartialEq, EnumCount, EnumIter, Display)]
enum Editing {
    #[default]
    #[strum(serialize = "Name(*)")]
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
    #[strum(serialize = "Use SSH Tunnel")]
    UseSshTunnel,
    #[strum(serialize = "SSH Host")]
    SshHost,
    #[strum(serialize = "SSH Port")]
    SshPort,
    #[strum(serialize = "SSH Username")]
    SshUsername,
    #[strum(serialize = "SSH Password")]
    SshPassword,
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
            name_text_area: TextArea::default(),
            host_text_area: TextArea::default(),
            port_text_area: TextArea::default(),
            enabled_authentication: true,
            username_text_area: TextArea::default(),
            password_text_area: TextArea::default(),
            use_tls: false,
            db_text_area: TextArea::default(),
            protocol: Protocol::RESP3,
            use_ssh_tunnel: false,
            ssh_host_text_area: TextArea::default(),
            ssh_port_text_area: TextArea::default(),
            ssh_username_text_area: TextArea::default(),
            ssh_password_text_area: TextArea::default(),
        };
        form.name_text_area.set_placeholder_text("must not be blank");
        form.name_text_area.set_placeholder_style(Style::default().fg(get_color(|t| &t.editor.warning)).dim());
        form.host_text_area.set_placeholder_text("127.0.0.1");
        form.port_text_area.set_placeholder_text("6379");
        form.username_text_area.set_placeholder_text("");
        form.password_text_area.set_placeholder_text("");
        form.db_text_area.set_placeholder_text("0");
        form.ssh_host_text_area.set_placeholder_text("127.0.0.1");
        form.ssh_port_text_area.set_placeholder_text("22");
        form.ssh_username_text_area.set_placeholder_text("root");
        form.ssh_password_text_area.set_placeholder_text("");

        form.name_text_area.set_cursor_style(Style::default());
        form.host_text_area.set_cursor_style(Style::default());
        form.port_text_area.set_cursor_style(Style::default());
        form.username_text_area.set_cursor_style(Style::default());
        form.password_text_area.set_cursor_style(Style::default());
        form.db_text_area.set_cursor_style(Style::default());
        form.ssh_host_text_area.set_cursor_style(Style::default());
        form.ssh_port_text_area.set_cursor_style(Style::default());
        form.ssh_username_text_area.set_cursor_style(Style::default());
        form.ssh_password_text_area.set_cursor_style(Style::default());

        form.name_text_area.insert_str(Uuid::new_v4().to_string());
        form.name_text_area.select_all();
        form.password_text_area.set_mask_char('•');
        form.ssh_password_text_area.set_mask_char('•');

        form
    }
}

impl Form {

    pub fn from_data(data: &Data) -> Self {
        let mut form = Self::default();
        form.name_text_area.delete_line_by_head();
        form.name_text_area.insert_str(data.name.clone());
        form.host_text_area.insert_str(data.database.host.clone());
        form.port_text_area.insert_str(data.database.port.to_string());
        form.enabled_authentication = data.database.password.is_some();
        form.username_text_area.insert_str(data.database.username.clone().unwrap_or_default());
        form.password_text_area.insert_str(data.database.password.clone().unwrap_or_default());
        form.use_tls = data.database.use_tls;
        form.db_text_area.insert_str(data.db.clone());
        form.protocol = data.database.protocol.clone();
        form.use_ssh_tunnel = data.database.use_ssh_tunnel;
        if let Some(ref ssh_tunnel) = data.database.ssh_tunnel {
            form.ssh_host_text_area.insert_str(ssh_tunnel.host.clone());
            form.ssh_port_text_area.insert_str(ssh_tunnel.port.to_string());
            form.ssh_username_text_area.insert_str(ssh_tunnel.username.clone());
            form.ssh_password_text_area.insert_str(ssh_tunnel.password.clone());
        }
        form
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
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
        let use_ssh_tunnel = self.use_ssh_tunnel;
        let ssh_tunnel = if use_ssh_tunnel {
            let ssh_host = self.ssh_host_text_area.lines().get(0).cloned().filter(|x| !x.is_empty()).unwrap_or(self.ssh_host_text_area.placeholder_text().to_string());
            let ssh_port = self.ssh_port_text_area.lines().get(0).cloned().filter(|x| !x.is_empty()).unwrap_or(self.ssh_port_text_area.placeholder_text().to_string()).parse::<u16>().unwrap_or(6379);
            let ssh_username = self.ssh_username_text_area.lines().get(0).cloned().filter(|x| !x.is_empty()).unwrap_or(self.ssh_username_text_area.placeholder_text().to_string());
            let ssh_password = self.ssh_password_text_area.lines().get(0).cloned().filter(|x| !x.is_empty()).unwrap_or_default();
            Some(SshTunnel {
                host: ssh_host,
                port: ssh_port,
                username: ssh_username,
                password: ssh_password,
            })
        } else { None };
        Database {
            host,
            port,
            username,
            password,
            use_tls,
            db,
            protocol,
            use_ssh_tunnel,
            ssh_tunnel,
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
        if !self.use_ssh_tunnel {
            let editing = self.current();
            if editing == Editing::SshHost || editing == Editing::SshPort || editing == Editing::SshUsername || editing == Editing::SshPassword {
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
        if !self.use_ssh_tunnel {
            let editing = self.current();
            if editing == Editing::SshHost || editing == Editing::SshPort || editing == Editing::SshUsername || editing == Editing::SshPassword {
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

        self.name_text_area.cancel_selection();
        self.host_text_area.cancel_selection();
        self.port_text_area.cancel_selection();
        self.username_text_area.cancel_selection();
        self.password_text_area.cancel_selection();
        self.db_text_area.cancel_selection();

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

    fn render_name(&mut self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(18), Fill(0)]);
        let rc = horizontal.split(rect);
        let key = self.span(Editing::Name);
        self.name_text_area.set_style(key.style);
        let value = &self.name_text_area;
        frame.render_widget(key, rc[0]);
        frame.render_widget(value, rc[1]);
    }

    fn render_host_port(&mut self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Percentage(65), Percentage(35)]);
        let rc = horizontal.split(rect);
        let host_area = Layout::horizontal([Length(18), Fill(0)]).split(rc[0]);
        let port_area = Layout::horizontal([Length(5), Fill(0)]).split(rc[1]);
        {
            let key = self.span(Editing::Host);
            self.host_text_area.set_style(key.style);
            let value = &self.host_text_area;
            frame.render_widget(key, host_area[0]);
            frame.render_widget(value, host_area[1]);
        }
        {
            let key = self.span(Editing::Port);
            self.port_text_area.set_style(key.style);
            let value = &self.port_text_area;
            frame.render_widget(key, port_area[0]);
            frame.render_widget(value, port_area[1]);
        }
    }

    fn render_enabled_auth(&mut self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(18), Fill(0)]);
        let rc = horizontal.split(rect);
        let key = self.span(Editing::EnabledAuthentication);
        let value = Span::raw(if self.enabled_authentication { "◄ Username & Password ►" } else { "◄ None ►" }).style(key.style);
        frame.render_widget(key, rc[0]);
        frame.render_widget(value, rc[1]);
    }

    fn render_username(&mut self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(18), Fill(0)]);
        let rc = horizontal.split(rect);
        let key = self.span(Editing::Username);
        self.username_text_area.set_style(key.style);
        let value = &self.username_text_area;
        frame.render_widget(key, rc[0]);
        frame.render_widget(value, rc[1]);
    }

    fn render_password(&mut self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(18), Fill(0)]);
        let rc = horizontal.split(rect);
        let key = self.span(Editing::Password);
        self.password_text_area.set_style(key.style);
        let value = &self.password_text_area;
        frame.render_widget(key, rc[0]);
        frame.render_widget(value, rc[1]);
    }

    fn render_use_tls(&self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(18), Fill(0)]);
        let rc = horizontal.split(rect);
        let key = self.span(Editing::UseTls);
        let value = Span::raw(if self.use_tls { "◄ Yes ►" } else { "◄ No ►" }).style(key.style);
        frame.render_widget(key, rc[0]);
        frame.render_widget(value, rc[1]);
    }

    fn render_db(&mut self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(18), Fill(0)]);
        let rc = horizontal.split(rect);
        let key = self.span(Editing::Db);
        self.db_text_area.set_style(key.style);
        let value = &self.db_text_area;
        frame.render_widget(key, rc[0]);
        frame.render_widget(value, rc[1]);
    }

    fn render_protocol(&self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(18), Fill(0)]);
        let rc = horizontal.split(rect);
        let key = self.span(Editing::Protocol);
        let value = Span::raw(format!("◄ {} ►", self.protocol.to_string())).style(key.style);
        frame.render_widget(key, rc[0]);
        frame.render_widget(value, rc[1]);
    }

    fn render_use_ssh_tunnel(&self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(18), Fill(0)]);
        let rc = horizontal.split(rect);
        let key = self.span(Editing::UseSshTunnel);
        let value = Span::raw(if self.use_ssh_tunnel { "◄ Yes ►" } else { "◄ No ►" }).style(key.style);
        frame.render_widget(key, rc[0]);
        frame.render_widget(value, rc[1]);
    }

    fn render_ssh_host_port(&mut self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Percentage(65), Percentage(35)]);
        let rc = horizontal.split(rect);
        let host_area = Layout::horizontal([Length(18), Fill(0)]).split(rc[0]);
        let port_area = Layout::horizontal([Length(9), Fill(0)]).split(rc[1]);
        {
            let key = self.span(Editing::SshHost);
            self.ssh_host_text_area.set_style(key.style);
            let value = &self.ssh_host_text_area;
            frame.render_widget(key, host_area[0]);
            frame.render_widget(value, host_area[1]);
        }
        {
            let key = self.span(Editing::SshPort);
            self.ssh_port_text_area.set_style(key.style);
            let value = &self.ssh_port_text_area;
            frame.render_widget(key, port_area[0]);
            frame.render_widget(value, port_area[1]);
        }
    }

    fn render_ssh_username(&mut self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(18), Fill(0)]);
        let rc = horizontal.split(rect);
        let key = self.span(Editing::SshUsername);
        self.ssh_username_text_area.set_style(key.style);
        let value = &self.ssh_username_text_area;
        frame.render_widget(key, rc[0]);
        frame.render_widget(value, rc[1]);
    }

    fn render_ssh_password(&mut self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Length(18), Fill(0)]);
        let rc = horizontal.split(rect);
        let key = self.span(Editing::SshPassword);
        self.ssh_password_text_area.set_style(key.style);
        let value = &self.ssh_password_text_area;
        frame.render_widget(key, rc[0]);
        frame.render_widget(value, rc[1]);
    }

}

impl Renderable for Form {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> anyhow::Result<()> {
        let mut total_height = 9;
        if self.enabled_authentication {
            total_height += 2;
        }
        let ssh_height = 3;
        if self.use_ssh_tunnel {
            total_height += ssh_height;
        }
        let blank_length = (rect.height - total_height) / 2;
        let area = Layout::vertical([Length(blank_length), Length(total_height), Length(blank_length)]).split(rect)[1];
        let area = Layout::horizontal([Percentage(20), Percentage(60), Percentage(20)]).split(area)[1];
        // let area = centered_rect(50, 70, rect);
        frame.render_widget(Clear::default(), area);
        let block = Block::bordered()
            .title(self.title.clone())
            .border_type(BorderType::Rounded);
        let block_inner_area = block
            .inner(area);
        let block_inner_area = Layout::horizontal([Length(1), Fill(0), Length(1)]).split(block_inner_area)[1];
        let inner_area_vertical = Layout::vertical([Fill(0), Length(ssh_height)]).split(block_inner_area);
        let base_area = if self.use_ssh_tunnel {
            inner_area_vertical[0]
        } else {
            block_inner_area
        };

        if !self.enabled_authentication {
            let vertical = Layout::vertical([
                Length(1), // name
                Length(1), // host + port
                Length(1), // enabled auth
                Length(1), // tls
                Length(1), // db
                Length(1), // protocol
                Length(1), // use ssh
            ]);
            let rc = vertical.split(base_area);
            self.render_name(frame, rc[0]);
            self.render_host_port(frame, rc[1]);
            self.render_enabled_auth(frame, rc[2]);
            self.render_use_tls(frame, rc[3]);
            self.render_db(frame, rc[4]);
            self.render_protocol(frame, rc[5]);
            self.render_use_ssh_tunnel(frame, rc[6]);
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
                Length(1), // use ssh
            ]);
            let rc = vertical.split(base_area);
            self.render_name(frame, rc[0]);
            self.render_host_port(frame, rc[1]);
            self.render_enabled_auth(frame, rc[2]);
            self.render_username(frame, rc[3]);
            self.render_password(frame, rc[4]);
            self.render_use_tls(frame, rc[5]);
            self.render_db(frame, rc[6]);
            self.render_protocol(frame, rc[7]);
            self.render_use_ssh_tunnel(frame, rc[8]);
        }

        if self.use_ssh_tunnel {
            let rc = Layout::vertical([
                Length(1), // host + port
                Length(1), // username
                Length(1), // password
            ]).split(inner_area_vertical[1]);
            self.render_ssh_host_port(frame, rc[0]);
            self.render_ssh_username(frame, rc[1]);
            self.render_ssh_password(frame, rc[2]);
        }
        frame.render_widget(block, area);
        Ok(())
    }

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        let mut elements = vec![];
        elements.push(("Tab", "Next"));
        elements.push(("BackTab", "Prev"));

        let editing = self.current();
        if editing == Editing::Db || editing == Editing::Port {
            elements.push(("↑", "+10"));
            elements.push(("↓", "-10"));
            elements.push(("→", "+1"));
            elements.push(("←", "-1"));
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
            Editing::Name => Some(&mut self.name_text_area),
            Editing::Host => Some(&mut self.host_text_area),
            Editing::Port => Some(&mut self.port_text_area),
            Editing::Username => Some(&mut self.username_text_area),
            Editing::Password => Some(&mut self.password_text_area),
            Editing::Db => Some(&mut self.db_text_area),
            Editing::SshHost => Some(&mut self.ssh_host_text_area),
            Editing::SshPort => Some(&mut self.ssh_port_text_area),
            Editing::SshUsername => Some(&mut self.ssh_username_text_area),
            Editing::SshPassword => Some(&mut self.ssh_password_text_area),
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
                    if editing == Editing::Port || editing == Editing::Db || editing == Editing::SshPort {
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
                                    let port = first_line.parse::<u16>().unwrap_or(0);
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
                KeyCode::Esc => {
                    return Ok(false);
                },
                KeyCode::Char('h') | KeyCode::Left => {
                    match editing {
                        Editing::EnabledAuthentication => self.enabled_authentication = !self.enabled_authentication,
                        Editing::UseTls => self.use_tls = !self.use_tls,
                        Editing::Protocol => self.protocol = match self.protocol {
                            Protocol::RESP2 => Protocol::RESP3,
                            Protocol::RESP3 => Protocol::RESP2,
                        },
                        Editing::UseSshTunnel => self.use_ssh_tunnel = !self.use_ssh_tunnel,
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
                        Editing::UseSshTunnel => self.use_ssh_tunnel = !self.use_ssh_tunnel,
                        _ => {}
                    }
                }
                _ => {}
            }
            Ok(true)
        }
    }
}