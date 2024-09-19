#[path = "common/lib.rs"]
mod common;

use crate::common::client::dead_pool;
use anyhow::Result;
use redis::{cmd, AsyncCommands, Value};

#[tokio::main]
async fn main() -> Result<()> {
    let pool = dead_pool()?;

    let mut connection = pool.get().await?;
    let x: Value = cmd("info").arg("server").query_async(&mut connection).await?;
    println!("{:?}", x);

    Ok(())
}