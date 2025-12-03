use std::ops::Deref;

use crate::{config::AppConfig, domain::Domain};

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub domain: Domain,
}

impl Deref for AppState {
    type Target = Domain;

    fn deref(&self) -> &Self::Target {
        &self.domain
    }
}
