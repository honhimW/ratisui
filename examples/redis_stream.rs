#[path = "common/lib.rs"]
mod common;
use crate::common::client::dead_pool;
use anyhow::Result;
use redis::{Cmd, Value};

#[tokio::main]
async fn main() -> Result<()> {
    let pool = dead_pool()?;
    let mut connection = pool.get().await?;
    let cmd = str_cmd!("xrevrange stream + -");
    let x: Value = cmd.query_async(&mut connection).await?;
    println!("{:?}", x);

    Ok(())
}
