use crate::config::meta::ENV_PREFIX;

pub fn local_mode() -> bool {
    std::env::var(format!("{}_LOCAL_MODE", ENV_PREFIX.as_str())).is_ok()
}
