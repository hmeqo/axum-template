use sea_orm::DatabaseConnection;
use std::sync::Arc;

use crate::{
    config::AppConfig,
    domain::{
        db::init_db,
        repository::{
            PermissionRepository, RolePermissionRepository, RoleRepository, UserRepository,
            UserRoleRepository,
        },
        service::{AuthService, PermissionService, RoleService, UserService},
    },
    error::Result,
};

#[derive(Clone)]
pub struct Repos {
    pub user: Arc<UserRepository>,
    pub role: Arc<RoleRepository>,
    pub permission: Arc<PermissionRepository>,
    pub user_role: Arc<UserRoleRepository>,
    pub role_permission: Arc<RolePermissionRepository>,
}

impl Repos {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        let user = Arc::new(UserRepository {
            db: Arc::clone(&db),
        });
        let role = Arc::new(RoleRepository {
            db: Arc::clone(&db),
        });
        let permission = Arc::new(PermissionRepository {
            db: Arc::clone(&db),
        });
        let user_role = Arc::new(UserRoleRepository {
            db: Arc::clone(&db),
        });
        let role_permission = Arc::new(RolePermissionRepository {
            db: Arc::clone(&db),
        });

        Self {
            user,
            role,
            permission,
            user_role,
            role_permission,
        }
    }
}

#[derive(Clone)]
pub struct Services {
    pub user: Arc<UserService>,
    pub auth: Arc<AuthService>,
    pub role: Arc<RoleService>,
    pub permission: Arc<PermissionService>,
}

impl Services {
    pub fn new(repos: &Repos) -> Self {
        let user = Arc::new(UserService {
            repo: Arc::clone(&repos.user),
        });
        let role = Arc::new(RoleService {
            role_repo: Arc::clone(&repos.role),
            permission_repo: Arc::clone(&repos.permission),
            user_role_repo: Arc::clone(&repos.user_role),
            role_permission_repo: Arc::clone(&repos.role_permission),
        });
        let permission = Arc::new(PermissionService {
            repo: Arc::clone(&repos.permission),
        });
        let auth = Arc::new(AuthService {
            user_service: Arc::clone(&user),
        });

        Self {
            user,
            auth,
            role,
            permission,
        }
    }
}

#[derive(Clone)]
pub struct Domain {
    pub db: Arc<DatabaseConnection>,
    pub repos: Repos,
    pub services: Services,
}

impl Domain {
    pub async fn from_config(config: &AppConfig) -> Result<Self> {
        let db = Arc::new(init_db(&config.database.url).await?);

        let repos = Repos::new(Arc::clone(&db));

        let services = Services::new(&repos);

        Ok(Self {
            db,
            repos,
            services,
        })
    }
}
