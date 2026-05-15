use std::sync::Arc;

use arc_swap::Guard;
use toasty::Db;

use crate::{
    config::{AppConfig, AppConfigManager},
    domain::{Services, db::init_db},
    error::Result,
};

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: AppConfigManager,
    pub db: Db,
    pub services: Services,
}

impl AppState {
    pub async fn from_config(config: AppConfigManager) -> Result<Self> {
        let cfg = config.current();
        let db = init_db(&cfg.database.url).await?;
        let services = Services::new(db.clone(), &cfg.auth.jwt_secret);
        Ok(Self {
            config,
            db,
            services,
        })
    }
}

impl AppState {
    pub fn config(&self) -> Guard<Arc<AppConfig>> {
        self.config.current()
    }

    pub fn db(&self) -> &Db {
        &self.db
    }

    pub fn services(&self) -> &Services {
        &self.services
    }
}
