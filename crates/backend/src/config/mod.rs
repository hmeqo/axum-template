pub mod manager;
pub mod schema;

pub use manager::*;
pub use schema::*;

use crate::{Result, dirs};

impl Configurable for AppConfig {
    type Patch = AppConfigPatch;
}

pub type AppConfigManager = Config<AppConfig>;

impl AppConfigManager {
    /// Load config from the default path (dirs::CONFIG_TOML).
    pub fn load() -> Result<Self> {
        Self::from_file(dirs::CONFIG_TOML)
    }
}
