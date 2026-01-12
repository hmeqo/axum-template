use crate::{
    app::{
        AppState,
        router::{create_listener, create_router},
    },
    config::AppConfig,
    domain::Domain,
    error::Result,
    infra::logging::init_tracing,
};

#[derive(Debug)]
pub struct AppBootstrap {
    pub config: AppConfig,
    pub domain: Option<Domain>,
    pub app_state: Option<AppState>,
}

impl AppBootstrap {
    pub fn load() -> Result<Self> {
        let _ = dotenvy::dotenv();

        let config = AppConfig::load()?;
        Ok(Self {
            config,
            domain: None,
            app_state: None,
        })
    }

    pub fn init_tracing(&self) {
        init_tracing(&self.config.log);
    }

    pub async fn init_domain(&mut self) -> Result<()> {
        self.domain = Some(Domain::from_config(&self.config).await?);
        Ok(())
    }

    pub fn init_app_state(&mut self) {
        self.app_state = Some(AppState {
            config: self.config.clone(),
            domain: self.domain.take().unwrap(),
        });
    }

    pub async fn create_listener(&self) -> Result<tokio::net::TcpListener> {
        create_listener(&self.config).await
    }

    pub async fn create_router(&mut self) -> Result<axum::Router> {
        create_router(self.app_state.take().unwrap()).await
    }
}
