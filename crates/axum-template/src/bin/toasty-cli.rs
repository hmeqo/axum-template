#![cfg_attr(not(test), warn(clippy::unwrap_used))]

use anyhow::Result;
use axum_template::config::{AppConfig, env};

#[tokio::main]
async fn main() -> Result<()> {
    env::load_env_file()?;
    let app_config = AppConfig::load()?;

    let cli_config = toasty_cli::Config::load()?;
    let db = toasty::Db::builder()
        .models(axum_template::app_models!())
        .connect(&app_config.database.url)
        .await?;

    let cli = toasty_cli::ToastyCli::with_config(db, cli_config);
    cli.parse_and_run().await?;

    Ok(())
}
