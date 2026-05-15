use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};

use crate::app::AppState;

pub async fn ws_handler(State(_state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(text) = msg {
            let reply = format!("echo: {}", text);
            if let Err(e) = socket.send(Message::text(reply)).await {
                tracing::error!("Failed to send WebSocket message: {}", e);
                break;
            }
        }
    }
}
