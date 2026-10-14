//! ConnectRPC：`POST /api/rpc/{package}.{Service}/{Method}`，一元调用走普通 HTTP。
//!
//! schema 由构建期生成的服务定义与 gRPC 反射发布，没有手写的文档文件。

use anyhow::Result;
use axum::http::StatusCode;
use axum_template::{
    app::rpc::{
        SCHEMA_PATH,
        proto::template::v1::{ErrorDetail, TemplateServiceClient},
    },
    domain::vo::Perm,
};
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD as BASE64};
use buffa::Message as _;
use buffa_types::google::protobuf::Empty;
use connectrpc::client::{ClientConfig, Http2Connection, HttpClient};
use connectrpc_reflection::{
    ServerReflectionClient,
    wire::v1::{ServerReflectionRequest, ServerReflectionResponse, server_reflection_request},
};
use serde_json::json;

mod common;

use common::TestServer;

const SERVICE: &str = "template.v1.TemplateService";
const ME: &str = "/api/rpc/template.v1.TemplateService/Me";
const LIST_USERS: &str = "/api/rpc/template.v1.TemplateService/ListUsers";

#[tokio::test]
async fn unauthenticated_calls_are_rejected_with_a_challenge() -> Result<()> {
    let server = TestServer::start().await?;

    let res = server.post(ME).json(&json!({})).send().await?;

    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(res.json()["code"], "unauthenticated");
    assert_eq!(
        res.header("www-authenticate"),
        Some("Bearer realm=\"api\""),
        "受 `Bearer` 的组在 401 上要给出挑战: {}",
        res.text()
    );

    Ok(())
}

#[tokio::test]
async fn me_answers_for_either_credential() -> Result<()> {
    let server = TestServer::start().await?;
    let session = server.login("rpc_me", Some("admin")).await?;
    let account = server.account("rpc_me_token", Some("admin")).await?;
    let token = server.access_token(&account).await?;

    let by_cookie = server
        .post(ME)
        .cookie(&session)
        .json(&json!({}))
        .send()
        .await?
        .ok()
        .json();
    assert_eq!(by_cookie["user"]["username"], session.username());

    let by_token = server
        .post(ME)
        .bearer(&token)
        .json(&json!({}))
        .send()
        .await?
        .ok()
        .json();
    assert_eq!(by_token["user"]["username"], account.username);

    Ok(())
}

/// 权限门与 REST 的 `Require<UserRead>` 是同一套：`user:write` 不够。
#[tokio::test]
async fn a_method_without_the_perm_is_denied() -> Result<()> {
    let server = TestServer::start().await?;
    let role = server
        .ctx
        .srv()
        .role
        .create("rpc-writer".to_owned(), None, &[Perm::UserWrite])
        .await?;
    let account = server.account("rpc_writer", None).await?;
    server
        .ctx
        .srv()
        .role
        .assign_to_user(account.id, role.id)
        .await?;
    let session = server.open_session(account).await?;

    let res = server
        .post(LIST_USERS)
        .cookie(&session)
        .json(&json!({"page": 1, "perPage": 10}))
        .send()
        .await?;

    assert_eq!(res.status, StatusCode::FORBIDDEN);
    assert_eq!(res.json()["code"], "permission_denied");

    Ok(())
}

#[tokio::test]
async fn list_users_paginates_and_filters() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server.login("rpc_list", Some("admin")).await?;
    let target = server.account("rpc_target", None).await?;

    let page = server
        .post(LIST_USERS)
        .cookie(&admin)
        .json(&json!({"page": 1, "perPage": 1}))
        .send()
        .await?
        .ok()
        .json();
    assert_eq!(page["users"].as_array().map(Vec::len), Some(1));
    assert_eq!(page["page"], "1");
    assert_eq!(page["perPage"], "1");
    let total: u64 = page["total"].as_str().unwrap_or_default().parse()?;
    assert!(total >= 2, "总数应涵盖已建账号: {page}");
    assert!(page["users"][0]["createdAt"].is_string());

    let filtered = server
        .post(LIST_USERS)
        .cookie(&admin)
        .json(&json!({"page": 1, "perPage": 10, "username": "RPC_TARGET"}))
        .send()
        .await?
        .ok()
        .json();
    let users = filtered["users"].as_array().cloned().unwrap_or_default();
    assert_eq!(users.len(), 1, "过滤词不区分大小写: {users:?}");
    assert_eq!(users[0]["username"], target.username);

    Ok(())
}

