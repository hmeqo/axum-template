use crate::{
    bail,
    config::AppConfig,
    domain::{
        Services,
        model::{DefaultRole, Perm, role::DEFAULT_ROLE_PERMISSIONS},
    },
    error::{ErrorKind, Result},
};

pub async fn init_rbac(services: &Services) -> Result<()> {
    println!("Initializing permissions...");

    for perm in Perm::all() {
        if let Err(e) = services.permission.create(perm).await {
            if services.permission.find(perm).await?.is_some() {
                println!("  Skipped {} (already exists)", perm.code());
            } else {
                return Err(e);
            }
        } else {
            println!("  Created permission: {}", perm.code());
        }
    }

    println!("\nInitializing roles...");

    for role in DefaultRole::all() {
        let name = role.name().to_owned();
        let desc = Some(role.description().to_owned());
        match services.role.create(name, desc).await {
            Ok(r) => println!("  Created role: {}", r.name),
            Err(_) if services.role.find_by_name(role.name()).await?.is_some() => {
                println!("  Skipped {} (already exists)", role.name());
            }
            Err(e) => return Err(e),
        }
    }

    for (role, perms) in DEFAULT_ROLE_PERMISSIONS {
        let Some(role_model) = services.role.find_by_name(role.name()).await? else {
            bail!(
                ErrorKind::NotFound,
                "Role {} not found after init",
                role.name()
            );
        };
        println!("\nAssigning permissions to {} role...", role.name());
        for perm in *perms {
            let Some(perm_model) = services.permission.find(*perm).await? else {
                println!("  Skipped {} (permission not found)", perm.code());
                continue;
            };
            services
                .role
                .add_permission(role_model.id, perm_model.id)
                .await?;
            println!("  Added {} to {}", perm.code(), role.name());
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
        bail!(
            ErrorKind::NotFound,
            "Superuser role not found. Run 'init' first."
        );
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
        bail!(ErrorKind::NotFound, "Role not found");
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
        bail!(ErrorKind::NotFound, "Role not found");
    };

    let Some(permission) = services.permission.find(perm).await? else {
        bail!(ErrorKind::NotFound, "Permission {} not found", perm.code());
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
