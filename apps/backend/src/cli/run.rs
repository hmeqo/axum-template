use anyhow::Result;
use clap::Parser;

use crate::{
    app::{AppBootstrap, serve},
    domain::Services,
};

use super::{
    command::{Cli, Commands, PermissionCommands, RoleCommands},
    command_impl,
};

pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => {
            serve().await?;
            Ok(())
        }
        Some(Commands::Init { force }) => {
            let services = get_services().await?;
            command_impl::init_rbac(&services, force).await
        }
        Some(Commands::CreateSuperuser { username, password }) => {
            let services = get_services().await?;
            command_impl::create_superuser(&services, username, password).await
        }
        Some(Commands::Role(cmd)) => {
            let services = get_services().await?;
            match cmd {
                RoleCommands::List => command_impl::list_roles(&services).await,
                RoleCommands::Create { name, description } => {
                    command_impl::create_role(&services, name, description).await
                }
                RoleCommands::Delete { name } => command_impl::delete_role(&services, name).await,
                RoleCommands::AddPermission { role, perm } => {
                    command_impl::add_permission_to_role(&services, role, perm).await
                }
            }
        }
        Some(Commands::Permission(cmd)) => match cmd {
            PermissionCommands::List => {
                let services = get_services().await?;
                command_impl::list_permissions(&services).await
            }
        },
        Some(Commands::Config) => {
            let config = AppBootstrap::load()?.config.load();
            command_impl::print_config(&config)
        }
    }
}

async fn get_services() -> Result<Services> {
    let mut b = AppBootstrap::load()?;
    Ok(b.create_domain().await?.services)
}
