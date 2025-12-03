// Re-export permissions from model
pub use crate::domain::model::permission::Permission;

/// Default roles to initialize
pub const DEFAULT_ROLES: &[(&str, &str)] = &[
    ("superuser", "Super administrator with all permissions"),
    ("admin", "Administrator with management permissions"),
    ("user", "Regular user with basic permissions"),
];
