//! 实时面（`/ws`）：订阅制事件总线（`Hub`）+ 活连接登记表（`Sockets`）。

use std::time::Duration;

use anyhow::Result;
use axum_template::domain::model::DefaultRole;
use serde_json::json;

mod common;

use common::{Socket, TestServer, close_frame, next_frame, send, wait_for};

/// 「不该收到帧」的等待窗口。
const SILENT: Duration = Duration::from_millis(300);

async fn subscribe(ws: &mut Socket, topic: &str) -> Result<()> {
    send(ws, json!({"type": "subscribe", "topic": topic})).await
}

async fn assert_silent(ws: &mut Socket) {
    match tokio::time::timeout(SILENT, next_frame(ws)).await {
        Err(_) => {}
        Ok(Ok(frame)) => panic!("received an unexpected frame: {frame}"),
        Ok(Err(e)) => panic!("connection ended while it should have stayed silent: {e:#}"),
    }
}

#[tokio::test]
async fn lobby_events_reach_every_subscriber() -> Result<()> {
    let server = TestServer::start().await?;
    let alice_session = server.login("lobby_alice", None).await?;
    let bob_session = server.login("lobby_bob", None).await?;

    server.ws_without_session_is_rejected("/ws").await?;

    let mut alice = server.ws("/ws", Some(&alice_session)).await?;
    subscribe(&mut alice, "lobby").await?;
    assert_eq!(
        next_frame(&mut alice).await?,
        json!({"type": "online", "topic": "lobby", "count": 1})
    );

    let mut bob = server.ws("/ws", Some(&bob_session)).await?;
    subscribe(&mut bob, "lobby").await?;
    for socket in [&mut alice, &mut bob] {
        assert_eq!(
            next_frame(socket).await?,
            json!({"type": "online", "topic": "lobby", "count": 2})
        );
    }

    send(&mut bob, json!({"type": "chat", "text": "hello"})).await?;
    for socket in [&mut alice, &mut bob] {
        let chat = wait_for(socket, |frame| frame["type"] == "chat").await?;
        assert_eq!(chat["text"], "hello");
        assert_eq!(
            chat["from"],
            json!(bob_session.user_id()),
            "should carry the sender id: {chat}"
        );
        assert_eq!(chat["topic"], "lobby");
    }

    Ok(())
}

#[tokio::test]
async fn unsubscribing_stops_delivery() -> Result<()> {
    let server = TestServer::start().await?;
    let alice = server.login("unsub_alice", None).await?;
    let bob = server.login("unsub_bob", None).await?;

    let mut alice_ws = server.ws("/ws", Some(&alice)).await?;
    subscribe(&mut alice_ws, "lobby").await?;
    next_frame(&mut alice_ws).await?; // count 1

    let mut bob_ws = server.ws("/ws", Some(&bob)).await?;
    subscribe(&mut bob_ws, "lobby").await?;
    wait_for(&mut alice_ws, |frame| frame["count"] == 2).await?;
    next_frame(&mut bob_ws).await?; // count 2

    send(
        &mut bob_ws,
        json!({"type": "unsubscribe", "topic": "lobby"}),
    )
    .await?;
    wait_for(&mut alice_ws, |frame| frame["count"] == 1).await?;

    send(&mut bob_ws, json!({"type": "chat", "text": "anyone?"})).await?;
    assert_silent(&mut bob_ws).await;
    assert_eq!(
        wait_for(&mut alice_ws, |frame| frame["type"] == "chat").await?["text"],
        "anyone?"
    );

    Ok(())
}

#[tokio::test]
async fn an_unknown_topic_is_rejected() -> Result<()> {
    let server = TestServer::start().await?;
    let outsider = server.login("topic_outsider", None).await?;
    let member = server.login("topic_member", None).await?;

    let mut outsider_ws = server.ws("/ws", Some(&outsider)).await?;
    send(
        &mut outsider_ws,
        json!({"type": "subscribe", "topic": "nope"}),
    )
    .await?;
    let error = next_frame(&mut outsider_ws).await?;
    assert_eq!(error["type"], "error");
    assert_eq!(error["code"], "realtime.topic_forbidden");

    // 被拒的连接也不是该 topic 的订阅者。
    let mut member_ws = server.ws("/ws", Some(&member)).await?;
    subscribe(&mut member_ws, "lobby").await?;
    next_frame(&mut member_ws).await?; // count 1
    send(
        &mut member_ws,
        json!({"type": "chat", "text": "members only"}),
    )
    .await?;
    wait_for(&mut member_ws, |frame| frame["type"] == "chat").await?;
    assert_silent(&mut outsider_ws).await;

    Ok(())
}

