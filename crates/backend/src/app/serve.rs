use crate::{
    app::{
        router::{create_listener, create_router},
        state::AppState,
    },
    config::AppConfig,
    error::Result,
    infra::logging::init_tracing,
};

pub async fn serve() -> Result<()> {
    let _ = dotenvy::dotenv();
    let cfg = AppConfig::load()?;

    init_tracing(&cfg.log);

    let listener = create_listener(&cfg).await?;

    let app_state = AppState::new(cfg).await?;

    let router = create_router(app_state).await?;
    let addr = listener.local_addr()?;

    tracing::info!("App listening on {}", addr);

    tokio::spawn(async move {
        if let Err(err) = axum::serve(listener, router).await {
            tracing::error!("Server error: {}", err);
        }
    })
    .await?;

    Ok(())
}
