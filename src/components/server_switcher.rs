use crate::configuration::Databases;
use tui_textarea::TextArea;

pub struct Servers {
    databases: Databases,
    host_state: TextArea<'static>,
    port_state: TextArea<'static>,
    username_state: TextArea<'static>,
    password_state: TextArea<'static>,
    use_tls_state: TextArea<'static>,
    db_state: TextArea<'static>,
    protocol_state: TextArea<'static>,
}

impl Servers {
    pub fn new(databases: Databases) -> Self {
        Self {
            databases,
            host_state: Default::default(),
            port_state: Default::default(),
            username_state: Default::default(),
            password_state: Default::default(),
            use_tls_state: Default::default(),
            db_state: Default::default(),
            protocol_state: Default::default(),
        }
    }
}