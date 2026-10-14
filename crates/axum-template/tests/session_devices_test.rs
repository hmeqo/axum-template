//! 登录设备列表与单台撤销。

use anyhow::{Context, Result};
use axum::http::{HeaderName, HeaderValue, StatusCode, header};
use axum_template::domain::model::DefaultRole;
use serde_json::{Value, json};

mod common;

use common::{Account, Session, TestServer, close_frame, in_secs, login_body, send};

async fn sessions(server: &TestServer, session: &Session) -> Result<Value> {
    let res = server
        .get("/api/auth/sessions")
        .cookie(session)
        .send()
        .await?
        .ok();

    Ok(res.json())
}

fn entry<'a>(body: &'a Value, user_agent: &str) -> &'a Value {
    body["sessions"]
        .as_array()
        .expect("sessions should be an array")
        .iter()
        .find(|entry| entry["user_agent"] == user_agent)
        .unwrap_or_else(|| panic!("no session with user_agent {user_agent}: {body}"))
}

fn id_of(body: &Value, user_agent: &str) -> Value {
    entry(body, user_agent)["id"].clone()
}

#[tokio::test]
async fn a_login_records_the_client_and_marks_itself_current() -> Result<()> {
    let server = TestServer::start().await?;
    let account = server
        .account("device_probe", Some(DefaultRole::Admin.name()))
        .await?;

    let session = server
        .login_with_headers(&account, &[(header::USER_AGENT, "omp-test/1.0")])
        .await?;

    let body = sessions(&server, &session).await?;
    assert_eq!(body["sessions"].as_array().map(Vec::len), Some(1), "{body}");

    let entry = entry(&body, "omp-test/1.0");
    assert_eq!(entry["ip"], "127.0.0.1", "{body}");
    assert_eq!(entry["current"], true, "{body}");
    assert!(entry["created_at"].is_string(), "{body}");
    assert!(entry["expires_at"].is_string(), "{body}");

    Ok(())
}

#[tokio::test]
async fn a_second_login_is_a_second_device_and_can_be_revoked_alone() -> Result<()> {
    let server = TestServer::start().await?;
    let account = server
        .account("device_pair", Some(DefaultRole::Admin.name()))
        .await?;

    let first = server
        .login_with_headers(&account, &[(header::USER_AGENT, "probe-a/1.0")])
        .await?;
    let second = server
        .login_with_headers(&account, &[(header::USER_AGENT, "probe-b/1.0")])
        .await?;

    let body = sessions(&server, &first).await?;
    assert_eq!(body["sessions"].as_array().map(Vec::len), Some(2), "{body}");
    assert_eq!(entry(&body, "probe-a/1.0")["current"], true, "{body}");
    assert_eq!(entry(&body, "probe-b/1.0")["current"], false, "{body}");

    let second_id = id_of(&body, "probe-b/1.0");
    server
        .delete(format!("/api/auth/sessions/{second_id}"))
        .cookie(&first)
        .send()
        .await?
        .ok();

    server
        .get("/api/auth/me")
        .cookie(&second)
        .send()
        .await?
        .expect(StatusCode::UNAUTHORIZED);
    server.get("/api/auth/me").cookie(&first).send().await?.ok();

    Ok(())
}

#[tokio::test]
async fn logging_out_one_device_leaves_the_other() -> Result<()> {
    let server = TestServer::start().await?;
    let account = server
        .account("device_logout", Some(DefaultRole::User.name()))
        .await?;

    let first = server
        .login_with_headers(&account, &[(header::USER_AGENT, "probe-a/1.0")])
        .await?;
    let second = server
        .login_with_headers(&account, &[(header::USER_AGENT, "probe-b/1.0")])
        .await?;

    server
        .post("/api/auth/logout")
        .cookie(&first)
        .send()
        .await?
        .ok();

    server
        .get("/api/auth/me")
        .cookie(&first)
        .send()
        .await?
        .expect(StatusCode::UNAUTHORIZED);
    server
        .get("/api/auth/me")
        .cookie(&second)
        .send()
        .await?
        .ok();

    Ok(())
}

