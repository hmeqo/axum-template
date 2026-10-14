//! 把一个 topic 订阅泵进连接的出向队列。

use serde::Serialize;
use tokio::sync::{mpsc, watch};

use crate::{
    app::socket::protocol::ServerFrame,
    infra::{
        realtime::{Hub, Recv, Subscription},
        signal::draining,
    },
};

/// 把一个订阅泵进连接的出向队列，直到被停止、连接关闭或通道关闭。
pub fn spawn_pump<E, F>(
    mut subscription: Subscription<E>,
    hub: Hub<E>,
    outbound: mpsc::Sender<ServerFrame<E>>,
    mut stop: watch::Receiver<bool>,
    membership: F,
) -> tokio::task::JoinHandle<()>
where
    E: Clone + Send + Serialize + 'static,
    F: Fn(usize) -> E + Send + 'static,
{
    tokio::spawn(async move {
        let topic = subscription.topic().clone();
        // 成员事件与业务事件同通道，且在订阅建立后才补发：别的连接的广播可能排在它前面，
        // 「先收到成员事件再看业务事件」不是协议保证，计数只作参考。
        hub.publish(&topic, membership(hub.subscribers(&topic)));

        loop {
            let frame = tokio::select! {
                _ = draining(&mut stop) => break,
                _ = outbound.closed() => break,
                received = subscription.recv() => match received {
                    Recv::Event(event) => ServerFrame::Event {
                        topic: topic.clone(),
                        event,
                    },
                    // 旧事件已被挤掉，客户端只能回源补拉。
                    Recv::Lagged(skipped) => {
                        tracing::warn!(topic = %topic, skipped, "realtime subscriber lagged");
                        ServerFrame::resync(topic.clone())
                    }
                    Recv::Closed => break,
                },
            };

            // 慢客户端在这里回压：出向队列满了就不再拉事件，`Hub` 侧随后报 `Lagged`。
            if outbound.send(frame).await.is_err() {
                break;
            }
        }

        drop(subscription);
        hub.publish(&topic, membership(hub.subscribers(&topic)));
    })
}
