use std::time::Instant;
use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
use lazy_static::lazy_static;

lazy_static! {
    static ref TOAST_CHANNEL: Channel = {
        let (tx, rx) = crossbeam_channel::bounded(4);
        Channel { tx, rx }
    };
}

pub struct Channel {
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

#[derive(Clone)]
pub enum Kind {
    Error,
    Warning,
    Info,
}

impl Message {
    pub fn info(msg: String) -> Self {
        Self::with_default(Kind::Info, msg)
    }

    pub fn error(msg: String) -> Self {
        Self::with_default(Kind::Error, msg)
    }

    pub fn warning(msg: String) -> Self {
        Self::with_default(Kind::Warning, msg)
    }

    fn with_default(kind: Kind, msg: String) -> Self {
        Self {
            kind,
            title: None,
            msg,
            expired_at: Instant::now() + std::time::Duration::from_secs(4),
        }
    }

    pub fn title(mut self, title: String) -> Self {
        self.title = Some(title);
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

pub fn get_sender() -> Result<Sender<Message>> {
    Ok(TOAST_CHANNEL.tx.clone())
}

pub fn get_receiver() -> Result<Receiver<Message>> {
    Ok(TOAST_CHANNEL.rx.clone())
}
