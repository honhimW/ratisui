use crate::configuration::{to_protocol_version, Database};
use crate::utils::split_args;
use anyhow::{anyhow, Context, Error, Result};
use crossbeam_channel::{Sender};
use deadpool_redis::redis::cmd;
use deadpool_redis::{Pool, Runtime};
use futures::StreamExt;
use log::{info};
use once_cell::sync::Lazy;
use redis::cluster::ClusterClient;
use redis::ConnectionAddr::{Tcp, TcpTls};
use redis::{AsyncCommands, AsyncIter, Client, Cmd, ConnectionAddr, ConnectionInfo, ConnectionLike, FromRedisValue, RedisConnectionInfo, ScanOptions, ToRedisArgs, Value, VerbatimFormat};
use std::collections::HashMap;
use std::future::Future;
use std::sync::RwLock;
use std::task::Poll;
use std::time::{Duration, Instant};
use tokio::time::interval;

#[macro_export]
macro_rules! str_cmd {
    ($cmd:expr) => {{
        let mut command = Cmd::new();
        let parts: Vec<String> = split_args($cmd);
        for arg in &parts {
            command.arg(arg);
        }
        command
    }};
}

pub static REDIS_OPERATIONS: Lazy<RwLock<Option<RedisOperations>>> = Lazy::new(|| RwLock::new(None));

/// ```
/// let sender = sender.clone();
/// let key_to_get = key.clone();
/// spawn_redis_opt(move |operations| async move {
///     let data: String = operations.get(key_to_get).await?;
///     sender.send(data.clone())?;
///     Ok::<(), Error>(())
/// })?;
/// ```

pub fn spawn_redis_opt<F, FUT, R>(opt: F) -> Result<()>
where
    F: FnOnce(RedisOperations) -> FUT + Send + 'static,
    FUT: Future<Output=Result<R>> + Send + 'static,
{
    let x = redis_operations();
    if let Some(c) = x {
        tokio::spawn(async move {
            opt(c.clone()).await?;
            Ok::<(), Error>(())
        });
        Ok(())
    } else {
        Ok(())
    }
}

/// ```
/// let value = async_redis_opt(|operations| async move {
///     Ok(operations.get::<_, String>("key_to_get").await?)
/// }).await?;
///
/// let value: String = async_redis_opt(|operations| async move {
///     Ok(operations.get("key_to_get").await?)
/// }).await?;
/// ```
pub async fn async_redis_opt<F, FUT, R>(opt: F) -> Result<R>
where
    F: FnOnce(RedisOperations) -> FUT,
    FUT: Future<Output=Result<R>>,
{
    let x = redis_operations();
    if let Some(c) = x {
        opt(c.clone()).await
    } else {
        Err(anyhow!("redis operations not initialized"))
    }
}

pub fn redis_operations() -> Option<RedisOperations> {
    let guard = REDIS_OPERATIONS.read().unwrap();
    guard.clone()
}

pub fn switch_client(name: impl Into<String>, database: &Database) -> Result<()> {
    let client = build_client(&database)?;
    let name = name.into();
    let mut operation = RedisOperations::new(name, database.clone(), client);
    operation.initialize()?;

    let result = REDIS_OPERATIONS.write();
    match result {
        Ok(mut x) => {
            *x = Some(operation);
        }
        Err(e) => {
            return Err(anyhow!("Failed to switch client: {}", e));
        }
    }

    Ok(())
}

fn build_client(database: &Database) -> Result<Client> {
    let mut client = Client::open(ConnectionInfo {
        addr: Tcp(database.host.clone(), database.port),
        redis: RedisConnectionInfo {
            db: database.db as i64,
            username: database.username.clone(),
            password: database.password.clone(),
            protocol: to_protocol_version(database.protocol.clone()),
        },
    })?;
    if client.check_connection() {
        Ok(client)
    } else {
        Err(anyhow::anyhow!("Failed to connect"))
    }
}

fn build_pool(database: &Database) -> Result<Pool> {
    let info = ConnectionInfo {
        addr: Tcp(database.host.clone(), database.port),
        redis: RedisConnectionInfo {
            db: database.db as i64,
            username: database.username.clone(),
            password: database.password.clone(),
            protocol: to_protocol_version(database.protocol.clone()),
        },
    };
    let config = deadpool_redis::Config::from_connection_info(deadpool_redis::ConnectionInfo::from(info));
    let pool = config.create_pool(Some(Runtime::Tokio1))?;
    Ok(pool)
}

