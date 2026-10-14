//! 缓存默认值：`/api` 的响应按用户区分，不许任何缓存存放。

use axum::{
    extract::Request,
    http::{HeaderValue, header},
    middleware::Next,
    response::Response,
};

/// 给还没自己声明缓存的响应补上 `Cache-Control: no-store`。
pub async fn attach(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .entry(header::CACHE_CONTROL)
        .or_insert(HeaderValue::from_static("no-store"));
    response
}
