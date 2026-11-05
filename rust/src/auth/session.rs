use chrono::Utc;
use redis::aio::ConnectionManager;
use redis::Script;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::config::REFRESH_SESSION_VERSION;
use crate::error::{ApiError, ApiResult};

const REFRESH_SESSION_PREFIX: &str = "auth:refresh-session:";
const LEGACY_REFRESH_PREFIX: &str = "auth:legacy-refresh-used:";

const ROTATE_SCRIPT: &str = r#"
-- refresh-session-rotate
local current_jti = redis.call('GET', KEYS[1])
if not current_jti or current_jti ~= ARGV[1] then
  return 0
end
redis.call('SET', KEYS[1], ARGV[2], 'EX', ARGV[3])
return 1
"#;

const MIGRATE_LEGACY_SCRIPT: &str = r#"
-- refresh-session-migrate-legacy
if redis.call('EXISTS', KEYS[1]) == 1 then
  return 0
end
redis.call('SET', KEYS[1], '1', 'EX', ARGV[1])
redis.call('SET', KEYS[2], ARGV[2], 'EX', ARGV[3])
return 1
"#;

fn sha256_hex(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

pub fn session_key(session_id: &str) -> String {
    format!("{REFRESH_SESSION_PREFIX}{}", sha256_hex(session_id))
}

pub fn legacy_key(token: &str) -> String {
    format!("{LEGACY_REFRESH_PREFIX}{}", sha256_hex(token))
}

pub fn is_managed(payload: &Value) -> bool {
    payload.get("session_version").and_then(Value::as_i64) == Some(REFRESH_SESSION_VERSION)
        && payload
            .get("sid")
            .and_then(Value::as_str)
            .map(|sid| !sid.is_empty())
            .unwrap_or(false)
        && payload
            .get("jti")
            .and_then(Value::as_str)
            .map(|jti| !jti.is_empty())
            .unwrap_or(false)
}

pub fn is_legacy(payload: &Value) -> bool {
    fn falsy(value: Option<&Value>) -> bool {
        match value {
            None | Some(Value::Null) => true,
            Some(Value::String(value)) => value.is_empty(),
            Some(Value::Bool(value)) => !*value,
            Some(Value::Number(value)) => value.as_i64() == Some(0),
            _ => false,
        }
    }
    falsy(payload.get("sid")) && falsy(payload.get("jti"))
}

pub fn token_ttl(payload: &Value) -> ApiResult<i64> {
    let expires_at = payload
        .get("exp")
        .and_then(|value| value.as_f64())
        .ok_or_else(|| ApiError::unauthorized("Invalid refresh token expiry"))?;
    let ttl = expires_at as i64 - Utc::now().timestamp();
    if ttl <= 0 {
        return Err(ApiError::unauthorized("Invalid or expired token"));
    }
    Ok(ttl)
}

fn store_unavailable() -> ApiError {
    tracing::error!("Refresh session store unavailable");
    ApiError::unavailable("Refresh session store is unavailable")
}

pub async fn register(
    redis: &ConnectionManager,
    session_id: &str,
    refresh_jti: &str,
    ttl: i64,
) -> ApiResult<()> {
    let mut connection = redis.clone();
    let result: Option<String> = redis::cmd("SET")
        .arg(session_key(session_id))
        .arg(refresh_jti)
        .arg("EX")
        .arg(ttl)
        .arg("NX")
        .query_async(&mut connection)
        .await
        .map_err(|_| store_unavailable())?;
    if result.is_none() {
        return Err(ApiError::unavailable("Could not create refresh session"));
    }
    Ok(())
}

pub async fn rotate(
    redis: &ConnectionManager,
    session_id: &str,
    old_jti: &str,
    new_jti: &str,
    ttl: i64,
) -> ApiResult<()> {
    let mut connection = redis.clone();
    let rotated: i64 = Script::new(ROTATE_SCRIPT)
        .key(session_key(session_id))
        .arg(old_jti)
        .arg(new_jti)
        .arg(ttl)
        .invoke_async(&mut connection)
        .await
        .map_err(|_| store_unavailable())?;
    if rotated != 1 {
        return Err(ApiError::unauthorized(
            "Refresh session is no longer active",
        ));
    }
    Ok(())
}

pub async fn migrate_legacy(
    redis: &ConnectionManager,
    legacy_token: &str,
    legacy_ttl: i64,
    session_id: &str,
    refresh_jti: &str,
    refresh_ttl: i64,
) -> ApiResult<()> {
    let mut connection = redis.clone();
    let migrated: i64 = Script::new(MIGRATE_LEGACY_SCRIPT)
        .key(legacy_key(legacy_token))
        .key(session_key(session_id))
        .arg(legacy_ttl)
        .arg(refresh_jti)
        .arg(refresh_ttl)
        .invoke_async(&mut connection)
        .await
        .map_err(|_| store_unavailable())?;
    if migrated != 1 {
        return Err(ApiError::unauthorized(
            "Legacy refresh token has already been used",
        ));
    }
    Ok(())
}

pub async fn revoke(redis: &ConnectionManager, payload: &Value, token: &str) -> ApiResult<()> {
    let mut connection = redis.clone();
    if is_managed(payload) {
        let session_id = payload["sid"].as_str().unwrap_or_default();
        let _: i64 = redis::cmd("DEL")
            .arg(session_key(session_id))
            .query_async(&mut connection)
            .await
            .map_err(|_| store_unavailable())?;
        return Ok(());
    }
    if is_legacy(payload) {
        let ttl = token_ttl(payload)?;
        let _: Option<String> = redis::cmd("SET")
            .arg(legacy_key(token))
            .arg("1")
            .arg("EX")
            .arg(ttl)
            .arg("NX")
            .query_async(&mut connection)
            .await
            .map_err(|_| store_unavailable())?;
        return Ok(());
    }
    Err(ApiError::unauthorized("Unknown refresh session"))
}
