use axum::http::HeaderValue;
use tower_http::cors::{AllowHeaders, AllowMethods, Any, CorsLayer};

use crate::config::CorsConfig;

pub fn cors(cfg: &CorsConfig) -> CorsLayer {
    if cfg.allowed_origins.is_empty() {
        return CorsLayer::new().allow_methods(Any).allow_headers(Any);
    }

    // 凭据与任一 `*` 通配符不能共存,否则 tower-http 在构造期断言失败(浏览器同样拒绝),
    // 因此回显预检请求声明的 header 与 method,而不是声明通配符。
    let origins: Vec<HeaderValue> = cfg
        .allowed_origins
        .iter()
        // 白名单已由 `AppConfig::validate` 按 `Origin` 形状定下,这里的解析不会失败。
        .filter_map(|o| o.parse().ok())
        .collect();

    CorsLayer::new()
        .allow_methods(AllowMethods::mirror_request())
        .allow_headers(AllowHeaders::mirror_request())
        .allow_origin(origins)
        .allow_credentials(true)
}
