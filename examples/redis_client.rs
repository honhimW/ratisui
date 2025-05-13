#[path = "common/lib.rs"]
mod common;

use crate::common::client::dead_pool;
use anyhow::Result;
use deadpool_redis::redis::{cmd, Value};

#[tokio::main]
async fn main() -> Result<()> {
    let pool = dead_pool()?;

    let mut connection = pool.get().await?;
    let x: Value = cmd("ping").query_async(&mut connection).await?;
    dbg!(x);
    Ok(())
}
