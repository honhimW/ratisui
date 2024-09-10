use ratatui::crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::{Layout, Rect};
use ratatui::layout::Constraint::{Length, Percentage};
use strum::{EnumCount, EnumIter, IntoEnumIterator};
use tui_textarea::TextArea;
use crate::app::{Listenable, Renderable};
use crate::configuration::Protocol;

pub struct Form {
    editing: usize,
    host_text_area: TextArea<'static>,
    port_text_area: TextArea<'static>,
    enabled_authentication: bool,
    username_text_area: TextArea<'static>,
    password_text_area: TextArea<'static>,
    use_tls: bool,
    db_text_area: TextArea<'static>,
    protocol: Protocol,
}

#[derive(Default, Eq, PartialEq, EnumCount, EnumIter)]
enum Editing {
    #[default]
    Host,
    Port,
    EnabledAuthentication,
    Username,
    Password,
    UseTls,
    Db,
    Protocol,
}

impl Default for Form {
    fn default() -> Self {
        let mut form = Self {
            editing: 0,
            host_text_area: TextArea::default(),
            port_text_area: TextArea::default(),
            enabled_authentication: false,
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
        form
    }
}

impl Form {

    pub fn next(&mut self) {
        let tmp = self.editing + 1;
        self.editing = tmp % Editing::COUNT;
    }

    pub fn prev(&mut self) {
        let tmp = self.editing + (Editing::COUNT - 1);
        self.editing = tmp % Editing::COUNT;
    }

    fn current(&self) -> Editing {
        Editing::iter().get(self.editing).unwrap()
    }

    // fn current_input(&self) -> &TextArea<'static> {
    //     let editing = Editing::iter().get(self.editing).unwrap();
    //     match editing {
    //         Editing::Host => &self.host_text_area,
    //         Editing::Port => &self.port_text_area,
    //         Editing::Username => &self.username_text_area,
    //         Editing::Password => &self.password_text_area,
    //         Editing::UseTls => &self.use_tls_text_area,
    //         Editing::Db => &self.db_text_area,
    //         Editing::Protocol => &self.protocol_text_area,
    //     }
    // }

    fn render_host_port(&self, frame: &mut Frame, rect: Rect) {
        let horizontal = Layout::horizontal([Percentage(80), Percentage(20)]);
        let rc = horizontal.split(rect);
        frame.render_widget(&self.host_text_area, rc[0]);
        frame.render_widget(&self.port_text_area, rc[1]);
    }

}

impl Renderable for Form {
    fn render_frame(&mut self, frame: &mut Frame, rect: Rect) -> anyhow::Result<()> {
        if self.enabled_authentication {
            let vertical = Layout::vertical([
                Length(1), // host + port
                Length(1), // enabled auth
                Length(1), // tls
                Length(1), // db
                Length(1), // protocol
            ]);
            let rc = vertical.split(rect);
            self.render_host_port(frame, rc[0]);
            self.render_enabled_auth(frame, rc[1]);
            self.render_use_tls(frame, rc[2]);
            self.render_db(frame, rc[3]);
            self.render_protocol(frame, rc[4]);
        } else {
            let vertical = Layout::vertical([
                Length(1), // host + port
                Length(1), // enabled auth
                Length(1), // username
                Length(1), // password
                Length(1), // tls
                Length(1), // db
                Length(1), // protocol
            ]);
            let rc = vertical.split(rect);
            self.render_host_port(frame, rc[0]);
            self.render_enabled_auth(frame, rc[1]);
            self.render_username(frame, rc[2]);
            self.render_password(frame, rc[3]);
            self.render_use_tls(frame, rc[4]);
            self.render_db(frame, rc[5]);
            self.render_protocol(frame, rc[6]);
        }
        Ok(())
    }

    fn footer_elements(&self) -> Vec<(&str, &str)> {
        todo!()
    }
}

impl Listenable for Form {
    fn handle_key_event(&mut self, _key_event: KeyEvent) -> anyhow::Result<bool> {
        todo!()
    }
}