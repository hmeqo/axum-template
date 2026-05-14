use anyhow::Result;
use clap::Parser;

use super::{
    command::{Cli, Commands, PermissionCommands, RoleCommands},
    command_impl,
};
use crate::{config::AppConfigManager, domain::Domain};

pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    let services = match cli.command {
        None => {
            crate::app::serve().await?;
            return Ok(());
        }
        _ => get_services().await?,
    };

    match cli.command {
        Some(Commands::Init { force }) => command_impl::init_rbac(&services, force).await,
        Some(Commands::CreateSuperuser { username, password }) => {
            command_impl::create_superuser(&services, username, password).await
        }
        Some(Commands::Role(cmd)) => match cmd {
            RoleCommands::List => command_impl::list_roles(&services).await,
            RoleCommands::Create { name, description } => {
                command_impl::create_role(&services, name, description).await
            }
            RoleCommands::Delete { name } => command_impl::delete_role(&services, name).await,
            RoleCommands::AddPermission { role, perm } => {
                command_impl::add_permission_to_role(&services, role, perm).await
            }
        },
        Some(Commands::Permission(cmd)) => match cmd {
            PermissionCommands::List => command_impl::list_permissions(&services).await,
        },
        Some(Commands::Config) => {
            let config = AppConfigManager::default()?;
            command_impl::print_config(&config.load())
        }
        None => unreachable!(),
    }
}

async fn get_services() -> Result<crate::domain::Services> {
    let _ = dotenvy::dotenv();
    let config = AppConfigManager::default()?;
    let domain = Domain::from_config(&config.load()).await?;
    Ok(domain.services)
}
