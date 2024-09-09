use lazy_static::lazy_static;
use tokio::sync::{broadcast, RwLock};
use anyhow::{Context, Result};

lazy_static! {
    static ref TOAST_CHANNEL: RwLock<Channel> = {
        let (tx, rx) = broadcast::channel(16);
        RwLock::new(Channel { tx, rx })
    };
}

pub struct Channel {
    pub tx: broadcast::Sender<Message>,
    pub rx: broadcast::Receiver<Message>,
}

#[derive(Clone)]
pub enum Message {
    Error(String),
    Warning(String),
    Info(String),
}

async fn publish_event(event: Message) -> Result<()> {
    TOAST_CHANNEL.read().await.send(event).context("Publishing failed")?;
    Ok(())
}

async fn subscribe_event() -> broadcast::Receiver<Message> {
    TOAST_CHANNEL.read().await.tx.subscribe()
}

fn toast(event: Message) -> Result<()> {
    tokio::spawn(async move {
        publish_event(event).await?;
    });
    Ok(())
}

fn sub() -> Message {
    let handle = tokio::task::spawn_blocking(subscribe_event);
    let place = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async { handle.await })
    });
}