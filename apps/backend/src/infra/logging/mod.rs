use std::fs;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::LogConfig;

pub fn init_tracing(cfg: &LogConfig) {
    fs::create_dir_all("logs").ok();
    let file_appender = RollingFileAppender::new(Rotation::DAILY, "logs", "access.log");

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
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(file_appender),
        )
        .init();
}
