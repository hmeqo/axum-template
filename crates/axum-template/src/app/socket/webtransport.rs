//! WebTransport 示例：协作房间——同伴状态走数据报（不可靠、可丢），加入/离开事件走可靠单向流。
//!
//! 分工照标准的需求表：高频可丢的状态用数据报换最小延迟，低频不可丢的事件每条开一条新单向流
//! （可靠有序，且不与其他事件互相阻塞）。房间与 `/ws` 共用同一条 `AppEvent` 总线，靠 topic 分开。
//!
//! 建会话前先定身份：CONNECT 头里的 `Authorization` / `Cookie` 走与 HTTP 面同一套策略，
//! 缺失或无效只回 403——wtransport 没有 401，只有 403/404/429，也没有 `WWW-Authenticate`
//! 的地方可放；浏览器发起的 CONNECT 不带 cookie 也没有 HTTP 认证，凭证只能由客户端在
//! CONNECT 头里自己带上。

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use axum::http::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use wtransport::{
    Connection, Endpoint, Identity, ServerConfig, VarInt,
    endpoint::{IncomingSession, SessionRequest},
    tls::Sha256DigestFmt,
};

use crate::{
    app::{
        AppContext,
        event::AppEvent,
        identity::{JwtOrSession, Resolved, backend::resolve_headers},
        socket::CLOSE_REVOKED,
    },
    config::WebTransportConfig,
    domain::vo::{SessionId, UserId},
    error::{AppError, Result},
    infra::{
        realtime::{Hub, Recv, Subscription, Topic},
        signal::draining,
    },
};

/// 与 HTTP 面同一套凭证。
const AUTH: JwtOrSession = JwtOrSession;

/// 协作房间的 topic。
const TOPIC: &str = "collaboration";

/// 数据报里的同伴状态；丢掉一帧只意味着对方的位置停在上一帧。
#[derive(Debug, Clone, Copy, Deserialize)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Peer {
    id: usize,
    x: f32,
    y: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PresenceFrame {
    Roster { peers: Vec<Peer> },
    Joined { id: usize, x: f32, y: f32 },
    Moved { id: usize, x: f32, y: f32 },
    Leave { id: usize },
}

impl PresenceFrame {
    /// 帧所指的连接。
    fn actor(&self) -> Option<usize> {
        match self {
            Self::Roster { .. } => None,
            Self::Joined { id, .. } | Self::Moved { id, .. } | Self::Leave { id } => Some(*id),
        }
    }
}

/// 进程内在线表与出向广播。
struct Presence {
    peers: Mutex<HashMap<usize, Position>>,
    events: Hub<AppEvent>,
}

fn topic() -> Topic {
    Topic::new(TOPIC)
}

pub async fn serve(ctx: AppContext) -> Result<()> {
    let config = &ctx.cfg().webtransport;
    if !config.enabled {
        return Ok(());
    }

    let endpoint = Endpoint::server(server_config(config).await?).map_err(internal)?;
    tracing::info!("webtransport listening on {}", endpoint.local_addr()?);

    let presence = Arc::new(Presence::new(ctx.events().clone()));
    let mut shutdown = ctx.shutdown();
    loop {
        tokio::select! {
            incoming = endpoint.accept() => {
                let presence = presence.clone();
                let ctx = ctx.clone();
                tokio::spawn(async move {
                    if let Err(e) = session(incoming, &ctx, &presence).await {
                        tracing::debug!(error = %e, "webtransport session ended");
                    }
                });
            }
            _ = draining(&mut shutdown) => {
                tracing::info!("webtransport draining");
                break;
            }
        }
    }

    endpoint.close(VarInt::from_u32(0), b"server shutting down");
    Ok(())
}

/// CONNECT 头是本传输唯一的凭据来源；放行前就把身份定下来，会话 id 用来按会话吊销。
async fn authenticate(
    request: &SessionRequest,
    ctx: &AppContext,
) -> Option<(UserId, Option<SessionId>)> {
    let mut headers = HeaderMap::new();
    for (name, value) in request.headers() {
        match (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            (Ok(name), Ok(value)) => {
                headers.append(name, value);
            }
            _ => tracing::debug!(name, "webtransport header ignored"),
        }
    }

    match resolve_headers(&AUTH, headers, ctx).await {
        Resolved::Authenticated {
            user_id, session, ..
        } => Some((user_id, session)),
        resolved => {
            tracing::debug!(error = %resolved.rejection(), "webtransport credentials rejected");
            None
        }
    }
}

