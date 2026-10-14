//! WebSocket 收发：连接（可带会话）、发帧、按条件等帧。

use anyhow::{Context, Result};
use axum::http::HeaderValue;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};

use super::Session;

pub type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// 超时信息里带已收到的帧。
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const MAX_FRAMES: usize = 16;

/// 连某个 WS 路径；`session` 为 `None` 表示不带会话（升级应被拒）。
pub async fn connect_path(
    addr: std::net::SocketAddr,
    path: &str,
    session: Option<&Session>,
) -> Result<Socket> {
    let mut request = format!("ws://{addr}{path}").into_client_request()?;
    if let Some(session) = session {
        request
            .headers_mut()
            .insert("cookie", HeaderValue::from_str(&session.cookie)?);
    }

    Ok(connect_async(request).await?.0)
}

pub async fn wait_for(ws: &mut Socket, want: impl Fn(&Value) -> bool) -> Result<Value> {
    let mut seen = Vec::new();
    for _ in 0..MAX_FRAMES {
        let frame = next_frame(ws).await?;
        if want(&frame) {
            return Ok(frame);
        }
        seen.push(frame);
    }
    anyhow::bail!("no matching frame within {MAX_FRAMES} frames; frames received: {seen:?}")
}

pub async fn send(ws: &mut Socket, frame: Value) -> Result<()> {
    ws.send(Message::Text(frame.to_string().into())).await?;
    Ok(())
}

pub async fn next_frame(ws: &mut Socket) -> Result<Value> {
    let message = tokio::time::timeout(TIMEOUT, ws.next())
        .await
        .with_context(|| format!("no frame received within {TIMEOUT:?}"))?
        .context("connection closed")??;

    Ok(serde_json::from_str(message.to_text()?)?)
}

/// 读到关闭帧；返回 `(code, reason)`，之前的普通帧会被跳过。
pub async fn close_frame(ws: &mut Socket) -> Result<(u16, String)> {
    for _ in 0..MAX_FRAMES {
        let message = tokio::time::timeout(TIMEOUT, ws.next())
            .await
            .with_context(|| format!("no close frame within {TIMEOUT:?}"))?
            .context("connection closed before sending a close frame")??;

        match message {
            Message::Close(Some(frame)) => {
                return Ok((u16::from(frame.code), frame.reason.to_string()));
            }
            Message::Close(None) => anyhow::bail!("close frame without a code"),
            _ => {}
        }
    }

    anyhow::bail!("no close frame within {MAX_FRAMES} frames")
}
