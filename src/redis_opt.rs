use anyhow::{anyhow, Context, Result};
use redis::cluster::{ClusterClient, ClusterConnection};
use redis::ConnectionAddr::Tcp;
use redis::{AsyncCommands, Client, Cmd, Commands, Connection, ConnectionInfo, ConnectionLike, Iter, ProtocolVersion, RedisConnectionInfo, ScanOptions, Value};
use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard};
use once_cell::sync::Lazy;
use redis::aio::ConnectionManager;
use crate::configuration::{to_protocol_version, Database};

pub static REDIS_OPERATIONS: Lazy<RwLock<Option<RedisOperations>>> = Lazy::new(|| RwLock::new(None));
pub static REDIS_CLIENT: Lazy<RwLock<Option<Client>>> = Lazy::new(|| RwLock::new(None));

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

#[derive(Clone)]
pub struct RedisOperations {
    database: Database,
    client: Client,
    server_info: Option<String>,
    cluster_client: Option<ClusterClient>,
    nodes: HashMap<String, NodeClientHolder>,
}

#[derive(Clone)]
struct NodeClientHolder {
    node_client: Client,
    is_master: bool,
}

impl RedisOperations {
    fn new(database: Database, client: Client) -> Self {
        Self {
            database,
            client,
            server_info: None,
            cluster_client: None,
            nodes: HashMap::new(),
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

    pub fn scan(mut self, pattern: String, count: usize) -> Result<Vec<String>> {
        if self.is_cluster() {
            let mut all_node_keys = Vec::new();
            for (k, v) in self.nodes {
                if v.is_master {
                    let mut connection = v.node_client.get_connection()?;
                    let iter: Iter<String> = connection.scan_options(ScanOptions::default().with_pattern(pattern.clone()).with_count(count))?;
                    let vec: Vec<String> = iter.collect();
                    all_node_keys.extend(vec);
                }
            }
            Ok(all_node_keys)
        } else {
            let mut connection = self.get_connection()?;
            let iter: Iter<String> = connection.scan_options(ScanOptions::default().with_pattern(pattern).with_count(count))?;
            let vec: Vec<String> = iter.collect();
            Ok(vec)
        }
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
            for (host, port, id) in redis_nodes {
                let mut database = Database::from(self.database.clone());
                database.host = host;
                database.port = port;
                let node_client = build_client(&database)?;
                let is_master = node_kind_map.get(&id).unwrap_or(&false);
                node_holders.insert(id, NodeClientHolder {
                    node_client,
                    is_master: *is_master,
                });
            }

            self.nodes = node_holders;
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

}

#[cfg(test)]
mod tests {
    use crate::redis_opt::{build_client, switch_client, Database, RedisOperations};
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

    #[test]
    fn test_initialize_cluster() -> Result<()> {
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
        let op = RedisOperations::new(db, client);
        assert!(op.is_cluster());
        let vec = op.scan("*".to_string(), 100)?;
        vec.iter().for_each(|item| {
            println!("{}", item);
        });

        Ok(())
    }
}