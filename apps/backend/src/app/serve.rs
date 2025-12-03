use super::{AppBootstrap, router::create_router};
use crate::{config::AppConfig, error::Result};

/// Create TCP listener for the server
pub async fn create_listener(config: &AppConfig) -> Result<tokio::net::TcpListener> {
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Server listening on {}", addr);
    Ok(listener)
}

/// Start the server with all default configurations
pub async fn serve() -> Result<()> {
    let bootstrap = AppBootstrap::load()?;
    bootstrap.init_tracing();
    let state = bootstrap.build_app_state(bootstrap.build_domain().await?)?;

    let listener = create_listener(&state.config).await?;
    let router = create_router(state).await?;

    axum::serve(listener, router).await?;

    tracing::info!("App has terminated");

    Ok(())
}
