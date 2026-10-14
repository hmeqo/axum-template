//! 请求关联 ID：同值用于请求扩展、日志 span 与响应头。

use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};
use tracing::Instrument;
use uuid::Uuid;

pub const REQUEST_ID_HEADER: &str = "x-request-id";

#[derive(Debug, Clone)]
pub struct RequestId(pub String);

pub async fn attach(mut req: Request, next: Next) -> Response {
    let id = Uuid::new_v4().to_string();
    let value = HeaderValue::from_str(&id).expect("uuid hex is a valid header value");

    req.extensions_mut().insert(RequestId(id.clone()));

    let span = tracing::info_span!("request", request_id = %id);
    let mut response = next.run(req).instrument(span).await;

    response.headers_mut().insert(REQUEST_ID_HEADER, value);
    response.extensions_mut().insert(RequestId(id));
    response
}
