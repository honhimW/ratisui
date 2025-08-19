use std::time::Instant;
use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
use log::{error, info, warn};
use once_cell::sync::Lazy;
use strum::Display;

pub struct Bus<T: Clone> {
    pub tx: Sender<T>,
    pub rx: Receiver<T>,
}

static TOAST_CHANNEL: Lazy<Bus<Message>> = Lazy::new(|| {
    let (tx, rx) = crossbeam_channel::bounded(4);
    Bus { tx, rx }
});

#[derive(Clone)]
pub struct Message {
    pub kind: Kind,
    pub title: Option<String>,
    pub msg: String,
    pub expired_at: Instant,
}

#[derive(Clone, Display)]
pub enum Kind {
    Error,
    Warn,
    Info,
}

#[allow(unused)]
impl Message {
    pub fn info(msg: impl Into<String>) -> Self {
        Self::with_default(Kind::Info, msg)
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self::with_default(Kind::Error, msg)
    }

    pub fn warning(msg: impl Into<String>) -> Self {
        Self::with_default(Kind::Warn, msg)
    }

    fn with_default(kind: Kind, msg: impl Into<String>) -> Self {
        Self {
            kind,
            title: None,
            msg: msg.into(),
            expired_at: Instant::now() + std::time::Duration::from_secs(4),
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }
    
    pub fn format(&self) -> String {
        let mut s = String::new();
        if let Some(title) = &self.title {
            s.push_str(title);
            s.push_str(": ")
        }
        s.push_str(&self.msg);
        s
    }
}

pub fn publish_msg(event: Message) -> Result<()> {
    match event.kind {
        Kind::Error => error!("{}", event.format()),
        Kind::Warn => warn!("{}", event.format()),
        Kind::Info => info!("{}", event.format()),
    }
    TOAST_CHANNEL.tx.send(event).context("Publishing failed")?;
    Ok(())
}

pub fn subscribe_message_channel() -> Result<Receiver<Message>> {
    Ok(TOAST_CHANNEL.rx.clone())
}

#[allow(unused)]
pub fn get_sender() -> Result<Sender<Message>> {
    Ok(TOAST_CHANNEL.tx.clone())
}

#[allow(unused)]
pub fn get_receiver() -> Result<Receiver<Message>> {
    Ok(TOAST_CHANNEL.rx.clone())
}

static GLOBAL_CHANNEL: Lazy<Bus<GlobalEvent>> = Lazy::new(|| {
    let (tx, rx) = crossbeam_channel::bounded(16);
    Bus { tx, rx }
});

#[derive(Clone, Debug)]
pub enum GlobalEvent {
    Exit,
    Restart,
    // Force refresh ui
    Tick,
    ClientChanged,
    #[allow(unused)]
    Dynamic(String),
}

pub fn publish_event(event: GlobalEvent) -> Result<()> {
    GLOBAL_CHANNEL.tx.send(event).context("Publishing GlobalEvent failed")?;
    Ok(())
}

pub fn subscribe_global_channel() -> Result<Receiver<GlobalEvent>> {
    Ok(GLOBAL_CHANNEL.rx.clone())
}
