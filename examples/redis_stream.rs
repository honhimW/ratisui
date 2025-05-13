#[path = "common/lib.rs"]
mod common;
use crate::common::client::dead_pool;
use anyhow::Result;
use itertools::Itertools;
use deadpool_redis::redis::{Cmd, Value};
use ratisui_core::utils::bytes_to_string;

#[tokio::main]
async fn main() -> Result<()> {
    let pool = dead_pool()?;
    let mut connection = pool.get().await?;
    let cmd = str_cmd!("xrevrange stream + -");
    let x: Vec<(Vec<String>, Vec<Vec<String>>)> = cmd.query_async(&mut connection).await?;
    let x: Value = cmd.query_async(&mut connection).await?;
    println!("{:?}", x);

    let value = Value::BulkString("".as_bytes().to_vec());

    if let Value::BulkString(bulk_string) = value {
        let bulk_string = bytes_to_string(bulk_string.clone()).unwrap_or_else(|e| e.to_string());
        let bulk_string = bulk_string.replace("\t", "\\t");
        // let bulk_string = format!("\"{}\"", bulk_string);
        let lines = bulk_string.lines();
        let format = |str: &str| {
            format!("{str}")
        };
        let vec = lines.map(|line| {
            format(line)
        }).collect_vec();
    }

    Ok(())
}