#[tokio::test]
async fn revoking_the_current_session_clears_the_cookie() -> Result<()> {
    let server = TestServer::start().await?;
    let account = server
        .account("device_self", Some(DefaultRole::Admin.name()))
        .await?;
    let session = server
        .login_with_headers(&account, &[(header::USER_AGENT, "probe-self/1.0")])
        .await?;

    let body = sessions(&server, &session).await?;
    let current_id = id_of(&body, "probe-self/1.0");

    let res = server
        .delete(format!("/api/auth/sessions/{current_id}"))
        .cookie(&session)
        .send()
        .await?
        .ok();

    let name = session.cookie.split_once('=').expect("name=value").0;
    let cleared = res
        .set_cookie()
        .context("revoking the current session must clear the cookie")?;
    assert!(cleared.starts_with(&format!("{name}=")), "{cleared}");

    server
        .get("/api/auth/me")
        .cookie(&session)
        .send()
        .await?
        .expect(StatusCode::UNAUTHORIZED);

    Ok(())
}

#[tokio::test]
async fn another_users_session_id_is_not_found() -> Result<()> {
    let server = TestServer::start().await?;
    let owner = server
        .account("device_owner", Some(DefaultRole::Admin.name()))
        .await?;
    let intruder = server
        .account("device_intruder", Some(DefaultRole::Admin.name()))
        .await?;

    let owner_session = server
        .login_with_headers(&owner, &[(header::USER_AGENT, "owner/1.0")])
        .await?;
    let intruder_session = server
        .login_with_headers(&intruder, &[(header::USER_AGENT, "intruder/1.0")])
        .await?;

    let body = sessions(&server, &owner_session).await?;
    let owner_id = id_of(&body, "owner/1.0");

    server
        .delete(format!("/api/auth/sessions/{owner_id}"))
        .cookie(&intruder_session)
        .send()
        .await?
        .expect(StatusCode::NOT_FOUND);

    server
        .get("/api/auth/me")
        .cookie(&owner_session)
        .send()
        .await?
        .ok();

    Ok(())
}

#[tokio::test]
async fn a_forwarded_header_is_ignored_without_trusted_proxies() -> Result<()> {
    let server = TestServer::start_with(|config| config.server.trusted_proxies.clear()).await?;
    let account = server
        .account("device_spoof", Some(DefaultRole::Admin.name()))
        .await?;

    let session = server
        .login_with_headers(
            &account,
            &[
                (header::USER_AGENT, "probe-spoof/1.0"),
                (HeaderName::from_static("x-forwarded-for"), "203.0.113.7"),
            ],
        )
        .await?;

    let body = sessions(&server, &session).await?;
    assert_eq!(entry(&body, "probe-spoof/1.0")["ip"], "127.0.0.1", "{body}");

    Ok(())
}

#[tokio::test]
async fn a_slide_moves_expires_at_forward() -> Result<()> {
    let server = TestServer::start().await?;
    let account = server
        .account("device_slide", Some(DefaultRole::Admin.name()))
        .await?;
    let session = server
        .login_with_headers(&account, &[(header::USER_AGENT, "probe-slide/1.0")])
        .await?;

    let before = sessions(&server, &session).await?;
    let before_expiry = entry(&before, "probe-slide/1.0")["expires_at"]
        .as_str()
        .context("expires_at should be a string")?
        .to_owned();

    // 滑动只在「剩余不足半个 TTL」时才写,故先把到期时间拨近;不 sleep。
    server.age_session(&session.token, in_secs(60)).await?;
    server
        .get("/api/auth/me")
        .cookie(&session)
        .send()
        .await?
        .ok();

    let after = sessions(&server, &session).await?;
    let after_expiry = entry(&after, "probe-slide/1.0")["expires_at"]
        .as_str()
        .context("expires_at should be a string")?
        .to_owned();

    assert!(
        after_expiry > before_expiry,
        "before={before_expiry} after={after_expiry}"
    );

    Ok(())
}

