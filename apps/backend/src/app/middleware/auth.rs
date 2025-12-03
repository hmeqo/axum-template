use std::sync::Arc;

use axum_login::{AuthManagerLayer, AuthManagerLayerBuilder};
use tower_sessions::{
    MemoryStore, SessionManagerLayer,
    session::{Id, Record},
    session_store::{Result as StoreResult, SessionStore},
};
use tower_sessions_seaorm_store::PostgresStore;

use crate::{
    app::{AppState, helper::auth::Backend},
    error::{AppError, Result},
};

#[derive(Debug, Clone)]
pub enum DynamicSessionStore {
    Memory(MemoryStore),
    Database(PostgresStore),
}

impl DynamicSessionStore {
    pub async fn from_config(state: &AppState) -> Result<Self> {
        let url = &state.config.database.url;
        if url.starts_with("postgres://") {
            let store = PostgresStore::new(state.db.as_ref().clone());
            store.migrate().await.map_err(AppError::SeaOrmStore)?;
            Ok(Self::Database(store))
        } else {
            Ok(Self::Memory(MemoryStore::default()))
        }
    }
}

#[async_trait::async_trait]
impl SessionStore for DynamicSessionStore {
    async fn create(&self, record: &mut Record) -> StoreResult<()> {
        match self {
            Self::Memory(store) => store.create(record).await,
            Self::Database(store) => store.create(record).await,
        }
    }

    async fn save(&self, record: &Record) -> StoreResult<()> {
        match self {
            Self::Memory(store) => store.save(record).await,
            Self::Database(store) => store.save(record).await,
        }
    }

    async fn load(&self, session_id: &Id) -> StoreResult<Option<Record>> {
        match self {
            Self::Memory(store) => store.load(session_id).await,
            Self::Database(store) => store.load(session_id).await,
        }
    }

    async fn delete(&self, session_id: &Id) -> StoreResult<()> {
        match self {
            Self::Memory(store) => store.delete(session_id).await,
            Self::Database(store) => store.delete(session_id).await,
        }
    }
}

pub async fn session(state: &AppState) -> Result<AuthManagerLayer<Backend, DynamicSessionStore>> {
    let store = DynamicSessionStore::from_config(state).await?;

    let layer = SessionManagerLayer::new(store)
        .with_name("session_id")
        .with_expiry(tower_sessions::Expiry::OnInactivity(
            state.config.session.inactivity_timeout(),
        ));

    let backend = Backend {
        auth_service: Arc::clone(&state.services.auth),
    };

    Ok(AuthManagerLayerBuilder::new(backend, layer).build())
}
