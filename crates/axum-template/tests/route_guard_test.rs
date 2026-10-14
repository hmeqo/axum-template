//! 路由守卫：文档里每条路由在无会话时都应是 401（除非在 `PUBLIC` 里）。

use std::str::FromStr;

use anyhow::Result;
use axum::http::{Method, StatusCode};

mod common;

use common::TestServer;

const PUBLIC: &[&str] = &[
    "/api/error",
    "/api/auth/login",
    "/api/auth/jwt/login",
    "/api/auth/jwt/refresh",
];

/// 文档之外的入口：RPC 的方法路径挂在 `/api/rpc` 之下，同样要凭据。
///
/// 反射服务不在此列：它挂在 `/api-docs/rpc` 与其它文档入口同级，本来就公开。
const UNDOCUMENTED_GUARDED: &[(&str, &str)] = &[
    ("post", "/api/rpc/template.v1.TemplateService/Me"),
    ("post", "/api/rpc/template.v1.TemplateService/ListUsers"),
];

const METHODS: &[&str] = &["get", "post", "put", "patch", "delete"];

fn concrete(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            if segment.starts_with('{') {
                "1"
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[tokio::test]
async fn api_routes_require_auth_unless_whitelisted() -> Result<()> {
    let server = TestServer::start().await?;
    let doc = server.openapi().await?;
    let paths = doc["paths"].as_object().expect("openapi paths").clone();

    let mut guarded = 0;
    for (path, item) in &paths {
        for method in METHODS {
            if item.get(method).is_none() {
                continue;
            }

            let res = server
                .request(Method::from_str(&method.to_uppercase())?, concrete(path))
                .body("application/json", "{}")
                .send()
                .await?;

            if PUBLIC.contains(&path.as_str()) {
                continue;
            }

            assert_eq!(
                res.status,
                StatusCode::UNAUTHORIZED,
                "{method} {path} without a session should be 401; if public, add to PUBLIC: {}",
                res.text()
            );
            guarded += 1;
        }
    }

    assert!(guarded > 0, "no protected routes were exercised");

    for (method, uri) in UNDOCUMENTED_GUARDED {
        server
            .request(Method::from_str(&method.to_uppercase())?, *uri)
            .body("application/json", "{}")
            .send()
            .await?
            .expect(StatusCode::UNAUTHORIZED);
    }

    Ok(())
}
