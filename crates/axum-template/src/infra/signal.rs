//! 停止信号：`watch::Receiver<bool>` 在长循环里的唯一用法。

use tokio::sync::watch;

/// 等到信号变 `true`（发送端被丢弃也算）后返回；长循环把它当 `select!` 的一臂。
pub async fn draining(stopping: &mut watch::Receiver<bool>) {
    // 必须包一层：`wait_for` 的 `Ref` 非 `Send`，直接作为 `select!` 分支的值会让整个循环失去 `Send`；
    // 也不能改用 `changed()`——在信号已经置位之后才订阅的接收者等不到它。
    let _ = stopping.wait_for(|stopping| *stopping).await;
}
