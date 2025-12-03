use crate::{
    app::AppState, config::AppConfig, domain::Domain, error::Result, logging::init_tracing,
};

#[derive(Clone)]
pub struct AppBootstrap {
    pub config: AppConfig,
}

impl AppBootstrap {
    pub fn load() -> Result<Self> {
        let _ = dotenvy::dotenv();

        let config = AppConfig::load()?;
        Ok(Self { config })
    }

    pub fn init_tracing(&self) {
        init_tracing(&self.config.log);
    }

    pub async fn build_domain(&self) -> Result<Domain> {
        Domain::from_config(&self.config).await
    }

    pub fn build_app_state(&self, domain: Domain) -> Result<AppState> {
        Ok(AppState {
            config: self.config.clone(),
            domain,
        })
    }
}
