use crate::{
    config::AppConfigManager,
    domain::{Domain, Services},
};

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: AppConfigManager,
    pub domain: Domain,
}

impl AppState {
    pub fn services(&self) -> &Services {
        &self.domain.services
    }
}
