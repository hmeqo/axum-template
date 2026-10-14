//! 实时样例（`/ws`）：用 `Hub` + `Sockets` + 帧契约拼出的一条长连接面。
//!
//! 客户端连上后先发 `{"type":"subscribe","topic":"lobby"}`，之后这条连接就是该 topic 的一个订阅者；
//! 会话被吊销（登出、吊销单条会话、改密时吊销该用户的全部会话）时连接立即以 `CLOSE_REVOKED` 关闭。

use std::collections::HashMap;

use axum::{
    Router,
    extract::{
        State,
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
    routing::get,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, watch};

use crate::{
    app::{
        AppContext,
        event::AppEvent,
        identity::{Auth, SessionAuth, SessionIdentity},
        middleware::auth::auth,
        socket::{
            CLOSE_DRAINING, CLOSE_REVOKED, MAX_TEXT_FRAME_BYTES,
            protocol::{ControlFrame, Inbound, ServerFrame, parse_inbound},
            pump::spawn_pump,
        },
    },
    domain::vo::UserId,
    error::{AppError, ErrorCode},
    infra::{realtime::Topic, signal::draining},
};

/// 所有登录用户共用一个房间。
const LOBBY: &str = "lobby";

const OUTBOUND_CAPACITY: usize = 32;

/// 样例业务事件（服务端 → 客户端）。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LobbyEvent {
    Chat { from: UserId, text: String },
    Online { count: usize },
}

/// 样例业务命令（客户端 → 服务端）。
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LobbyCommand {
    Chat { text: String },
}

fn lobby() -> Topic {
    Topic::new(LOBBY)
}

/// 订阅授权：业务决定谁能看哪个 topic，样例只放行 [`lobby`]。
fn allow_subscribe(topic: &Topic) -> Result<(), AppError> {
    if topic.as_str() == LOBBY {
        return Ok(());
    }

    Err(AppError::business_msg(
        ErrorCode::RealtimeTopicForbidden,
        format!("Cannot subscribe to {topic}"),
    ))
}

/// `GET /ws` + `Upgrade`：升级时的会话 cookie 就是身份（按会话吊销要 `session_id`）。
pub async fn upgrade(
    Auth(auth): Auth<SessionAuth>,
    State(ctx): State<AppContext>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.max_message_size(MAX_TEXT_FRAME_BYTES)
        .max_frame_size(MAX_TEXT_FRAME_BYTES)
        .on_upgrade(move |socket| serve(socket, ctx, auth))
}

async fn serve(mut socket: WebSocket, ctx: AppContext, session: SessionIdentity) {
    let (id, mut revoked) = ctx
        .sockets()
        .register(session.user_id, Some(session.session_id));
    // 队列满了由泵在 `send` 上回压，慢客户端随后收到 `resync`。
    let (outbound, mut frames) = mpsc::channel::<ServerFrame<AppEvent>>(OUTBOUND_CAPACITY);
    let mut shutdown = ctx.shutdown();
    // 每个已订阅 topic 一个停止信号：从表里移除即 drop，泵随之退出。
    let mut subscriptions: HashMap<Topic, watch::Sender<bool>> = HashMap::new();

    loop {
        tokio::select! {
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    handle_frame(&ctx, &session, &outbound, &mut subscriptions, &mut socket, &text)
                        .await;
                }
                // Ping 由协议层自动回 Pong，Close 由协议层回执（下一轮 `recv()` 得到 `None`）；
                // Binary / Pong 本样例不处理。
                Some(Ok(_)) => {}
                Some(Err(e)) => {
                    tracing::warn!(error = %e, "websocket error");
                    break;
                }
                None => break,
            },
            frame = frames.recv() => match frame {
                Some(frame) => {
                    if !send_frame(&mut socket, &frame).await {
                        break;
                    }
                }
                None => break,
            },
            _ = draining(&mut revoked) => {
                let _ = socket.send(close_frame(CLOSE_REVOKED, "session revoked")).await;
                break;
            },
            _ = draining(&mut shutdown) => {
                let _ = socket.send(close_frame(CLOSE_DRAINING, "server shutting down")).await;
                break;
            },
        }
    }

    // 订阅先释放，剩下的人数才是离开后的值。
    subscriptions.clear();
    ctx.sockets().unregister(session.user_id, id);
}

/// 一条上行帧：控制帧就地生效，业务帧发布到总线，坏帧回错误帧且连接继续。
async fn handle_frame(
    ctx: &AppContext,
    session: &SessionIdentity,
    outbound: &mpsc::Sender<ServerFrame<AppEvent>>,
    subscriptions: &mut HashMap<Topic, watch::Sender<bool>>,
    socket: &mut WebSocket,
    raw: &str,
) {
    let inbound = match parse_inbound::<LobbyCommand>(raw) {
        Ok(inbound) => inbound,
        Err(error) => {
            tracing::warn!(error = %error, "bad realtime frame");
            send_frame(socket, &ServerFrame::error(&error)).await;
            return;
        }
    };

    match inbound {
        Inbound::Control(ControlFrame::Subscribe { topic }) => {
            if let Err(error) = allow_subscribe(&topic) {
                send_frame(socket, &ServerFrame::error(&error)).await;
                return;
            }
            if subscriptions.contains_key(&topic) {
                return;
            }

            // 先订阅再起泵：人数与投递都从订阅集合量起；句柄丢弃即 detach，停止靠 `stop`。
            let subscription = ctx.events().subscribe(&topic);
            let (stop, stopping) = watch::channel(false);
            subscriptions.insert(topic, stop);
            drop(spawn_pump(
                subscription,
                ctx.events().clone(),
                outbound.clone(),
                stopping,
                |count| AppEvent::Lobby(LobbyEvent::Online { count }),
            ));
        }
        Inbound::Control(ControlFrame::Unsubscribe { topic }) => {
            subscriptions.remove(&topic);
        }
        Inbound::Control(ControlFrame::Ping) => {
            send_frame(socket, &ServerFrame::pong()).await;
        }
        Inbound::Business(LobbyCommand::Chat { text }) => {
            ctx.events().publish(
                &lobby(),
                AppEvent::Lobby(LobbyEvent::Chat {
                    from: session.user_id,
                    text,
                }),
            );
        }
    }
}

/// 写一帧；连接已断时返回 `false`。
async fn send_frame(socket: &mut WebSocket, frame: &ServerFrame<AppEvent>) -> bool {
    socket
        .send(Message::Text(to_frame(frame).into()))
        .await
        .is_ok()
}

fn to_frame<E: Serialize>(frame: &ServerFrame<E>) -> String {
    serde_json::to_string(frame).expect("serializable")
}

fn close_frame(code: u16, reason: &str) -> Message {
    Message::Close(Some(CloseFrame {
        code,
        reason: reason.into(),
    }))
}

pub fn router(ctx: &AppContext) -> Router<AppContext> {
    Router::new()
        .route("/ws", get(upgrade))
        .route_layer(auth(ctx, SessionAuth))
}
