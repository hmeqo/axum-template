//! 用户列表过滤：按词 AND、忽略大小写、通配符按字面、total 跟随过滤。

use anyhow::Result;
use axum::http::StatusCode;
use axum_template::domain::model::DefaultRole;
use serde_json::json;

mod common;

use common::{Account, TestServer};

#[tokio::test]
async fn username_filters_the_page_and_the_total() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server
        .login("filter_admin", Some(DefaultRole::Admin.name()))
        .await?;

    for username in ["Zhang San", "San Zhang", "solo%user", "soloXuser"] {
        server
            .post("/api/users")
            .cookie(&admin)
            .json(&json!({"username": username, "password": Account::PASSWORD}))
            .send()
            .await?
            .ok();
    }

    // 全小写、两词:证明忽略大小写 + 切词 + 词序无关(字面子串只能命中其中一个)。
    let res = server
        .get("/api/users?username=san+zhang")
        .cookie(&admin)
        .send()
        .await?
        .ok();
    let body = res.json();
    assert_eq!(body["total"], 2, "{body}");
    let mut names: Vec<&str> = body["users"]
        .as_array()
        .expect("users should be an array")
        .iter()
        .map(|user| user["username"].as_str().expect("username is a string"))
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["San Zhang", "Zhang San"], "{body}");

    // `%` 是字面字符而非通配符:只命中真的含 `%` 的那个。
    let res = server
        .get("/api/users?username=solo%25user")
        .cookie(&admin)
        .send()
        .await?
        .ok();
    let body = res.json();
    assert_eq!(body["total"], 1, "{body}");
    assert_eq!(body["users"][0]["username"], "solo%user", "{body}");

    // 空白词 = 不过滤:与不带该参数的结果一致。
    let unfiltered = server.get("/api/users").cookie(&admin).send().await?.ok();
    let blank = server
        .get("/api/users?username=")
        .cookie(&admin)
        .send()
        .await?
        .ok();
    assert_eq!(unfiltered.json()["total"], blank.json()["total"]);
    assert!(unfiltered.json()["total"].as_u64().unwrap() >= 5);

    // 超过 32 字符是字段级 400。
    let long = "a".repeat(33);
    let refused = server
        .get(format!("/api/users?username={long}"))
        .cookie(&admin)
        .send()
        .await?
        .expect(StatusCode::BAD_REQUEST);
    assert!(
        refused.json()["errors"]["username"].is_array(),
        "{}",
        refused.text()
    );

    Ok(())
}
