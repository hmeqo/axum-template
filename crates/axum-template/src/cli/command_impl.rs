use crate::{
    bail,
    config::ConfigData,
    domain::{
        Services,
        service::bootstrap,
        vo::{Password, Perm, Username},
    },
    error::{AppError, Result},
};

pub async fn init_rbac(services: &Services) -> Result<()> {
    println!("Initializing roles...");

    let report = bootstrap::init_rbac(services).await?;

    for role in &report.created {
        println!(
            "  Created role: {} with {} permission(s)",
            role.name, role.permissions
        );
    }
    for name in &report.skipped {
        println!("  Skipped {} (already exists)", name);
    }

    println!("\nRBAC initialization complete!");
    Ok(())
}

pub async fn create_superuser(
    services: &Services,
    username: Option<String>,
    password: Option<String>,
) -> Result<()> {
    let username = Username::new(match username {
        Some(u) => u,
        None => inquire::Text::new("Username:").prompt()?,
    })?;
    let password = Password::new(match password {
        Some(p) => p,
        None => inquire::Password::new("Enter password:").prompt()?,
    })?;

    println!("Creating superuser: {}", username.as_str());
    let user = bootstrap::create_superuser(services, username, password).await?;
    println!("  User created with ID: {}", user.id);
    println!("  Assigned superuser role to user");

    println!("\nSuperuser '{}' created successfully!", user.username);
    Ok(())
}

pub async fn list_roles(services: &Services) -> Result<()> {
    let roles = services.role.list_all().await?;

    println!("Roles:");
    println!("{:-<80}", "");
    for role in roles {
        let perms = role.perm_codes().join(", ");
        println!(
            "  {} - {} [{}]",
            role.name,
            role.description.unwrap_or_default(),
            perms
        );
    }
    Ok(())
}

pub async fn create_role(
    services: &Services,
    name: String,
    description: Option<String>,
    perm_codes: Option<String>,
) -> Result<()> {
    let perms: Vec<Perm> = perm_codes
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|code| !code.is_empty())
        .map(|code| {
            Perm::from_code(code)
                .ok_or_else(|| AppError::malformed_request(format!("unknown permission: {code}")))
        })
        .collect::<Result<Vec<_>>>()?;
    let role = services.role.create(name, description, &perms).await?;
    println!(
        "Created role: {} (ID: {}) with {} permission(s)",
        role.name,
        role.id,
        perms.len()
    );
    Ok(())
}

pub async fn delete_role(services: &Services, name: String) -> Result<()> {
    let Some(role) = services.role.find_by_name(&name).await? else {
        bail!(AppError::not_found("Role not found"));
    };

    services.role.delete(role.id).await?;
    println!("Deleted role: {}", name);
    Ok(())
}

pub fn list_permissions() -> Result<()> {
    println!("Available permissions:");
    println!("{:-<40}", "");
    for perm in Perm::all() {
        println!("  {} - {}", perm.code(), perm.description());
    }
    Ok(())
}

pub fn print_config(config: &ConfigData) -> Result<()> {
    let json = serde_json::to_string_pretty(config)?;
    println!("{}", json);
    Ok(())
}
