pub mod manager;
pub mod schema;

pub use manager::*;
pub use schema::*;

impl Configurable for AppConfig {
    type Patch = AppConfigPatch;
}

pub type AppConfigManager = Config<AppConfig>;