#[derive(Clone)]
pub struct RedisOperations {
    pub name: String,
    database: Database,
    client: Client,
    pool: Pool,
    server_info: Option<String>,
    cluster_client: Option<ClusterClient>,
    nodes: HashMap<String, NodeClientHolder>,
    cluster_pool: Option<deadpool_redis::cluster::Pool>,
}

#[derive(Clone, Debug)]
struct NodeClientHolder {
    node_client: Client,
    pool: Pool,
    is_master: bool,
}

impl RedisOperations {
    fn new(name: impl Into<String>, database: Database, client: Client) -> Self {
        let info = deadpool_redis::ConnectionInfo::from(client.get_connection_info().clone());
        let config = deadpool_redis::Config::from_connection_info(info);
        let pool = config.create_pool(Some(Runtime::Tokio1)).unwrap();
        Self {
            name: name.into(),
            database,
            client,
            pool,
            server_info: None,
            cluster_client: None,
            nodes: HashMap::new(),
            cluster_pool: None,
        }
    }

    // async fn get_connection(&self) -> Result<Box<dyn redis::aio::ConnectionLike>> {
    //     if self.is_cluster() {
    //         let pool = &self.cluster_pool.clone().context("should be cluster")?;
    //         let connection = pool.get().await?;
    //         Ok(Box::new(connection))
    //     } else {
    //         let connection = self.pool.get().await?;
    //         Ok(Box::new(connection))
    //     }
    // }

    // pub fn get_database(&self) -> Database {
    //     self.database.clone()
    // }

    pub fn is_cluster(&self) -> bool {
        self.cluster_client.is_some()
    }

    fn print(&self) {
        if self.is_cluster() {
            info!("Cluster mode");
            info!("Cluster nodes: {}", self.nodes.len());
            for (s, node) in self.nodes.iter() {
                info!("{s} - location: {} - master: {}", node.node_client.get_connection_info().addr, node.is_master);
            }
        } else {
            info!("Standalone mode: {}", self.client.get_connection_info().addr);
        }
    }

    fn initialize(&mut self) -> Result<()> {
        let mut connection = self.client.get_connection()?;
        let value = connection.req_command(&Cmd::new().arg("INFO").arg("SERVER"))?;
        if let Value::VerbatimString { text, .. } = value { // RESP3
            self.server_info = Some(text);
            let redis_mode = self.get_server_info("redis_mode").context("there will always contain redis_mode property")?;
            if redis_mode == "cluster" {
                self.initialize_cluster()?;
            } else {
                let config = deadpool_redis::Config::from_connection_info(deadpool_redis::ConnectionInfo::from(self.client.get_connection_info().clone()));
                let pool = config.create_pool(Some(Runtime::Tokio1))?;
                self.pool = pool;
            }
            self.print();
            Ok(())
        } else if let Value::BulkString { 0: text, .. } = value {  // RESP2
            let text = String::from_utf8(text)?;
            self.server_info = Some(text);
            let redis_mode = self.get_server_info("redis_mode").context("there will always contain redis_mode property")?;
            if redis_mode == "cluster" {
                self.initialize_cluster()?;
            } else {
                let config = deadpool_redis::Config::from_connection_info(deadpool_redis::ConnectionInfo::from(self.client.get_connection_info().clone()));
                let pool = config.create_pool(Some(Runtime::Tokio1))?;
                self.pool = pool;
            }
            self.print();
            Ok(())
        } else {
            Err(anyhow!("Failed to initialize"))
        }
    }

