use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "kebab-case")]
pub struct ConfigData {
    pub server: ServerConfig,
    pub log: LogConfig,
    pub database: DatabaseConfig,
    pub auth: AuthConfig,
    pub storage: StorageConfig,
    pub realtime: RealtimeConfig,
    pub cors: CorsConfig,
    pub webtransport: WebTransportConfig,
}

/// 进程内实时总线（`/ws` 与其它长连接面共用）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct RealtimeConfig {
    /// 每个 topic 的广播通道容量：满了就挤掉最旧的事件，订阅者由此得到回源补拉的信号。
    pub channel_capacity: usize,
}

impl Default for RealtimeConfig {
    fn default() -> Self {
        Self {
            channel_capacity: 64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct WebTransportConfig {
    pub enabled: bool,
    pub port: u16,
    /// PEM 证书与私钥；留空则自签（浏览器需 `serverCertificateHashes`）。
    pub cert: Option<PathBuf>,
    pub key: Option<PathBuf>,
}

impl Default for WebTransportConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 4433,
            cert: None,
            key: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub request_timeout_seconds: u64,
    pub shutdown_timeout_seconds: u64,
    /// 可信反向代理的地址或网段;为空表示永不采纳转发头。
    pub trusted_proxies: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8000,
            request_timeout_seconds: 30,
            shutdown_timeout_seconds: 10,
            trusted_proxies: Vec::new(),
        }
    }
}

/// 跨源访问白名单。空列表 = 不允许任何跨源请求。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "kebab-case")]
pub struct CorsConfig {
    pub allowed_origins: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "kebab-case")]
pub struct AuthConfig {
    pub session: SessionConfig,
    pub jwt: JwtConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct SessionConfig {
    pub cookie_name: String,
    pub ttl_hours: u64,
    pub secure: bool,
    pub same_site: SameSite,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            cookie_name: "session_id".to_string(),
            ttl_hours: 24,
            secure: false,
            same_site: SameSite::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SameSite {
    #[default]
    Lax,
    Strict,
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct JwtConfig {
    pub secret: String,
    pub expires_in_seconds: u64,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: "change-me-in-production".to_string(),
            expires_in_seconds: 900,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct StorageConfig {
    pub max_file_size: u64,
    pub media_url_prefix: String,
    pub media_dir: String,
    pub gc_interval_minutes: u64,
    pub unattached_ttl_hours: u64,
    pub orphan_grace_minutes: u64,
}

const MULTIPART_OVERHEAD_MARGIN: usize = 1024 * 1024;

impl StorageConfig {
    pub fn request_body_limit(&self) -> usize {
        self.max_file_size as usize + MULTIPART_OVERHEAD_MARGIN
    }

    pub fn gc_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.gc_interval_minutes * 60)
    }

    pub fn unattached_ttl(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.unattached_ttl_hours * 3600)
    }

    pub fn orphan_grace(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.orphan_grace_minutes * 60)
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            max_file_size: 10 * 1024 * 1024,
            media_url_prefix: "/media".to_string(),
            media_dir: "media".to_string(),
            gc_interval_minutes: 60,
            unattached_ttl_hours: 24,
            orphan_grace_minutes: 60,
        }
    }
}
