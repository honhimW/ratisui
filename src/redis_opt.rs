use anyhow::{anyhow, Context, Result};
use redis::cluster::{ClusterClient, ClusterConnection};
use redis::ConnectionAddr::{Tcp, TcpTls};
use redis::{AsyncCommands, AsyncIter, Client, Cmd, Commands, Connection, ConnectionAddr, ConnectionInfo, ConnectionLike, FromRedisValue, Iter, ProtocolVersion, RedisConnectionInfo, RedisResult, ScanOptions, ToRedisArgs, Value};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{RwLock, RwLockReadGuard};
use deadpool_redis::{Pool, Runtime};
use deadpool_redis::redis::cmd;
use once_cell::sync::Lazy;
use redis::aio::ConnectionManager;
use crate::configuration::{to_protocol_version, Database};

pub static REDIS_OPERATIONS: Lazy<RwLock<Option<RedisOperations>>> = Lazy::new(|| RwLock::new(None));
pub static POOL: Lazy<RwLock<Option<Pool>>> = Lazy::new(|| RwLock::new(None));
pub static CLUSTER_POOL: Lazy<RwLock<Option<deadpool_redis::cluster::Pool>>> = Lazy::new(|| RwLock::new(None));

pub fn redis_opt<F, R>(opt: F) -> Result<R>
where
    F: FnOnce(&RedisOperations) -> Result<R>,
{
    let x = redis_operations();
    if let Some(c) = x {
        opt(&c)
    } else {
        Err(anyhow!(""))
    }
}

pub async fn async_redis_opt<F, FUT, R>(opt: F) -> Result<R>
where
    F: FnOnce(RedisOperations) -> FUT,
    FUT: Future<Output=Result<R>>
{

    let x = redis_operations();
    if let Some(c) = x {
        opt(c.clone()).await
    } else {
        Err(anyhow!(""))
    }
}

pub fn redis_operations() -> Option<RedisOperations> {
    let guard = REDIS_OPERATIONS.read().unwrap();
    guard.clone()
}

pub fn switch_client(database: &Database) -> Result<()> {
    let client = build_client(&database)?;
    let mut operation = RedisOperations::new(database.clone(), client);
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
    database: Database,
    client: Client,
    pool: Pool,
    server_info: Option<String>,
    cluster_client: Option<ClusterClient>,
    nodes: HashMap<String, NodeClientHolder>,
    cluster_pool: Option<deadpool_redis::cluster::Pool>
}

#[derive(Clone)]
struct NodeClientHolder {
    node_client: Client,
    pool: Pool,
    is_master: bool,
}

impl RedisOperations {
    fn new(database: Database, client: Client) -> Self {
        let info = deadpool_redis::ConnectionInfo::from(client.get_connection_info().clone());
        let config = deadpool_redis::Config::from_connection_info(info);
        let pool = config.create_pool(Some(Runtime::Tokio1)).unwrap();
        Self {
            database,
            client,
            pool,
            server_info: None,
            cluster_client: None,
            nodes: HashMap::new(),
            cluster_pool: None,
        }
    }

    pub async fn get_connection_manager(&self) -> Result<ConnectionManager> {
        Ok(self.client.get_connection_manager().await?)
    }

    pub fn get_connection(&self) -> Result<Connection> {
        Ok(self.client.get_connection()?)
    }

    pub fn get_cluster_connection(&self) -> Result<ClusterConnection> {
        self.assert_cluster()?;
        let cluster_client = self.cluster_client.as_ref().unwrap();
        Ok(cluster_client.get_connection()?)
    }

    fn assert_cluster(&self) -> Result<()> {
        if self.is_cluster() {
            Ok(())
        } else {
            Err(anyhow!("Not in cluster mode"))
        }
    }

    fn is_cluster(&self) -> bool {
        self.cluster_client.is_some()
    }

    fn initialize(&mut self) -> Result<()> {
        let mut connection = self.client.get_connection()?;
        let value = connection.req_command(&Cmd::new().arg("INFO").arg("SERVER"))?;
        if let Value::VerbatimString { text, .. } = value {
            self.server_info = Some(text);
            let redis_mode = self.get_server_info("redis_mode").context("there will always contain redis_mode property")?;
            if redis_mode == "cluster" {
               self.initialize_cluster()?;
            } else {
                let config = deadpool_redis::Config::from_connection_info(deadpool_redis::ConnectionInfo::from(self.client.get_connection_info().clone()));
                let pool = config.create_pool(Some(Runtime::Tokio1))?;
                self.pool = pool;
            }
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
                    let nodes = item.get(2).context("node(s) should exist")?;
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
            let mut cluster_client_infos: Vec<ConnectionInfo> = Vec::new();
            let mut node_holders: HashMap<String, NodeClientHolder> = HashMap::new();
            let connection_info = self.client.get_connection_info();
            for (host, port, id) in redis_nodes.clone() {
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
            for (host, port, id) in redis_nodes.clone() {
                let addr: ConnectionAddr;
                if self.database.use_tls {
                    addr = ConnectionAddr::TcpTls {
                        host,
                        port,
                        insecure: true,
                        tls_params: None,
                    };
                } else {
                    addr = ConnectionAddr::Tcp(host, port);
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

    pub async fn scan(&self, pattern: String, count: usize) -> Result<Vec<String>> {
        if self.is_cluster() {
            let mut all_node_keys = Vec::new();
            for (k, v) in &self.nodes {
                if v.is_master {
                    let mut connection = v.pool.get().await?;
                    let mut iter: AsyncIter<String> = connection.scan_options(ScanOptions::default().with_pattern(pattern.clone()).with_count(count)).await?;
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

    pub async fn sscan<K: ToRedisArgs + Send + Sync>(&self, key: K) -> Result<Vec<String>> {
        if self.is_cluster() {
            let pool = &self.cluster_pool.clone().context("should be cluster")?;
            let mut connection = pool.get().await?;
            let mut iter: AsyncIter<String> = connection.sscan(key).await?;
            let mut vec = vec![];
            while let Some(item) = iter.next_item().await {
                vec.push(item);
            }
            Ok(vec)
        } else {
            let mut connection = self.pool.get().await?;
            let mut iter: AsyncIter<String> = connection.sscan(key).await?;
            let mut vec = vec![];
            while let Some(item) = iter.next_item().await {
                vec.push(item);
            }
            Ok(vec)
        }
    }

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


}

#[cfg(test)]
mod tests {
    use crate::redis_opt::{async_redis_opt, build_client, switch_client, Database, RedisOperations};
    use anyhow::Result;
    use redis::{Commands, ProtocolVersion};
    use crate::configuration::Protocol;

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
        let op = RedisOperations::new(db, client);
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
        let mut op = RedisOperations::new(db, client);
        op.initialize()?;
        assert!(op.is_cluster());
        let vec = op.scan("*".to_string(), 100).await?;
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

        switch_client(&db)?;
        let string1 = async_redis_opt(|op| async move {
            let string = op.key_type("json").await?;
            Ok(string)
        }).await?;

        println!("json: {}", string1);

        Ok(())
    }
}