    fn initialize_cluster(&mut self) -> Result<()> {
        let mut connection = self.client.get_connection()?;
        let cluster_slots = connection.req_command(&Cmd::new().arg("CLUSTER").arg("SLOTS"))?;
        if let Value::Array { 0: item, .. } = cluster_slots {
            let mut redis_nodes: Vec<(String, u16, String)> = Vec::new();
            for slot in item {
                if let Value::Array { 0: item, .. } = slot {
                    // let start = item.get(0).context("start slot should exist")?;
                    // let stop = item.get(1).context("stop slot should exist")?;
                    for i in 2..item.len() {
                        let nodes = item.get(i).context("node(s) should exist")?;
                        if let Value::Array { 0: item, .. } = nodes {
                            let host = item.get(0).context("host should exist")?;
                            let port = item.get(1).context("port should exist")?;
                            let id = item.get(2).context("id should exist")?;
                            let mut _host = "".to_string();
                            let mut _port = 0;
                            let mut _id = "".to_string();
                            if let Value::BulkString(host) = host {
                                _host = String::from_utf8(host.clone())?;
                            }
                            if let Value::Int(port) = port {
                                _port = *port as u16;
                            }
                            if let Value::BulkString(id) = id {
                                _id = String::from_utf8(id.clone())?;
                            }
                            redis_nodes.push((_host, _port, _id));
                        }
                    }
                }
            }
            let mut cluster_client_infos: Vec<ConnectionInfo> = Vec::new();
            let mut node_holders: HashMap<String, NodeClientHolder> = HashMap::new();
            let connection_info = self.client.get_connection_info();
            for (host, port, _) in redis_nodes.clone() {
                cluster_client_infos.push(ConnectionInfo {
                    addr: Tcp(host.clone(), port.clone()),
                    redis: connection_info.redis.clone(),
                });
            }
            let cluster_client = ClusterClient::new(cluster_client_infos)?;
            self.cluster_client = Some(cluster_client);

            let cluster_nodes = connection.req_command(&Cmd::new().arg("CLUSTER").arg("NODES"))?;
            let mut node_kind_map: HashMap<String, bool> = HashMap::new();
            if let Value::VerbatimString { text, .. } = cluster_nodes {
                for line in text.lines() {
                    let split: Vec<&str> = line.split(" ").collect();
                    let node_kind = split[2];
                    node_kind_map.insert(split[0].to_string(), node_kind.contains("master"));
                }
            }
            for (host, port, id) in redis_nodes.clone() {
                let mut database = Database::from(self.database.clone());
                database.host = host;
                database.port = port;
                let node_client = build_client(&database)?;
                let pool = build_pool(&database)?;
                let is_master = node_kind_map.get(&id).unwrap_or(&false);
                node_holders.insert(id, NodeClientHolder {
                    node_client,
                    pool,
                    is_master: *is_master,
                });
            }
            self.nodes = node_holders;
            let mut cluster_urls = vec![];
            for (host, port, _) in redis_nodes.clone() {
                let addr: ConnectionAddr;
                if self.database.use_tls {
                    addr = TcpTls {
                        host,
                        port,
                        insecure: true,
                        tls_params: None,
                    };
                } else {
                    addr = Tcp(host, port);
                }
                let info = ConnectionInfo {
                    addr,
                    redis: RedisConnectionInfo {
                        db: 0,
                        username: self.database.username.clone(),
                        password: self.database.password.clone(),
                        protocol: to_protocol_version(self.database.protocol.clone()),
                    },
                };
                cluster_urls.push(deadpool_redis::ConnectionInfo::from(info))
            }
            let config = deadpool_redis::cluster::Config {
                urls: None,
                connections: Some(cluster_urls),
                pool: None,
                read_from_replicas: true,
            };
            let pool = config.create_pool(Some(Runtime::Tokio1))?;
            self.cluster_pool = Some(pool);
            Ok(())
        } else {
            Err(anyhow!("Failed to initialize cluster"))
        }
    }

