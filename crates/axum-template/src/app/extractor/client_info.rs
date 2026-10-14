//! 把请求里的对端地址、`X-Forwarded-For`(仅在来源可信时)与 `User-Agent` 映射成 `ClientInfo`。

use std::{
    convert::Infallible,
    net::{IpAddr, SocketAddr},
};

use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{Extensions, HeaderMap, HeaderName, Request, header, request::Parts},
};

use crate::{app::AppContext, config::TrustedProxies, domain::vo::ClientInfo};

// `http` 的 `header` 模块没有这个常量。
static X_FORWARDED_FOR: HeaderName = HeaderName::from_static("x-forwarded-for");

pub fn from_parts(parts: &Parts, trusted: &TrustedProxies) -> ClientInfo {
    ClientInfo {
        ip: resolve_ip(&parts.extensions, &parts.headers, trusted),
        user_agent: user_agent(&parts.headers),
    }
}

pub fn ip_from_request<B>(req: &Request<B>, trusted: &TrustedProxies) -> Option<IpAddr> {
    resolve_ip(req.extensions(), req.headers(), trusted)
}

fn resolve_ip(
    extensions: &Extensions,
    headers: &HeaderMap,
    trusted: &TrustedProxies,
) -> Option<IpAddr> {
    let peer = extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|connect_info| connect_info.0.ip());
    let forwarded = forwarded_chain(headers);

    peer.map(|peer| trusted.resolve(peer, forwarded.as_deref()))
}

fn forwarded_chain(headers: &HeaderMap) -> Option<String> {
    let mut values = headers.get_all(&X_FORWARDED_FOR).iter();
    let mut chain = values.next()?.to_str().ok()?.to_owned();
    for value in values {
        if let Ok(value) = value.to_str() {
            chain.push(',');
            chain.push_str(value);
        }
    }
    Some(chain)
}

fn user_agent(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

impl FromRequestParts<AppContext> for ClientInfo {
    // 永远成功:缺失 `ConnectInfo` 只是 `ip` 为 `None`,不该让请求 500。
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppContext,
    ) -> Result<Self, Self::Rejection> {
        Ok(from_parts(parts, state.trusted_proxies()))
    }
}
