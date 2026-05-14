use toasty::Db;

use crate::{config::AppConfig, domain::db::init_db, error::Result};

pub mod db;
pub mod model;
pub mod service;

pub use service::*;

#[derive(Debug, Clone)]
pub struct Domain {
    pub db: Db,
    pub services: Services,
}

impl Domain {
    pub async fn from_config(config: &AppConfig) -> Result<Self> {
        let db = init_db(&config.database.url).await?;
        let services = Services::new(db.clone());

        Ok(Self { db, services })
    }
}

#[derive(Debug, Clone)]
pub struct Services {
    pub user: service::UserService,
    pub role: service::RoleService,
    pub permission: service::PermissionService,
    pub auth: service::AuthService,
}

impl Services {
    pub fn new(db: Db) -> Self {
        Self {
            user: service::UserService::new(db.clone()),
            role: service::RoleService::new(db.clone()),
            permission: service::PermissionService::new(db.clone()),
            auth: service::AuthService::new(db),
        }
    }
}
