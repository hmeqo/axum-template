//! 初始数据装配：默认角色/权限与首个超级用户。

use crate::{
    domain::{
        Services,
        model::{DefaultRole, User},
        vo::{Password, Username},
    },
    error::{AppError, Result},
};

pub struct RbacInit {
    pub created: Vec<CreatedRole>,
    pub skipped: Vec<String>,
}

pub struct CreatedRole {
    pub name: String,
    pub permissions: usize,
}

pub async fn init_rbac(services: &Services) -> Result<RbacInit> {
    let mut report = RbacInit {
        created: Vec::new(),
        skipped: Vec::new(),
    };

    for role in DefaultRole::all() {
        let name = role.name().to_owned();

        if services.role.find_by_name(&name).await?.is_some() {
            report.skipped.push(name);
            continue;
        }

        let perms = role.default_permissions();
        services
            .role
            .create(name.clone(), Some(role.description().to_owned()), perms)
            .await?;
        report.created.push(CreatedRole {
            name,
            permissions: perms.len(),
        });
    }

    Ok(report)
}

pub async fn create_superuser(
    services: &Services,
    username: Username,
    password: Password,
) -> Result<User> {
    let superuser = services
        .role
        .find_by_name(DefaultRole::Superuser.name())
        .await?
        .ok_or_else(|| AppError::not_found("Superuser role not found. Run 'init' first."))?;

    let user = services.user.create(username, password).await?;
    services.role.assign_to_user(user.id, superuser.id).await?;
    Ok(user)
}
