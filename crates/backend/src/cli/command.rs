use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = env!("CARGO_CRATE_NAME"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Print configuration as JSON
    Config,

    /// Initialize default roles with permissions
    Init,

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

    /// List all available permissions
    Perms,
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

        /// Permission codes (e.g. "user:read,user:write")
        #[arg(short, long)]
        perms: Option<String>,
    },

    /// Delete a role
    Delete {
        /// Role name
        #[arg(short, long)]
        name: String,
    },
}
