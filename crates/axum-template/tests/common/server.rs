//! 被测服务：真实 `AppContext` + 真实监听地址（WS 测试要连它）。

use std::{
    net::SocketAddr,
    sync::atomic::{AtomicU32, Ordering},
};

use anyhow::{Context, Result};
use axum::{
    Extension, Router,
    extract::ConnectInfo,
    http::{HeaderName, HeaderValue, Method, StatusCode},
};
use axum_template::{
    app::{AppContext, router::create_router},
    config::{AppConfig, ConfigData},
    domain::vo::{Password, UserId, Username},
};
use serde_json::json;
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_tungstenite::tungstenite;

use super::{Scratch, TestRequest, TestResponse, ws};

#[derive(Debug, Clone)]
pub struct Account {
    pub id: UserId,
    pub username: String,
}

impl Account {
    pub const PASSWORD: &'static str = "password123";
}

#[derive(Debug, Clone)]
pub struct Session {
    pub account: Account,
    pub cookie: String,
    pub token: String,
}

impl Session {
    pub fn user_id(&self) -> UserId {
        self.account.id
    }

    pub fn username(&self) -> &str {
        &self.account.username
    }
}

pub struct TestServer {
    pub ctx: AppContext,
    pub app: Router,
    pub addr: SocketAddr,
    /// 本服务器独占的库与媒体目录；字段顺序保证连接池先于它释放。
    scratch: Scratch,
    serve: JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.serve.abort();
    }
}

impl TestServer {
    /// 起一台真实服务：自建自删的独占库、独占媒体目录、真实监听地址。
    ///
    /// 库建好时就带 migrations 与默认角色。
    pub async fn start() -> Result<Self> {
        Self::start_with(|_| {}).await
    }

    /// 与 `start()` 相同,额外的 `mutate` 在 scratch 覆盖之后生效。
    pub async fn start_with(mutate: impl FnOnce(&mut ConfigData)) -> Result<Self> {
        let loaded = AppConfig::load()?;
        let scratch = Scratch::prepare(&loaded).await?;
        let mut config = ConfigData::clone(&loaded);
        scratch.apply(&mut config);
        mutate(&mut config);

        let ctx = AppContext::new(AppConfig::new(config)?).await?;
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        // 请求走 `oneshot`(不经监听器),真实的 `ConnectInfo` 注入不参与;这里照 make-service 的做法补上。
        let app = create_router(ctx.clone())
            .await?
            .layer(Extension(ConnectInfo(addr)));
        let served = app.clone();
        let serve = tokio::spawn(async move {
            let _ = axum::serve(
                listener,
                served.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await;
        });

        Ok(Self {
            ctx,
            app,
            addr,
            scratch,
            serve,
        })
    }

    pub async fn account(&self, name: &str, role: Option<&str>) -> Result<Account> {
        let username = unique_username(name);
        let user = self
            .ctx
            .srv()
            .user
            .create(
                Username::new(username.clone())?,
                Password::new(Account::PASSWORD)?,
            )
            .await?;

        if let Some(role) = role {
            let role = self
                .ctx
                .srv()
                .role
                .find_by_name(role)
                .await?
                .with_context(|| format!("role {role} not found: init_rbac did not create it?"))?;
            self.ctx.srv().role.assign_to_user(user.id, role.id).await?;
        }

        Ok(Account {
            id: user.id,
            username,
        })
    }

    pub async fn login(&self, name: &str, role: Option<&str>) -> Result<Session> {
        let account = self.account(name, role).await?;
        self.open_session(account).await
    }

    pub async fn open_session(&self, account: Account) -> Result<Session> {
        let res = self
            .post("/api/auth/login")
            .json(&login_body(&account.username, Account::PASSWORD))
            .send()
            .await?
            .ok();

        session_from(res, account)
    }

    /// 带额外请求头的登录；`headers` 用于 `User-Agent`、`X-Forwarded-For` 等客户端指纹。
    pub async fn login_with_headers(
        &self,
        account: &Account,
        headers: &[(HeaderName, &str)],
    ) -> Result<Session> {
        let mut request = self
            .post("/api/auth/login")
            .json(&login_body(&account.username, Account::PASSWORD));
        for (name, value) in headers {
            request = request.header(name.clone(), HeaderValue::from_str(value)?);
        }

        session_from(request.send().await?.ok(), account.clone())
    }

    /// 用已建好的账号换一份访问令牌。
    pub async fn access_token(&self, account: &Account) -> Result<String> {
        let res = self
            .post("/api/auth/jwt/login")
            .json(&login_body(&account.username, Account::PASSWORD))
            .send()
            .await?
            .ok();

        Ok(res.json()["access_token"]
            .as_str()
            .expect("access_token")
            .to_owned())
    }

    /// 通过 ORM 把会话的到期时间改到指定时刻。
    pub async fn age_session(&self, token: &str, expires_at: jiff::Timestamp) -> Result<()> {
        let mut db = self.ctx.db().clone();
        let mut session = self
            .ctx
            .srv()
            .session
            .find_by_token(token)
            .await?
            .expect("session exists");

        toasty::update!(session {
            expires_at: expires_at,
        })
        .exec(&mut db)
        .await?;
        Ok(())
    }

    /// OpenAPI 文档（路由守卫与文档契约都以它为准）。
    pub async fn openapi(&self) -> Result<serde_json::Value> {
        let res = self.get("/api-docs/openapi.json").send().await?.ok();
        Ok(res.json())
    }

    pub async fn ws(&self, path: &str, session: Option<&Session>) -> Result<ws::Socket> {
        ws::connect_path(self.addr, path, session).await
    }

    /// 不带会话的升级必须被认证拒（`401`），而不是路由缺失或握手错误。
    pub async fn ws_without_session_is_rejected(&self, path: &str) -> Result<()> {
        let failure = match ws::connect_path(self.addr, path, None).await {
            Ok(_) => anyhow::bail!("{path} accepted an upgrade without a session"),
            Err(failure) => failure,
        };

        match failure.downcast_ref::<tungstenite::Error>() {
            Some(tungstenite::Error::Http(response)) => anyhow::ensure!(
                response.status() == StatusCode::UNAUTHORIZED,
                "{path}: expected 401 without a session, got {}",
                response.status()
            ),
            _ => anyhow::bail!(
                "{path}: upgrade without a session failed before an HTTP response: {failure:#}"
            ),
        }

        Ok(())
    }

    pub fn request(&self, method: Method, uri: impl Into<String>) -> TestRequest<'_> {
        TestRequest::new(self, method, uri.into())
    }

