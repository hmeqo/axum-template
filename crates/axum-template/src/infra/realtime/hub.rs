//! 按 topic 的进程内广播：一个 topic 一条通道，没有订阅者的 topic 不占内存。
//!
//! 约定：`Recv::Lagged(n)` 表示旧事件已丢，订阅者必须回源补拉，不能静默跳过；
//! `Recv::Closed` 只在 `Hub` 被丢弃时出现（订阅者不影响其他订阅者）。

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, Weak},
};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// 订阅范围的名字。约定：会话 `conv:{id}`、个人 `user:{id}`、面级固定名（如 `lobby`、`collaboration`）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Topic(String);

impl Topic {
    pub fn new(topic: impl Into<String>) -> Self {
        Self(topic.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Topic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recv<E> {
    Event(E),
    /// 落后 `n` 条：旧事件已从通道里挤掉。
    Lagged(u64),
    Closed,
}

#[derive(Debug)]
struct HubInner<E> {
    capacity: usize,
    topics: Mutex<HashMap<Topic, broadcast::Sender<E>>>,
}

pub struct Hub<E> {
    inner: Arc<HubInner<E>>,
}

impl<E> Clone for Hub<E> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<E> std::fmt::Debug for Hub<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hub")
            .field("capacity", &self.inner.capacity)
            .field("topics", &self.topics())
            .finish_non_exhaustive()
    }
}

impl<E> Hub<E> {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(HubInner {
                capacity,
                topics: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// 已开通的 topic 数：没有订阅者的 topic 不在表里。
    pub fn topics(&self) -> usize {
        self.inner.lock().len()
    }

    pub fn subscribers(&self, topic: &Topic) -> usize {
        self.inner
            .lock()
            .get(topic)
            .map_or(0, broadcast::Sender::receiver_count)
    }
}

impl<E: Clone + Send + 'static> Hub<E> {
    /// 订阅一个 topic；该 topic 尚未开通就现开通。
    pub fn subscribe(&self, topic: &Topic) -> Subscription<E> {
        let mut topics = self.inner.lock();
        let events = topics
            .entry(topic.clone())
            .or_insert_with(|| broadcast::channel(self.inner.capacity).0)
            .subscribe();

        Subscription {
            topic: topic.clone(),
            events,
            hub: Arc::downgrade(&self.inner),
        }
    }

    /// 投递一个事件，返回收到的订阅者数；无人订阅时返回 0，且不为此开通 topic。
    pub fn publish(&self, topic: &Topic, event: E) -> usize {
        self.inner
            .lock()
            .get(topic)
            .and_then(|sender| sender.send(event).ok())
            .unwrap_or(0)
    }
}

impl<E> HubInner<E> {
    fn lock(&self) -> MutexGuard<'_, HashMap<Topic, broadcast::Sender<E>>> {
        self.topics
            .lock()
            .expect("no code panics while holding the hub lock")
    }
}

/// 一个 topic 的订阅；`Drop` 时若自己是最后一个订阅者，就把该 topic 从 [`Hub`] 里摘掉。
pub struct Subscription<E> {
    topic: Topic,
    events: broadcast::Receiver<E>,
    hub: Weak<HubInner<E>>,
}

impl<E> Subscription<E> {
    pub fn topic(&self) -> &Topic {
        &self.topic
    }
}

impl<E: Clone + Send + 'static> Subscription<E> {
    pub async fn recv(&mut self) -> Recv<E> {
        match self.events.recv().await {
            Ok(event) => Recv::Event(event),
            Err(broadcast::error::RecvError::Lagged(skipped)) => Recv::Lagged(skipped),
            Err(broadcast::error::RecvError::Closed) => Recv::Closed,
        }
    }
}

impl<E> Drop for Subscription<E> {
    fn drop(&mut self) {
        // `Hub` 已销毁时整张表随之释放，没有什么可摘的。
        let Some(hub) = self.hub.upgrade() else {
            return;
        };

        let mut topics = hub.lock();
        // 自己的接收者此刻还没析构，所以"最后一人"是 1。
        if let Some(sender) = topics.get(&self.topic)
            && sender.receiver_count() <= 1
        {
            topics.remove(&self.topic);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAPACITY: usize = 4;

    #[test]
    fn an_unsubscribed_topic_is_never_created() {
        let hub = Hub::<&str>::new(CAPACITY);
        let lobby = Topic::new("lobby");

        assert_eq!(hub.publish(&lobby, "nobody"), 0);
        assert_eq!(hub.topics(), 0);
        assert_eq!(hub.subscribers(&lobby), 0);
    }

    #[tokio::test]
    async fn topics_are_isolated_and_the_last_subscriber_reaps_them() {
        let hub = Hub::new(CAPACITY);
        let lobby = Topic::new("lobby");
        let room = Topic::new("conv:1");

        let mut in_lobby = hub.subscribe(&lobby);
        let mut in_room = hub.subscribe(&room);
        assert_eq!(hub.topics(), 2);
        assert_eq!(hub.subscribers(&lobby), 1);
        assert_eq!(hub.publish(&lobby, "lobby only"), 1);

        assert_eq!(in_lobby.recv().await, Recv::Event("lobby only"));
        assert_eq!(in_lobby.topic(), &lobby);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), in_room.recv())
                .await
                .is_err(),
            "another topic must not see this event"
        );

        drop(in_lobby);
        assert_eq!(hub.topics(), 1, "only `conv:1` is left");
        assert_eq!(hub.subscribers(&lobby), 0);

        drop(in_room);
        assert_eq!(hub.topics(), 0);
    }

    #[tokio::test]
    async fn a_lagged_subscriber_is_told_how_many_it_missed() {
        let hub = Hub::new(1);
        let topic = Topic::new("lobby");
        let mut events = hub.subscribe(&topic);

        hub.publish(&topic, "one");
        hub.publish(&topic, "two");
        hub.publish(&topic, "three");

        assert_eq!(events.recv().await, Recv::Lagged(2));
        assert_eq!(events.recv().await, Recv::Event("three"));
    }

    #[tokio::test]
    async fn a_dropped_hub_closes_its_subscriptions() {
        let hub = Hub::<&str>::new(CAPACITY);
        let mut events = hub.subscribe(&Topic::new("lobby"));

        drop(hub);

        assert_eq!(events.recv().await, Recv::Closed);
        // 订阅者后于 `Hub` 销毁也不能出错。
        drop(events);
    }
}