    fn get_server_info(&self, key: &str) -> Option<String> {
        if let Some(server_info) = &self.server_info {
            for line in server_info.lines() {
                if !line.starts_with("#") {
                    let mut split = line.splitn(2, ":");
                    if let Some(k) = split.next() {
                        if k == key {
                            if let Some(v) = split.next() {
                                return Some(v.to_string());
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub async fn str_cmd(&self, cmd: impl Into<String>) -> Result<Value> {
        let cmd = cmd.into();
        let cmd = str_cmd!(cmd.as_str());
        self.cmd(cmd).await
    }

    pub async fn cmd(&self, cmd: Cmd) -> Result<Value> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            let v: Value = cmd.query_async(&mut connection).await?;
            Ok(v)
        } else {
            let mut connection = self.pool.get().await?;
            let v: Value = cmd.query_async(&mut connection).await?;
            Ok(v)
        }
    }

    pub async fn monitor(&self, sender: Sender<Value>) -> Result<impl Disposable> {
        struct DisposableMonitor(tokio::sync::watch::Sender<bool>);

        impl Disposable for DisposableMonitor {
            fn disposable(&mut self) -> Result<()> {
                self.0.send(true)?;
                Ok(())
            }
        }
        tokio::sync::mpsc::channel::<bool>(2);
        let (tx, rx) = tokio::sync::watch::channel(false);
        let disposable_monitor = DisposableMonitor(tx);
        let mut streams = vec![];
        if self.is_cluster() {
            for (_, holder) in self.nodes.iter() {
                let mut monitor = holder.node_client.get_async_monitor().await?;
                let _ = monitor.monitor().await?;
                let stream = monitor.into_on_message::<Value>();
                streams.push(stream);
            }
        } else {
            let mut monitor = self.client.get_async_monitor().await?;
            let _ = monitor.monitor().await?;
            let stream = monitor.into_on_message::<Value>();
            streams.push(stream);
        }
        tokio::spawn(async move {
            let mut gap = Duration::from_secs(60);
            let mut anchor = Instant::now();
            let mut loop_interval = interval(Duration::from_millis(50));
            loop {
                loop_interval.tick().await;
                match rx.has_changed() {
                    Ok(has_changed) => {
                        if has_changed {
                            let stop = *rx.borrow();
                            if stop {
                                break;
                            }
                        }
                    }
                    Err(_) => {
                        // means tx is release
                        break;
                    }
                }
                let waker = futures::task::noop_waker_ref();
                let mut context = std::task::Context::from_waker(waker);
                for stream in streams.iter_mut() {
                    loop {
                        let poll = stream.poll_next_unpin(&mut context);
                        match poll {
                            Poll::Ready(Some(v)) => {
                                sender.send(v)?;
                                anchor = Instant::now();
                                gap = Duration::from_secs(60);
                            }
                            Poll::Ready(None) => {
                                break;
                            }
                            Poll::Pending => {
                                let duration = anchor.elapsed();
                                if duration > gap {
                                    sender.send(Value::SimpleString(format!("Pending {}s ...", duration.as_secs())))?;
                                    gap = gap + Duration::from_secs(60);
                                }
                                break;
                            }
                        };
                    }
                }
            }
            drop(streams);
            sender.send(Value::VerbatimString {
                format: VerbatimFormat::Unknown("PROMPT".to_string()),
                text: "Monitor has gracefully shut down.".to_string(),
            })?;
            Ok::<(), Error>(())
        });
        Ok(disposable_monitor)
    }

    pub async fn subscribe<K: ToRedisArgs + Send + Sync>(&self, key: K, sender: Sender<Value>) -> Result<impl Disposable> {
        struct DisposableMonitor(tokio::sync::watch::Sender<bool>);

        impl Disposable for DisposableMonitor {
            fn disposable(&mut self) -> Result<()> {
                self.0.send(true)?;
                Ok(())
            }
        }
        tokio::sync::mpsc::channel::<bool>(2);
        let (tx, rx) = tokio::sync::watch::channel(false);
        let disposable_monitor = DisposableMonitor(tx);
        let mut streams = vec![];
        if self.is_cluster() {
            for (_, holder) in self.nodes.iter() {
                if holder.is_master {
                    let mut pub_sub = holder.node_client.get_async_pubsub().await?;
                    pub_sub.subscribe(&key).await?;
                    let stream = pub_sub.into_on_message();
                    streams.push(stream);
                }
            }
        } else {
            let mut pub_sub = self.client.get_async_pubsub().await?;
            pub_sub.subscribe(&key).await?;
            let stream = pub_sub.into_on_message();
            streams.push(stream);
        }
        tokio::spawn(async move {
            let mut gap = Duration::from_secs(60);
            let mut anchor = Instant::now();
            let mut loop_interval = interval(Duration::from_millis(50));
            loop {
                loop_interval.tick().await;
                match rx.has_changed() {
                    Ok(has_changed) => {
                        if has_changed {
                            let stop = *rx.borrow();
                            if stop {
                                break;
                            }
                        }
                    }
                    Err(_) => {
                        // means tx is release
                        break;
                    }
                }
                let waker = futures::task::noop_waker_ref();
                let mut context = std::task::Context::from_waker(waker);
                for stream in streams.iter_mut() {
                    loop {
                        let poll = stream.poll_next_unpin(&mut context);
                        match poll {
                            Poll::Ready(Some(msg)) => {
                                let channel_name = msg.get_channel_name();
                                let payload = msg.get_payload::<Value>()?;
                                let value = Value::Map(vec![(Value::SimpleString(channel_name.to_string()), payload)]);
                                sender.send(value)?;
                                anchor = Instant::now();
                                gap = Duration::from_secs(60);
                            }
                            Poll::Ready(None) => {
                                break;
                            }
                            Poll::Pending => {
                                let duration = anchor.elapsed();
                                if duration > gap {
                                    sender.send(Value::SimpleString(format!("Pending {}s ...", duration.as_secs())))?;
                                    gap = gap + Duration::from_secs(60);
                                }
                                break;
                            }
                        };
                    }
                }
            }
            drop(streams);
            sender.send(Value::VerbatimString {
                format: VerbatimFormat::Unknown("PROMPT".to_string()),
                text: "Subscriber has gracefully shut down.".to_string(),
            })?;
            Ok::<(), Error>(())
        });
        Ok(disposable_monitor)
    }

    pub async fn psubscribe<K: ToRedisArgs + Send + Sync>(&self, key: K, sender: Sender<Value>) -> Result<impl Disposable> {
        struct DisposableMonitor(tokio::sync::watch::Sender<bool>);

        impl Disposable for DisposableMonitor {
            fn disposable(&mut self) -> Result<()> {
                self.0.send(true)?;
                Ok(())
            }
        }
        tokio::sync::mpsc::channel::<bool>(2);
        let (tx, rx) = tokio::sync::watch::channel(false);
        let disposable_monitor = DisposableMonitor(tx);
        let mut streams = vec![];
        if self.is_cluster() {
            for (_, holder) in self.nodes.iter() {
                if holder.is_master {
                    let mut pub_sub = holder.node_client.get_async_pubsub().await?;
                    pub_sub.psubscribe(&key).await?;
                    let stream = pub_sub.into_on_message();
                    streams.push(stream);
                }
            }
        } else {
            let mut pub_sub = self.client.get_async_pubsub().await?;
            pub_sub.psubscribe(&key).await?;
            let stream = pub_sub.into_on_message();
            streams.push(stream);
        }
        tokio::spawn(async move {
            let mut gap = Duration::from_secs(60);
            let mut anchor = Instant::now();
            let mut loop_interval = interval(Duration::from_millis(50));
            loop {
                loop_interval.tick().await;
                match rx.has_changed() {
                    Ok(has_changed) => {
                        if has_changed {
                            let stop = *rx.borrow();
                            if stop {
                                break;
                            }
                        }
                    }
                    Err(_) => {
                        // means tx is release
                        break;
                    }
                }
                let waker = futures::task::noop_waker_ref();
                let mut context = std::task::Context::from_waker(waker);
                for stream in streams.iter_mut() {
                    loop {
                        let poll = stream.poll_next_unpin(&mut context);
                        match poll {
                            Poll::Ready(Some(msg)) => {
                                let channel_name = msg.get_channel_name();
                                let payload = msg.get_payload::<Value>()?;
                                let value = Value::Map(vec![(Value::SimpleString(channel_name.to_string()), payload)]);
                                sender.send(value)?;
                                anchor = Instant::now();
                                gap = Duration::from_secs(60);
                            }
                            Poll::Ready(None) => {
                                break;
                            }
                            Poll::Pending => {
                                let duration = anchor.elapsed();
                                if duration > gap {
                                    sender.send(Value::SimpleString(format!("Pending {}s ...", duration.as_secs())))?;
                                    gap = gap + Duration::from_secs(60);
                                }
                                break;
                            }
                        };
                    }
                }
            }
            drop(streams);
            sender.send(Value::VerbatimString {
                format: VerbatimFormat::Unknown("PROMPT".to_string()),
                text: "P-Subscriber has gracefully shut down.".to_string(),
            })?;
            Ok::<(), Error>(())
        });
        Ok(disposable_monitor)
    }

    pub async fn scan(&self, pattern: impl Into<String>, count: usize) -> Result<Vec<String>> {
        let pattern = &pattern.into();
        if self.is_cluster() {
            let mut all_node_keys = Vec::new();
            for (_, v) in &self.nodes {
                if v.is_master {
                    let mut connection = v.pool.get().await?;
                    let mut iter: AsyncIter<String> = connection.scan_options(ScanOptions::default().with_pattern(pattern).with_count(count)).await?;
                    let mut vec: Vec<String> = vec![];
                    while let Some(item) = iter.next_item().await {
                        vec.push(item);
                    }
                    all_node_keys.extend(vec);
                }
            }
            Ok(all_node_keys)
        } else {
            let mut connection = self.pool.get().await?;
            let mut iter: AsyncIter<String> = connection.scan_options(ScanOptions::default().with_pattern(pattern).with_count(count)).await?;
            let mut vec: Vec<String> = vec![];
            while let Some(item) = iter.next_item().await {
                vec.push(item);
            }
            Ok(vec)
        }
    }

    pub async fn get<K: ToRedisArgs + Send + Sync, V: FromRedisValue>(&self, key: K) -> Result<V> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            let v: V = connection.get(key).await?;
            Ok(v)
        } else {
            let mut connection = self.pool.get().await?;
            let v: V = connection.get(key).await?;
            Ok(v)
        }
    }

    pub async fn get_list<K: ToRedisArgs + Send + Sync, V: FromRedisValue>(&self, key: K) -> Result<V> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            let v: V = connection.lrange(key, 0, -1).await?;

            Ok(v)
        } else {
            let mut connection = self.pool.get().await?;
            let v: V = connection.lrange(key, 0, -1).await?;
            Ok(v)
        }
    }

    pub async fn get_set<K: ToRedisArgs + Send + Sync, V: FromRedisValue>(&self, key: K) -> Result<V> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            let v: V = connection.smembers(key).await?;

            Ok(v)
        } else {
            let mut connection = self.pool.get().await?;
            let v: V = connection.smembers(key).await?;
            Ok(v)
        }
    }

    pub async fn get_zset<K: ToRedisArgs + Send + Sync, V: FromRedisValue>(&self, key: K) -> Result<V> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            let v: V = connection.zrange_withscores(key, 0, -1).await?;
            Ok(v)
        } else {
            let mut connection = self.pool.get().await?;
            let v: V = connection.zrange_withscores(key, 0, -1).await?;
            Ok(v)
        }
    }

    pub async fn get_hash<K: ToRedisArgs + Send + Sync, V: FromRedisValue>(&self, key: K) -> Result<V> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            let v: V = connection.hgetall(key).await?;
            Ok(v)
        } else {
            let mut connection = self.pool.get().await?;
            let v: V = connection.hgetall(key).await?;
            Ok(v)
        }
    }

    pub async fn key_type<K: ToRedisArgs + Send + Sync>(&self, key: K) -> Result<String> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            let v: String = connection.key_type(key).await?;
            Ok(v)
        } else {
            let mut connection = self.pool.get().await?;
            let v: String = connection.key_type(key).await?;
            Ok(v)
        }
    }

    pub async fn ttl<K: ToRedisArgs + Send + Sync>(&self, key: K) -> Result<i64> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            let v: i64 = connection.ttl(key).await?;
            Ok(v)
        } else {
            let mut connection = self.pool.get().await?;
            let v: i64 = connection.ttl(key).await?;
            Ok(v)
        }
    }

    pub async fn expire<K: ToRedisArgs + Send + Sync>(&self, key: K, seconds: i64) -> Result<()> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            connection.expire(key, seconds).await?;
            Ok(())
        } else {
            let mut connection = self.pool.get().await?;
            connection.expire(key, seconds).await?;
            Ok(())
        }
    }

    #[allow(unused)]
    pub async fn persist<K: ToRedisArgs + Send + Sync>(&self, key: K) -> Result<()> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            connection.persist(key).await?;
            Ok(())
        } else {
            let mut connection = self.pool.get().await?;
            connection.persist(key).await?;
            Ok(())
        }
    }

    pub async fn strlen<K: ToRedisArgs + Send + Sync>(&self, key: K) -> Result<usize> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            let v: usize = connection.strlen(key).await?;
            Ok(v)
        } else {
            let mut connection = self.pool.get().await?;
            let v: usize = connection.strlen(key).await?;
            Ok(v)
        }
    }

    pub async fn llen<K: ToRedisArgs + Send + Sync>(&self, key: K) -> Result<usize> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            let v: usize = connection.llen(key).await?;
            Ok(v)
        } else {
            let mut connection = self.pool.get().await?;
            let v: usize = connection.llen(key).await?;
            Ok(v)
        }
    }

    pub async fn scard<K: ToRedisArgs + Send + Sync>(&self, key: K) -> Result<usize> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            let v: usize = connection.scard(key).await?;
            Ok(v)
        } else {
            let mut connection = self.pool.get().await?;
            let v: usize = connection.scard(key).await?;
            Ok(v)
        }
    }

    pub async fn zcard<K: ToRedisArgs + Send + Sync>(&self, key: K) -> Result<usize> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            let v: usize = connection.zcard(key).await?;
            Ok(v)
        } else {
            let mut connection = self.pool.get().await?;
            let v: usize = connection.zcard(key).await?;
            Ok(v)
        }
    }

    pub async fn hlen<K: ToRedisArgs + Send + Sync>(&self, key: K) -> Result<usize> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            let v: usize = connection.hlen(key).await?;
            Ok(v)
        } else {
            let mut connection = self.pool.get().await?;
            let v: usize = connection.hlen(key).await?;
            Ok(v)
        }
    }

    pub async fn xlen<K: ToRedisArgs + Send + Sync>(&self, key: K) -> Result<usize> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            let v: usize = connection.xlen(key).await?;
            Ok(v)
        } else {
            let mut connection = self.pool.get().await?;
            let v: usize = connection.xlen(key).await?;
            Ok(v)
        }
    }

    // pub async fn sscan<K: ToRedisArgs + Send + Sync>(&self, key: K) -> Result<Vec<String>> {
    //     if self.is_cluster() {
    //         let pool = &self.cluster_pool.clone().context("should be cluster")?;
    //         let mut connection = pool.get().await?;
    //         let mut iter: AsyncIter<String> = connection.sscan(key).await?;
    //         let mut vec = vec![];
    //         while let Some(item) = iter.next_item().await {
    //             vec.push(item);
    //         }
    //         Ok(vec)
    //     } else {
    //         let mut connection = self.pool.get().await?;
    //         let mut iter: AsyncIter<String> = connection.sscan(key).await?;
    //         let mut vec = vec![];
    //         while let Some(item) = iter.next_item().await {
    //             vec.push(item);
    //         }
    //         Ok(vec)
    //     }
    // }

    pub async fn mem_usage<K: ToRedisArgs + Send + Sync>(&self, key: K) -> Result<i64> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            let v: Value = cmd("MEMORY").arg("USAGE")
                .arg(key)
                .arg("SAMPLES").arg("0")
                .query_async(&mut connection)
                .await?;
            if let Value::Int(int) = v {
                Ok(int)
            } else {
                Ok(0)
            }
        } else {
            let mut connection = self.pool.get().await?;
            let v: Value = cmd("MEMORY").arg("USAGE")
                .arg(key)
                .arg("SAMPLES").arg("0")
                .query_async(&mut connection)
                .await?;
            if let Value::Int(int) = v {
                Ok(int)
            } else {
                Ok(0)
            }
        }
    }

    pub async fn del<K: ToRedisArgs + Send + Sync>(&self, key: K) -> Result<()> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            connection.del(key).await?;
            Ok(())
        } else {
            let mut connection = self.pool.get().await?;
            connection.del(key).await?;
            Ok(())
        }
    }

    pub async fn rename_nx<K: ToRedisArgs + Send + Sync>(&self, old_key: K, new_key: K) -> Result<()> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            connection.rename_nx(old_key, new_key).await?;
            Ok(())
        } else {
            let mut connection = self.pool.get().await?;
            connection.rename_nx(old_key, new_key).await?;
            Ok(())
        }
    }

    #[allow(unused)]
    pub async fn exists<K: ToRedisArgs + Send + Sync>(&self, key: K) -> Result<bool> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            let exists: bool = connection.exists(key).await?;
            Ok(exists)
        } else {
            let mut connection = self.pool.get().await?;
            let exists: bool = connection.exists(key).await?;
            Ok(exists)
        }
    }

    pub async fn set_nx<K: ToRedisArgs + Send + Sync, V: ToRedisArgs + Send + Sync>(&self, key: K, value: V) -> Result<()> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            connection.set_nx(key, value).await?;
            Ok(())
        } else {
            let mut connection = self.pool.get().await?;
            connection.set_nx(key, value).await?;
            Ok(())
        }
    }

    pub async fn hset_nx<K: ToRedisArgs + Send + Sync, F: ToRedisArgs + Send + Sync, V: ToRedisArgs + Send + Sync>(&self, key: K, field: F, value: V) -> Result<()> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            connection.hset_nx(key, field, value).await?;
            Ok(())
        } else {
            let mut connection = self.pool.get().await?;
            connection.hset_nx(key, field, value).await?;
            Ok(())
        }
    }

    /// rpushx key element [element ...]
    /// Appends an element to a list only when the list exists.
    #[allow(unused)]
    pub async fn rpush<K: ToRedisArgs + Send + Sync, V: ToRedisArgs + Send + Sync>(&self, key: K, value: V) -> Result<()> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            connection.rpush(key, value).await?;
            Ok(())
        } else {
            let mut connection = self.pool.get().await?;
            connection.rpush(key, value).await?;
            Ok(())
        }
    }

    pub async fn lpush<K: ToRedisArgs + Send + Sync, V: ToRedisArgs + Send + Sync>(&self, key: K, value: V) -> Result<()> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            connection.lpush(key, value).await?;
            Ok(())
        } else {
            let mut connection = self.pool.get().await?;
            connection.lpush(key, value).await?;
            Ok(())
        }
    }

    pub async fn sadd<K: ToRedisArgs + Send + Sync, V: ToRedisArgs + Send + Sync>(&self, key: K, value: V) -> Result<()> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            connection.sadd(key, value).await?;
            Ok(())
        } else {
            let mut connection = self.pool.get().await?;
            connection.sadd(key, value).await?;
            Ok(())
        }
    }

    pub async fn zadd<K: ToRedisArgs + Send + Sync, V: ToRedisArgs + Send + Sync>(&self, key: K, value: V, score: f64) -> Result<()> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            connection.zadd(key, value, score).await?;
            Ok(())
        } else {
            let mut connection = self.pool.get().await?;
            connection.zadd(key, value, score).await?;
            Ok(())
        }
    }

    pub async fn xadd<K: ToRedisArgs + Send + Sync, F: ToRedisArgs + Send + Sync, V: ToRedisArgs + Send + Sync>(&self, key: K, field: F, value: V) -> Result<()> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            connection.xadd(key, "*", &[(field, value)]).await?;
            Ok(())
        } else {
            let mut connection = self.pool.get().await?;
            connection.xadd(key, "*", &[(field, value)]).await?;
            Ok(())
        }
    }
}

