//! HTTP 表面：API 404、SPA 回退、健康探针、URL 里的 id 不存在。

use anyhow::Result;
use axum::http::StatusCode;
use axum_template::domain::model::DefaultRole;

mod common;

use common::TestServer;

/// `/api` 下的未知路径由嵌套路由自己的 fallback 兜住，不能掉进 SPA 回退。
#[tokio::test]
async fn unknown_api_paths_answer_json_404_without_a_business_code() -> Result<()> {
    let server = TestServer::start().await?;

    for uri in ["/api", "/api/does-not-exist"] {
        let res = server.get(uri).send().await?.expect(StatusCode::NOT_FOUND);
        assert!(
            res.header("content-type")
                .unwrap_or_default()
                .starts_with("application/json"),
            "{uri} should answer JSON: {}",
            res.text()
        );
        assert_eq!(res.code(), None, "{uri}: {}", res.text());
    }

    Ok(())
}

#[tokio::test]
async fn spa_routes_fall_back_to_the_embedded_index() -> Result<()> {
    let server = TestServer::start().await?;

    let res = server.get("/no/such/spa/route").send().await?.ok();

    assert!(
        res.header("content-type")
            .unwrap_or_default()
            .starts_with("text/html"),
        "{}",
        res.header("content-type").unwrap_or_default()
    );
    assert!(res.text().contains("<html"));
    Ok(())
}

#[tokio::test]
async fn health_probes_report_process_and_database() -> Result<()> {
    let server = TestServer::start().await?;

    let live = server.get("/health").send().await?.ok();
    assert_eq!(live.json()["status"], "ok");

    let ready = server.get("/health/ready").send().await?.ok();
    assert_eq!(ready.json()["status"], "ready");

    Ok(())
}

/// `/api` 的响应按用户区分，默认不许任何缓存存放；这条规则只覆盖 `/api` 子树。
#[tokio::test]
async fn api_responses_forbid_caching() -> Result<()> {
    let server = TestServer::start().await?;

    for uri in ["/api/error", "/api/no-such-route"] {
        let res = server.get(uri).send().await?;
        assert_eq!(
            res.header("cache-control"),
            Some("no-store"),
            "{uri} must forbid caching: {}",
            res.text()
        );
    }

    let spa = server.get("/").send().await?;
    assert!(
        spa.header("cache-control").is_none(),
        "the api cache default is scoped to /api, so the SPA keeps its own policy"
    );

    Ok(())
}

#[tokio::test]
async fn missing_user_id_is_not_found() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server
        .login("routing_missing", Some(DefaultRole::Admin.name()))
        .await?;

    server
        .get("/api/users/999999")
        .cookie(&admin)
        .send()
        .await?
        .expect(StatusCode::NOT_FOUND);

    Ok(())
}
