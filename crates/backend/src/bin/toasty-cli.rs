#![recursion_limit = "8192"]

use anyhow::Result;
use backend::{
    config::AppConfigManager,
    domain::model::{Permission, RefreshToken, Role, RolePermission, User, UserRole},
};

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    let app_config = AppConfigManager::default()?.load();

    let cli_config = toasty_cli::Config::load()?;
    let db = toasty::Db::builder()
        .models(toasty::models!(User, Role, Permission, UserRole, RolePermission, RefreshToken))
        .connect(&app_config.database.url)
        .await?;

    let cli = toasty_cli::ToastyCli::with_config(db, cli_config);
    cli.parse_and_run().await?;

    Ok(())
}
