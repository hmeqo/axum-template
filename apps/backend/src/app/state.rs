use crate::{
    config::AppConfig,
    domain::{Domain, Services},
};

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub domain: Domain,
}

impl AppState {
    pub fn services(&self) -> &Services {
        &self.domain.services
    }
}
