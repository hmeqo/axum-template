use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use struct_patch::Patch;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, Patch)]
#[patch(attribute(derive(Debug, Default, Clone, Serialize, Deserialize)))]
#[patch(attribute(skip_serializing_none))]
#[serde(default, rename_all = "kebab-case")]
pub struct AppConfig {
    #[patch(name = "ServerConfigPatch")]
    pub server: ServerConfig,
    #[patch(name = "LogConfigPatch")]
    pub log: LogConfig,
    #[patch(name = "DatabaseConfigPatch")]
    pub database: DatabaseConfig,
    #[patch(name = "AuthConfigPatch")]
    pub auth: AuthConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Patch)]
#[patch(attribute(derive(Debug, Default, Clone, Serialize, Deserialize)))]
#[patch(attribute(skip_serializing_none))]
#[serde(default, rename_all = "kebab-case")]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Patch)]
#[patch(attribute(derive(Debug, Default, Clone, Serialize, Deserialize)))]
#[patch(attribute(skip_serializing_none))]
#[serde(default, rename_all = "kebab-case")]
pub struct AuthConfig {
    pub jwt_secret: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Patch)]
#[patch(attribute(derive(Debug, Default, Clone, Serialize, Deserialize)))]
#[patch(attribute(skip_serializing_none))]
#[serde(default, rename_all = "kebab-case")]
pub struct LogConfig {
    pub level: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Patch)]
#[patch(attribute(derive(Debug, Default, Clone, Serialize, Deserialize)))]
#[patch(attribute(skip_serializing_none))]
#[serde(default, rename_all = "kebab-case")]
pub struct DatabaseConfig {
    pub url: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8000,
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: "change-me-in-production".to_string(),
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


