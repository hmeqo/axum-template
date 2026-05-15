use crate::{
    app::{
        router::{create_listener, create_router},
        state::AppState,
    },
    config::AppConfigManager,
    error::Result,
    infra::logging::init_tracing,
};

/// Start the HTTP server.
pub async fn serve() -> Result<()> {
    let _ = dotenvy::dotenv();
    let cfg_mgr = AppConfigManager::load()?;

    let cfg = cfg_mgr.current();

    init_tracing(&cfg.log);

    let app_state = AppState::from_config(cfg_mgr).await?;

    let router = create_router(app_state).await?;
    let listener = create_listener(&cfg).await?;
    let addr = listener.local_addr()?;

    tracing::info!("App listening on {}", addr);

    axum::serve(listener, router).await?;

    Ok(())
}
