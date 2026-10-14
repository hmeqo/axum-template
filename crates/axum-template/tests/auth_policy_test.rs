//! 组认证策略：每请求只解析一份凭证，`Bearer` 优先于会话 cookie，凭证无效不降级。
//!
//! 这里钉的是解析顺序与失败语义，含 401 上的 `WWW-Authenticate` 挑战。

use anyhow::Result;
use axum::http::StatusCode;
use axum_template::domain::model::DefaultRole;
use serde_json::json;

mod common;

use common::{Account, TestServer, in_secs, login_body};

#[tokio::test]
async fn a_resource_group_accepts_either_credential() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server
        .account("policy_admin", Some(DefaultRole::Admin.name()))
        .await?;
    let user = server
        .account("policy_user", Some(DefaultRole::User.name()))
        .await?;

    // 只带令牌、不带 cookie。
    let admin_token = server.access_token(&admin).await?;
    server
        .get("/api/users")
        .bearer(&admin_token)
        .send()
        .await?
        .ok();

    // 只带 cookie、不带令牌。
    let session = server.open_session(user).await?;
    server.get("/api/users").cookie(&session).send().await?.ok();

    Ok(())
}

#[tokio::test]
async fn the_identity_endpoint_answers_for_either_credential() -> Result<()> {
    let server = TestServer::start().await?;
    let account = server
        .account("policy_me", Some(DefaultRole::User.name()))
        .await?;

    let session = server.open_session(account.clone()).await?;
    let by_cookie = server
        .get("/api/auth/me")
        .cookie(&session)
        .send()
        .await?
        .ok();
    assert_eq!(
        by_cookie.json()["user"]["username"].as_str(),
        Some(account.username.as_str())
    );

    let token = server.access_token(&account).await?;
    let by_token = server.get("/api/auth/me").bearer(&token).send().await?.ok();
    assert_eq!(
        by_token.json()["user"]["username"].as_str(),
        Some(account.username.as_str())
    );

    Ok(())
}

/// 自助改密与 `/me` 同组：访问令牌也够；令牌身份不在任何会话上，该账号的会话全部失效。
#[tokio::test]
async fn self_service_password_change_accepts_a_token() -> Result<()> {
    let server = TestServer::start().await?;
    let session = server
        .login("policy_self", Some(DefaultRole::User.name()))
        .await?;
    let token = server.access_token(&session.account).await?;

    server
        .put("/api/auth/password")
        .bearer(&token)
        .json(&json!({
            "old_password": Account::PASSWORD,
            "new_password": "password456",
        }))
        .send()
        .await?
        .ok();

    server
        .get("/api/auth/me")
        .cookie(&session)
        .send()
        .await?
        .expect(StatusCode::UNAUTHORIZED);

    server
        .post("/api/auth/login")
        .json(&login_body(&session.account.username, "password456"))
        .send()
        .await?
        .ok();

    Ok(())
}

#[tokio::test]
async fn a_group_rejects_the_credential_it_does_not_accept() -> Result<()> {
    let server = TestServer::start().await?;
    let session = server
        .login("policy_group", Some(DefaultRole::User.name()))
        .await?;
    let token = server.access_token(&session.account).await?;

    // 会话专用端点不认令牌。
    server
        .get("/api/auth/sessions")
        .bearer(&token)
        .send()
        .await?
        .expect(StatusCode::UNAUTHORIZED);

    // 令牌专用端点不认 cookie。
    server
        .post("/api/auth/jwt/logout")
        .cookie(&session)
        .send()
        .await?
        .expect(StatusCode::UNAUTHORIZED);

    Ok(())
}

#[tokio::test]
async fn an_invalid_token_is_never_downgraded_to_the_cookie() -> Result<()> {
    let server = TestServer::start().await?;
    let session = server
        .login("policy_strict", Some(DefaultRole::Admin.name()))
        .await?;

    server.get("/api/users").cookie(&session).send().await?.ok();

    server
        .get("/api/users")
        .cookie(&session)
        .bearer("garbage")
        .send()
        .await?
        .expect(StatusCode::UNAUTHORIZED);

    Ok(())
}

#[tokio::test]
async fn a_stale_cookie_does_not_block_login() -> Result<()> {
    let server = TestServer::start().await?;
    let session = server
        .login("policy_stale", Some(DefaultRole::User.name()))
        .await?;
    server.age_session(&session.token, in_secs(-60)).await?;

    let expired = server
        .get("/api/users")
        .cookie(&session)
        .send()
        .await?
        .expect(StatusCode::UNAUTHORIZED);
    assert_eq!(expired.code().as_deref(), Some("auth.session_expired"));

    // 上面那次请求已经删掉了过期会话行；这里再带同一份 cookie，登录仍要成功。
    server
        .post("/api/auth/login")
        .cookie(&session)
        .json(&login_body(&session.account.username, Account::PASSWORD))
        .send()
        .await?
        .ok();

    Ok(())
}

/// 挑战只广告本组接受的方案，凭证无效时补 `error="invalid_token"`。
#[tokio::test]
async fn a_challenge_names_only_the_accepted_scheme() -> Result<()> {
    let server = TestServer::start().await?;

    let anonymous = server
        .get("/api/users")
        .send()
        .await?
        .expect(StatusCode::UNAUTHORIZED);
    assert_eq!(
        anonymous.header("www-authenticate"),
        Some("Bearer realm=\"api\"")
    );

    let invalid = server
        .get("/api/users")
        .bearer("garbage")
        .send()
        .await?
        .expect(StatusCode::UNAUTHORIZED);
    assert_eq!(
        invalid.header("www-authenticate"),
        Some("Bearer realm=\"api\", error=\"invalid_token\"")
    );

    // 只认 cookie 的组没有 `Bearer` 方案可广告。
    let cookie_only = server
        .get("/api/auth/sessions")
        .send()
        .await?
        .expect(StatusCode::UNAUTHORIZED);
    assert_eq!(cookie_only.header("www-authenticate"), None);

    Ok(())
}
