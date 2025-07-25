#[path = "common/lib.rs"]
mod common;

use deadpool_redis::redis::aio::{ConnectionLike, MultiplexedConnection};
use deadpool_redis::Connection;
use crate::common::client::dead_pool;
use anyhow::Result;
use deadpool_redis::redis::{cmd, Arg, AsyncCommands, AsyncIter, Cmd, Pipeline, RedisFuture, RedisResult, Value};
use log::info;

#[tokio::main]
async fn main() -> Result<()> {
    let pool = dead_pool()?;

    let mut connection = pool.get().await?;
    let mut connection = IAsyncConnection { real: connection};
    let x: Value = cmd("ping").query_async(&mut connection).await?;
    let mut iter: AsyncIter<String> = connection.scan().await?;

    dbg!(x);
    Ok(())
}

struct IAsyncConnection {
    real: Connection,
}

impl ConnectionLike for IAsyncConnection {
    fn req_packed_command<'a>(&'a mut self, cmd: &'a Cmd) -> RedisFuture<'a, Value> {
        log_cmd(cmd);
        self.real.req_packed_command(cmd)
    }

    fn req_packed_commands<'a>(&'a mut self, cmd: &'a Pipeline, offset: usize, count: usize) -> RedisFuture<'a, Vec<Value>> {
        self.real.req_packed_commands(cmd, offset, count)
    }

    fn get_db(&self) -> i64 {
        self.real.get_db()
    }
}

pub(crate) fn log_cmd(cmd: &Cmd) {
    println!(">> {}", format_cmd(cmd));
}

fn format_cmd(cmd: &Cmd) -> String {
    let mut s = String::new();
    for x in cmd.args_iter() {
        match x {
            Arg::Simple(a) => {
                s.push_str(&format!(r#""{}""#, String::from_utf8_lossy(a)));
                s.push_str(" ");
            }
            Arg::Cursor => {
                s.push_str(r#""0""#);
            }
        }
    }
    s
}

