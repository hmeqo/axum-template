//! JWT 认证：访问令牌 + 单次消费的刷新令牌。

use anyhow::Result;
use axum::http::StatusCode;
use axum_template::domain::model::DefaultRole;
use serde_json::{Value, json};

mod common;

use common::{Account, TestResponse, TestServer, login_body};

async fn login(server: &TestServer, account: &Account) -> Result<Value> {
    let res = server
        .post("/api/auth/jwt/login")
        .json(&login_body(&account.username, Account::PASSWORD))
        .send()
        .await?
        .ok();

    Ok(res.json())
}

async fn refresh(server: &TestServer, refresh_token: &str) -> Result<TestResponse> {
    server
        .post("/api/auth/jwt/refresh")
        .json(&json!({"refresh_token": refresh_token}))
        .send()
        .await
}

#[tokio::test]
async fn access_token_authenticates_me() -> Result<()> {
    let server = TestServer::start().await?;
    let account = server
        .account("jwt_tokens", Some(DefaultRole::User.name()))
        .await?;
    let tokens = login(&server, &account).await?;
    let access = tokens["access_token"]
        .as_str()
        .expect("access_token")
        .to_owned();
    assert!(tokens["refresh_token"].is_string(), "{tokens}");

    let me = server
        .get("/api/auth/me")
        .bearer(&access)
        .send()
        .await?
        .ok();
    assert_eq!(me.json()["user"]["username"], account.username);

    Ok(())
}

#[tokio::test]
async fn refresh_rotates_the_pair_and_consumes_the_old_token() -> Result<()> {
    let server = TestServer::start().await?;
    let account = server.account("jwt_rotate", None).await?;
    let refresh_token = login(&server, &account).await?["refresh_token"]
        .as_str()
        .expect("refresh_token")
        .to_owned();

    let rotated = refresh(&server, &refresh_token).await?.ok().json();
    assert!(rotated["access_token"].is_string(), "{rotated}");
    assert_ne!(
        rotated["refresh_token"], refresh_token,
        "the refresh token must rotate, otherwise a single leak grants permanent access"
    );

    refresh(&server, &refresh_token)
        .await?
        .expect(StatusCode::UNAUTHORIZED)
        .expect_code("auth.refresh_token_invalid");

    Ok(())
}

/// 消费刷新令牌的条件删除必须原子：并发刷新只能有一个赢。
#[tokio::test]
async fn concurrent_refresh_lets_exactly_one_request_win() -> Result<()> {
    let server = TestServer::start().await?;
    let account = server.account("jwt_concurrent", None).await?;
    let refresh_token = login(&server, &account).await?["refresh_token"]
        .as_str()
        .expect("refresh_token")
        .to_owned();

    let (a, b) = tokio::join!(
        refresh(&server, &refresh_token),
        refresh(&server, &refresh_token),
    );
    let responses = [a?, b?];
    let winners = responses
        .iter()
        .filter(|res| res.status == StatusCode::OK)
        .count();
    assert_eq!(winners, 1, "exactly one concurrent rotation should succeed");

    responses
        .into_iter()
        .find(|res| res.status != StatusCode::OK)
        .expect("exactly one concurrent rotation must lose")
        .expect(StatusCode::UNAUTHORIZED)
        .expect_code("auth.refresh_token_invalid");

    Ok(())
}

#[tokio::test]
async fn logout_invalidates_refresh_tokens() -> Result<()> {
    let server = TestServer::start().await?;
    let account = server.account("jwt_logout", None).await?;
    let tokens = login(&server, &account).await?;
    let access = tokens["access_token"]
        .as_str()
        .expect("access_token")
        .to_owned();
    let refresh_token = tokens["refresh_token"]
        .as_str()
        .expect("refresh_token")
        .to_owned();

    server
        .post("/api/auth/jwt/logout")
        .bearer(&access)
        .send()
        .await?
        .ok();

    refresh(&server, &refresh_token)
        .await?
        .expect(StatusCode::UNAUTHORIZED);

    Ok(())
}

#[tokio::test]
async fn requests_without_a_token_are_rejected() -> Result<()> {
    let server = TestServer::start().await?;

    server
        .post("/api/auth/jwt/logout")
        .send()
        .await?
        .expect(StatusCode::UNAUTHORIZED);
    server
        .post("/api/auth/jwt/logout")
        .bearer("not-a-jwt")
        .send()
        .await?
        .expect(StatusCode::UNAUTHORIZED);

    Ok(())
}
