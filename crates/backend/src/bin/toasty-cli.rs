use anyhow::Result;
use backend::{config::AppConfigManager, domain::model};

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    let app_config = AppConfigManager::load()?.current();

    let cli_config = toasty_cli::Config::load()?;
    let db = toasty::Db::builder()
        .models(toasty::models!(
            model::User,
            model::Role,
            model::Permission,
            model::UserRole,
            model::RolePermission,
            model::RefreshToken
        ))
        .connect(&app_config.database.url)
        .await?;

    let cli = toasty_cli::ToastyCli::with_config(db, cli_config);
    cli.parse_and_run().await?;

    Ok(())
}
