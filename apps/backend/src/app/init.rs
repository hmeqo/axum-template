use std::net::SocketAddr;

use axum::Router;
use tokio::task::JoinHandle;

use crate::{
    app::{
        AppState, jsonrpc,
        router::{create_listener, create_router},
    },
    config::AppConfigManager,
    domain::Domain,
    error::Result,
    infra::logging::init_tracing,
};

#[derive(Debug)]
pub struct AppBootstrap {
    pub config: AppConfigManager,
}

impl AppBootstrap {
    pub fn load() -> Result<Self> {
        let _ = dotenvy::dotenv();

        let config = AppConfigManager::from_file("data/config.toml")?;
        Ok(Self { config })
    }

    pub fn init_tracing(&self) {
        init_tracing(&self.config.load().log);
    }

    pub async fn create_domain(&mut self) -> Result<Domain> {
        Domain::from_config(&self.config.load()).await
    }

    pub fn create_app_state(&mut self, domain: Domain) -> AppState {
        AppState {
            config: self.config.clone(),
            domain,
        }
    }

    pub async fn create_router(&self, app_state: AppState) -> Result<Router> {
        create_router(app_state).await
    }

    pub async fn start_axum(&self, router: Router) -> Result<(JoinHandle<()>, SocketAddr)> {
        let listener = create_listener(&self.config.load()).await?;
        let addr = listener.local_addr()?;

        let handler = tokio::spawn(async {
            axum::serve(listener, router).await.unwrap();
        });

        Ok((handler, addr))
    }

    pub async fn start_jsonrpc(&self, app_state: AppState) -> Result<SocketAddr> {
        let config = self.config.load();
        let addr = format!("{}:{}", config.server.rpc_host, config.server.rpc_port);
        jsonrpc::start(app_state, addr).await
    }
}
