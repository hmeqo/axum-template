use std::fs;

use tracing_appender::{
    non_blocking::WorkerGuard,
    rolling::{RollingFileAppender, Rotation},
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    config::{LogConfig, paths},
    error::Result,
};

/// 返回的 guard 必须持有到进程结束（drop 时冲刷非阻塞缓冲）。
pub fn init_tracing(cfg: &LogConfig) -> Result<WorkerGuard> {
    let log_dir = paths::log_dir();
    fs::create_dir_all(&log_dir)?;
    let file_appender = RollingFileAppender::new(Rotation::DAILY, &log_dir, "access.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!(
                    "{0}={1},tower_http={1}",
                    env!("CARGO_CRATE_NAME"),
                    cfg.level
                )
                .into()
            }),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(true)
                .with_writer(std::io::stdout),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_writer(non_blocking),
        )
        .init();

    Ok(guard)
}
