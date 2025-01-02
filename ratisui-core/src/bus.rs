use std::time::Instant;
use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
use once_cell::sync::Lazy;
use strum::Display;

static TOAST_CHANNEL: Lazy<ToastChannel> = Lazy::new(|| {
    let (tx, rx) = crossbeam_channel::bounded(4);
    ToastChannel { tx, rx }
});

pub struct ToastChannel {
    pub tx: Sender<Message>,
    pub rx: Receiver<Message>,
}

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
}

pub fn publish_msg(event: Message) -> Result<()> {
    TOAST_CHANNEL.tx.send(event).context("Publishing failed")?;
    Ok(())
}

pub fn try_take_msg() -> Result<Message> {
    TOAST_CHANNEL.rx.try_recv().context("Taking failed")
}

#[allow(unused)]
pub fn get_sender() -> Result<Sender<Message>> {
    Ok(TOAST_CHANNEL.tx.clone())
}

#[allow(unused)]
pub fn get_receiver() -> Result<Receiver<Message>> {
    Ok(TOAST_CHANNEL.rx.clone())
}

static GLOBAL_CHANNEL: Lazy<GlobalChannel> = Lazy::new(|| {
    let (tx, rx) = crossbeam_channel::bounded(16);
    GlobalChannel { tx, rx }
});

pub struct GlobalChannel {
    pub tx: Sender<GlobalEvent>,
    pub rx: Receiver<GlobalEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GlobalEvent {
    Exit,
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