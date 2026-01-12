//! CLI command implementations

use anyhow::Result;
use strum::IntoEnumIterator;

use crate::domain::{
    Services,
    initdata::{DEFAULT_ROLES, Permission},
};

/// Initialize default roles and permissions
pub async fn init_rbac(services: &Services, _force: bool) -> Result<()> {
    let role_service = &services.role;
    let permission_service = &services.permission;

    println!("Initializing permissions...");

    // Create default permissions
    for perm in Permission::iter() {
        match permission_service
            .create(
                perm.resource_str().to_string(),
                perm.action_str().to_string(),
                Some(perm.description().to_string()),
            )
            .await
        {
            Ok(p) => println!("  Created permission: {}:{}", p.resource, p.action),
            Err(e) => println!(
                "  Skipped {}:{} ({})",
                perm.resource_str(),
                perm.action_str(),
                e
            ),
        }
    }

    println!("\nInitializing roles...");

    // Create default roles
    for (name, description) in DEFAULT_ROLES {
        match role_service
            .create(name.to_string(), Some(description.to_string()))
            .await
        {
            Ok(r) => println!("  Created role: {}", r.name),
            Err(e) => println!("  Skipped {} ({})", name, e),
        }
    }

    // Assign all permissions to superuser role
    if let Ok(Some(superuser)) = role_service.find_by_name("superuser").await {
        println!("\nAssigning all permissions to superuser...");
        let permissions = permission_service.list_all().await?;
        for perm in permissions {
            if role_service
                .add_permission(superuser.id, perm.id)
                .await
                .is_ok()
            {
                println!("  Added {}:{} to superuser", perm.resource, perm.action);
            }
        }
    }

    // Assign basic permissions to user role
    if let Ok(Some(user_role)) = role_service.find_by_name("user").await {
        println!("\nAssigning basic permissions to user role...");
        let basic_perms = [("user", "read")];
        for (resource, action) in basic_perms {
            if let Ok(Some(perm)) = permission_service
                .find_by_resource_action(resource, action)
                .await
                && role_service
                    .add_permission(user_role.id, perm.id)
                    .await
                    .is_ok()
            {
                println!("  Added {}:{} to user", resource, action);
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
    resource: String,
    action: String,
) -> Result<()> {
    let role_service = &services.role;
    let permission_service = &services.permission;

    let Some(role) = role_service.find_by_name(&role_name).await? else {
        anyhow::bail!("Role not found")
    };

    let Some(permission) = permission_service
        .find_by_resource_action(&resource, &action)
        .await?
    else {
        anyhow::bail!("Permission not found")
    };

    role_service.add_permission(role.id, permission.id).await?;
    println!(
        "Added permission {}:{} to role {}",
        resource, action, role_name
    );
    Ok(())
}

/// List all permissions
pub async fn list_permissions(services: &Services) -> Result<()> {
    let permission_service = &services.permission;
    let permissions = permission_service.list_all().await?;

    println!("Permissions:");
    println!("{:-<60}", "");
    for perm in permissions {
        println!(
            "  {}:{} - {}",
            perm.resource,
            perm.action,
            perm.description.unwrap_or_default()
        );
    }
    Ok(())
}
