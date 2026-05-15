use crate::{
    bail,
    config::RawAppConfig,
    domain::{
        Services,
        model::{DefaultRole, Perm},
    },
    error::{ErrorKind, Result},
};

pub async fn init_rbac(services: &Services) -> Result<()> {
    println!("Initializing roles...");

    for role in DefaultRole::all() {
        let name = role.name().to_owned();
        let desc = Some(role.description().to_owned());
        let perms = role.default_permissions();
        match services.role.create(name, desc, perms).await {
            Ok(r) => println!(
                "  Created role: {} with {} permission(s)",
                r.name,
                r.parse_perms().len()
            ),
            Err(_) if services.role.find_by_name(role.name()).await?.is_some() => {
                println!("  Skipped {} (already exists)", role.name());
            }
            Err(e) => return Err(e),
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
        .filter_map(|c| Perm::from_code(c.trim()))
        .collect();
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
        bail!(ErrorKind::NotFound, "Role not found");
    };

    services.role.delete(role.id).await?;
    println!("Deleted role: {}", name);
    Ok(())
}

pub async fn list_permissions() -> Result<()> {
    println!("Available permissions:");
    println!("{:-<40}", "");
    for perm in Perm::all() {
        println!("  {} - {}", perm.code(), perm.description());
    }
    Ok(())
}

pub fn print_config(config: &RawAppConfig) -> Result<()> {
    let json = serde_json::to_string_pretty(config)?;
    println!("{}", json);
    Ok(())
}
