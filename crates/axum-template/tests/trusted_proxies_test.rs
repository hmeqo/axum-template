//! 可信代理:只有来自配置里那些地址/网段的 `X-Forwarded-For` 才被采纳。

use anyhow::Result;
use axum::http::HeaderName;
use axum_template::domain::model::DefaultRole;
use serde_json::Value;

mod common;

use common::{Session, TestServer};

async fn recorded_ip(server: &TestServer, session: &Session) -> Result<Value> {
    let body = server
        .get("/api/auth/sessions")
        .cookie(session)
        .send()
        .await?
        .ok()
        .json();

    Ok(body["sessions"][0]["ip"].clone())
}

#[tokio::test]
async fn a_trusted_peer_may_forward_the_client_address() -> Result<()> {
    let server = TestServer::start_with(|config| {
        config.server.trusted_proxies = vec!["127.0.0.1".to_owned()];
    })
    .await?;
    let account = server
        .account("proxy_single", Some(DefaultRole::Admin.name()))
        .await?;

    let session = server
        .login_with_headers(
            &account,
            &[(HeaderName::from_static("x-forwarded-for"), "203.0.113.7")],
        )
        .await?;

    assert_eq!(recorded_ip(&server, &session).await?, "203.0.113.7");

    Ok(())
}

#[tokio::test]
async fn trusted_hops_are_skipped_from_the_right() -> Result<()> {
    let server = TestServer::start_with(|config| {
        config.server.trusted_proxies = vec!["127.0.0.1".to_owned(), "10.0.0.0/8".to_owned()];
    })
    .await?;
    let account = server
        .account("proxy_chain", Some(DefaultRole::Admin.name()))
        .await?;

    let session = server
        .login_with_headers(
            &account,
            &[(
                HeaderName::from_static("x-forwarded-for"),
                "203.0.113.7, 10.0.0.5",
            )],
        )
        .await?;

    assert_eq!(recorded_ip(&server, &session).await?, "203.0.113.7");

    Ok(())
}

#[tokio::test]
async fn a_second_forwarded_field_line_is_read_too() -> Result<()> {
    let server = TestServer::start_with(|config| {
        config.server.trusted_proxies = vec!["127.0.0.1".to_owned()];
    })
    .await?;
    let account = server
        .account("proxy_field_lines", Some(DefaultRole::Admin.name()))
        .await?;

    // 客户端自填第一行,可信代理把真实来源追加成第二行:判定必须看完整条链,而不是只读第一行。
    let session = server
        .login_with_headers(
            &account,
            &[
                (HeaderName::from_static("x-forwarded-for"), "203.0.113.7"),
                (HeaderName::from_static("x-forwarded-for"), "198.51.100.9"),
            ],
        )
        .await?;

    assert_eq!(recorded_ip(&server, &session).await?, "198.51.100.9");

    Ok(())
}

#[tokio::test]
async fn an_untrusted_peer_keeps_the_forwarded_header_out() -> Result<()> {
    let server = TestServer::start_with(|config| {
        config.server.trusted_proxies = vec!["10.0.0.0/8".to_owned()];
    })
    .await?;
    let account = server
        .account("proxy_untrusted", Some(DefaultRole::Admin.name()))
        .await?;

    let session = server
        .login_with_headers(
            &account,
            &[(HeaderName::from_static("x-forwarded-for"), "203.0.113.7")],
        )
        .await?;

    assert_eq!(recorded_ip(&server, &session).await?, "127.0.0.1");

    Ok(())
}
