#[path = "common/lib.rs"]
mod common;

use crate::common::client::build_pool;
use anyhow::Error;
use anyhow::Result;
use async_trait::async_trait;
use redis::cmd;
use russh::client::{Config, Handler};
use russh::keys::key;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::Arc;
use tokio::net::TcpListener;
use ratisui::ssh_tunnel;
use ratisui::ssh_tunnel::SshTunnel;

const SSH_HOST: &str = "10.37.1.133";
const SSH_PORT: u16 = 22;
const SSH_USER: &str = "guest";
const SSH_PASSWORD: &str = "123";

const REDIS_HOST: &str = "redis-16430.c1.asia-northeast1-1.gce.redns.redis-cloud.com";
const REDIS_PORT: u16 = 16430;

const LOCAL_HOST: &str = "127.0.0.1";

struct IHandler;

#[async_trait]
impl Handler for IHandler {
    type Error = Error;
    async fn check_server_key(&mut self, _: &key::PublicKey) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

// #[tokio::main]
// async fn main() -> Result<()> {
//     let mut client = russh::client::connect(
//         Arc::new(Config::default()),
//         format!("{SSH_HOST}:{SSH_PORT}"),
//         IHandler {},
//     ).await?;
//
//     client.authenticate_password(SSH_USER, SSH_PASSWORD).await?;
//     let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).await?;
//     let addr = listener.local_addr()?;
//
//     let channel = client.channel_open_direct_tcpip(
//         REDIS_HOST,
//         REDIS_PORT as u32,
//         LOCAL_HOST,
//         addr.port() as u32,
//     ).await?;
//
//     let mut remote_stream = channel.into_stream();
//     tokio::spawn(async move {
//         if let Ok((mut local_stream, _)) = listener.accept().await {
//             tokio::io::copy_bidirectional_with_sizes(&mut local_stream, &mut remote_stream, 255, 8 * 1024).await?;
//         }
//         Ok::<(), Error>(())
//     });
//
//     let pool = build_pool(common::client::Config {
//         port: addr.port(),
//         username: Some(String::from("default")),
//         password: Some("9JRCAjglNSTc4pXWOggLT7BKljwuoSSy".to_string()),
//         ..Default::default()
//     })?;
//     let mut connection = pool.get().await?;
//     let pong: String = cmd("PING").query_async(&mut connection).await?;
//     assert!("PONG".eq_ignore_ascii_case(pong.as_str()));
//
//     Ok(())
// }

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

    let addr = ssh_tunnel.open_ssh_tunnel().await?;
    println!("{}", addr);

    let pool = build_pool(common::client::Config {
        host: addr.ip().to_string(),
        port: addr.port(),
        username: Some(String::from("default")),
        password: Some("9JRCAjglNSTc4pXWOggLT7BKljwuoSSy".to_string()),
        ..Default::default()
    })?;
    let mut connection = pool.get().await?;
    let pong: String = cmd("PING").query_async(&mut connection).await?;
    assert!("PONG".eq_ignore_ascii_case(pong.as_str()));
    let mut connection = pool.get().await?;
    let pong: String = cmd("PING").query_async(&mut connection).await?;
    assert!("PONG".eq_ignore_ascii_case(pong.as_str()));
    let mut connection = pool.get().await?;
    let pong: String = cmd("PING").query_async(&mut connection).await?;
    assert!("PONG".eq_ignore_ascii_case(pong.as_str()));
    ssh_tunnel.close().await?;
    assert!(!ssh_tunnel.is_connected());

    Ok(())
}