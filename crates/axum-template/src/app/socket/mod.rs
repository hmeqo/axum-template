//! 长连接面清单：`realtime`（`/ws`，合进主路由，升级时只认会话 cookie）与 `webtransport`
//! （独立 UDP 监听，`webtransport.enabled` 时由 `startup` 拉起；CONNECT 头里的凭证走同一套策略，
//! 被拒时只能回 403）。
//!
//! `/ws` 用 `protocol` 的帧契约；两条面共用 `infra::realtime` 的 `Hub`（一条 `AppEvent` 总线，各自
//! 的 topic）与 `Sockets` 登记（吊销即关闭），WebTransport 另有自己的帧格式与在线表。

pub mod protocol;
pub mod pump;
pub mod realtime;
#[cfg(feature = "webtransport")]
pub mod webtransport;

/// `/ws` 入站文本帧上限。
pub(crate) const MAX_TEXT_FRAME_BYTES: usize = 1024 * 1024;

/// 服务端 graceful drain：客户端可以重连到别的实例。
pub(crate) const CLOSE_DRAINING: u16 = 1001;

/// 会话被吊销：客户端不应重连。
pub(crate) const CLOSE_REVOKED: u16 = 1000;
