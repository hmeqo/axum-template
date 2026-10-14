//! 活连接登记表：谁在线、以及按用户/按会话吊销。
//!
//! 只负责生命周期，不负责投递——消息一律走 [`super::Hub`]，这里不发任何东西。

use std::{
    collections::HashMap,
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
};

use tokio::sync::watch;

use crate::domain::vo::{SessionId, UserId};

/// 只用于日志与去重。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnectionId(pub u64);

#[derive(Debug)]
struct Connection {
    /// 建连用的会话；`None` = 该连接不随任何一条会话吊销（例如 bearer 令牌建的长连接）。
    session: Option<SessionId>,
    revoke: watch::Sender<bool>,
}

#[derive(Debug, Default)]
pub struct Sockets {
    by_user: Mutex<HashMap<UserId, HashMap<ConnectionId, Connection>>>,
    next_id: AtomicU64,
}

impl Sockets {
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记一条连接；返回的吊销信号变 `true` 时，该连接必须关闭且不再重连。
    pub fn register(
        &self,
        user_id: UserId,
        session: Option<SessionId>,
    ) -> (ConnectionId, watch::Receiver<bool>) {
        let id = ConnectionId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let (revoke, receiver) = watch::channel(false);
        self.lock()
            .entry(user_id)
            .or_default()
            .insert(id, Connection { session, revoke });
        (id, receiver)
    }

    /// 连接结束时注销；重复调用无副作用。
    pub fn unregister(&self, user_id: UserId, id: ConnectionId) {
        let mut by_user = self.lock();
        let Some(connections) = by_user.get_mut(&user_id) else {
            return;
        };

        connections.remove(&id);
        if connections.is_empty() {
            by_user.remove(&user_id);
        }
    }

    /// 吊销该用户的全部连接，返回吊销条数。
    pub fn revoke_user(&self, user_id: UserId) -> usize {
        let mut by_user = self.lock();
        let Some(connections) = by_user.remove(&user_id) else {
            return 0;
        };

        let revoked = connections.len();
        for connection in connections.into_values() {
            let _ = connection.revoke.send(true);
        }
        revoked
    }

    /// 吊销该用户除 `keep` 那条会话之外的全部连接，返回吊销条数；没有会话归属的连接不动。
    pub fn revoke_user_except(&self, user_id: UserId, keep: SessionId) -> usize {
        let mut by_user = self.lock();
        let Some(connections) = by_user.get_mut(&user_id) else {
            return 0;
        };

        let mut revoked = 0;
        connections.retain(|_, connection| match connection.session {
            Some(session) if session != keep => {
                let _ = connection.revoke.send(true);
                revoked += 1;
                false
            }
            _ => true,
        });
        if connections.is_empty() {
            by_user.remove(&user_id);
        }
        revoked
    }

    /// 只吊销来自某条会话的连接，返回吊销条数。
    pub fn revoke_session(&self, session_id: SessionId) -> usize {
        let mut by_user = self.lock();
        let mut revoked = 0;
        by_user.retain(|_, connections| {
            connections.retain(|_, connection| {
                if connection.session == Some(session_id) {
                    let _ = connection.revoke.send(true);
                    revoked += 1;
                    false
                } else {
                    true
                }
            });
            !connections.is_empty()
        });
        revoked
    }

    pub fn online(&self, user_id: UserId) -> usize {
        self.lock().get(&user_id).map_or(0, HashMap::len)
    }

    pub fn connections(&self) -> usize {
        self.lock().values().map(HashMap::len).sum()
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<UserId, HashMap<ConnectionId, Connection>>> {
        self.by_user
            .lock()
            .expect("no code panics while holding the sockets lock")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(id: i64) -> UserId {
        UserId(id)
    }

    fn session(id: i64) -> SessionId {
        SessionId(id)
    }

    #[test]
    fn registering_and_unregistering_moves_the_counts() {
        let sockets = Sockets::new();
        let alice = user(1);

        let (id, revoked) = sockets.register(alice, Some(session(7)));
        assert_eq!(sockets.online(alice), 1);
        assert_eq!(sockets.connections(), 1);
        assert!(!*revoked.borrow());

        sockets.unregister(alice, id);
        sockets.unregister(alice, id);
        assert_eq!(sockets.online(alice), 0);
        assert_eq!(sockets.connections(), 0);
    }

    #[test]
    fn revoking_a_user_fires_every_signal_once() {
        let sockets = Sockets::new();
        let alice = user(1);

        let (_, first) = sockets.register(alice, None);
        let (_, second) = sockets.register(alice, None);
        let (_, other) = sockets.register(user(2), None);

        assert_eq!(sockets.revoke_user(alice), 2);
        assert!(*first.borrow());
        assert!(*second.borrow());
        assert!(!*other.borrow(), "another user's connection is untouched");
        assert_eq!(sockets.revoke_user(alice), 0, "the entry is gone");
        assert_eq!(sockets.connections(), 1);
    }

    #[test]
    fn revoking_a_session_only_fires_that_session() {
        let sockets = Sockets::new();
        let alice = user(1);

        let (_, desktop) = sockets.register(alice, Some(session(7)));
        let (_, phone) = sockets.register(alice, Some(session(8)));
        let (_, anonymous) = sockets.register(alice, None);

        assert_eq!(sockets.revoke_session(session(7)), 1);
        assert!(*desktop.borrow());
        assert!(!*phone.borrow());
        assert!(
            !*anonymous.borrow(),
            "a session-less connection is untouched"
        );
        assert_eq!(sockets.online(alice), 2);
        assert_eq!(sockets.revoke_session(session(7)), 0);
    }

    #[test]
    fn revoking_a_user_but_one_leaves_the_session_less_alone() {
        let sockets = Sockets::new();
        let alice = user(1);

        let (_, kept) = sockets.register(alice, Some(session(7)));
        let (_, dropped) = sockets.register(alice, Some(session(8)));
        let (_, anonymous) = sockets.register(alice, None);

        assert_eq!(sockets.revoke_user_except(alice, session(7)), 1);
        assert!(!*kept.borrow(), "the kept session stays connected");
        assert!(*dropped.borrow());
        assert!(
            !*anonymous.borrow(),
            "a session-less connection is untouched"
        );
        assert_eq!(sockets.online(alice), 2);
        assert_eq!(sockets.revoke_user_except(alice, session(7)), 0);
    }
}
