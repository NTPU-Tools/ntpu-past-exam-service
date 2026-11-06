use redis::aio::ConnectionManager;
use redis::Script;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::config::REFRESH_SESSION_VERSION;
use crate::error::{ApiError, ApiResult};

const REFRESH_SESSION_PREFIX: &str = "auth:refresh-session:";

const ROTATE_SCRIPT: &str = r#"
-- refresh-session-rotate
local current_jti = redis.call('GET', KEYS[1])
if not current_jti or current_jti ~= ARGV[1] then
  return 0
end
redis.call('SET', KEYS[1], ARGV[2], 'EX', ARGV[3])
return 1
"#;

fn sha256_hex(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

pub fn session_key(session_id: &str) -> String {
    format!("{REFRESH_SESSION_PREFIX}{}", sha256_hex(session_id))
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

pub async fn revoke(redis: &ConnectionManager, payload: &Value) -> ApiResult<()> {
    if is_managed(payload) {
        let mut connection = redis.clone();
        let session_id = payload["sid"].as_str().unwrap_or_default();
        let _: i64 = redis::cmd("DEL")
            .arg(session_key(session_id))
            .query_async(&mut connection)
            .await
            .map_err(|_| store_unavailable())?;
        return Ok(());
    }
    if is_legacy(payload) {
        // Legacy sessions were never stored server-side after the migration
        // window closed (2026-11-03); deleting the cookie is all that remains.
        return Ok(());
    }
    Err(ApiError::unauthorized("Unknown refresh session"))
}