#[tokio::test]
async fn an_unknown_frame_keeps_the_connection() -> Result<()> {
    let server = TestServer::start().await?;
    let session = server.login("frame_unknown", None).await?;
    let mut ws = server.ws("/ws", Some(&session)).await?;
    subscribe(&mut ws, "lobby").await?;
    next_frame(&mut ws).await?; // count 1

    send(&mut ws, json!({"type": "nope"})).await?;
    let error = next_frame(&mut ws).await?;
    assert_eq!(error["type"], "error");
    assert_eq!(error["code"], "realtime.unknown_frame");

    send(&mut ws, json!({"type": "chat", "text": "still here"})).await?;
    let chat = wait_for(&mut ws, |frame| frame["type"] == "chat").await?;
    assert_eq!(chat["text"], "still here");

    Ok(())
}

#[tokio::test]
async fn the_app_level_ping_is_answered() -> Result<()> {
    let server = TestServer::start().await?;
    let session = server.login("app_ping", None).await?;
    let mut ws = server.ws("/ws", Some(&session)).await?;

    send(&mut ws, json!({"type": "ping"})).await?;
    assert_eq!(next_frame(&mut ws).await?, json!({"type": "pong"}));

    Ok(())
}

#[tokio::test]
async fn logging_out_revokes_the_socket() -> Result<()> {
    let server = TestServer::start().await?;
    let session = server.login("logout_revokes", None).await?;
    let mut ws = server.ws("/ws", Some(&session)).await?;
    subscribe(&mut ws, "lobby").await?;
    next_frame(&mut ws).await?; // count 1

    server
        .post("/api/auth/logout")
        .cookie(&session)
        .send()
        .await?
        .ok();

    assert_eq!(
        close_frame(&mut ws).await?,
        (1000, "session revoked".into())
    );

    Ok(())
}

#[tokio::test]
async fn revoking_one_session_leaves_the_other() -> Result<()> {
    let server = TestServer::start().await?;
    let account = server.account("two_devices", None).await?;
    let first = server.open_session(account.clone()).await?;
    let second = server.open_session(account).await?;

    let mut first_ws = server.ws("/ws", Some(&first)).await?;
    subscribe(&mut first_ws, "lobby").await?;
    next_frame(&mut first_ws).await?; // count 1

    let mut second_ws = server.ws("/ws", Some(&second)).await?;
    subscribe(&mut second_ws, "lobby").await?;
    next_frame(&mut second_ws).await?; // count 2

    let sessions = server
        .get("/api/auth/sessions")
        .cookie(&first)
        .send()
        .await?
        .ok()
        .json();
    let other = sessions["sessions"]
        .as_array()
        .expect("a list of sessions")
        .iter()
        .find(|session| session["current"] == json!(false))
        .expect("the other device's session");
    let other_id = other["id"].as_i64().expect("a numeric session id");

    server
        .delete(format!("/api/auth/sessions/{other_id}"))
        .cookie(&first)
        .send()
        .await?
        .ok();

    assert_eq!(
        close_frame(&mut second_ws).await?,
        (1000, "session revoked".into())
    );

    send(
        &mut first_ws,
        json!({"type": "chat", "text": "still connected"}),
    )
    .await?;
    let chat = wait_for(&mut first_ws, |frame| frame["type"] == "chat").await?;
    assert_eq!(chat["text"], "still connected");

    Ok(())
}

#[tokio::test]
async fn disconnecting_a_client_updates_the_online_count() -> Result<()> {
    let server = TestServer::start().await?;
    let alice = server.login("drop_alice", None).await?;
    let bob = server.login("drop_bob", None).await?;

    let mut alice_ws = server.ws("/ws", Some(&alice)).await?;
    subscribe(&mut alice_ws, "lobby").await?;
    next_frame(&mut alice_ws).await?; // count 1

    let mut bob_ws = server.ws("/ws", Some(&bob)).await?;
    subscribe(&mut bob_ws, "lobby").await?;
    wait_for(&mut alice_ws, |frame| frame["count"] == 2).await?;

    drop(bob_ws);
    wait_for(&mut alice_ws, |frame| {
        frame["type"] == "online" && frame["count"] == 1
    })
    .await?;

    Ok(())
}

#[tokio::test]
async fn deleting_a_user_revokes_its_socket() -> Result<()> {
    let server = TestServer::start().await?;
    let admin = server
        .login("delete_revokes", Some(DefaultRole::Admin.name()))
        .await?;
    let victim = server
        .open_session(server.account("delete_victim", None).await?)
        .await?;
    let mut ws = server.ws("/ws", Some(&victim)).await?;
    subscribe(&mut ws, "lobby").await?;
    next_frame(&mut ws).await?; // count 1

    server
        .delete(format!("/api/users/{}", victim.user_id()))
        .cookie(&admin)
        .send()
        .await?
        .ok();

    assert_eq!(
        close_frame(&mut ws).await?,
        (1000, "session revoked".into())
    );

    Ok(())
}

#[tokio::test]
async fn draining_closes_the_socket_with_1001() -> Result<()> {
    let server = TestServer::start().await?;
    let session = server.login("drain_close", None).await?;
    let mut ws = server.ws("/ws", Some(&session)).await?;

    server.ctx.trigger_shutdown();

    assert_eq!(
        close_frame(&mut ws).await?,
        (1001, "server shutting down".into())
    );

    Ok(())
}