    pub fn get(&self, uri: impl Into<String>) -> TestRequest<'_> {
        self.request(Method::GET, uri)
    }

    pub fn post(&self, uri: impl Into<String>) -> TestRequest<'_> {
        self.request(Method::POST, uri)
    }

    pub fn put(&self, uri: impl Into<String>) -> TestRequest<'_> {
        self.request(Method::PUT, uri)
    }

    pub fn delete(&self, uri: impl Into<String>) -> TestRequest<'_> {
        self.request(Method::DELETE, uri)
    }
}

/// 登录请求体：`/api/auth/login` 与 `/api/auth/jwt/login` 形状相同。
pub fn login_body(username: &str, password: &str) -> serde_json::Value {
    json!({ "username": username, "password": password })
}

/// 用户名策略要求 3–20 字符；每台服务器一个库，故「前缀 + 进程内序号」已足够唯一。
fn unique_username(prefix: &str) -> String {
    static SEQ: AtomicU32 = AtomicU32::new(0);

    let suffix = format!("_{:x}", SEQ.fetch_add(1, Ordering::Relaxed));
    let head = if prefix.is_empty() { "u" } else { prefix };
    let keep = Username::MAX_CHARS - suffix.len();

    format!("{}{suffix}", head.chars().take(keep).collect::<String>())
}

fn session_from(res: TestResponse, account: Account) -> Result<Session> {
    let cookie = res.set_cookie().context("login sent no session cookie")?;
    let token = cookie
        .split_once('=')
        .with_context(|| format!("malformed session cookie: {cookie}"))?
        .1
        .to_owned();

    Ok(Session {
        account,
        cookie,
        token,
    })
}

/// 整秒时间戳：Postgres 存微秒，整秒可精确往返。
pub fn in_secs(offset: i64) -> jiff::Timestamp {
    jiff::Timestamp::from_second(jiff::Timestamp::now().as_second() + offset).expect("valid second")
}
