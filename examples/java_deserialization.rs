#[path = "common/lib.rs"]
mod common;

use crate::common::client::dead_pool;
use anyhow::Result;
use jaded::Parser;
use deadpool_redis::redis::AsyncCommands;
use ron::ser::PrettyConfig;
use std::io::Cursor;

#[tokio::main]
async fn main() -> Result<()> {
    let pool = dead_pool()?;
    let mut connection = pool.get().await?;
    let java: Vec<u8> = connection.get("java").await?;

    let cursor = Cursor::new(java);
    let mut parser = Parser::new(cursor)?;

    let content = parser.read()?;

    let string = serde_json::to_string_pretty(&content)?;
    println!("{}", string);
    let ron = ron::ser::to_string_pretty(&content, PrettyConfig::default())?;
    println!("{}", ron);
    Ok(())
}
