use std::sync::Arc;

use toasty::Db;

use crate::{
    config::schema::AuthConfig,
    infra::{realtime::Sockets, storage::MediaStore},
};

pub mod db;
pub mod model;
pub mod service;
pub mod vo;

pub use service::*;

#[derive(Debug, Clone)]
pub struct Services {
    pub user: service::UserService,
    pub role: service::RoleService,
    pub auth: service::AuthService,
    pub session: service::SessionService,
    pub token: service::TokenService,
    pub file: service::FileService,
}

impl Services {
    pub fn new(db: Db, auth: &AuthConfig, media: MediaStore, sockets: Arc<Sockets>) -> Self {
        let user = service::UserService::new(db.clone());
        let role = service::RoleService::new(db.clone());
        let session = service::SessionService::new(db.clone(), auth.session.ttl_hours, sockets);
        let file = service::FileService::new(db.clone(), media);
        let token = service::TokenService::new(
            db,
            user.clone(),
            auth.jwt.secret.clone(),
            auth.jwt.expires_in_seconds,
        );
        let auth_service =
            service::AuthService::new(user.clone(), role.clone(), session.clone(), token.clone());

        Self {
            user,
            role,
            session,
            token,
            auth: auth_service,
            file,
        }
    }
}
