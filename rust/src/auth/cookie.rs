use axum::http::header::COOKIE;
use axum::http::HeaderMap;
use cookie::time::{Duration, OffsetDateTime};
use cookie::{Cookie, SameSite};

use crate::config::{Config, REFRESH_COOKIE_NAME, REFRESH_COOKIE_PATH};

fn days(expire: i64) -> Duration {
    Duration::days(expire)
}

pub fn refresh_cookie(config: &Config, token: &str) -> String {
    Cookie::build((REFRESH_COOKIE_NAME, token))
        .path(REFRESH_COOKIE_PATH)
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(config.cookie_secure)
        .max_age(days(crate::config::REFRESH_TOKEN_EXPIRE_DAYS))
        .expires(OffsetDateTime::now_utc() + days(crate::config::REFRESH_TOKEN_EXPIRE_DAYS))
        .build()
        .to_string()
}

pub fn delete_cookie(config: &Config) -> String {
    Cookie::build((REFRESH_COOKIE_NAME, ""))
        .path(REFRESH_COOKIE_PATH)
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(config.cookie_secure)
        .max_age(Duration::ZERO)
        .expires(OffsetDateTime::UNIX_EPOCH)
        .build()
        .to_string()
}

pub fn request_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    for value in headers.get_all(COOKIE).iter() {
        let Ok(value) = value.to_str() else {
            continue;
        };
        for cookie in Cookie::split_parse(value).flatten() {
            if cookie.name() == name {
                return Some(cookie.value().to_string());
            }
        }
    }
    None
}

pub fn validate_request_origin(
    config: &Config,
    headers: &HeaderMap,
    required: bool,
) -> crate::error::ApiResult<()> {
    let origin = headers.get("origin").and_then(|value| value.to_str().ok());
    match origin {
        None if required => Err(crate::error::ApiError::forbidden(
            "Origin header is required",
        )),
        None => Ok(()),
        Some(origin) => {
            if origin == "null" || !config.allowed_auth_origins.contains(origin) {
                Err(crate::error::ApiError::forbidden("Origin is not allowed"))
            } else {
                Ok(())
            }
        }
    }
}

pub fn validate_browser_mutation(
    config: &Config,
    headers: &HeaderMap,
) -> crate::error::ApiResult<()> {
    validate_request_origin(config, headers, true)?;
    let csrf = headers
        .get(crate::config::CSRF_HEADER_NAME)
        .and_then(|value| value.to_str().ok());
    if csrf != Some(crate::config::CSRF_HEADER_VALUE) {
        return Err(crate::error::ApiError::forbidden(
            "X-CSRF-Protection header is required",
        ));
    }
    Ok(())
}
