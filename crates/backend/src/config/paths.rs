use std::path::PathBuf;

use crate::config::{PROJECT_DIR, env};

pub struct Paths;

impl Paths {
    fn local_dir() -> PathBuf {
        PathBuf::from(format!(".{PROJECT_DIR}"))
    }

    fn is_local_mode() -> bool {
        Self::local_dir().exists() || env::local_mode()
    }

    pub fn config_dir() -> PathBuf {
        let local = Self::local_dir();
        if Self::is_local_mode() {
            return local;
        }
        dirs::config_dir()
            .map(|p| p.join(PROJECT_DIR))
            .unwrap_or(local)
    }

    pub fn data_dir() -> PathBuf {
        let local = Self::local_dir();
        if Self::is_local_mode() {
            return local;
        }
        dirs::data_dir()
            .map(|p| p.join(PROJECT_DIR))
            .unwrap_or(local)
    }

    pub fn config_file() -> PathBuf {
        Self::config_dir().join("config.toml")
    }
}
