//! 接口文档后处理：自述、标签与 operationId 前缀。

use utoipa::{
    PartialSchema, ToSchema,
    openapi::{Info, OpenApi, path::Operation},
};
use utoipa_axum::router::OpenApiRouter;

use crate::{
    app::{AppContext, http::error::ErrorResp},
    error::ErrorCode,
};

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
