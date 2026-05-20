use std::fs;

use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::{LogConfig, Paths};

pub fn init_tracing(cfg: &LogConfig) {
    let log_dir = Paths::log_dir();
    fs::create_dir_all(&log_dir).ok();
    let file_appender = RollingFileAppender::new(Rotation::DAILY, &log_dir, "access.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    // keep worker alive for program lifetime
    Box::leak(Box::new(guard));

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
}
