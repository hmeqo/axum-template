#![cfg(feature = "webtransport")]

//! WebTransport 示例：CONNECT 头里的凭证先定下身份，然后位置走不可靠数据报、名单走单向流。

use anyhow::Result;
use axum_template::{
    app::AppContext,
    config::{AppConfig, ConfigData},
    domain::{
        service::bootstrap,
        vo::{ClientInfo, Password, Username},
    },
    error::Result as AppResult,
};
use serde_json::Value;
use wtransport::{
    ClientConfig, Connection, Endpoint, endpoint::ConnectOptions, error::ConnectingError,
};

mod common;

use common::Scratch;

const PASSWORD: &str = "TransportPass123";

fn free_udp_port() -> Result<u16> {
    Ok(std::net::UdpSocket::bind("127.0.0.1:0")?
        .local_addr()?
        .port())
}

// HTTP 面的凭证在 CONNECT 头里同样有效，测试因此用同一套签发。
struct Credentials {
    bearer: String,
    cookie: String,
    cookie_name: String,
}

async fn connect_once(port: u16, headers: &[(&str, &str)]) -> Result<Connection> {
    let config = ClientConfig::builder()
        .with_bind_default()
        .with_no_cert_validation()
        .build();
    let endpoint = Endpoint::client(config)?;

    let mut options = ConnectOptions::builder(format!("https://127.0.0.1:{port}"));
    for (name, value) in headers {
        options = options.add_header(*name, *value);
    }

    Ok(endpoint.connect(options.build()).await?)
}

/// 监听刚起时端口可能还没绑好，重试到连上。
async fn connect(port: u16, headers: &[(&str, &str)]) -> Result<Connection> {
    let mut last = None;
    for _ in 0..40 {
        match connect_once(port, headers).await {
            Ok(connection) => return Ok(connection),
            Err(e) => {
                last = Some(e);
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }
    }

    Err(anyhow::anyhow!("connect failed: {last:?}"))
}

/// 服务端每发一个事件就开一条单向流；加入时的第一条是名单。
async fn next_event(connection: &Connection) -> Result<Value> {
    let mut stream = connection.accept_uni().await?;
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    while let Some(read) = stream.read(&mut chunk).await? {
        buffer.extend_from_slice(&chunk[..read]);
    }
    Ok(serde_json::from_slice(&buffer)?)
}

async fn next_datagram(connection: &Connection) -> Result<Value> {
    let datagram = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        connection.receive_datagram(),
    )
    .await
    .expect("datagram within 5s")?;

    Ok(serde_json::from_slice(&datagram)?)
}

struct Harness {
    ctx: AppContext,
    port: u16,
    listener: Option<tokio::task::JoinHandle<AppResult<()>>>,
    // 最后才丢：库与媒体目录要活到监听停掉之后。
    _scratch: Scratch,
}

impl Harness {
    async fn stop(mut self) -> Result<()> {
        self.ctx.trigger_shutdown();
        if let Some(listener) = self.listener.take() {
            listener.await??;
        }
        Ok(())
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        if let Some(listener) = self.listener.take() {
            listener.abort();
        }
    }
}

async fn start() -> Result<Harness> {
    let port = free_udp_port()?;
    let loaded = AppConfig::load()?;
    let scratch = Scratch::prepare(&loaded).await?;
    let mut config = ConfigData::clone(&loaded);
    scratch.apply(&mut config);
    config.webtransport.enabled = true;
    config.webtransport.port = port;

    let ctx = AppContext::new(AppConfig::new(config)?).await?;
    let listener = tokio::spawn(axum_template::app::socket::webtransport::serve(ctx.clone()));

    Ok(Harness {
        ctx,
        port,
        listener: Some(listener),
        _scratch: scratch,
    })
}

async fn credentials(ctx: &AppContext) -> Result<Credentials> {
    let user = bootstrap::create_superuser(
        ctx.srv(),
        Username::new("transport_probe")?,
        Password::new(PASSWORD)?,
    )
    .await?;
    let session_token = ctx
        .srv()
        .auth
        .login_session(&user.username, PASSWORD, &ClientInfo::default())
        .await?
        .expect("the user just created can log in")
        .session_token;

    Ok(Credentials {
        bearer: ctx.srv().token.encode_access_token(&user)?,
        cookie: session_token,
        cookie_name: ctx.cfg().auth.session.cookie_name.clone(),
    })
}

