use anyhow::{Context, Result};
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use log::debug;
use redis::ProtocolVersion;
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use ron::ser::PrettyConfig;
use strum::{Display, EnumCount, EnumIter};

pub fn load_app_configuration() -> Result<Configuration> {
    let mut configuration = Configuration::default();
    let app_config_path = get_file_path("config.ron")?;

    if let Ok(mut file) = File::open(&app_config_path) {
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        if !content.is_empty() {
            configuration = ron::from_str(&content)?;
        }
    }
    Ok(configuration)
}

pub fn load_database_configuration() -> Result<Databases> {
    let mut databases = Databases::empty();
    let db_config_path = get_file_path("databases.ron")?;

    if let Ok(mut file) = File::open(&db_config_path) {
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        if !content.is_empty() {
            databases = ron::from_str(&content)?;
        }
    }
    Ok(databases)
}

#[allow(unused)]
pub fn save_configuration(config: &Configuration) -> Result<()> {
    let app_config_path = get_file_path("config.ron")?;

    let ron_content = ron::to_string(&config)?;
    debug!("{}", &ron_content);
    if let Ok(mut file) = File::create(&app_config_path) {
        file.write_all(ron_content.as_ref())?;
    }
    Ok(())
}

pub fn save_database_configuration(databases: &Databases) -> Result<()> {
    let db_config_path = get_file_path("databases.ron")?;
    let ron_content = ron::ser::to_string_pretty(&databases, PrettyConfig::default())?;
    debug!("{}", &ron_content);
    if let Ok(mut file) = File::create(&db_config_path) {
        file.write_all(ron_content.as_ref())?;
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
    pub fn empty() -> Self {
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
    #[serde(default, serialize_with = "to_base64", deserialize_with = "from_base64")]
    pub password: Option<String>,
    pub use_tls: bool,
    pub use_ssh_tunnel: bool,
    pub db: u32,
    pub protocol: Protocol,
}

fn to_base64<S: Serializer>(password: &Option<String>, s: S) -> Result<S::Ok, S::Error> {
    match password {
        Some(p) => s.serialize_some(&BASE64_STANDARD.encode(p)),
        None => s.serialize_none(),
    }
}

fn from_base64<'d, S: Deserializer<'d>>(deserializer: S) -> Result<Option<String>, S::Error> {
    let option = Option::<String>::deserialize(deserializer)?;
    match option {
        Some(p) => {
            let bytes = BASE64_STANDARD.decode(p).map_err(|_| S::Error::custom("decode base64 error"))?;
            let string = String::from_utf8(bytes).map_err(|_| S::Error::custom("decode utf-8 error"))?;
            Ok(Some(string))
        }
        None => { Ok(None) }
    }
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
    pub fn from(other: Database) -> Self {
        Self {
            ..other
        }
    }
}