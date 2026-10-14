use clap::Parser;

use super::{
    command::{Cli, Commands, RoleCommands},
    command_impl,
};
use crate::error::Result;

pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => crate::app::serve().await,
        Some(cmd) => {
            let boot = crate::app::startup::boot().await?;
            let context = &boot.context;
            let services = context.srv();

            match cmd {
                Commands::Config => command_impl::print_config(context.cfg()),
                Commands::Init => command_impl::init_rbac(services).await,
                Commands::CreateSuperuser { username, password } => {
                    command_impl::create_superuser(services, username, password).await
                }
                Commands::Role(cmd) => match cmd {
                    RoleCommands::List => command_impl::list_roles(services).await,
                    RoleCommands::Create {
                        name,
                        description,
                        perms,
                    } => command_impl::create_role(services, name, description, perms).await,
                    RoleCommands::Delete { name } => {
                        command_impl::delete_role(services, name).await
                    }
                },
                Commands::Perms => command_impl::list_permissions(),
            }
        }
    }
}
