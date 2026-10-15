//! 接口文档后处理：自述、标签与 operationId 前缀。

use utoipa::{
    PartialSchema, ToSchema,
    openapi::{
        Info, OpenApi,
        path::Operation,
        security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme},
    },
};
use utoipa_axum::router::OpenApiRouter;

use crate::{
    app::{AppContext, http::error::ErrorResp},
    error::ErrorCode,
};

/// [`register_security_schemes`] 注册的会话 cookie 方案名；`#[utoipa::path(security(…))]` 按名引用。
pub const SECURITY_SESSION_COOKIE: &str = "sessionCookie";
/// [`register_security_schemes`] 注册的 bearer 方案名；同上。
pub const SECURITY_BEARER: &str = "bearer";

pub trait OpenApiRouterExt {
    fn with_tags<T: Into<String>>(self, tags: impl IntoIterator<Item = T>) -> Self;
}

impl OpenApiRouterExt for OpenApiRouter<AppContext> {
    fn with_tags<T: Into<String>>(mut self, tags: impl IntoIterator<Item = T>) -> Self {
        set_tags(
            self.get_openapi_mut(),
            tags.into_iter().map(Into::into).collect(),
        );
        self
    }
}

pub fn set_tags(openapi: &mut OpenApi, tags: Vec<String>) {
    for_each_operation(openapi, |operation| {
        operation
            .tags
            .get_or_insert_with(Vec::new)
            .extend(tags.clone());
    });
}

pub fn describe_service(openapi: &mut OpenApi) {
    openapi.info = Info::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
}

/// [`ErrorResponses`](crate::app::http::error::ErrorResponses) 走 `IntoResponses`，utoipa 不为它
/// 收集组件；`ErrorResp::code` 再引用 `ErrorCode`，两个都要注册。
pub fn register_error_schemas(openapi: &mut OpenApi) {
    let schemas = &mut openapi.components.get_or_insert_default().schemas;

    for (name, schema) in [
        (ErrorResp::name(), ErrorResp::schema()),
        (ErrorCode::name(), ErrorCode::schema()),
    ] {
        schemas.insert(name.into_owned(), schema);
    }
}

/// 注册两个方案的组件：会话 cookie 的名字取自配置，bearer 是 HS256 访问令牌。
///
/// operation 侧用 [`SECURITY_SESSION_COOKIE`] / [`SECURITY_BEARER`] 引用；名字对不上就没有定义，
/// `tests/openapi_test.rs` 会拦下来。
pub fn register_security_schemes(openapi: &mut OpenApi, cookie_name: &str) {
    let components = openapi.components.get_or_insert_default();

    components.add_security_scheme(
        SECURITY_SESSION_COOKIE,
        SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new(cookie_name))),
    );
    components.add_security_scheme(
        SECURITY_BEARER,
        SecurityScheme::Http(
            HttpBuilder::new()
                .scheme(HttpAuthScheme::Bearer)
                .bearer_format("JWT")
                .build(),
        ),
    );
}

/// 给每个 operation id 加上分组前缀：`login` → `jwt__login`；消费方按第一个 `__` 还原。
///
/// 分组取自首个标签，因此必须在 [`set_tags`] 之后调用；无标签的 operation 保持原样。
pub fn prefix_operation_ids(openapi: &mut OpenApi) {
    for_each_operation(openapi, |operation| {
        let Some(group) = operation
            .tags
            .as_ref()
            .and_then(|tags| tags.first())
            .cloned()
        else {
            return;
        };
        let Some(id) = operation.operation_id.take() else {
            return;
        };
        operation.operation_id = Some(format!("{group}__{id}"));
    });
}

/// 覆盖 get/post/put/delete/patch。
fn for_each_operation(openapi: &mut OpenApi, mut f: impl FnMut(&mut Operation)) {
    for path_item in openapi.paths.paths.values_mut() {
        for operation in [
            &mut path_item.get,
            &mut path_item.post,
            &mut path_item.put,
            &mut path_item.delete,
            &mut path_item.patch,
        ]
        .into_iter()
        .flatten()
        {
            f(operation);
        }
    }
}
