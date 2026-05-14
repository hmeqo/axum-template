use crate::{
    app::{router::create_listener, router::create_router, state::AppState},
    config::AppConfigManager,
    domain::Domain,
    error::Result,
    infra::logging::init_tracing,
};

/// Start the HTTP server.
pub async fn serve() -> Result<()> {
    let _ = dotenvy::dotenv();
    let config = AppConfigManager::default()?;
    let app_cfg = config.load();

    init_tracing(&app_cfg.log);

    let domain = Domain::from_config(&app_cfg).await?;
    let app_state = AppState { config, domain };

    let router = create_router(app_state).await?;
    let listener = create_listener(&app_cfg).await?;
    let addr = listener.local_addr()?;

    tracing::info!("App listening on {}", addr);

    axum::serve(listener, router).await?;

    Ok(())
}
