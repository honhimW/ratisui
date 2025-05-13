use anyhow::{Context, Result};
use deadpool_redis::Runtime;
use deadpool_redis::redis::ConnectionAddr::Tcp;
use deadpool_redis::redis::{ConnectionInfo, ProtocolVersion, RedisConnectionInfo};

pub fn dead_pool() -> Result<deadpool_redis::Pool> {
    build_pool(Config {
        host: "beta.vm.internal".to_string(),
        port: 6380,
        username: None,
        password: Some("123456".to_string()),
        db: 0,
        protocol: ProtocolVersion::RESP3,
    })
    // build_pool(Config {
    //     host: "redis-16430.c1.asia-northeast1-1.gce.redns.redis-cloud.com".to_string(),
    //     port: 16430,
    //     username: Some(String::from("default")),
    //     password: Some("9JRCAjglNSTc4pXWOggLT7BKljwuoSSy".to_string()),
    //     db: 0,
    //     protocol: ProtocolVersion::RESP3,
    // })
}

pub fn build_pool(config: Config) -> Result<deadpool_redis::Pool> {
    let config = deadpool_redis::Config::from_connection_info(ConnectionInfo {
        addr: Tcp(config.host, config.port),
        redis: RedisConnectionInfo {
            db: config.db as i64,
            username: config.username,
            password: config.password,
            protocol: config.protocol,
        },
    });
    config
        .create_pool(Some(Runtime::Tokio1))
        .context("Failed to create pool")
}

pub struct Config {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub db: u8,
    pub protocol: ProtocolVersion,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 6379,
            username: None,
            password: None,
            db: 0,
            protocol: ProtocolVersion::RESP3,
        }
    }
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
