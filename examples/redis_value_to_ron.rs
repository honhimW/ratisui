#[path = "common/lib.rs"]
mod common;
use crate::common::client::dead_pool;
use anyhow::Result;
use ratisui_core::serde_wrapper::IValue;
use deadpool_redis::redis::{Cmd, Value};
use ron::ser::PrettyConfig;

#[tokio::main]
async fn main() -> Result<()> {
    let pool = dead_pool()?;
    let mut connection = pool.get().await?;
    let x: Cmd = str_cmd!("ft.info movieIndex");
    let x1: Value = x.query_async(&mut connection).await?;
    let value = IValue::from(x1);
    println!(
        "{}",
        ron::ser::to_string_pretty(&value, PrettyConfig::default())?
    );
    Ok(())
}
