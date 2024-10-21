use std::io::Read;
use anyhow::Result;
use russh::client::{Config, Handle, Handler, Msg};
use russh::Channel;
use std::net::{SocketAddrV4, TcpStream};
use std::str::FromStr;
use std::sync::Arc;
use async_trait::async_trait;
use russh_keys::key::PublicKey;
use tokio::io::AsyncReadExt;

async fn create_ssh_tunnel() -> Result<(TcpStream, Channel<Msg>)> {
    let config = Config::default();
    let handler = IHandler {};
    let mut client = russh::client::connect(Arc::new(config), SocketAddrV4::from_str("xxx:xx")?, handler).await?;
    let x = client.authenticate_password("guest", "123").await?;
    println!("{}", x);
    let channel = client.channel_open_direct_tcpip(
        "xxx",
        6379,
        "127.0.0.1",
        6379,
    ).await?;
    Ok((TcpStream::connect("127.0.0.1:6379")?, channel))
}

#[tokio::main]
async fn main() -> Result<()> {
    let (mut stream, _channel) = create_ssh_tunnel().await?;

    let mut buffer = vec![0; 1024];
    let i = stream.read_to_end(&mut buffer)?;

    Ok(())
}

struct IHandler {}

#[async_trait]
impl Handler for IHandler {
    type Error = anyhow::Error;

    async fn check_server_key(&mut self, _: &PublicKey) -> std::result::Result<bool, Self::Error> {
        Ok(true)
    }


}
