use anyhow::{Context, Result};
use log::debug;
use redis::ProtocolVersion;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use strum::{Display, EnumCount, EnumIter, IntoEnumIterator};

pub fn load_app_configuration() -> Result<Configuration> {
    let mut configuration = Configuration::default();
    let app_config_path = get_file_path("config.toml")?;

    if let Ok(mut file) = File::open(&app_config_path) {
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        if !content.is_empty() {
            configuration =  toml::from_str(&content)?;
        }
    }
    Ok(configuration)
}

pub fn load_database_configuration() -> Result<Databases> {
    let mut databases = Databases::empty();
    let db_config_path = get_file_path("databases.toml")?;

    if let Ok(mut file) = File::open(&db_config_path) {
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        if !content.is_empty() {
            databases = toml::from_str(&content)?;
        }
    }
    Ok(databases)
}

pub fn save_configuration(config: &Configuration) -> Result<()> {
    let app_config_path = get_file_path("config.toml")?;

    let toml_content = toml::to_string(&config)?;
    debug!("{}", &toml_content);
    if let Ok(mut file) = File::create(&app_config_path) {
        file.write_all(toml_content.as_ref())?;
    }
    Ok(())
}

pub fn save_database_configuration(databases: &Databases) -> Result<()> {
    let db_config_path = get_file_path("databases.toml")?;
    let toml_content = toml::to_string(&databases)?;
    debug!("{}", &toml_content);
    if let Ok(mut file) = File::create(&db_config_path) {
        file.write_all(toml_content.as_ref())?;
    }
    Ok(())
}

fn get_dir_path() -> Result<std::path::PathBuf> {
    let mut dir_path = dirs::home_dir().context("cannot get home directory")?;
    dir_path.push(".config");
    dir_path.push("ratisui");
    let cloned = dir_path.clone();
    if !dir_path.try_exists()? {
        fs::create_dir_all(dir_path).context("cannot create config directory `~/.config/ratisui`")?;
    }
    Ok(cloned)
}

fn get_file_path(file_name: &str) -> Result<std::path::PathBuf> {
    let mut dir_path = get_dir_path()?;
    dir_path.push(file_name);
    Ok(dir_path)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Configuration {
    pub fps: Option<u8>,
    pub scan_size: Option<u16>,
}

#[derive(Serialize, Deserialize)]
pub struct Databases {
    pub default_database: Option<String>,
    pub databases: HashMap<String, Database>,
}

impl Configuration {
    fn default() -> Self {
        Self {
            fps: Some(30),
            scan_size: Some(2_000),
        }
    }
}

impl Databases {
    fn empty() -> Self {
        Self {
            default_database: None,
            databases: HashMap::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Database {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub use_tls: bool,
    pub use_ssh_tunnel: bool,
    pub db: u32,
    pub protocol: Protocol,
}

impl Display for Database {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &self.password.clone().map(|p| "*".repeat(p.len())))
            .field("use_tls", &self.use_tls)
            .field("use_ssh_tunnel", &self.use_ssh_tunnel)
            .field("db", &self.db)
            .field("protocol", &self.protocol)
            .finish()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Display, EnumCount, EnumIter)]
pub enum Protocol {
    RESP2,
    RESP3,
}

pub fn to_protocol_version(protocol: Protocol) -> ProtocolVersion {
    match protocol {
        Protocol::RESP2 => ProtocolVersion::RESP2,
        Protocol::RESP3 => ProtocolVersion::RESP3,
    }
}

impl Database {
    pub fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 6379,
            username: None,
            password: None,
            use_tls: false,
            use_ssh_tunnel: false,
            db: 0,
            protocol: Protocol::RESP3,
        }
    }

    pub fn from(other: Database) -> Self {
        Self {
            ..other
        }
    }
}