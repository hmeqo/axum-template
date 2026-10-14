//! 分页契约：默认值、结果窗口边界、非法 `page`/`per_page` 的 400。

use anyhow::Result;
use axum::http::StatusCode;
use axum_template::domain::model::DefaultRole;

mod common;

use common::TestServer;

#[tokio::test]
async fn defaults_live_inside_the_result_window() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server
        .login("pagination_defaults", Some(DefaultRole::Admin.name()))
        .await?;

    let first = server.get("/api/users").cookie(&admin).send().await?.ok();
    assert_eq!(first.json()["page"], 1, "{}", first.text());
    assert_eq!(first.json()["per_page"], 10, "{}", first.text());

    // 窗口内的最后一页合法,但已越过全部数据:页内容为空,`total` 不变。
    let last = server
        .get("/api/users?page=1001&per_page=10")
        .cookie(&admin)
        .send()
        .await?
        .ok();
    assert_eq!(
        last.json()["users"],
        serde_json::json!([]),
        "{}",
        last.text()
    );
    assert_eq!(last.json()["total"], first.json()["total"]);

    // 再往后一页就越过窗口:400,而不是一个空页。
    let beyond = server
        .get("/api/users?page=1002&per_page=10")
        .cookie(&admin)
        .send()
        .await?
        .expect(StatusCode::BAD_REQUEST);
    assert_eq!(beyond.code(), None, "{}", beyond.text());

    Ok(())
}

#[tokio::test]
async fn page_and_per_page_are_range_checked() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server
        .login("pagination_range", Some(DefaultRole::Admin.name()))
        .await?;

    for (query, field) in [
        ("page=0", "page"),
        ("per_page=0", "per_page"),
        ("per_page=101", "per_page"),
    ] {
        let refused = server
            .get(format!("/api/users?{query}"))
            .cookie(&admin)
            .send()
            .await?
            .expect(StatusCode::BAD_REQUEST);
        assert!(
            refused.json()["errors"][field].is_array(),
            "{query}: {}",
            refused.text()
        );
    }

    Ok(())
}

#[tokio::test]
async fn an_unparseable_query_is_a_codeless_bad_request() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server
        .login("pagination_parse", Some(DefaultRole::Admin.name()))
        .await?;

    let refused = server
        .get("/api/users?page=abc")
        .cookie(&admin)
        .send()
        .await?
        .expect(StatusCode::BAD_REQUEST);
    assert_eq!(refused.code(), None, "{}", refused.text());

    Ok(())
}
