//! 建立会话时的客户端信息;`ip` 是对端地址或可信转发头解析后的规范地址。

use std::net::IpAddr;

/// 没有 `ConnectInfo`(如未经 make-service 起服务)或没有 `User-Agent` 时为 `None`。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientInfo {
    pub ip: Option<IpAddr>,
    pub user_agent: Option<String>,
}
