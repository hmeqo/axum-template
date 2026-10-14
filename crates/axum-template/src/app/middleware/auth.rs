//! 组认证层：把请求交给本组的认证后端，解析一次写进请求扩展；本层自己不产生响应。
//!
//! 挂载点在各组自己的 `router()`，没有全局认证层。

use std::{
    convert::Infallible,
    pin::Pin,
    task::{Context, Poll},
};

use axum::{
    extract::Request,
    http::{HeaderValue, StatusCode, header},
    response::Response,
};
use tower::{Layer, Service};

use crate::app::{
    AppContext,
    identity::{
        AuthBackend, Resolved,
        session::{RenewedSession, session_cookie},
    },
};

#[derive(Debug, Clone)]
pub struct AuthLayer<B> {
    ctx: AppContext,
    backend: B,
}

pub fn auth<B: AuthBackend>(ctx: &AppContext, backend: B) -> AuthLayer<B> {
    AuthLayer {
        ctx: ctx.clone(),
        backend,
    }
}

impl<S, B: AuthBackend> Layer<S> for AuthLayer<B> {
    type Service = AuthService<S, B>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthService {
            inner,
            ctx: self.ctx.clone(),
            backend: self.backend.clone(),
        }
    }
}

#[derive(Clone)]
pub struct AuthService<S, B> {
    inner: S,
    ctx: AppContext,
    backend: B,
}

// 手写 `Service` 而非返回 `middleware::from_fn_with_state` 的层：helper 只能声明
// `impl Layer<Route>`，而 `route_layer` 要求 `L::Service: Service<Request>`——不透明类型给不出该 impl。

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

// 续了期就重发 cookie，让浏览器与行一起前移；处理器自己已经写过这条 cookie
// （`logout` 清除）时不再写，免得把清除覆盖回去。
fn resend_session_cookie(response: &mut Response, ctx: &AppContext, renewed: &RenewedSession) {
    let name = &ctx.cfg().auth.session.cookie_name;
    let prefix = format!("{name}=");
    let already_written = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|value| value.starts_with(&prefix));
    if already_written {
        return;
    }

    let cookie = session_cookie(ctx, &renewed.token, renewed.expires_at);
    match HeaderValue::from_str(&cookie.to_string()) {
        Ok(value) => {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
        Err(error) => tracing::warn!(error = %error, "invalid session cookie"),
    }
}

impl<S, B> Service<Request> for AuthService<S, B>
where
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + Sync + 'static,
    S::Future: Send + 'static,
    B: AuthBackend,
{
    type Response = Response;
    type Error = Infallible;
    type Future = BoxFuture<Result<Response, Infallible>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let ready = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, ready);
        let backend = self.backend.clone();
        let ctx = self.ctx.clone();

        Box::pin(async move {
            let (mut parts, body) = req.into_parts();

            // 已定论就沿用（嵌套挂载时外层先跑过）：同样的凭证不解析第二遍。
            let resolved = match Resolved::cached(&parts.extensions) {
                Some(resolved) => resolved,
                None => {
                    let resolved = backend.resolve(&mut parts, &ctx).await;
                    parts.extensions.insert(resolved);
                    resolved
                }
            };

            let renewed = parts.extensions.get::<RenewedSession>().cloned();
            let mut response = inner.call(Request::from_parts(parts, body)).await?;
            if let Some(renewed) = renewed {
                resend_session_cookie(&mut response, &ctx, &renewed);
            }
            if response.status() == StatusCode::UNAUTHORIZED
                && let Some(challenge) = backend.challenge(&resolved)
            {
                match HeaderValue::from_str(challenge) {
                    Ok(value) => {
                        response
                            .headers_mut()
                            .insert(header::WWW_AUTHENTICATE, value);
                    }
                    // 后端的挑战值不是合法头：跳过，别让 401 变成 panic。
                    Err(error) => {
                        tracing::warn!(error = %error, challenge, "invalid WWW-Authenticate");
                    }
                }
            }
            Ok(response)
        })
    }
}
