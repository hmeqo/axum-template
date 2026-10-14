//! 改名、改密与重置都走与创建同一条策略（策略本体在 `domain::vo::credential`）。

use anyhow::Result;
use axum::http::StatusCode;
use axum_template::domain::model::DefaultRole;
use serde_json::json;

mod common;

use common::{Account, TestServer};

#[tokio::test]
async fn change_password_enforces_the_same_policy_as_create() -> Result<()> {
    let server = TestServer::start().await?;
    let session = server.login("pw_policy", None).await?;

    let refused = server
        .put("/api/auth/password")
        .cookie(&session)
        .json(&json!({
            "old_password": Account::PASSWORD,
            "new_password": "short",
        }))
        .send()
        .await?
        .expect(StatusCode::BAD_REQUEST)
        .expect_code("input.invalid_fields");
    assert!(
        refused.json()["errors"]["new_password"].is_array(),
        "should report a field-level error: {}",
        refused.text()
    );

    server
        .put("/api/auth/password")
        .cookie(&session)
        .json(&json!({
            "old_password": Account::PASSWORD,
            "new_password": "password456",
        }))
        .send()
        .await?
        .ok();

    let stored = server.ctx.srv().user.get_by_id(session.user_id()).await?;
    assert!(
        stored.password.verify("password456")?,
        "the new password should be stored"
    );

    Ok(())
}

#[tokio::test]
async fn reset_password_enforces_the_same_policy_as_create() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server
        .login("reset_policy", Some(DefaultRole::Admin.name()))
        .await?;
    let target = server
        .account("reset_policy_target", Some(DefaultRole::User.name()))
        .await?;

    let refused = server
        .post(format!("/api/users/{}/password/reset", target.id))
        .cookie(&admin)
        .json(&json!({"new_password": "short"}))
        .send()
        .await?
        .expect(StatusCode::BAD_REQUEST)
        .expect_code("input.invalid_fields");
    assert!(
        refused.json()["errors"]["new_password"].is_array(),
        "should report a field-level error: {}",
        refused.text()
    );

    Ok(())
}

#[tokio::test]
async fn update_username_enforces_the_same_policy_as_create() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server
        .login("name_policy", Some(DefaultRole::Admin.name()))
        .await?;
    let target = server.account("name_target", None).await?;
    let uri = format!("/api/users/{}/username", target.id);

    let refused = server
        .put(&uri)
        .cookie(&admin)
        .json(&json!({"username": "ab"}))
        .send()
        .await?
        .expect(StatusCode::BAD_REQUEST)
        .expect_code("input.invalid_fields");
    assert!(
        refused.json()["errors"]["username"].is_array(),
        "should report a field-level error: {}",
        refused.text()
    );

    let renamed = format!("{}_r", target.username);
    let res = server
        .put(&uri)
        .cookie(&admin)
        .json(&json!({"username": renamed}))
        .send()
        .await?
        .ok();

    assert_eq!(
        res.json()["username"],
        renamed,
        "the rename should be persisted and echoed back"
    );

    Ok(())
}

#[tokio::test]
async fn a_password_change_needs_the_old_password() -> Result<()> {
    let server = TestServer::start().await?;
    let session = server.login("pw_old_needed", None).await?;

    server
        .put("/api/auth/password")
        .cookie(&session)
        .json(&json!({
            "old_password": "wrong_password",
            "new_password": "password456",
        }))
        .send()
        .await?
        .expect(StatusCode::BAD_REQUEST)
        .expect_code("user.invalid_old_password");

    // 失败不能有副作用：新口令没有被写进去。
    server
        .post("/api/auth/login")
        .json(&json!({
            "username": session.username(),
            "password": "password456",
        }))
        .send()
        .await?
        .expect(StatusCode::UNAUTHORIZED)
        .expect_code("auth.invalid_credentials");

    Ok(())
}
