use super::AppBootstrap;
use crate::error::Result;

/// Start the server with all default configurations
pub async fn serve() -> Result<()> {
    let mut bootstrap = AppBootstrap::load()?;
    bootstrap.init_tracing();
    let domain = bootstrap.create_domain().await?;
    let app_state = bootstrap.create_app_state(domain);

    let (handle, addr) = bootstrap
        .start_axum(bootstrap.create_router(app_state.clone()).await?)
        .await?;
    let rpc_addr = bootstrap.start_jsonrpc(app_state).await;

    tracing::info!("App listening on {}", addr);
    tracing::info!("JSONRPC listening on {}", rpc_addr.unwrap());

    handle.await.unwrap();

    tracing::info!("App has terminated");

    Ok(())
}
