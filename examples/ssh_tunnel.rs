use anyhow::Result;
use anyhow::Error;
use async_trait::async_trait;
use russh::client::{Config, Handler};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::Arc;
use russh::keys::key;
use tokio::net::TcpListener;

const SSH_HOST: &str = "10.37.1.133";
const SSH_PORT: u16 = 22;

const REDIS_HOST: &str = "10.37.1.132";
const REDIS_PORT: u16 = 6379;

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
    let mut client = russh::client::connect(
        Arc::new(Config::default()),
        format!("{SSH_HOST}:{SSH_PORT}"),
        IHandler {},
    ).await?;

    client.authenticate_password("guest", "123").await?;
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).await?;
    let addr = listener.local_addr()?;

    let channel = client.channel_open_direct_tcpip(
        REDIS_HOST,
        REDIS_PORT as u32,
        LOCAL_HOST,
        addr.port() as u32,
    ).await?;

    let mut remote_stream = channel.into_stream();
    tokio::spawn(async move {
        if let Ok((mut local_stream, _)) = listener.accept().await {
            tokio::io::copy_bidirectional_with_sizes(&mut local_stream, &mut remote_stream, 255, 8 * 1024).await?;
        }
        Ok::<(), Error>(())
    });

    let client = redis::Client::open(format!("redis://:123456@{LOCAL_HOST}:{}/", addr.port()))?;
    let mut con = client.get_connection()?;

    let pong: String = redis::cmd("PING").query(&mut con)?;
    println!("Redis PING response: {}", pong);
    let pong: String = redis::cmd("PING").query(&mut con)?;
    println!("Redis PING response: {}", pong);
    let pong: String = redis::cmd("PING").query(&mut con)?;
    println!("Redis PING response: {}", pong);

    Ok(())
}
