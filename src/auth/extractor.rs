use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::HeaderMap;
use serde_json::Value;

use crate::app::AppState;
use crate::db::{self, UserRow};
use crate::error::{ApiError, ApiResult};

use super::jwt;

pub struct AuthUser(pub UserRow);

impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let state = state.as_ref();
        let token = bearer_token(&parts.headers)?;
        let claims = jwt::decode(&state.config, &token, "access")?;
        let user = resolve_from_claims(state, &claims).await?;
        Ok(AuthUser(user))
    }
}

pub fn bearer_token(headers: &HeaderMap) -> ApiResult<String> {
    let value = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let parts: Vec<&str> = value.split(' ').collect();
    if parts.len() != 2 || parts[0] != "Bearer" {
        return Err(ApiError::unauthorized(
            "Invalid or missing authorization header",
        ));
    }
    Ok(parts[1].to_string())
}

pub fn bearer_token_credentials(headers: &HeaderMap) -> ApiResult<String> {
    let value = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let parts: Vec<&str> = value.split(' ').collect();
    if parts.len() != 2 || parts[0] != "Bearer" || parts[1].is_empty() {
        return Err(ApiError::unauthorized("Invalid credentials"));
    }
    Ok(parts[1].to_string())
}

/// Mode A: `id` claim first, then the legacy `sub` claim (id, then username).
pub async fn resolve_from_claims(state: &AppState, claims: &Value) -> ApiResult<UserRow> {
    let id = claims
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty());
    let mut user = None;
    if let Some(id) = id {
        user = db::get_user(&state.db, id).await?;
    }
    if user.is_none() {
        if let Some(sub) = claims
            .get("sub")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        {
            user = db::get_user(&state.db, sub).await?;
            if user.is_none() {
                user = db::get_user_by_username(&state.db, sub).await?;
            }
        }
    }
    user.ok_or_else(|| ApiError::unauthorized("Invalid credentials"))
}

/// Mode B used by `/threads*`: requires the `id` claim.
pub async fn authenticate_thread(state: &AppState, headers: &HeaderMap) -> ApiResult<UserRow> {
    let token = bearer_token(headers)?;
    let claims = jwt::decode(&state.config, &token, "access")?;
    let id = claims
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::unauthorized("Invalid token: missing user id"))?;
    db::get_user(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("User not found"))
}
