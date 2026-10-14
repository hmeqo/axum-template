//! 会话认证：cookie 是唯一凭据，空闲窗口由任何带 cookie 的已认证请求隐式推移。

use anyhow::Result;
use axum::http::StatusCode;
use axum_template::domain::model::DefaultRole;
use serde_json::json;

mod common;

use common::{TestServer, in_secs};

#[tokio::test]
async fn login_issues_a_usable_session_cookie() -> Result<()> {
    let server = TestServer::start().await?;
    let session = server
        .login("session_login", Some(DefaultRole::User.name()))
        .await?;

    let me = server
        .get("/api/auth/me")
        .cookie(&session)
        .send()
        .await?
        .ok();
    assert_eq!(me.json()["user"]["username"], session.username());
    assert_eq!(
        me.json()["permissions"],
        json!(["user:read"]),
        "permissions come from roles and must be visible in the session"
    );

    Ok(())
}

#[tokio::test]
async fn logout_deletes_the_session_and_clears_the_cookie() -> Result<()> {
    let server = TestServer::start().await?;
    let session = server.login("session_logout", None).await?;

    let res = server
        .post("/api/auth/logout")
        .cookie(&session)
        .send()
        .await?
        .ok();

    let cleared = res.set_cookies().join(" | ");
    let cookie_name = &server.ctx.cfg().auth.session.cookie_name;
    assert!(
        cleared.contains(cookie_name),
        "logout must clear the session cookie: {cleared}"
    );
    assert!(
        cleared.contains("Max-Age=0") || cleared.contains("Expires="),
        "clearing the cookie must expire it immediately: {cleared}"
    );

    assert!(
        server
            .ctx
            .srv()
            .session
            .find_by_token(&session.token)
            .await?
            .is_none(),
        "logout should delete the session row, not just clear the client cookie"
    );
    server
        .get("/api/auth/me")
        .cookie(&session)
        .send()
        .await?
        .expect(StatusCode::UNAUTHORIZED);

    Ok(())
}

/// 请求路径上顺带滑动：剩余超过半个 TTL 不动窗口，过半之后续期并重发 cookie。
#[tokio::test]
async fn authenticated_requests_slide_the_window_past_half_the_ttl() -> Result<()> {
    let server = TestServer::start().await?;
    let session = server.login("slide_push", None).await?;
    let ttl = server.ctx.cfg().auth.session.ttl_hours as i64 * 3600;

    // 剩余超过半个 TTL：普通已认证请求既不写库，也不重发 cookie。
    let almost_full = in_secs(ttl - ttl / 4);
    server.age_session(&session.token, almost_full).await?;
    let steady = server.get("/api/auth/me").cookie(&session).send().await?;
    let steady = steady.ok();
    assert!(
        steady.set_cookie().is_none(),
        "more than half a TTL is left, so the response must not resend the cookie"
    );
    let untouched = server
        .ctx
        .srv()
        .session
        .find_by_token(&session.token)
        .await?
        .expect("session");
    assert_eq!(
        untouched.expires_at, almost_full,
        "the window must not move yet"
    );

    // 剩余不足半个 TTL：同一个普通请求顺带续期，并把新寿命随 cookie 一起发出。
    let near = in_secs(60);
    server.age_session(&session.token, near).await?;
    let slid = server.get("/api/auth/me").cookie(&session).send().await?;
    let slid = slid.ok();

    let cookie = slid
        .set_cookies()
        .into_iter()
        .next()
        .expect("a request past half the TTL must resend the cookie");
    let max_age: i64 = cookie
        .split(';')
        .find_map(|attribute| attribute.trim().strip_prefix("Max-Age="))
        .expect("the cookie carries Max-Age")
        .parse()?;

    let renewed = server
        .ctx
        .srv()
        .session
        .find_by_token(&session.token)
        .await?
        .expect("session");
    assert!(
        renewed.expires_at > near,
        "the request must have pushed the window forward"
    );
    let remaining = renewed.expires_at.as_second() - jiff::Timestamp::now().as_second();
    assert!(
        (max_age - remaining).abs() <= 2,
        "cookie Max-Age {max_age}s must match the row's remaining {remaining}s"
    );

    Ok(())
}

/// 续期与清除不会互相覆盖：剩余不足半个 TTL 时登出，响应里只能有清除用的那条 cookie。
#[tokio::test]
async fn a_renewal_does_not_resurrect_a_cookie_the_handler_cleared() -> Result<()> {
    let server = TestServer::start().await?;
    let session = server.login("slide_logout", None).await?;
    server.age_session(&session.token, in_secs(60)).await?;

    let res = server
        .post("/api/auth/logout")
        .cookie(&session)
        .send()
        .await?;
    let res = res.ok();

    let name = &server.ctx.cfg().auth.session.cookie_name;
    let cookies = res.set_cookies();
    assert_eq!(
        cookies.len(),
        1,
        "logout must write only the clearing cookie: {cookies:?}"
    );
    assert!(
        cookies[0].starts_with(&format!("{name}=")),
        "the clearing cookie names the session cookie: {cookies:?}"
    );

    Ok(())
}
