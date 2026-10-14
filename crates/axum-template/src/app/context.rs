use std::sync::Arc;

use derive_more::Deref;
use toasty::Db;
use tokio::sync::watch;

use crate::{
    app::event::AppEvent,
    config::{AppConfig, TrustedProxies},
    domain::{Services, db::init_db},
    error::Result,
    infra::{
        realtime::{Hub, Sockets},
        storage::MediaStore,
    },
};

#[derive(Debug)]
pub struct AppContextInner {
    config: AppConfig,
    db: Db,
    services: Services,
    media: MediaStore,
    events: Arc<Hub<AppEvent>>,
    sockets: Arc<Sockets>,
    trusted_proxies: TrustedProxies,
    shutdown_tx: watch::Sender<bool>,
}

#[derive(Debug, Clone, Deref)]
pub struct AppContext {
    #[deref]
    inner: Arc<AppContextInner>,
}

impl AppContext {
    pub async fn new(config: AppConfig) -> Result<Self> {
        let db = init_db(&config.database.url).await?;
        let media = MediaStore::new(&config.storage);
        media.ensure_dir().await?;
        let sockets = Arc::new(Sockets::new());
        let services = Services::new(db.clone(), &config.auth, media.clone(), sockets.clone());
        let trusted_proxies = config.trusted_proxies().clone();
        let (shutdown_tx, _) = watch::channel(false);
        let events = Arc::new(Hub::new(config.realtime.channel_capacity));

        Ok(Self {
            inner: Arc::new(AppContextInner {
                config,
                db,
                services,
                media,
                events,
                sockets,
                trusted_proxies,
                shutdown_tx,
            }),
        })
    }
}

impl AppContextInner {
    pub fn cfg(&self) -> &AppConfig {
        &self.config
    }

    pub fn db(&self) -> &Db {
        &self.db
    }

    pub fn srv(&self) -> &Services {
        &self.services
    }

    pub fn media(&self) -> &MediaStore {
        &self.media
    }

    pub fn events(&self) -> &Hub<AppEvent> {
        &self.events
    }

    pub fn sockets(&self) -> &Sockets {
        &self.sockets
    }

    pub fn trusted_proxies(&self) -> &TrustedProxies {
        &self.trusted_proxies
    }

    pub fn shutdown(&self) -> watch::Receiver<bool> {
        self.shutdown_tx.subscribe()
    }

    pub fn trigger_shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }
}
