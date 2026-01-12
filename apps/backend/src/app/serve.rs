use super::AppBootstrap;
use crate::error::Result;

/// Start the server with all default configurations
pub async fn serve() -> Result<()> {
    let mut bootstrap = AppBootstrap::load()?;
    bootstrap.init_tracing();
    bootstrap.init_domain().await?;
    bootstrap.init_app_state();
    let listener = bootstrap.create_listener().await?;
    let router = bootstrap.create_router().await?;

    axum::serve(listener, router).await?;

    tracing::info!("App has terminated");

    Ok(())
}
