use anyhow::{Context, Result};
use deadpool_redis::Runtime;
use redis::ConnectionAddr::Tcp;
use redis::{Cmd, ConnectionInfo, ProtocolVersion, RedisConnectionInfo};

pub fn dead_pool() -> Result<deadpool_redis::Pool> {
    let config = deadpool_redis::Config::from_connection_info(ConnectionInfo {
        addr: Tcp("redis-16430.c1.asia-northeast1-1.gce.redns.redis-cloud.com".to_string(), 16430),
        redis: RedisConnectionInfo {
            db: 0,
            username: Some(String::from("default")),
            password: Some("9JRCAjglNSTc4pXWOggLT7BKljwuoSSy".to_string()),
            protocol: ProtocolVersion::RESP3,
        },
    });
    config.create_pool(Some(Runtime::Tokio1)).context("Failed to create pool")
}

#[macro_export]
macro_rules! str_cmd {
    ($cmd:expr) => {{
        let mut command = Cmd::new();
        let parts: Vec<&str> = $cmd.split_whitespace().collect();
        for arg in &parts[0..] {
            command.arg(arg);
        }
        command
    }};
}