#[tokio::test]
async fn changing_a_password_rotates_this_device_and_kills_the_others() -> Result<()> {
    let server = TestServer::start().await?;
    let account = server
        .account("device_password", Some(DefaultRole::Admin.name()))
        .await?;

    let current = server
        .login_with_headers(&account, &[(header::USER_AGENT, "password-a/1.0")])
        .await?;
    let other = server
        .login_with_headers(&account, &[(header::USER_AGENT, "password-b/1.0")])
        .await?;

    // 另一台设备还持有一份刷新令牌与一条长连接。
    let refresh_token = server
        .post("/api/auth/jwt/login")
        .json(&login_body(&account.username, Account::PASSWORD))
        .send()
        .await?
        .ok()
        .json()["refresh_token"]
        .as_str()
        .context("refresh_token should be a string")?
        .to_owned();

    let mut other_ws = server.ws("/ws", Some(&other)).await?;
    send(
        &mut other_ws,
        json!({"type": "subscribe", "topic": "lobby"}),
    )
    .await?;

    let changed = server
        .put("/api/auth/password")
        .cookie(&current)
        .header(
            header::USER_AGENT,
            HeaderValue::from_static("password-a/1.0"),
        )
        .json(&json!({
            "old_password": Account::PASSWORD,
            "new_password": "password456",
        }))
        .send()
        .await?;
    let changed = changed.ok();

    // 轮换：新 cookie 随这个响应下发，旧 cookie 在改密这一刻作废。
    let rotated_cookie = changed
        .set_cookie()
        .context("changing a password must hand back a rotated session cookie")?;
    let rotated = Session {
        account: current.account.clone(),
        token: rotated_cookie
            .split_once('=')
            .context("the cookie is name=value")?
            .1
            .to_owned(),
        cookie: rotated_cookie,
    };

    server
        .get("/api/auth/me")
        .cookie(&rotated)
        .send()
        .await?
        .ok();
    server
        .get("/api/auth/me")
        .cookie(&current)
        .send()
        .await?
        .expect(StatusCode::UNAUTHORIZED);
    server
        .get("/api/auth/me")
        .cookie(&other)
        .send()
        .await?
        .expect(StatusCode::UNAUTHORIZED);

    assert_eq!(
        close_frame(&mut other_ws).await?,
        (1000, "session revoked".to_owned())
    );

    server
        .post("/api/auth/jwt/refresh")
        .json(&json!({"refresh_token": refresh_token}))
        .send()
        .await?
        .expect(StatusCode::UNAUTHORIZED)
        .expect_code("auth.refresh_token_invalid");

    let body = sessions(&server, &rotated).await?;
    assert_eq!(body["sessions"].as_array().map(Vec::len), Some(1), "{body}");
    assert_eq!(entry(&body, "password-a/1.0")["current"], true, "{body}");

    Ok(())
}

/// 退登其它会话：当前设备留着，别的设备的会话立刻失效。
#[tokio::test]
async fn revoking_other_sessions_keeps_only_this_device() -> Result<()> {
    let server = TestServer::start().await?;
    let account = server
        .account("device_revoke_others", Some(DefaultRole::Admin.name()))
        .await?;

    let current = server
        .login_with_headers(&account, &[(header::USER_AGENT, "others-a/1.0")])
        .await?;
    let other = server
        .login_with_headers(&account, &[(header::USER_AGENT, "others-b/1.0")])
        .await?;

    server
        .post("/api/auth/sessions/revoke-others")
        .cookie(&current)
        .send()
        .await?
        .ok();

    server
        .get("/api/auth/me")
        .cookie(&current)
        .send()
        .await?
        .ok();
    server
        .get("/api/auth/me")
        .cookie(&other)
        .send()
        .await?
        .expect(StatusCode::UNAUTHORIZED);

    let body = sessions(&server, &current).await?;
    assert_eq!(body["sessions"].as_array().map(Vec::len), Some(1), "{body}");

    Ok(())
}
