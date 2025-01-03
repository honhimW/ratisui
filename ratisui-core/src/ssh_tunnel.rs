use anyhow::{Error, Result};
use async_trait::async_trait;
use log::{error, info, warn};
use russh::client::{Config, Handler};
use russh::keys::{PublicKey};
use russh::Disconnect;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::select;

/// The locally opened TcpListener here can be accessed by other processes, which may not be secure.
#[derive(Clone, Debug)]
pub struct SshTunnel {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub forwarding_host: String,
    pub forwarding_port: u16,
    tx: tokio::sync::watch::Sender<u8>,
    rx: tokio::sync::watch::Receiver<u8>,
    socket_addr: Option<SocketAddr>,
}

impl SshTunnel {
    pub fn new(host: String, port: u16, username: String, password: String, forwarding_host: String, forwarding_port: u16) -> Self {
        let (tx, rx) = tokio::sync::watch::channel::<u8>(1);
        Self {
            host,
            port,
            username,
            password,
            forwarding_host,
            forwarding_port,
            tx,
            rx,
            socket_addr: None,
        }
    }

    pub async fn open(&mut self) -> Result<SocketAddr> {
        if let Some(addr) = self.socket_addr {
            return Ok(addr);
        }
        let mut ssh_client = russh::client::connect(
            Arc::new(Config::default()),
            format!("{}:{}", self.host, self.port),
            IHandler {},
        ).await?;
        ssh_client.authenticate_password(self.username.clone(), self.password.clone()).await?;
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).await?;
        let addr = listener.local_addr()?;
        let forwarding_host = self.forwarding_host.clone();
        let forwarding_port = self.forwarding_port as u32;

        let rx_clone = self.rx.clone();
        tokio::spawn(async move {
            loop {
                let mut rx_clone_clone = rx_clone.clone();
                if let Ok((mut local_stream, _)) = listener.accept().await {
                    let channel = ssh_client.channel_open_direct_tcpip(
                        forwarding_host.clone(),
                        forwarding_port,
                        Ipv4Addr::LOCALHOST.to_string(),
                        addr.port() as u32,
                    ).await?;
                    let mut remote_stream = channel.into_stream();
                    tokio::spawn(async move {
                        select! {
                            result = tokio::io::copy_bidirectional_with_sizes(&mut local_stream, &mut remote_stream, 255, 8 * 1024) => {
                                if let Err(e) = result {
                                    error!("Error during bidirectional copy: {}", e);
                                }
                                warn!("Bidirectional copy stopped");
                            }
                            _ = rx_clone_clone.changed() => {
                                info!("Received close signal");
                            }
                        }
                        let _ = remote_stream.shutdown().await;
                    });
                }
                if rx_clone.has_changed()? {
                    ssh_client.disconnect(Disconnect::ByApplication, "exit", "none").await?;
                    break;
                }
            }
            drop(listener);
            info!("Stream closed");
            Ok::<(), Error>(())
        });

        self.socket_addr = Some(addr);
        Ok(addr)
    }

    pub async fn close(&mut self) -> Result<()> {
        self.tx.send(0)?;
        self.socket_addr = None;
        Ok(())
    }

    #[allow(unused)]
    pub fn is_connected(&self) -> bool {
        self.socket_addr.is_some()
    }
}

struct IHandler;

#[async_trait]
impl Handler for IHandler {
    type Error = Error;

    async fn check_server_key(&mut self, _: &PublicKey) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use std::net::{Ipv4Addr, SocketAddrV4};
    use std::time::{Duration, Instant};
    use tokio::net::{TcpListener, TcpStream};

    #[tokio::test]
    async fn tcp_listener() -> Result<()> {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).await?;
        let addr = listener.local_addr()?;

        tokio::spawn(async move {
            let now = Instant::now();
            loop {
                if let Ok((stream, _)) = listener.accept().await {
                    println!("{:?} {:?}", now.elapsed(), stream);
                } else {
                    println!("No connection");
                }
            }
        });
        tokio::time::sleep(Duration::from_secs(1)).await;
        let _ = TcpStream::connect(addr).await?;
        tokio::time::sleep(Duration::from_secs(1)).await;
        let _ = TcpStream::connect(addr).await?;
        tokio::time::sleep(Duration::from_secs(1)).await;
        let _ = TcpStream::connect(addr).await?;
        tokio::time::sleep(Duration::from_secs(1)).await;
        let _ = TcpStream::connect(addr).await?;
        tokio::time::sleep(Duration::from_secs(1)).await;
        Ok(())
    }
}