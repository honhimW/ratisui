#[path = "common/lib.rs"]
mod common;
use crate::common::client::dead_pool;
use anyhow::Result;
use log::info;
use redis::{AsyncCommands, AsyncIter, Cmd, ScanOptions, Value};
use crate::common::enable_log;

#[tokio::main]
async fn main() -> Result<()> {
    enable_log();
    let pool = dead_pool()?;
    let mut connection = pool.get().await?;
    let cmd = str_cmd!("info    server      ");
    let x: Value = cmd.query_async(&mut connection).await?;
    println!("{:?}", x);

    let mut iter: AsyncIter<String> = connection.scan_options(ScanOptions::default().with_pattern("*").with_count(1)).await?;
    while let Some(_) = iter.next_item().await {
        info!("next");
    }
    Ok(())
}
