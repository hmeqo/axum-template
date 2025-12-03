use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};
use time::Duration;

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub log: LogConfig,
    pub database: DatabaseConfig,
    pub session: SessionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LogConfig {
    pub level: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "debug".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DatabaseConfig {
    pub url: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgres://postgres:@localhost:5432/db".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionConfig {
    pub inactivity_timeout: u64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            inactivity_timeout: 14 * 24 * 60 * 60,
        }
    }
}

impl SessionConfig {
    pub fn inactivity_timeout(&self) -> Duration {
        Duration::seconds(self.inactivity_timeout as i64)
    }
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let settings = Config::builder()
            .add_source(File::with_name("config/default").required(false))
            .add_source(File::with_name("config/local").required(false))
            .add_source(Environment::with_prefix("APP"))
            .build()?;

        settings.try_deserialize().map_err(Into::into)
    }
}
