use toasty::Db;

use crate::{
    config::AppConfig,
    domain::{Services, db::init_db},
    error::Result,
};

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db: Db,
    pub services: Services,
}

impl AppState {
    pub async fn new(config: AppConfig) -> Result<Self> {
        let db = init_db(&config.database.url).await?;
        let services = Services::new(
            db.clone(),
            &config.auth.jwt.secret,
            config.auth.jwt.expires_in_seconds,
            config.auth.session.ttl_hours,
        );
        Ok(Self {
            config,
            db,
            services,
        })
    }
}

impl AppState {
    pub fn cfg(&self) -> &AppConfig {
        &self.config
    }

    pub fn db(&self) -> &Db {
        &self.db
    }

    pub fn srv(&self) -> &Services {
        &self.services
    }
}
