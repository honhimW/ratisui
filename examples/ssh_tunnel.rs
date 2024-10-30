#[path = "common/lib.rs"]
mod common;

use crate::common::client::build_pool;
use anyhow::Error;
use anyhow::Result;
use async_trait::async_trait;
use ratisui::ssh_tunnel::SshTunnel;
use redis::cmd;
use russh::client::Handler;
use russh::keys::key;
use std::ops::Deref;

const SSH_HOST: &str = "10.37.1.133";
const SSH_PORT: u16 = 22;
const SSH_USER: &str = "guest";
const SSH_PASSWORD: &str = "123";

const REDIS_HOST: &str = "redis-16430.c1.asia-northeast1-1.gce.redns.redis-cloud.com";
const REDIS_PORT: u16 = 16430;
const REDIS_USER: Some(String) = Some(String::from("default"));
const REDIS_PASSWORD: Some(String) = Some(String::from("9JRCAjglNSTc4pXWOggLT7BKljwuoSSy"));

const LOCAL_HOST: &str = "127.0.0.1";

struct IHandler;

#[async_trait]
impl Handler for IHandler {
    type Error = Error;
    async fn check_server_key(&mut self, _: &key::PublicKey) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut ssh_tunnel = SshTunnel::new(
        SSH_HOST.to_string(),
        SSH_PORT,
        SSH_USER.to_string(),
        SSH_PASSWORD.to_string(),
        REDIS_HOST.to_string(),
        REDIS_PORT,
    );

    let addr = ssh_tunnel.open().await?;
    println!("{}", addr);

    let pool = build_pool(common::client::Config {
        host: addr.ip().to_string(),
        port: addr.port(),
        username: REDIS_USER.deref().clone(),
        password: REDIS_PASSWORD.deref().clone(),
        ..Default::default()
    })?;
    let mut connection = pool.get().await?;
    let pong: String = cmd("PING").query_async(&mut connection).await?;
    assert_eq!(pool.status().size, 1);
    assert!("PONG".eq_ignore_ascii_case(pong.as_str()));
    let mut connection = pool.get().await?;
    let pong: String = cmd("PING").query_async(&mut connection).await?;
    assert_eq!(pool.status().size, 3);
    assert!("PONG".eq_ignore_ascii_case(pong.as_str()));
    let mut connection = pool.get().await?;
    let pong: String = cmd("PING").query_async(&mut connection).await?;
    assert_eq!(pool.status().size, 3);
    assert!("PONG".eq_ignore_ascii_case(pong.as_str()));
    ssh_tunnel.close().await?;
    assert!(!ssh_tunnel.is_connected());

    Ok(())
}