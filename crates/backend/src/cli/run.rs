use clap::Parser;

use super::{
    command::{Cli, Commands, PermissionCommands, RoleCommands},
    command_impl,
};
use crate::{app::AppState, config::AppConfigManager, error::Result};

pub async fn run() -> Result<()> {
    let _ = dotenvy::dotenv();

    let cli = Cli::parse();

    match cli.command {
        None => crate::app::serve().await,
        Some(cmd) => {
            let services = get_services().await?;

            match cmd {
                Commands::Config => {
                    let config = AppConfigManager::load()?;
                    command_impl::print_config(&config.current())
                }
                Commands::Init => command_impl::init_rbac(&services).await,
                Commands::CreateSuperuser { username, password } => {
                    command_impl::create_superuser(&services, username, password).await
                }
                Commands::Role(cmd) => match cmd {
                    RoleCommands::List => command_impl::list_roles(&services).await,
                    RoleCommands::Create { name, description } => {
                        command_impl::create_role(&services, name, description).await
                    }
                    RoleCommands::Delete { name } => {
                        command_impl::delete_role(&services, name).await
                    }
                    RoleCommands::AddPermission { role, perm } => {
                        command_impl::add_permission_to_role(&services, role, perm).await
                    }
                },
                Commands::Permission(cmd) => match cmd {
                    PermissionCommands::List => command_impl::list_permissions(&services).await,
                },
            }
        }
    }
}

async fn get_services() -> Result<crate::domain::Services> {
    let config = AppConfigManager::load()?;
    let app_state = AppState::from_config(config).await?;
    Ok(app_state.services)
}
