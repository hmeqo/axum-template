use anyhow::Result;

use crate::{
    config::AppConfig,
    domain::{
        Services,
        model::{DefaultRole, Perm, role::DEFAULT_ROLE_PERMISSIONS},
    },
};

pub async fn init_rbac(services: &Services, _force: bool) -> Result<()> {
    println!("Initializing permissions...");

    for perm in Perm::all() {
        match services.permission.create(perm).await {
            Ok(p) => println!("  Created permission: {}", p.code),
            Err(e) => println!("  Skipped {} ({})", perm.code(), e),
        }
    }

    println!("\nInitializing roles...");

    for role in DefaultRole::all() {
        match services
            .role
            .create(role.name().to_owned(), Some(role.description().to_owned()))
            .await
        {
            Ok(r) => println!("  Created role: {}", r.name),
            Err(e) => println!("  Skipped {} ({})", role.name(), e),
        }
    }

    for (role, perms) in DEFAULT_ROLE_PERMISSIONS {
        if let Ok(Some(role_model)) = services.role.find_by_name(role.name()).await {
            println!("\nAssigning permissions to {} role...", role.name());
            for perm in *perms {
                if let Ok(Some(perm_model)) = services.permission.find(*perm).await
                    && services
                        .role
                        .add_permission(role_model.id, perm_model.id)
                        .await
                        .is_ok()
                {
                    println!("  Added {} to {}", perm.code(), role.name());
                }
            }
        }
    }

    println!("\nRBAC initialization complete!");
    Ok(())
}

pub async fn create_superuser(
    services: &Services,
    username: Option<String>,
    password: Option<String>,
) -> Result<()> {
    let username = match username {
        Some(u) => u,
        None => inquire::Text::new("Username:").prompt()?,
    };
    let password = match password {
        Some(p) => p,
        None => inquire::Password::new("Enter password:").prompt()?,
    };

    println!("Creating superuser: {}", username);
    let user = services.user.create(username.clone(), password).await?;
    println!("  User created with ID: {}", user.id);

    let Some(superuser_role) = services.role.find_by_name("superuser").await? else {
        anyhow::bail!("Superuser role not found. Run 'init' first.")
    };

    services
        .role
        .assign_to_user(user.id, superuser_role.id)
        .await?;
    println!("  Assigned superuser role to user");

    println!("\nSuperuser '{}' created successfully!", username);
    Ok(())
}

pub async fn list_roles(services: &Services) -> Result<()> {
    let roles = services.role.list_all().await?;

    println!("Roles:");
    println!("{:-<50}", "");
    for role in roles {
        println!("  {} - {}", role.name, role.description.unwrap_or_default());
    }
    Ok(())
}

pub async fn create_role(
    services: &Services,
    name: String,
    description: Option<String>,
) -> Result<()> {
    let role = services.role.create(name, description).await?;
    println!("Created role: {} (ID: {})", role.name, role.id);
    Ok(())
}

pub async fn delete_role(services: &Services, name: String) -> Result<()> {
    let Some(role) = services.role.find_by_name(&name).await? else {
        anyhow::bail!("Role not found")
    };

    services.role.delete(role.id).await?;
    println!("Deleted role: {}", name);
    Ok(())
}

pub async fn add_permission_to_role(
    services: &Services,
    role_name: String,
    perm: Perm,
) -> Result<()> {
    let Some(role) = services.role.find_by_name(&role_name).await? else {
        anyhow::bail!("Role not found")
    };

    let Some(permission) = services.permission.find(perm).await? else {
        anyhow::bail!("Permission not found")
    };

    services.role.add_permission(role.id, permission.id).await?;
    println!("Added permission {} to role {}", perm.code(), role_name);
    Ok(())
}

pub async fn list_permissions(services: &Services) -> Result<()> {
    let permissions = services.permission.list_all().await?;

    println!("Permissions:");
    println!("{:-<60}", "");
    for perm in permissions {
        println!("  {} - {}", perm.code, perm.description.unwrap_or_default());
    }
    Ok(())
}

pub fn print_config(config: &AppConfig) -> Result<()> {
    let json = serde_json::to_string_pretty(config)?;
    println!("{}", json);
    Ok(())
}
