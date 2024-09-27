#[path = "common/lib.rs"]
mod common;

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
    let x: Value = cmd("ping").query_async(&mut connection).await?;
    dbg!(x);
    Ok(())
}