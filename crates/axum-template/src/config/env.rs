use std::path::Path;

use crate::{
    config::{meta::ENV_PREFIX, paths},
    error::{AppError, Result},
};

fn var(suffix: &str) -> Option<String> {
    std::env::var(format!("{}_{suffix}", ENV_PREFIX.as_str()))
        .ok()
        .filter(|v| !v.is_empty())
}

/// 强制 `paths` 使用项目内 `.<project>` 目录。
pub fn local_mode() -> bool {
    var("LOCAL_MODE").is_some()
}

/// 运行 profile（`AXUM_TEMPLATE_ENV`），决定叠加哪个 `config.<name>.toml`。
pub fn profile() -> Option<String> {
    var("ENV")
}

/// 显式指定的配置文件，设置后不叠加其它来源。
pub fn config_file_override() -> Option<String> {
    var("CONFIG_FILE")
}

/// 加载 `.env`：只填充尚未设置的环境变量，因此真实环境变量始终优先。
pub fn load_env_file() -> Result<()> {
    if let Some(path) = var("ENV_FILE") {
        return load_dotenv(Path::new(&path));
    }

    let path = paths::env_file();
    if path.is_file() {
        load_dotenv(&path)?;
    }
    Ok(())
}

fn load_dotenv(path: &Path) -> Result<()> {
    dotenvy::from_path(path).map_err(|e| AppError::config(format!("{}: {e}", path.display())))
}
