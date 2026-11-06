use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::config::{
    Config, ACCESS_TOKEN_EXPIRE_SECONDS, REFRESH_SESSION_VERSION, REFRESH_TOKEN_EXPIRE_DAYS,
};
use crate::db::UserRow;
use crate::error::{ApiError, ApiResult};

fn invalid() -> ApiError {
    ApiError::unauthorized("Invalid or expired token")
}

pub fn decode(config: &Config, token: &str, expected_type: &str) -> ApiResult<Value> {
    decode_with_key(&config.hash_key, token, expected_type)
}

pub(crate) fn decode_with_key(
    hash_key: &str,
    token: &str,
    expected_type: &str,
) -> ApiResult<Value> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.leeway = 0;
    validation.validate_aud = false;
    validation.validate_nbf = true;
    validation.required_spec_claims = ["exp", "sub"]
        .iter()
        .map(|claim| (*claim).to_string())
        .collect();

    let decoded = jsonwebtoken::decode::<Value>(
        token,
        &DecodingKey::from_secret(hash_key.as_bytes()),
        &validation,
    )
    .map_err(|_| invalid())?;
    let claims = decoded.claims;

    // python-jose rejects an audience when none is expected.
    if claims.get("aud").is_some() {
        return Err(invalid());
    }
    if !claims.get("sub").map(Value::is_string).unwrap_or(false) {
        return Err(invalid());
    }
    if let Some(iat) = claims.get("iat") {
        if !iat.is_number() {
            return Err(invalid());
        }
    }
    if let Some(jti) = claims.get("jti") {
        if !jti.is_string() {
            return Err(invalid());
        }
    }
    if claims.get("type").and_then(Value::as_str) != Some(expected_type) {
        return Err(ApiError::unauthorized("Invalid token type"));
    }
    Ok(claims)
}

pub fn adm_claim(department_ids: &[String]) -> String {
    let encoded: Vec<String> = department_ids
        .iter()
        .map(|id| serde_json::to_string(id).unwrap_or_else(|_| "\"\"".into()))
        .collect();
    format!("[{}]", encoded.join(", "))
}

pub struct Tokens {
    pub access_token: String,
    pub refresh_token: String,
    pub session_id: String,
    pub refresh_jti: String,
}

fn encode(config: &Config, claims: &Value) -> ApiResult<String> {
    jsonwebtoken::encode(
        &Header::new(Algorithm::HS256),
        claims,
        &EncodingKey::from_secret(config.hash_key.as_bytes()),
    )
    .map_err(ApiError::internal)
}

pub fn build_tokens(
    config: &Config,
    user: &UserRow,
    admin_department_ids: &[String],
    session_id: Option<String>,
) -> ApiResult<Tokens> {
    let now = Utc::now().timestamp();
    let adm = adm_claim(admin_department_ids);
    let access = json!({
        "sub": user.id,
        "type": "access",
        "isu": user.is_super_user,
        "id": user.id,
        "adm": adm,
        "exp": now + ACCESS_TOKEN_EXPIRE_SECONDS,
    });
    let session_id = session_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let refresh_jti = Uuid::new_v4().to_string();
    let refresh = json!({
        "sub": user.id,
        "type": "refresh",
        "isu": user.is_super_user,
        "id": user.id,
        "adm": adm,
        "session_version": REFRESH_SESSION_VERSION,
        "sid": session_id,
        "jti": refresh_jti,
        "exp": now + REFRESH_TOKEN_EXPIRE_DAYS * 24 * 60 * 60,
    });
    Ok(Tokens {
        access_token: encode(config, &access)?,
        refresh_token: encode(config, &refresh)?,
        session_id,
        refresh_jti,
    })
}
