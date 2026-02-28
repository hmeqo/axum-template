//! CLI command implementations

use anyhow::Result;

use crate::{
    config::AppConfig,
    domain::{
        Services,
        model::{DefaultRole, Perm, role::DEFAULT_ROLE_PERMISSIONS},
    },
};

/// Initialize default roles and permissions
pub async fn init_rbac(services: &Services, _force: bool) -> Result<()> {
    let role_service = &services.role;
    let permission_service = &services.permission;

    println!("Initializing permissions...");

    // Create default permissions
    for perm in Perm::all() {
        match permission_service.create(perm).await {
            Ok(p) => println!("  Created permission: {}", p.code),
            Err(e) => println!("  Skipped {} ({})", perm.code(), e),
        }
    }

    println!("\nInitializing roles...");

    // Create default roles
    for role in DefaultRole::all() {
        match role_service
            .create(role.name().to_owned(), Some(role.description().to_owned()))
            .await
        {
            Ok(r) => println!("  Created role: {}", r.name),
            Err(e) => println!("  Skipped {} ({})", role.name(), e),
        }
    }

    for (role, perms) in DEFAULT_ROLE_PERMISSIONS {
        if let Ok(Some(role_model)) = role_service.find_by_name(role.name()).await {
            println!("\nAssigning permissions to {} role...", role.name());
            for perm in *perms {
                if let Ok(Some(perm_model)) = permission_service.find(*perm).await
                    && role_service
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

/// Create a superuser with all permissions
pub async fn create_superuser(
    services: &Services,
    username: Option<String>,
    password: Option<String>,
) -> Result<()> {
    let user_service = &services.user;
    let role_service = &services.role;

    let username = match username {
        Some(u) => u,
        None => inquire::Text::new("Username:").prompt()?,
    };
    let password = match password {
        Some(p) => p,
        None => inquire::Password::new("Enter password:").prompt()?,
    };

    // Create user
    println!("Creating superuser: {}", username);
    let user = user_service.create(username.clone(), password).await?;
    println!("  User created with ID: {}", user.id);

    // Find superuser role
    let Some(superuser_role) = role_service.find_by_name("superuser").await? else {
        anyhow::bail!("Superuser role not found. Run 'init' first.")
    };

    // Assign superuser role
    role_service
        .assign_to_user(user.id, superuser_role.id)
        .await?;
    println!("  Assigned superuser role to user");

    println!("\nSuperuser '{}' created successfully!", username);
    Ok(())
}

/// List all roles
pub async fn list_roles(services: &Services) -> Result<()> {
    let role_service = &services.role;
    let roles = role_service.list_all().await?;

    println!("Roles:");
    println!("{:-<50}", "");
    for role in roles {
        println!("  {} - {}", role.name, role.description.unwrap_or_default());
    }
    Ok(())
}

/// Create a new role
pub async fn create_role(
    services: &Services,
    name: String,
    description: Option<String>,
) -> Result<()> {
    let role_service = &services.role;
    let role = role_service.create(name, description).await?;
    println!("Created role: {} (ID: {})", role.name, role.id);
    Ok(())
}

/// Delete a role
pub async fn delete_role(services: &Services, name: String) -> Result<()> {
    let role_service = &services.role;
    let Some(role) = role_service.find_by_name(&name).await? else {
        anyhow::bail!("Role not found")
    };

    role_service.delete(role.id).await?;
    println!("Deleted role: {}", name);
    Ok(())
}

/// Add permission to role
pub async fn add_permission_to_role(
    services: &Services,
    role_name: String,
    perm: Perm,
) -> Result<()> {
    let role_service = &services.role;
    let permission_service = &services.permission;

    let Some(role) = role_service.find_by_name(&role_name).await? else {
        anyhow::bail!("Role not found")
    };

    let Some(permission) = permission_service.find(perm).await? else {
        anyhow::bail!("Permission not found")
    };

    role_service.add_permission(role.id, permission.id).await?;
    println!("Added permission {} to role {}", perm.code(), role_name);
    Ok(())
}

/// List all permissions
pub async fn list_permissions(services: &Services) -> Result<()> {
    let permission_service = &services.permission;
    let permissions = permission_service.list_all().await?;

    println!("Permissions:");
    println!("{:-<60}", "");
    for perm in permissions {
        println!("  {} - {}", perm.code, perm.description.unwrap_or_default());
    }
    Ok(())
}

/// Print configuration as JSON
pub fn print_config(config: &AppConfig) -> Result<()> {
    let json = serde_json::to_string_pretty(config)?;
    println!("{}", json);
    Ok(())
}
