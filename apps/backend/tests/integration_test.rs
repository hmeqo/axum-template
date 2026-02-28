use anyhow::Result;
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use backend::app::AppBootstrap;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn create_router_with_state() -> Result<Router> {
    let mut bootstrap = AppBootstrap::load()?;
    let domain = bootstrap.create_domain().await?;
    let app_state = bootstrap.create_app_state(domain);
    Ok(bootstrap.create_router(app_state).await?)
}

#[tokio::test]
async fn test_root_endpoint() -> Result<()> {
    let app = create_router_with_state().await?;

    let request = Request::builder()
        .method("GET")
        .uri("/api")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn test_hello_endpoint() -> Result<()> {
    let app = create_router_with_state().await?;

    let request = Request::builder()
        .method("POST")
        .uri("/api/hello")
        .header("content-type", "application/json")
        .body(Body::from(json!({"name": "World"}).to_string()))?;

    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let body: Value = serde_json::from_slice(&body)?;

    assert_eq!(body["message"], "Hello World");
    Ok(())
}
