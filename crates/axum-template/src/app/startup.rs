use std::time::Duration;

use axum::Router;
use tokio::net::TcpListener;
use tracing_appender::non_blocking::WorkerGuard;

use crate::{
    app::{AppContext, router::create_router},
    config::{AppConfig, ConfigData, env},
    error::Result,
    infra::logging::init_tracing,
};

pub struct Boot {
    pub context: AppContext,
    log_guard: WorkerGuard,
}

pub async fn boot() -> Result<Boot> {
    env::load_env_file()?;
    let config = AppConfig::load()?;
    let log_guard = init_tracing(&config.log)?;
    let context = AppContext::new(config).await?;
    Ok(Boot { context, log_guard })
}

pub async fn serve() -> Result<()> {
    let Boot { context, log_guard } = boot().await?;
    let listener = create_listener(context.cfg()).await?;
    let router = create_router(context.clone()).await?;

    tracing::info!("listening on {}", listener.local_addr()?);

    let scheduler = tokio::spawn(crate::infra::scheduler::run(context.clone()));

    #[cfg(feature = "webtransport")]
    let webtransport = tokio::spawn(super::socket::webtransport::serve(context.clone()));

    let served = serve_with_drain(listener, router, &context).await;

    scheduler.abort();

    #[cfg(feature = "webtransport")]
    webtransport.abort();

    drop(log_guard);
    served
}

async fn serve_with_drain(
    listener: TcpListener,
    router: Router,
    context: &AppContext,
) -> Result<()> {
    let (begin_drain_tx, begin_drain_rx) = tokio::sync::oneshot::channel::<()>();
    let serve = axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        let _ = begin_drain_rx.await;
    })
    .into_future();
    tokio::pin!(serve);

    tokio::select! {
        result = &mut serve => result?,
        _ = shutdown_signal() => {
            tracing::info!("shutdown signal received, draining");
            context.trigger_shutdown();
            let _ = begin_drain_tx.send(());

            let drain = Duration::from_secs(context.cfg().server.shutdown_timeout_seconds);
            if tokio::time::timeout(drain, &mut serve).await.is_err() {
                tracing::warn!(
                    "drain exceeded {}s, exiting with in-flight work",
                    drain.as_secs()
                );
            }
        }
    }

    Ok(())
}

pub async fn create_listener(config: &ConfigData) -> Result<TcpListener> {
    let addr = format!("{}:{}", config.server.host, config.server.port);
    Ok(TcpListener::bind(&addr).await?)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install ctrl-c handler")
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install sigterm handler")
            .recv()
            .await
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = terminate => {} }
}
