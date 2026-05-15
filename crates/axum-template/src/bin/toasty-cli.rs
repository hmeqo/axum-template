use anyhow::Result;
use axum_template::{config::AppConfig, domain::model};

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    let app_config = AppConfig::load()?;

    let cli_config = toasty_cli::Config::load()?;
    let db = toasty::Db::builder()
        .models(toasty::models!(
            model::User,
            model::Role,
            model::UserRole,
            model::RefreshToken,
            model::Session
        ))
        .connect(&app_config.database.url)
        .await?;

    let cli = toasty_cli::ToastyCli::with_config(db, cli_config);
    cli.parse_and_run().await?;

    Ok(())
}
