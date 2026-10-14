//! 实时基础设施：按 topic 的进程内 fan-out（[`Hub`]）与活连接登记表（[`Sockets`]）。
//!
//! 两者都在本进程内生效：跨进程（多实例、外部 pub/sub）不在这一层，多实例要另接外部总线。

mod hub;
mod sockets;

pub use hub::{Hub, Recv, Subscription, Topic};
pub use sockets::{ConnectionId, Sockets};
