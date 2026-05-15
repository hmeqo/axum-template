use toasty::Db;

pub mod db;
pub mod model;
pub mod service;

pub use service::*;

#[derive(Debug, Clone)]
pub struct Services {
    pub user: UserService,
    pub role: RoleService,
    pub permission: PermissionService,
    pub auth: AuthService,
    pub token: TokenService,
}

impl Services {
    pub fn new(db: Db, jwt_secret: &str) -> Self {
        Self {
            user: UserService::new(db.clone()),
            role: RoleService::new(db.clone()),
            permission: PermissionService::new(db.clone()),
            auth: AuthService::new(db.clone()),
            token: TokenService::new(db, jwt_secret.to_owned()),
        }
    }
}
