use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use struct_patch::Patch;
use time::Duration;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, Patch)]
#[patch(attribute(derive(Debug, Default, Clone, Serialize, Deserialize)))]
#[patch(attribute(skip_serializing_none))]
pub struct AppConfig {
    #[patch(name = "ServerConfigPatch")]
    pub server: ServerConfig,
    #[patch(name = "LogConfigPatch")]
    pub log: LogConfig,
    #[patch(name = "DatabaseConfigPatch")]
    pub database: DatabaseConfig,
    #[patch(name = "SessionConfigPatch")]
    pub session: SessionConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Patch)]
#[patch(attribute(derive(Debug, Default, Clone, Serialize, Deserialize)))]
#[patch(attribute(skip_serializing_none))]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub rpc_host: String,
    pub rpc_port: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Patch)]
#[patch(attribute(derive(Debug, Default, Clone, Serialize, Deserialize)))]
#[patch(attribute(skip_serializing_none))]
pub struct LogConfig {
    pub level: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Patch)]
#[patch(attribute(derive(Debug, Default, Clone, Serialize, Deserialize)))]
#[patch(attribute(skip_serializing_none))]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Patch)]
#[patch(attribute(derive(Debug, Default, Clone, Serialize, Deserialize)))]
#[patch(attribute(skip_serializing_none))]
pub struct SessionConfig {
    pub inactivity_timeout: i64,
}

impl SessionConfig {
    pub fn inactivity_timeout(&self) -> Duration {
        Duration::seconds(self.inactivity_timeout)
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8000,
            rpc_host: "0.0.0.0".to_string(),
            rpc_port: 8001,
        }
    }
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "debug".to_string(),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgres://postgres:@localhost:5432/db".to_string(),
        }
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            inactivity_timeout: 14 * 24 * 60 * 60,
        }
    }
}
