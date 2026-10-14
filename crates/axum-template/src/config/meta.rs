use std::sync::LazyLock;

pub static PROJECT_NAME: &str = env!("CARGO_PKG_NAME");

pub static PROJECT_DIR: &str = PROJECT_NAME;

pub static ENV_PREFIX: LazyLock<String> =
    LazyLock::new(|| PROJECT_NAME.to_uppercase().replace('-', "_"));
