//! 实时总线上的事件。

use serde::Serialize;

use crate::app::socket::realtime::LobbyEvent;

/// 一条长连接面能订阅到的东西。`untagged` 让线上的形状就是负载本身（每个负载自带 `type`），
/// 变体名不出现在协议里。
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum AppEvent {
    Lobby(LobbyEvent),
    #[cfg(feature = "webtransport")]
    Presence(crate::app::socket::webtransport::PresenceFrame),
}
