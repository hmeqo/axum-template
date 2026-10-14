//! 用户资源的成功路径：创建 → 取单条 → 删除 → 再取为 404。

use anyhow::Result;
use axum::http::StatusCode;
use axum_template::domain::model::DefaultRole;
use serde_json::json;

mod common;

use common::{Account, TestServer};

#[tokio::test]
async fn a_created_user_can_be_fetched_and_deleted() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server
        .login("crud_admin", Some(DefaultRole::Admin.name()))
        .await?;

    let created = server
        .post("/api/users")
        .cookie(&admin)
        .json(&json!({
            "username": "crud_target",
            "password": Account::PASSWORD,
        }))
        .send()
        .await?
        .ok();
    assert_eq!(
        created.json()["username"],
        "crud_target",
        "{}",
        created.text()
    );
    let id = created.json()["id"].as_i64().expect("a numeric user id");

    let fetched = server
        .get(format!("/api/users/{id}"))
        .cookie(&admin)
        .send()
        .await?
        .ok();
    assert_eq!(fetched.json()["username"], "crud_target");

    let deleted = server
        .delete(format!("/api/users/{id}"))
        .cookie(&admin)
        .send()
        .await?
        .ok();
    assert!(deleted.json()["message"].is_string());

    server
        .get(format!("/api/users/{id}"))
        .cookie(&admin)
        .send()
        .await?
        .expect(StatusCode::NOT_FOUND);

    Ok(())
}
