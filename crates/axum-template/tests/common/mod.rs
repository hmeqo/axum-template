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
