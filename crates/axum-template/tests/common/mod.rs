//! 集成测试共用设施。
#![allow(dead_code, unused_imports)] // 每个测试二进制各编译一份，未必用到全部工具

mod http;
mod scratch;
mod server;
mod ws;

pub use http::{TestRequest, TestResponse};
pub use scratch::Scratch;
pub use server::{Account, Session, TestServer, in_secs, login_body};
pub use ws::{Socket, close_frame, next_frame, send, wait_for};

/// 文档里不需要凭据的路径：`route_guard_test` 按它跳过 401 断言，`openapi_test` 按它要求其余
/// operation 声明凭证。一条路由变公开时只改这里。
pub const PUBLIC_PATHS: &[&str] = &[
    "/api/error",
    "/api/auth/login",
    "/api/auth/jwt/login",
    "/api/auth/jwt/refresh",
];
