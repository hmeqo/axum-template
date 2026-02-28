use clap::{Parser, Subcommand};

use crate::domain::model::Perm;

#[derive(Parser)]
#[command(name = env!("CARGO_CRATE_NAME"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the web server
    Serve,

    /// Initialize default roles and permissions
    Init {
        /// Force re-initialization (will not duplicate existing data)
        #[arg(short, long)]
        force: bool,
    },

    /// Create a superuser with all permissions
    CreateSuperuser {
        /// Username for the superuser
        #[arg(short, long)]
        username: Option<String>,

        /// Password for the superuser
        #[arg(short, long)]
        password: Option<String>,
    },

    /// Manage roles
    #[command(subcommand)]
    Role(RoleCommands),

    /// Manage permissions
    #[command(subcommand)]
    Permission(PermissionCommands),

    /// Print configuration as JSON
    Config,
}

#[derive(Subcommand)]
pub enum RoleCommands {
    /// List all roles
    List,

    /// Create a new role
    Create {
        /// Role name
        #[arg(short, long)]
        name: String,

        /// Role description
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Delete a role
    Delete {
        /// Role name
        #[arg(short, long)]
        name: String,
    },

    /// Add permission to role
    AddPermission {
        /// Role name
        #[arg(short, long)]
        role: String,

        /// Resource name
        #[arg(long)]
        perm: Perm,
    },
}

#[derive(Subcommand)]
pub enum PermissionCommands {
    /// List all permissions
    List,
}
