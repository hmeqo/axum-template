use toasty::Db;

pub mod db;
pub mod model;
pub mod service;

pub use service::*;

#[derive(Debug, Clone)]
pub struct Services {
    pub user: service::UserService,
    pub role: service::RoleService,
    pub auth: service::AuthService,
    pub session: service::SessionService,
    pub token: service::TokenService,
}

impl Services {
    pub fn new(
        db: Db,
        jwt_secret: &str,
        jwt_expires_in_seconds: u64,
        session_ttl_hours: u64,
    ) -> Self {
        Self {
            user: service::UserService::new(db.clone()),
            role: service::RoleService::new(db.clone()),
            auth: service::AuthService::new(db.clone()),
            session: service::SessionService::new(db.clone(), session_ttl_hours),
            token: service::TokenService::new(db, jwt_secret.to_owned(), jwt_expires_in_seconds),
        }
    }
}