async fn session(incoming: IncomingSession, ctx: &AppContext, presence: &Presence) -> Result<()> {
    let request = incoming.await.map_err(internal)?;
    let Some((user_id, session_id)) = authenticate(&request, ctx).await else {
        request.forbidden().await;
        return Ok(());
    };
    let connection = request.accept().await.map_err(internal)?;
    let id = connection.stable_id();
    // 先订阅再登记：登记之后别人广播的帧都会进这个接收者。
    let mut events = presence.subscribe();
    let (connection_id, mut revoked) = ctx.sockets().register(user_id, session_id);

    let peers = presence.register(id);
    if let Err(e) = write_event(&connection, &PresenceFrame::Roster { peers }).await {
        // 名单可能已被并发加入者取走，必须广播离开。
        presence.leave(id);
        ctx.sockets().unregister(user_id, connection_id);
        return Err(e);
    }
    presence.publish(PresenceFrame::Joined { id, x: 0.0, y: 0.0 });
    tracing::debug!(id, user_id = %user_id, "webtransport peer joined");

    let result = loop {
        tokio::select! {
            datagram = connection.receive_datagram() => match datagram {
                Ok(datagram) => {
                    if let Ok(position) = serde_json::from_slice::<Position>(&datagram) {
                        presence.moved(id, position);
                    }
                }
                Err(e) => break Err(internal(e)),
            },
            frame = events.recv() => match frame {
                Recv::Event(AppEvent::Presence(frame)) => {
                    // 自己的帧不必回给自己。
                    if frame.actor() == Some(id) {
                        continue;
                    }
                    match frame {
                        // 状态可丢：数据报。
                        PresenceFrame::Moved { .. } => {
                            if let Err(e) = connection.send_datagram(encode(&frame)) {
                                tracing::trace!(error = %e, "datagram dropped");
                            }
                        }
                        // 事件不可丢：每条一条新单向流。
                        PresenceFrame::Joined { .. } | PresenceFrame::Leave { .. } => {
                            if let Err(e) = write_event(&connection, &frame).await {
                                break Err(e);
                            }
                        }
                        PresenceFrame::Roster { .. } => {}
                    }
                }
                // 本 topic 只发自己的载荷。
                Recv::Event(AppEvent::Lobby(_)) => {}
                // 位置帧可丢：跳过的旧帧不补发。
                Recv::Lagged(skipped) => {
                    tracing::debug!(skipped, "webtransport subscriber lagged");
                }
                Recv::Closed => break Ok(()),
            },
            _ = draining(&mut revoked) => {
                // 会话被吊销：客户端不应重连，所以先关连接再退出。
                connection.close(VarInt::from_u32(u32::from(CLOSE_REVOKED)), b"session revoked");
                break Ok(());
            },
        }
    };

    presence.leave(id);
    ctx.sockets().unregister(user_id, connection_id);
    result
}

impl Presence {
    fn new(events: Hub<AppEvent>) -> Self {
        Self {
            peers: Mutex::new(HashMap::new()),
            events,
        }
    }

    fn subscribe(&self) -> Subscription<AppEvent> {
        self.events.subscribe(&topic())
    }

    /// 登记本连接并取回名单。
    fn register(&self, id: usize) -> Vec<Peer> {
        let mut peers = self.lock();
        // 登记与快照在同一把锁内：同时加入的两个连接会互相出现在名单里。
        peers.insert(id, Position { x: 0.0, y: 0.0 });
        roster(&peers, id)
    }

    /// 只改状态不广播；广播由 [`Presence::leave`] 负责。
    fn remove(&self, id: usize) {
        self.lock().remove(&id);
    }

    fn leave(&self, id: usize) {
        self.remove(id);
        self.publish(PresenceFrame::Leave { id });
    }

    fn moved(&self, id: usize, position: Position) {
        self.lock().insert(id, position);
        self.publish(PresenceFrame::Moved {
            id,
            x: position.x,
            y: position.y,
        });
    }

    fn publish(&self, frame: PresenceFrame) {
        self.events.publish(&topic(), AppEvent::Presence(frame));
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<usize, Position>> {
        self.peers
            .lock()
            .expect("no code panics while holding the presence lock")
    }
}

/// 名单快照；`except` 是自己，客户端已经知道自己。
fn roster(peers: &HashMap<usize, Position>, except: usize) -> Vec<Peer> {
    peers
        .iter()
        .filter(|(id, _)| **id != except)
        .map(|(id, position)| Peer {
            id: *id,
            x: position.x,
            y: position.y,
        })
        .collect()
}

/// 一条事件一条单向流：可靠有序，且不与其他事件互相阻塞。
async fn write_event(connection: &Connection, frame: &PresenceFrame) -> Result<()> {
    let mut stream = connection
        .open_uni()
        .await
        .map_err(internal)?
        .await
        .map_err(internal)?;
    stream.write_all(&encode(frame)).await.map_err(internal)?;
    stream.finish().await.map_err(internal)
}

fn encode(frame: &PresenceFrame) -> Vec<u8> {
    serde_json::to_vec(frame).expect("serializable")
}

async fn server_config(config: &WebTransportConfig) -> Result<ServerConfig> {
    Ok(ServerConfig::builder()
        .with_bind_default(config.port)
        .with_identity(identity(config).await?)
        .keep_alive_interval(Some(Duration::from_secs(3)))
        .build())
}

async fn identity(config: &WebTransportConfig) -> Result<Identity> {
    if let (Some(cert), Some(key)) = (&config.cert, &config.key) {
        return Identity::load_pemfiles(cert, key)
            .await
            .map_err(|e| AppError::config(format!("webtransport pem: {e}")));
    }

    let identity = Identity::self_signed(["localhost", "127.0.0.1", "::1"])
        .map_err(|e| AppError::config(format!("webtransport self-signed: {e}")))?;
    let digest = identity.certificate_chain().as_slice()[0].hash();
    tracing::warn!(
        hash = %digest.fmt(Sha256DigestFmt::BytesArray),
        "webtransport uses a self-signed certificate; browsers must trust this hash"
    );
    Ok(identity)
}

fn internal(error: impl std::error::Error + Send + Sync + 'static) -> AppError {
    AppError::wrap(error)
}
