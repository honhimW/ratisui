#[path = "common/lib.rs"]
mod common;

use std::task::{Context, Poll};
use std::time::Duration;
use crate::common::client::dead_pool;
use anyhow::Result;
use futures::{StreamExt, TryStreamExt};
use itertools::Itertools;
use redis::{cmd, AsyncCommands, Value};

#[tokio::main]
async fn main() -> Result<()> {
    let pool = dead_pool()?;
    let client = &pool.manager().client;

    let mut connection = pool.get().await?;
    let _ = cmd("monitor").query_async(&mut connection).await?;
    let mut monitor = client.get_async_monitor().await?;
    let _ = monitor.monitor().await?;
    println!("monitoring");
    let mut stream = monitor.on_message::<Value>();
    let waker = futures::task::noop_waker_ref();
    let mut context = Context::from_waker(waker);
    loop {
        match stream.poll_next_unpin(&mut context) {
            Poll::Ready(Some(v)) => {
                println!("{:?}", v);
            }
            Poll::Ready(None) => {
                println!("None");
            }
            Poll::Pending => {
                println!("Empty");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        };
    }
    Ok(())
}