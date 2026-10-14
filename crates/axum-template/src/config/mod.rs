pub mod env;
pub mod meta;
pub mod paths;
pub mod schema;
pub mod trusted_proxies;

use std::{path::PathBuf, sync::Arc};

use axum::http::Uri;
use config::{Environment, File};
use derive_more::Deref;
pub use meta::*;
pub use schema::*;
pub use trusted_proxies::TrustedProxies;

use crate::{
    bail,
    error::{AppError, Result},
};

struct ConfigFile {
    path: PathBuf,
    required: bool,
}

impl ConfigFile {
    fn optional(path: PathBuf) -> Self {
        Self {
            path,
            required: false,
        }
    }

    fn required(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            required: true,
        }
    }
}

#[derive(Debug, Clone, Deref)]
pub struct AppConfig {
    #[deref]
    inner: Arc<ConfigData>,
    trusted_proxies: TrustedProxies,
}

impl AppConfig {
    /// 解析一次可信代理表；语法错误在这里就失败。
    pub fn new(config: ConfigData) -> Result<Self> {
        let trusted_proxies = TrustedProxies::new(&config.server.trusted_proxies)?;
        Ok(Self {
            inner: Arc::new(config),
            trusted_proxies,
        })
    }

    pub fn trusted_proxies(&self) -> &TrustedProxies {
        &self.trusted_proxies
    }

    /// 环境变量覆盖形如 `AXUM_TEMPLATE__<SECTION>__<KEY>`（kebab-case）。
    pub fn load() -> Result<Self> {
        let mut builder =
            config::Config::builder().add_source(config::Config::try_from(&ConfigData::default())?);

        for file in Self::file_sources() {
            builder = builder.add_source(File::from(file.path).required(file.required));
        }

        let data: ConfigData = builder
            .add_source(
                Environment::with_prefix(meta::ENV_PREFIX.as_str())
                    .separator("__")
                    .convert_case(config::Case::Kebab),
            )
            .build()?
            .try_deserialize()?;

        let config = Self::new(data)?;
        config.validate()?;
        Ok(config)
    }

    /// 后者覆盖前者：`config.toml` → `config.<profile>.toml`；`AXUM_TEMPLATE_CONFIG_FILE` 取代整层。
    fn file_sources() -> Vec<ConfigFile> {
        match env::config_file_override() {
            Some(path) => vec![ConfigFile::required(path)],
            None => {
                let mut files = vec![ConfigFile::optional(paths::config_file())];
                if let Some(profile) = env::profile() {
                    files.push(ConfigFile::required(paths::config_file_for(&profile)));
                }
                files
            }
        }
    }

    pub fn validate(&self) -> Result<()> {
        let cfg = &self.inner;

        positive("server.port", cfg.server.port)?;
        if cfg.webtransport.enabled {
            #[cfg(not(feature = "webtransport"))]
            bail!(AppError::config(
                "webtransport.enabled requires the `webtransport` cargo feature"
            ));
            #[cfg(feature = "webtransport")]
            positive("webtransport.port", cfg.webtransport.port)?;
        }
        if cfg.webtransport.cert.is_some() != cfg.webtransport.key.is_some() {
            bail!(AppError::config(
                "webtransport.cert and webtransport.key must be set together"
            ));
        }
        if cfg.database.url.is_empty() {
            bail!(AppError::config("database.url must not be empty"));
        }
        positive(
            "server.request-timeout-seconds",
            cfg.server.request_timeout_seconds,
        )?;
        positive(
            "server.shutdown-timeout-seconds",
            cfg.server.shutdown_timeout_seconds,
        )?;

        let prefix = &cfg.storage.media_url_prefix;
        if !prefix.starts_with('/') {
            bail!(AppError::config(
                "storage.media-url-prefix must start with '/'"
            ));
        }
        if prefix.contains('{') || prefix.contains('}') {
            bail!(AppError::config(
                "storage.media-url-prefix must not contain '{' or '}'"
            ));
        }
        positive(
            "storage.gc-interval-minutes",
            cfg.storage.gc_interval_minutes,
        )?;
        positive(
            "storage.unattached-ttl-hours",
            cfg.storage.unattached_ttl_hours,
        )?;
        positive(
            "storage.orphan-grace-minutes",
            cfg.storage.orphan_grace_minutes,
        )?;
        positive("storage.max-file-size", cfg.storage.max_file_size)?;

        positive("realtime.channel-capacity", cfg.realtime.channel_capacity)?;

        for origin in &cfg.cors.allowed_origins {
            validate_origin(origin)?;
        }

        positive("auth.session.ttl-hours", cfg.auth.session.ttl_hours)?;
        positive(
            "auth.jwt.expires-in-seconds",
            cfg.auth.jwt.expires_in_seconds,
        )?;

        if cfg.auth.session.same_site == SameSite::None && !cfg.auth.session.secure {
            bail!(AppError::config(
                "auth.session.secure must be true when auth.session.same-site is none"
            ));
        }

        Ok(())
    }
}

/// 配置里必须大于 0 的整数项；命中即启动失败，`T::default()` 就是那个 0。
fn positive<T: Default + PartialEq>(key: &str, value: T) -> Result<()> {
    if value == T::default() {
        bail!(AppError::config(format!("{key} must be > 0")));
    }
    Ok(())
}

/// 白名单条目必须是 `Origin` 形状：`http(s)://host[:port]`，全小写，无 userinfo、路径、查询、片段。
fn validate_origin(origin: &str) -> Result<()> {
    // 浏览器按字节比较 `Origin`，任何偏差都只会让 CORS 静默失效。
    let reject = |why: &str| {
        Err(AppError::config(format!(
            "cors.allowed-origins entry must be an origin ({why}): {origin}"
        )))
    };

    let Ok(uri) = origin.parse::<Uri>() else {
        return reject("not a valid URI");
    };
    if !matches!(uri.scheme_str(), Some("http" | "https")) {
        return reject("scheme must be http:// or https://");
    }
    if uri.authority().is_none_or(|a| a.as_str().contains('@')) {
        return reject("host[:port] is required and userinfo is not allowed");
    }
    if uri.path_and_query().is_some_and(|p| p.as_str() != "/") || origin.ends_with('/') {
        return reject("no path, query or trailing slash");
    }
    if origin.contains('#') {
        return reject("no fragment");
    }
    if origin != origin.to_ascii_lowercase() {
        return reject("must be lowercase");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins_must_be_origin_shaped() {
        for good in [
            "https://app.example",
            "http://localhost:8000",
            "https://app.example:8443",
        ] {
            assert!(validate_origin(good).is_ok(), "{good}");
        }

        for bad in [
            "app.example",
            "https://App.Example",
            "https://app.example/",
            "https://app.example/path",
            "https://app.example?q=1",
            "https://app.example#f",
            "https://user@app.example",
            "https://exämple.com",
            "null",
        ] {
            assert!(validate_origin(bad).is_err(), "{bad}");
        }
    }
}
