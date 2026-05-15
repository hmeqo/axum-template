use clap::Parser;

use super::{
    command::{Cli, Commands, RoleCommands},
    command_impl,
};
use crate::{app::AppState, config::AppConfig, error::Result};

pub async fn run() -> Result<()> {
    let _ = dotenvy::dotenv();

    let cli = Cli::parse();

    match cli.command {
        None => crate::app::serve().await,
        Some(cmd) => {
            let services = get_services().await?;

            match cmd {
                Commands::Config => {
                    let cfg = AppConfig::load()?;
                    command_impl::print_config(&cfg)
                }
                Commands::Init => command_impl::init_rbac(&services).await,
                Commands::CreateSuperuser { username, password } => {
                    command_impl::create_superuser(&services, username, password).await
                }
                Commands::Role(cmd) => match cmd {
                    RoleCommands::List => command_impl::list_roles(&services).await,
                    RoleCommands::Create {
                        name,
                        description,
                        perms,
                    } => command_impl::create_role(&services, name, description, perms).await,
                    RoleCommands::Delete { name } => {
                        command_impl::delete_role(&services, name).await
                    }
                },
                Commands::Perms => command_impl::list_permissions().await,
            }
        }
    }
}

async fn get_services() -> Result<crate::domain::Services> {
    let cfg = AppConfig::load()?;
    let app_state = AppState::new(cfg).await?;
    Ok(app_state.services)
}
