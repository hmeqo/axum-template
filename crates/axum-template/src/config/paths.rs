//! 运行期路径的唯一出处。

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use crate::config::{PROJECT_DIR, env, schema::StorageConfig};

struct Dirs {
    config: PathBuf,
    data: PathBuf,
}

static DIRS: OnceLock<Dirs> = OnceLock::new();

impl Dirs {
    fn resolve() -> Self {
        let local = PathBuf::from(format!(".{PROJECT_DIR}"));
        if local.exists() || env::local_mode() {
            return Self {
                config: local.clone(),
                data: local,
            };
        }
        Self {
            config: dirs::config_dir().map_or_else(|| local.clone(), |p| p.join(PROJECT_DIR)),
            data: dirs::data_dir().map_or_else(|| local.clone(), |p| p.join(PROJECT_DIR)),
        }
    }
}

fn dirs() -> &'static Dirs {
    DIRS.get_or_init(Dirs::resolve)
}

/// 取代 `.<project>` / XDG 的自动解析。
///
/// # Panics
/// 在任何路径被解析之后再调用会 panic。
pub fn set_dirs(config: PathBuf, data: PathBuf) {
    if DIRS.set(Dirs { config, data }).is_err() {
        panic!("set_dirs must be called before any path is resolved");
    }
}

pub fn config_dir() -> &'static Path {
    &dirs().config
}

pub fn data_dir() -> &'static Path {
    &dirs().data
}

pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn config_file_for(profile: &str) -> PathBuf {
    config_dir().join(format!("config.{profile}.toml"))
}

pub fn env_file() -> PathBuf {
    config_dir().join(".env")
}

pub fn log_dir() -> PathBuf {
    data_dir().join("logs")
}

pub fn media_dir(storage: &StorageConfig) -> PathBuf {
    let configured = PathBuf::from(&storage.media_dir);
    if configured.is_absolute() {
        configured
    } else {
        data_dir().join(configured)
    }
}
