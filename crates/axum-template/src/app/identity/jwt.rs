use axum::http::request::Parts;

use super::CredentialSpec;
use crate::{app::AppContext, domain::vo::UserId, error::AppError};

pub(crate) const BEARER: CredentialSpec = CredentialSpec {
    name: "bearer token",
    rejected_challenge: Some("Bearer realm=\"api\", error=\"invalid_token\""),
};

pub(crate) const CHALLENGE: &str = "Bearer realm=\"api\"";

/// `Authorization: Bearer <token>` 的解析：没有该头是 `Ok(None)`，头不是 `Bearer …` 形状是 `Err`。
pub(crate) async fn resolve(
    parts: &mut Parts,
    state: &AppContext,
) -> Result<Option<UserId>, AppError> {
    let Some(value) = parts.headers.get("Authorization") else {
        return Ok(None);
    };

    let token = value
        .to_str()
        .ok()
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::unauthenticated("Malformed Authorization header"))?;

    let claims = state.srv().token.decode_access_token(token)?;
    Ok(Some(claims.sub))
}
