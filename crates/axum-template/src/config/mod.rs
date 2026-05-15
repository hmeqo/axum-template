pub mod env;
pub mod meta;
pub mod paths;
pub mod schema;

use std::sync::Arc;

use config::{Environment, File};
use derive_more::Deref;
pub use meta::*;
pub use schema::*;

use crate::error::Result;

#[derive(Debug, Clone, Deref)]
pub struct AppConfig {
    inner: Arc<RawAppConfig>,
}

impl AppConfig {
    pub fn new(config: RawAppConfig) -> Self {
        Self {
            inner: Arc::new(config),
        }
    }

    pub fn load() -> Result<Self> {
        let raw: RawAppConfig = config::Config::builder()
            .add_source(config::Config::try_from(&RawAppConfig::default())?)
            .add_source(File::with_name(&paths::Paths::config_file().to_string_lossy()).required(false))
            .add_source(Environment::with_prefix(meta::ENV_PREFIX.as_str()).separator("__"))
            .build()?
            .try_deserialize()?;
        Ok(Self::new(raw))
    }
}
