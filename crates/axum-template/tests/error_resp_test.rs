//! 错误响应契约（`{code?, message, errors?}`）与请求关联 ID。

use anyhow::Result;
use axum::http::StatusCode;
use axum_template::{
    app::middleware::request_id::RequestId, domain::model::DefaultRole, error::ErrorCode,
};
use serde_json::json;

mod common;

use common::{TestServer, login_body};

#[tokio::test]
async fn transport_failures_carry_no_code_and_no_field_errors() -> Result<()> {
    let server = TestServer::start().await?;

    let res = server
        .get("/api/does-not-exist")
        .send()
        .await?
        .expect(StatusCode::NOT_FOUND);

    assert!(
        res.code().is_none(),
        "404 carries no business code: {}",
        res.text()
    );
    assert!(
        res.json().get("errors").is_none(),
        "non-validation errors should not carry errors: {}",
        res.text()
    );

    Ok(())
}

#[tokio::test]
async fn malformed_bodies_are_a_codeless_400() -> Result<()> {
    let server = TestServer::start().await?;

    let res = server
        .post("/api/auth/login")
        .body("application/json", "{not json")
        .send()
        .await?
        .expect(StatusCode::BAD_REQUEST);

    assert!(
        res.code().is_none(),
        "a malformed request carries no business code: {}",
        res.text()
    );
    assert!(
        res.json().get("errors").is_none(),
        "should not carry errors: {}",
        res.text()
    );

    Ok(())
}

/// 两道体积上限同状态同码：`DefaultBodyLimit` 拦下的 JSON 体与 `MediaStore` 拦下的文件都是 413。
#[tokio::test]
async fn a_json_body_over_the_body_limit_is_413_with_the_file_too_large_code() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server
        .login("error_resp_body_limit", Some(DefaultRole::Admin.name()))
        .await?;

    let oversized = "a".repeat(server.ctx.cfg().storage.request_body_limit() + 1);

    server
        .post("/api/users")
        .cookie(&admin)
        .json(&json!({"username": "over", "password": oversized}))
        .send()
        .await?
        .expect(StatusCode::PAYLOAD_TOO_LARGE)
        .expect_code("file.too_large");

    Ok(())
}

/// 凭据错误是 401（未认证），不是 403（已认证但无权）；客户端据此决定是否重登。
#[tokio::test]
async fn invalid_credentials_are_401_with_their_own_code() -> Result<()> {
    let server = TestServer::start().await?;

    let res = server
        .post("/api/auth/login")
        .json(&login_body("nobody", "wrong-password"))
        .send()
        .await?;

    res.expect(StatusCode::UNAUTHORIZED)
        .expect_code("auth.invalid_credentials");

    Ok(())
}

#[tokio::test]
async fn validation_failures_name_the_offending_field() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server
        .login("error_resp_validation", Some(DefaultRole::Admin.name()))
        .await?;

    let res = server
        .post("/api/users")
        .cookie(&admin)
        .json(&json!({"username": "ab", "password": "short"}))
        .send()
        .await?
        .expect(StatusCode::BAD_REQUEST)
        .expect_code("input.invalid_fields");

    let errors = res.json()["errors"].clone();
    for field in ["username", "password"] {
        assert!(
            errors.get(field).is_some(),
            "the validation failure should name `{field}`: {}",
            res.text()
        );
    }

    Ok(())
}

#[tokio::test]
async fn request_id_is_on_the_api_subtree_and_the_media_route() -> Result<()> {
    let server = TestServer::start().await?;

    let api = server.get("/api/does-not-exist").send().await?;
    let id = api
        .header("x-request-id")
        .expect("API responses should carry x-request-id");
    assert!(
        uuid::Uuid::parse_str(id).is_ok(),
        "the correlation ID should be a UUID: {id}"
    );

    let probe = server.get("/health").send().await?.ok();
    assert!(
        probe.header("x-request-id").is_none(),
        "probe paths should not carry a correlation ID"
    );

    let media = server
        .get(format!(
            "{}/{}",
            server.ctx.cfg().storage.media_url_prefix,
            uuid::Uuid::nil()
        ))
        .send()
        .await?
        .expect(StatusCode::NOT_FOUND);
    assert!(
        media.header("x-request-id").is_some(),
        "the media download route carries a correlation ID"
    );

    Ok(())
}

#[tokio::test]
async fn request_id_is_reachable_from_the_response() -> Result<()> {
    let server = TestServer::start().await?;

    let res = server.get("/api/does-not-exist").send().await?;
    let header = res.header("x-request-id").map(str::to_owned);
    let stored = res.extension::<RequestId>().map(|id| id.0.clone());

    assert!(
        header.is_some(),
        "the response header should carry the correlation ID"
    );
    assert_eq!(
        header, stored,
        "the correlation ID in the header and in the response extension must match, otherwise error logs cannot be correlated with responses"
    );

    Ok(())
}

#[tokio::test]
async fn the_error_sample_endpoint_serves_its_body_with_200() -> Result<()> {
    let server = TestServer::start().await?;

    let res = server.get("/api/error").send().await?.ok();
    let body = res.json();

    assert_eq!(
        body["code"],
        serde_json::to_value(ErrorCode::InputInvalidFields)?,
        "the sample must carry the wire code the mapping produces"
    );
    assert_eq!(
        body["message"],
        ErrorCode::InputInvalidFields.message(),
        "the sample must carry the message that code stands for"
    );
    assert!(
        body.get("errors").is_none(),
        "the sample failure has no field errors: {}",
        res.text()
    );
    assert!(
        res.header("x-request-id").is_some(),
        "the /api subtree carries the correlation ID"
    );
    assert!(
        res.header("www-authenticate").is_none(),
        "a public sample carries no challenge"
    );

    Ok(())
}
