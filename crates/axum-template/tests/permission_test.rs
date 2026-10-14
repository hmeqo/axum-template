//! 权限门端到端：真登录 → 真 cookie → 403/200。

use anyhow::Result;
use axum::http::StatusCode;
use axum_template::domain::model::DefaultRole;
use serde_json::json;

mod common;

use axum_template::domain::vo::Perm;
use common::{Account, TestServer, login_body};

/// `user` 角色只有 `user:read`。
#[tokio::test]
async fn a_plain_user_reads_but_cannot_write() -> Result<()> {
    let server = TestServer::start().await?;
    let user = server
        .login("perm_user", Some(DefaultRole::User.name()))
        .await?;

    server.get("/api/users").cookie(&user).send().await?.ok();

    server
        .post("/api/users")
        .cookie(&user)
        .json(&json!({"username": "perm_denied", "password": Account::PASSWORD}))
        .send()
        .await?
        .expect(StatusCode::FORBIDDEN);

    server
        .delete("/api/users/999999")
        .cookie(&user)
        .send()
        .await?
        .expect(StatusCode::FORBIDDEN);

    Ok(())
}

/// `admin` 角色有 `user:*` 与 `role:*`。
#[tokio::test]
async fn an_admin_can_write_users() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server
        .login("perm_admin", Some(DefaultRole::Admin.name()))
        .await?;

    let created = server
        .post("/api/users")
        .cookie(&admin)
        .json(&json!({"username": "perm_created", "password": Account::PASSWORD}))
        .send()
        .await?
        .ok();

    assert_eq!(created.json()["username"], "perm_created");

    Ok(())
}

/// 重置他人密码是独立权限：`user:write` 不够，且重置会连目标的会话与刷新令牌一起清掉。
#[tokio::test]
async fn resetting_another_users_password_needs_its_own_permission() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server
        .login("perm_reset_admin", Some(DefaultRole::Admin.name()))
        .await?;

    let writer_role = server
        .ctx
        .srv()
        .role
        .create("user-writer".to_owned(), None, &[Perm::UserWrite])
        .await?;
    let writer = server.account("perm_reset_writer", None).await?;
    server
        .ctx
        .srv()
        .role
        .assign_to_user(writer.id, writer_role.id)
        .await?;
    let writer = server.open_session(writer).await?;

    let target = server
        .account("perm_reset_target", Some(DefaultRole::User.name()))
        .await?;
    let reset_uri = format!("/api/users/{}/password/reset", target.id);

    server
        .post(&reset_uri)
        .cookie(&writer)
        .json(&json!({"new_password": "password456"}))
        .send()
        .await?
        .expect(StatusCode::FORBIDDEN);

    let target_session = server.open_session(target.clone()).await?;
    let tokens = server
        .post("/api/auth/jwt/login")
        .json(&login_body(&target.username, Account::PASSWORD))
        .send()
        .await?
        .ok()
        .json();
    let refresh_token = tokens["refresh_token"]
        .as_str()
        .expect("refresh_token")
        .to_owned();

    server
        .post(&reset_uri)
        .cookie(&admin)
        .json(&json!({"new_password": "password456"}))
        .send()
        .await?
        .ok();

    server
        .post("/api/auth/login")
        .json(&login_body(&target.username, "password456"))
        .send()
        .await?
        .ok();
    server
        .post("/api/auth/login")
        .json(&login_body(&target.username, Account::PASSWORD))
        .send()
        .await?
        .expect(StatusCode::UNAUTHORIZED)
        .expect_code("auth.invalid_credentials");

    server
        .get("/api/auth/me")
        .cookie(&target_session)
        .send()
        .await?
        .expect(StatusCode::UNAUTHORIZED);
    server
        .post("/api/auth/jwt/refresh")
        .json(&json!({"refresh_token": refresh_token}))
        .send()
        .await?
        .expect(StatusCode::UNAUTHORIZED)
        .expect_code("auth.refresh_token_invalid");

    Ok(())
}

/// 权限判定先于路径与请求体校验：无权调用者拿到 403，即使请求本身是坏的。
#[tokio::test]
async fn a_permission_verdict_precedes_request_validation() -> Result<()> {
    let server = TestServer::start().await?;
    let plain = server
        .login("perm_order_plain", Some(DefaultRole::User.name()))
        .await?;
    let admin = server
        .login("perm_order_admin", Some(DefaultRole::Admin.name()))
        .await?;

    // 非法路径参数：无权是 403，有权才轮到 400。
    server
        .delete("/api/users/abc")
        .cookie(&plain)
        .send()
        .await?
        .expect(StatusCode::FORBIDDEN);
    server
        .delete("/api/users/abc")
        .cookie(&admin)
        .send()
        .await?
        .expect(StatusCode::BAD_REQUEST);

    // 坏 body 同理：请求体提取器只能排在末位，403 一定在它的结论之前。
    server
        .post("/api/users")
        .cookie(&plain)
        .body("application/json", "{not json")
        .send()
        .await?
        .expect(StatusCode::FORBIDDEN);
    server
        .post("/api/users")
        .cookie(&admin)
        .body("application/json", "{not json")
        .send()
        .await?
        .expect(StatusCode::BAD_REQUEST);

    Ok(())
}

/// 访问令牌带来的身份走同一道权限门：读写的判定与 cookie 身份完全一致。
#[tokio::test]
async fn a_token_identity_passes_the_same_gates() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server
        .account("perm_token_admin", Some(DefaultRole::Admin.name()))
        .await?;
    let admin_token = server.access_token(&admin).await?;

    let created = server
        .post("/api/users")
        .bearer(&admin_token)
        .json(&json!({"username": "perm_token_created", "password": Account::PASSWORD}))
        .send()
        .await?
        .ok();
    assert_eq!(created.json()["username"], "perm_token_created");

    let user = server
        .account("perm_token_user", Some(DefaultRole::User.name()))
        .await?;
    let user_token = server.access_token(&user).await?;

    server
        .delete("/api/users/999999")
        .bearer(&user_token)
        .send()
        .await?
        .expect(StatusCode::FORBIDDEN);

    Ok(())
}