/// 凭证是建会话的前提：有效 token 或有效会话 cookie 才行，缺失/无效一律被拒。
#[tokio::test]
async fn a_session_needs_valid_credentials() -> Result<()> {
    let harness = start().await?;
    let credentials = credentials(&harness.ctx).await?;
    let bearer = format!("Bearer {}", credentials.bearer);
    let cookie = format!("{}={}", credentials.cookie_name, credentials.cookie);

    for headers in [
        vec![("Authorization", bearer.as_str())],
        vec![("Cookie", cookie.as_str())],
    ] {
        let connection = connect(harness.port, &headers).await?;
        assert_eq!(
            next_event(&connection).await?["peers"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
    }

    for headers in [
        vec![],
        vec![("Authorization", "Bearer garbage")],
        vec![(
            "Cookie",
            format!("{}=garbage", credentials.cookie_name).as_str(),
        )],
    ] {
        let error = connect_once(harness.port, &headers)
            .await
            .expect_err("a rejected session must not connect");
        assert!(
            matches!(
                error.downcast_ref::<ConnectingError>(),
                Some(ConnectingError::SessionRejected)
            ),
            "expected a rejected session, got {error:?}"
        );
    }

    harness.stop().await
}

#[tokio::test]
async fn state_travels_as_datagrams_and_events_as_streams() -> Result<()> {
    let harness = start().await?;
    let credentials = credentials(&harness.ctx).await?;
    let bearer = format!("Bearer {}", credentials.bearer);
    let auth = [("Authorization", bearer.as_str())];

    let alice = connect(harness.port, &auth).await?;
    let empty = next_event(&alice).await?;
    assert_eq!(empty["peers"].as_array().unwrap().len(), 0);

    let bob = connect(harness.port, &auth).await?;
    // 加入是不可丢事件：走单向流，不走数据报。
    let joined_alice = next_event(&alice).await?;
    assert_eq!(joined_alice["type"], "joined");
    let bob_id = joined_alice["id"].clone();
    let bob_view = next_event(&bob).await?;
    assert_eq!(bob_view["peers"].as_array().unwrap().len(), 1);
    let alice_id = bob_view["peers"][0]["id"].clone();

    // 状态是高频可丢的：走数据报。
    bob.send_datagram(br#"{"x":1.5,"y":2.5}"#)?;
    let moved = next_datagram(&alice).await?;
    assert_eq!(moved["type"], "moved");
    assert_eq!(moved["x"], 1.5);
    assert_eq!(moved["y"], 2.5);

    drop(bob);
    let left = next_event(&alice).await?;
    assert_eq!(left["type"], "leave");
    assert_eq!(left["id"], bob_id);

    // 断开者要从名单里消失：新加入者只看到还连着的 alice。
    let carol = connect(harness.port, &auth).await?;
    let peers = next_event(&carol).await?["peers"].clone();
    assert_eq!(
        peers.as_array().unwrap().len(),
        1,
        "只有 alice 在线：{peers}"
    );
    assert_eq!(peers[0]["id"], alice_id, "离开的连接要清掉：{peers}");

    harness.stop().await
}

/// 会话被吊销（登出、改密吊销其它设备、删用户）时这条 QUIC 会话要立刻关闭，与 `/ws` 同一语义。
#[tokio::test]
async fn a_revoked_session_closes_the_connection() -> Result<()> {
    let harness = start().await?;
    let credentials = credentials(&harness.ctx).await?;
    let cookie = format!("{}={}", credentials.cookie_name, credentials.cookie);

    let connection = connect(harness.port, &[("Cookie", cookie.as_str())]).await?;
    let _ = next_event(&connection).await?;

    let session = harness
        .ctx
        .srv()
        .session
        .find_by_token(&credentials.cookie)
        .await?
        .expect("the session behind the cookie exists");
    harness.ctx.srv().session.revoke_own(session.id).await?;

    tokio::time::timeout(std::time::Duration::from_secs(5), connection.closed())
        .await
        .expect("a revoked session must close the connection within 5s");

    harness.stop().await
}
