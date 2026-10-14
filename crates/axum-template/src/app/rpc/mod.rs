//! ConnectRPC 面：契约在 `proto/template/v1/template.proto`，构建期生成消息与服务代码。
//!
//! 调用面挂在 `/api/rpc`（路径 `/{package}.{Service}/{Method}`，与 REST 组同一套组认证策略）；
//! schema 随 REST 文档一起公开挂 `/api-docs/rpc`，gRPC 反射据此发布。

mod error;
pub mod service;

pub mod proto {
    connectrpc::include_generated!();
}

use std::sync::Arc;

use axum::Router;
use connectrpc::Router as ConnectRouter;
use connectrpc_reflection::{Reflector, install};
use utoipa_axum::router::OpenApiRouter;

use self::{proto::template::v1::TemplateServiceExt as _, service::RpcService};
use crate::app::{AppContext, identity::JwtOrSession, middleware::auth::auth};

pub const PATH: &str = "/rpc";

/// schema 的挂载前缀，与 `PATH` 分开。
pub const SCHEMA_PATH: &str = "/api-docs/rpc";

/// 构建期导出的描述符集：反射发布的 schema 与生成代码同源。
const DESCRIPTOR_SET: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/template.fds.bin"));

pub fn router(ctx: &AppContext) -> OpenApiRouter<AppContext> {
    let router = Arc::new(RpcService::new(ctx.clone())).register(ConnectRouter::new());

    OpenApiRouter::new().nest_service(
        PATH,
        router.into_axum_router().layer(auth(ctx, JwtOrSession)),
    )
}

/// gRPC 反射（`v1` 与 `v1alpha`）：schema 的发布面。
pub fn schema_router() -> Router<AppContext> {
    Router::new().nest_service(
        SCHEMA_PATH,
        install(ConnectRouter::new(), reflector()).into_axum_router(),
    )
}

fn reflector() -> Reflector {
    Reflector::from_descriptor_set_bytes(DESCRIPTOR_SET)
        .expect("the embedded descriptor set is emitted by this crate's own build script")
}