pub trait Disposable: Send {
    fn disposable(&mut self) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use crate::configuration::Protocol;
    use crate::redis_opt::{async_redis_opt, build_client, switch_client, Database, RedisOperations};
    use anyhow::Result;
    use redis::Commands;

    #[test]
    fn test_get_server_info() -> Result<()> {
        let mut db = Database {
            host: "10.37.1.132".to_string(),
            port: 6379,
            username: None,
            password: Some("123456".to_string()),
            use_tls: false,
            use_ssh_tunnel: false,
            db: 0,
            protocol: Protocol::RESP3,
        };

        let client = build_client(&db)?;
        let op = RedisOperations::new("", db, client);
        let option = op.get_server_info("redis_version");
        print!("redis_version: {:?}", option);

        Ok(())
    }

    #[tokio::test]
    async fn test_initialize_cluster() -> Result<()> {
        let mut db = Database {
            host: "10.37.1.133".to_string(),
            port: 6001,
            username: None,
            password: Some("123456".to_string()),
            use_tls: false,
            use_ssh_tunnel: false,
            db: 0,
            protocol: Protocol::RESP3,
        };

        let client = build_client(&db)?;
        let mut op = RedisOperations::new("", db, client);
        op.initialize()?;
        assert!(op.is_cluster());
        let vec = op.scan("*", 100).await?;
        vec.iter().for_each(|item| {
            println!("{}", item);
        });

        Ok(())
    }

    #[tokio::test]
    async fn test_get_key_type() -> Result<()> {
        let mut db = Database {
            host: "10.37.1.132".to_string(),
            port: 6379,
            username: None,
            password: Some("123456".to_string()),
            use_tls: false,
            use_ssh_tunnel: false,
            db: 0,
            protocol: Protocol::RESP3,
        };

        switch_client("", &db)?;
        let string1 = async_redis_opt(|op| async move {
            let string = op.key_type("json").await?;
            Ok(string)
        }).await?;

        println!("json: {}", string1);

        Ok(())
    }
}