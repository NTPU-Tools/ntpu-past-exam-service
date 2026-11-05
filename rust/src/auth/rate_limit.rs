use std::net::IpAddr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::HeaderMap;
use axum::http::StatusCode;
use ipnet::IpNet;
use redis::aio::ConnectionManager;
use sha2::Digest;

use crate::app::AppState;
use crate::config::RateLimitConfig;
use crate::error::{ApiError, ApiResult};

const SCRIPT: &str = r#"
local current = redis.call('INCR', KEYS[1])
if current == 1 then
  redis.call('EXPIRE', KEYS[1], ARGV[1])
end
local ttl = redis.call('TTL', KEYS[1])
return {current, ttl}
"#;

static LAST_FAILURE_LOG: AtomicI64 = AtomicI64::new(0);

fn is_trusted(address: &IpAddr, networks: &[IpNet]) -> bool {
    networks.iter().any(|network| {
        // An IPv4-mapped IPv6 address must not match an IPv4 range.
        match (network, address) {
            (IpNet::V4(network), IpAddr::V4(address)) => network.contains(address),
            (IpNet::V6(network), IpAddr::V6(address)) => network.contains(address),
            _ => false,
        }
    })
}

pub fn client_ip(peer: IpAddr, headers: &HeaderMap, config: &RateLimitConfig) -> String {
    let direct = peer.to_string();
    if !config.trust_proxy || config.trusted_proxies.is_empty() {
        return direct;
    }
    if !is_trusted(&peer, &config.trusted_proxies) {
        return direct;
    }

    let mut entries = Vec::new();
    for value in headers.get_all("x-forwarded-for").iter() {
        let Ok(value) = value.to_str() else {
            return direct;
        };
        for entry in value.split(',') {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            match entry.parse::<IpAddr>() {
                Ok(address) => entries.push(address),
                Err(_) => return direct,
            }
        }
    }

    for address in entries.iter().rev() {
        if !is_trusted(address, &config.trusted_proxies) {
            return address.to_string();
        }
    }
    direct
}

fn sha256_hex(value: &str) -> String {
    hex::encode(sha2::Sha256::digest(value.as_bytes()))
}

pub(crate) fn window_key_at(identifier: &str, now: i64, window_seconds: i64) -> (String, i64) {
    let window = now / window_seconds;
    let retry_after = window_seconds - (now % window_seconds);
    (
        format!("rate-limit:auth:{window}:{}", &sha256_hex(identifier)[..32]),
        retry_after,
    )
}

fn log_fail_open_once() {
    let now = UtcSeconds::now();
    let last = LAST_FAILURE_LOG.load(Ordering::Relaxed);
    if now - last >= 60
        && LAST_FAILURE_LOG
            .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
    {
        tracing::warn!("Auth rate limiter unavailable; allowing request");
    }
}

struct UtcSeconds;

impl UtcSeconds {
    fn now() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0)
    }
}

enum WindowError {
    Limited(i64),
    Store,
}

async fn enforce_window(
    redis: &ConnectionManager,
    key: &str,
    limit: i64,
    window_ttl: i64,
) -> Result<(), WindowError> {
    let mut connection = redis.clone();
    let (current, ttl): (i64, i64) = redis::Script::new(SCRIPT)
        .key(key)
        .arg(window_ttl)
        .invoke_async(&mut connection)
        .await
        .map_err(|_| WindowError::Store)?;
    if current > limit {
        return Err(WindowError::Limited(ttl.max(1)));
    }
    Ok(())
}

/// Applies the global and per-account fixed-window limits. Redis failures are
/// fail-open (authentication stays available), logged at most once per minute.
pub async fn enforce(
    state: &AppState,
    peer: IpAddr,
    headers: &HeaderMap,
    subject: Option<&str>,
) -> ApiResult<()> {
    let config = &state.config.rate_limit;
    let now = UtcSeconds::now();
    let ip = client_ip(peer, headers, config);

    let (global, window_ttl) = window_key_at(&format!("global:{ip}"), now, config.window_seconds);
    match enforce_window(&state.redis, &global, config.global_limit, window_ttl).await {
        Ok(()) => {}
        Err(WindowError::Limited(ttl)) => return Err(limited(ttl)),
        Err(WindowError::Store) => {
            log_fail_open_once();
            return Ok(());
        }
    }

    if let Some(subject) = subject {
        let folded = caseless::default_case_fold_str(subject);
        let (key, _) = window_key_at(
            &format!("subject:{ip}:{folded}"),
            now,
            config.window_seconds,
        );
        match enforce_window(&state.redis, &key, config.subject_limit, window_ttl).await {
            Ok(()) => {}
            Err(WindowError::Limited(ttl)) => return Err(limited(ttl)),
            Err(WindowError::Store) => log_fail_open_once(),
        }
    }
    Ok(())
}

fn limited(retry_after: i64) -> ApiError {
    ApiError::with_headers(
        StatusCode::TOO_MANY_REQUESTS,
        "Too many authentication attempts",
        vec![("Retry-After", retry_after.max(1).to_string())],
    )
}
