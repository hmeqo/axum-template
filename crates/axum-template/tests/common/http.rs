//! HTTP 收发：链式构造请求 → `send()` → 带断言的 [`TestResponse`]。

use anyhow::Result;
use axum::{
    body::{Body, Bytes},
    http::{Extensions, HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header},
};
use serde::Serialize;
use serde_json::Value;
use tower::ServiceExt;

use super::{Session, TestServer};

pub struct TestRequest<'a> {
    server: &'a TestServer,
    method: Method,
    uri: String,
    headers: Vec<(HeaderName, HeaderValue)>,
    body: Body,
}

impl<'a> TestRequest<'a> {
    pub(super) fn new(server: &'a TestServer, method: Method, uri: String) -> Self {
        Self {
            server,
            method,
            uri,
            headers: Vec::new(),
            body: Body::empty(),
        }
    }

    pub fn header(mut self, name: HeaderName, value: HeaderValue) -> Self {
        self.headers.push((name, value));
        self
    }

    pub fn json<T: Serialize>(mut self, value: &T) -> Self {
        self.headers.push((
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        ));
        self.body = Body::from(serde_json::to_vec(value).expect("request body should serialize"));
        self
    }

    pub fn body(mut self, content_type: &str, bytes: impl Into<Bytes>) -> Self {
        self.headers.push((
            header::CONTENT_TYPE,
            HeaderValue::from_str(content_type)
                .expect("content-type should be a valid header value"),
        ));
        self.body = Body::from(bytes.into());
        self
    }

    pub fn cookie(self, session: &Session) -> Self {
        self.raw_cookie(&session.cookie)
    }

    pub fn raw_cookie(self, value: &str) -> Self {
        self.header(
            header::COOKIE,
            HeaderValue::from_str(value).expect("cookie should be a valid header value"),
        )
    }

    pub fn bearer(self, token: &str) -> Self {
        self.header(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}"))
                .expect("token should be a valid header value"),
        )
    }

    pub async fn send(self) -> Result<TestResponse> {
        let Self {
            server,
            method,
            uri,
            headers,
            body,
        } = self;

        let mut request = axum::http::Request::builder().method(&method).uri(&uri);
        for (name, value) in headers {
            request = request.header(name, value);
        }

        let response = server.app.clone().oneshot(request.body(body)?).await?;
        let status = response.status();
        let headers = response.headers().clone();
        let extensions = response.extensions().clone();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;

        Ok(TestResponse {
            request: format!("{method} {uri}"),
            status,
            headers,
            extensions,
            body,
        })
    }
}

pub struct TestResponse {
    request: String,
    pub status: StatusCode,
    headers: HeaderMap,
    extensions: Extensions,
    body: Bytes,
}

impl TestResponse {
    /// 空体或非 JSON 时返回 `null`。
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.body
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }

    /// 同名头的全部取值（`Vary` 会被多层各写一条）。
    pub fn header_values(&self, name: &str) -> Vec<String> {
        self.headers
            .get_all(name)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .map(str::to_owned)
            .collect()
    }

    pub fn set_cookie(&self) -> Option<String> {
        self.set_cookies()
            .into_iter()
            .next()
            .and_then(|cookie| cookie.split(';').next().map(str::to_owned))
    }

    pub fn set_cookies(&self) -> Vec<String> {
        self.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .map(str::to_owned)
            .collect()
    }

    pub fn code(&self) -> Option<String> {
        self.json()["code"].as_str().map(str::to_owned)
    }

    /// 响应扩展（例如 `RequestId`）；extensions 在进入中间件前就被挂上，故测试可读。
    pub fn extension<T: Clone + Send + Sync + 'static>(&self) -> Option<T> {
        self.extensions.get::<T>().cloned()
    }

    #[track_caller]
    pub fn expect(self, status: StatusCode) -> Self {
        assert_eq!(self.status, status, "{}", self.failure(&status));
        self
    }

    #[track_caller]
    pub fn ok(self) -> Self {
        self.expect(StatusCode::OK)
    }

    #[track_caller]
    pub fn expect_code(self, code: &str) -> Self {
        assert_eq!(
            self.code().as_deref(),
            Some(code),
            "{}",
            self.failure(&code)
        );
        self
    }

    fn failure(&self, expected: &dyn std::fmt::Display) -> String {
        format!(
            "{} expected {expected}, got {}: {}",
            self.request,
            self.status,
            self.text()
        )
    }
}
