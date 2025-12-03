use anyhow::Result;
use clap::{CommandFactory, Parser};

use crate::app::{AppBootstrap, serve};

use super::{
    command::{Cli, Commands, PermissionCommands, RoleCommands},
    command_impl,
};

pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => {
            Cli::command().print_help()?;
            Ok(())
        }
        Some(Commands::Serve) => {
            serve().await?;
            Ok(())
        }
        Some(Commands::Init { force }) => {
            let services = AppBootstrap::load()?.build_domain().await?.services;
            command_impl::init_rbac(&services, force).await
        }
        Some(Commands::CreateSuperuser { username, password }) => {
            let domain = AppBootstrap::load()?.build_domain().await?;
            command_impl::create_superuser(&domain.services, username, password).await
        }
        Some(Commands::Role(cmd)) => {
            let domain = AppBootstrap::load()?.build_domain().await?;
            match cmd {
                RoleCommands::List => command_impl::list_roles(&domain.services).await,
                RoleCommands::Create { name, description } => {
                    command_impl::create_role(&domain.services, name, description).await
                }
                RoleCommands::Delete { name } => {
                    command_impl::delete_role(&domain.services, name).await
                }
                RoleCommands::AddPermission {
                    role,
                    resource,
                    action,
                } => {
                    command_impl::add_permission_to_role(&domain.services, role, resource, action)
                        .await
                }
            }
        }
        Some(Commands::Permission(cmd)) => {
            let domain = AppBootstrap::load()?.build_domain().await?;
            match cmd {
                PermissionCommands::List => command_impl::list_permissions(&domain.services).await,
            }
        }
    }
}
