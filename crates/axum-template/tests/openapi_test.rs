//! OpenAPI 契约：operationId 由装配点统一加前缀，枚举取值必须与线上一致。

use anyhow::Result;
use axum_template::{
    domain::vo::{FileStateCode, Perm},
    error::ErrorCode,
};
use serde::Serialize;
use serde_json::Value;
use strum::IntoEnumIterator;

mod common;

use common::TestServer;

fn documented_values(doc: &Value, schema: &str) -> Vec<String> {
    doc["components"]["schemas"][schema]["enum"]
        .as_array()
        .unwrap_or_else(|| panic!("{schema} is not a string enum in the document"))
        .iter()
        .map(|value| value.as_str().expect("enum value is a string").to_owned())
        .collect()
}

fn wire_values<T: Serialize>(items: impl Iterator<Item = T>) -> Vec<String> {
    items
        .map(|item| {
            serde_json::to_value(item)
                .expect("variant is serializable")
                .as_str()
                .expect("variant serializes to a string")
                .to_owned()
        })
        .collect()
}

/// `operationId` 一律由 `create_router` 加上分组前缀：消费方按第一个 `__` 切分。
#[tokio::test]
async fn operation_ids_are_unique_and_carry_their_group_prefix() -> Result<()> {
    let server = TestServer::start().await?;
    let doc = server.openapi().await?;

    let operations: Vec<(&String, &Value)> = doc["paths"]
        .as_object()
        .expect("paths is an object")
        .iter()
        .flat_map(|(path, item)| {
            item.as_object()
                .expect("path item is an object")
                .iter()
                .filter(|(method, _)| {
                    matches!(
                        method.as_str(),
                        "get" | "put" | "post" | "delete" | "options" | "head" | "patch" | "trace"
                    )
                })
                .map(move |operation| (path, operation.1))
        })
        .collect();

    let ids: Vec<String> = operations
        .iter()
        .filter_map(|(_, operation)| operation.get("operationId").and_then(Value::as_str))
        .map(str::to_owned)
        .collect();
    assert!(!ids.is_empty(), "the document should contain operations");

    let unique: std::collections::HashSet<&String> = ids.iter().collect();
    assert_eq!(
        unique.len(),
        ids.len(),
        "operationId must be unique: {ids:?}"
    );

    for (path, operation) in operations {
        let id = operation["operationId"]
            .as_str()
            .expect("every operation carries an operationId");
        let tag = operation["tags"]
            .as_array()
            .and_then(|tags| tags.first())
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{id} ({path}) has no group tag"));

        assert!(
            id.starts_with(&format!("{tag}__")),
            "operationId must be prefixed with its first tag: {id} is not {tag}__…"
        );
    }

    Ok(())
}

#[tokio::test]
async fn error_code_values_match_the_document() -> Result<()> {
    let server = TestServer::start().await?;
    let doc = server.openapi().await?;

    assert_eq!(
        documented_values(&doc, "ErrorCode"),
        wire_values(ErrorCode::iter())
    );
    Ok(())
}

/// `Perm` 的取值由 `perms!` 宏声明，`ToSchema` 读不到宏里插值的 `serde(rename)`。
#[tokio::test]
async fn perm_values_match_the_document() -> Result<()> {
    let server = TestServer::start().await?;
    let doc = server.openapi().await?;

    assert_eq!(documented_values(&doc, "Perm"), wire_values(Perm::iter()));
    Ok(())
}

#[tokio::test]
async fn file_state_values_match_the_document() -> Result<()> {
    let server = TestServer::start().await?;
    let doc = server.openapi().await?;

    assert_eq!(
        documented_values(&doc, "FileStateCode"),
        wire_values(FileStateCode::iter())
    );
    Ok(())
}

/// 错误类型必须能从一条具体状态码的响应到达：只挂 `4XX` / `5XX` 通配响应的生成器会整块丢弃。
#[tokio::test]
async fn the_error_contract_is_reachable_under_a_concrete_status() -> Result<()> {
    let server = TestServer::start().await?;
    let doc = server.openapi().await?;

    let reachable = doc["paths"]
        .as_object()
        .expect("paths is an object")
        .values()
        .flat_map(|item| item.as_object().expect("path item is an object").values())
        .filter_map(|operation| operation["responses"].as_object())
        .any(|responses| {
            responses.iter().any(|(status, response)| {
                !matches!(status.as_str(), "4XX" | "5XX")
                    && response["content"]["application/json"]["schema"]["$ref"].as_str()
                        == Some("#/components/schemas/ErrorResp")
            })
        });

    assert!(
        reachable,
        "no operation publishes ErrorResp under a concrete status; \\
         generators that drop the 4XX/5XX wildcards then emit no error types"
    );

    Ok(())
}