/// 校验失败带业务码：Connect 的 detail 里是与 REST 同形的 `ErrorDetail`。
#[tokio::test]
async fn a_rejected_page_carries_the_field_code_as_a_detail() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server.login("rpc_detail", Some("admin")).await?;

    let res = server
        .post(LIST_USERS)
        .cookie(&admin)
        .json(&json!({"page": 1, "perPage": 1000}))
        .send()
        .await?;

    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    let body = res.json();
    assert_eq!(body["code"], "invalid_argument");
    assert_eq!(
        body["message"], "Validation failed",
        "与 REST 同一份文案: {body}"
    );

    let detail = body["details"][0].clone();
    assert_eq!(detail["type"], "template.v1.ErrorDetail");
    let raw = BASE64.decode(detail["value"].as_str().expect("base64 detail"))?;
    let decoded = ErrorDetail::decode_from_slice(&raw)?;
    assert_eq!(decoded.code, "input.invalid_fields");
    assert!(
        decoded.errors.keys().any(|field| field == "per_page"),
        "字段级错误应带页码字段: {decoded:?}"
    );

    Ok(())
}

/// 生成的客户端与生成的服务对同一份 wire 契约：直接对着真监听地址调一次。
#[tokio::test]
async fn the_generated_client_talks_to_the_server() -> Result<()> {
    let server = TestServer::start().await?;
    let account = server.account("rpc_client", Some("admin")).await?;
    let username = account.username.clone();
    let token = server.access_token(&account).await?;

    let config = ClientConfig::new(format!("http://{}/api/rpc", server.addr).parse()?)
        .with_default_header("authorization", format!("Bearer {token}"));
    let client = TemplateServiceClient::new(HttpClient::plaintext(), config);

    let response = client.me(Empty::default()).await?;
    let state = response.into_owned();

    assert_eq!(state.user.username, username);
    assert!(
        state.permissions.iter().any(|perm| perm == "user:*"),
        "admin 的权限应原样出现在身份里: {:?}",
        state.permissions
    );

    Ok(())
}

/// schema 面与其它 `/api-docs/*` 入口一样公开，且真的在服务：走 h2c 取一次服务列表。
#[tokio::test]
async fn the_schema_endpoint_serves_reflection() -> Result<()> {
    let server = TestServer::start().await?;
    let uri: axum::http::Uri = format!("http://{}{}", server.addr, SCHEMA_PATH).parse()?;
    let conn = Http2Connection::connect_plaintext(uri.clone()).await?;
    let client = ServerReflectionClient::new(conn.shared(1), ClientConfig::new(uri));

    let mut stream = client.server_reflection_info().await?;
    stream
        .send(ServerReflectionRequest {
            message_request: Some(server_reflection_request::MessageRequest::ListServices(
                "*".to_owned(),
            )),
            ..Default::default()
        })
        .await?;
    let response = stream
        .message::<ServerReflectionResponse>()
        .await?
        .expect("reflection answers the first request");
    let json = serde_json::to_string(&response.to_owned_message())?;
    assert!(json.contains(SERVICE), "反射应列出 {SERVICE}: {json}");
    stream.close_send();

    Ok(())
}

/// 方法表之外的名字不派发：未知方法按 Connect 的约定回 `unimplemented`。
#[tokio::test]
async fn unknown_methods_are_not_ok() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server.login("rpc_unknown", Some("admin")).await?;

    let res = server
        .post("/api/rpc/template.v1.TemplateService/Nope")
        .cookie(&admin)
        .json(&json!({}))
        .send()
        .await?;

    assert_ne!(res.status, StatusCode::OK, "{}", res.text());
    assert_eq!(res.json()["code"], "unimplemented");

    Ok(())
}

/// 未声明幂等的方法不接受 GET（Connect 只允许幂等方法走 GET）。
#[tokio::test]
async fn a_non_post_method_has_no_route() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server.login("rpc_get", Some("admin")).await?;

    let res = server
        .request(axum::http::Method::GET, ME)
        .cookie(&admin)
        .send()
        .await?;

    assert_eq!(res.status, StatusCode::METHOD_NOT_ALLOWED);

    Ok(())
}
