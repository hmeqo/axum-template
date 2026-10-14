//! Axum + Toasty 参考模板：`app/`（HTTP 边缘与装配）、`domain/`（模型/服务/值对象）、`infra/`（日志、调度、存储、实时）。
//!
//! 分层、约定与「为什么这么做」见 `AGENTS.md`；长文取舍见 `docs/architecture.md`。
#![cfg_attr(not(test), warn(clippy::unwrap_used))]

pub mod app;
pub mod cli;
pub mod config;
pub mod domain;
pub mod error;
pub mod infra;

pub use error::{AppError, Result};