/// 悬空 `$ref` 会让客户端生成器直接失败；`IntoResponses` 不参与组件收集，这里兜住。
#[tokio::test]
async fn every_reference_resolves_to_a_schema() -> Result<()> {
    let server = TestServer::start().await?;
    let doc = server.openapi().await?;

    let schemas = doc["components"]["schemas"]
        .as_object()
        .expect("schemas is an object");

    let mut refs = Vec::new();
    collect_refs(&doc, &mut refs);

    let dangling: Vec<&String> = refs
        .iter()
        .filter(|reference| {
            reference
                .strip_prefix("#/components/schemas/")
                .is_none_or(|name| !schemas.contains_key(name))
        })
        .collect();

    assert!(
        dangling.is_empty(),
        "the document has dangling $ref: {dangling:?}"
    );
    Ok(())
}

fn collect_refs(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                match (key.as_str(), child.as_str()) {
                    ("$ref", Some(reference)) => out.push(reference.to_owned()),
                    _ => collect_refs(child, out),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| collect_refs(item, out)),
        _ => {}
    }
}

/// 每个受保护的 operation 必须声明它接受的凭证，方案名必须在组件里有定义；公开的 operation 反过来
/// 不能声明。清单与 `route_guard_test` 同源，两边不会各说一套。
#[tokio::test]
async fn operations_declare_the_credentials_they_accept() -> Result<()> {
    let server = TestServer::start().await?;
    let doc = server.openapi().await?;

    let schemes: Vec<&String> = doc["components"]["securitySchemes"]
        .as_object()
        .expect("components.securitySchemes is an object")
        .keys()
        .collect();
    assert!(
        !schemes.is_empty(),
        "the document defines no security scheme, so no operation can declare one"
    );

    let mut guarded = 0;
    for (path, item) in doc["paths"].as_object().expect("paths is an object") {
        for (method, operation) in item.as_object().expect("path item is an object") {
            if !matches!(
                method.as_str(),
                "get" | "put" | "post" | "delete" | "options" | "head" | "patch" | "trace"
            ) {
                continue;
            }

            let declared = operation["security"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            if common::PUBLIC_PATHS.contains(&path.as_str()) {
                assert!(
                    declared.is_empty(),
                    "{method} {path} is in PUBLIC_PATHS but declares {declared:?}"
                );
                continue;
            }

            assert!(
                !declared.is_empty(),
                "{method} {path} declares no credential: add `security(…)` to its \
                 #[utoipa::path], or the path to common::PUBLIC_PATHS when it is public"
            );
            for requirement in &declared {
                let names = requirement
                    .as_object()
                    .expect("a security requirement is an object");
                assert!(
                    !names.is_empty(),
                    "{method} {path} declares an empty security requirement"
                );
                for name in names.keys() {
                    assert!(
                        schemes.contains(&name),
                        "{method} {path} requires {name}, which no component defines"
                    );
                }
            }
            guarded += 1;
        }
    }

    assert!(guarded > 0, "no guarded operation was inspected");

    Ok(())
}

/// 发布文案一律英文（`///` 会原样变成文档里的 `summary`／`description`）。
#[tokio::test]
async fn published_documents_contain_no_cjk() -> Result<()> {
    let server = TestServer::start().await?;

    let uri = "/api-docs/openapi.json";
    let doc = server.get(uri).send().await?.ok().json();
    let leaked = cjk(&serde_json::to_string(&doc)?);
    assert!(
        leaked.is_empty(),
        "{uri} publishes non-English text: {leaked:?}"
    );

    // proto 源文件本身是对外契约：反射发布的描述符集不带注释，字段含义只写在这里。
    let leaked = cjk(include_str!("../proto/template/v1/template.proto"));
    assert!(
        leaked.is_empty(),
        "the proto contract publishes non-English text: {leaked:?}"
    );

    Ok(())
}

fn cjk(text: &str) -> String {
    text.chars()
        .filter(|c| matches!(c, '\u{3000}'..='\u{9fff}' | '\u{ff00}'..='\u{ffef}'))
        .collect()
}
