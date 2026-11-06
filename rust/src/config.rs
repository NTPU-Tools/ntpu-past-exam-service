use std::collections::HashSet;

use anyhow::{bail, Context};
use ipnet::IpNet;

pub const REFRESH_COOKIE_NAME: &str = "ntpu-past-exam-refresh-token-v2";
pub const REFRESH_COOKIE_PATH: &str = "/refresh";
pub const REFRESH_TOKEN_EXPIRE_DAYS: i64 = 365;
pub const ACCESS_TOKEN_EXPIRE_SECONDS: i64 = 900;
pub const REFRESH_SESSION_VERSION: i64 = 1;
pub const CSRF_HEADER_NAME: &str = "x-csrf-protection";
pub const CSRF_HEADER_VALUE: &str = "1";

pub fn load_dotenv() {
    // Matches python-dotenv `load_dotenv()`: existing environment wins.
    let _ = dotenvy::dotenv();
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn required(name: &str) -> anyhow::Result<String> {
    match var(name) {
        Some(value) if !value.is_empty() => Ok(value),
        _ => bail!("{name} environment variable is not set or empty"),
    }
}

pub fn env_bool(name: &str, default: bool) -> bool {
    match var(name) {
        None => default,
        Some(value) => flag_value(&value),
    }
}

pub(crate) fn flag_value(value: &str) -> bool {
    matches!(
        value.trim().to_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn positive_int(name: &str, default: i64) -> i64 {
    parse_positive_int(var(name).as_deref(), default)
}

pub(crate) fn parse_positive_int(raw: Option<&str>, default: i64) -> i64 {
    match raw.and_then(|value| value.trim().parse::<i64>().ok()) {
        Some(value) if value > 0 => value,
        _ => default,
    }
}

pub(crate) fn trust_proxy_flag_value(value: &str) -> bool {
    matches!(value.to_lowercase().as_str(), "1" | "true" | "yes" | "on")
}

pub fn allowed_auth_origins_from_env() -> HashSet<String> {
    let configured = [var("AUTH_ALLOWED_ORIGINS"), var("ORIGIN")]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(",");
    configured
        .split(',')
        .map(|origin| origin.trim().trim_end_matches('/').to_string())
        .filter(|origin| !origin.is_empty())
        .collect()
}

pub struct RateLimitConfig {
    pub subject_limit: i64,
    pub global_limit: i64,
    pub window_seconds: i64,
    pub trust_proxy: bool,
    pub trusted_proxies: Vec<IpNet>,
}

pub struct MailConfig {
    pub sender: String,
    pub access_key: String,
    pub secret: String,
    pub endpoint: Option<String>,
}

pub struct Config {
    pub bind: String,
    pub port: u16,
    pub database_url: String,
    pub database_max_connections: u32,
    pub redis_url: String,
    pub r2_url: String,
    pub r2_access_key: String,
    pub r2_secret: String,
    pub r2_bucket: String,
    pub r2_file_path: String,
    pub hash_key: String,
    pub origin: Option<String>,
    pub allowed_auth_origins: HashSet<String>,
    pub cookie_secure: bool,
    pub email_subject_prefix: Option<String>,
    pub rate_limit: RateLimitConfig,
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub google_token_url: String,
    pub google_userinfo_url: String,
    pub lms_base_url: String,
    pub mail: Option<MailConfig>,
    pub log_tail_source_key: Option<String>,
    pub log_tail_host: String,
    pub service_name: String,
    pub commit_sha: Option<String>,
    pub schema_check: String,
    pub cache_prefix: String,
    pub cache_clear_legacy: bool,
    pub max_request_body: usize,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Config> {
        let database_url = match var("DATABASE_URL") {
            Some(url) => url,
            None => {
                let host = required("DATABASE_HOST")?;
                let port = var("DATABASE_PORT").unwrap_or_else(|| "5432".into());
                let user = required("DATABASE_USERNAME")?;
                let password = var("DATABASE_PASSWORD").unwrap_or_default();
                let database = required("DATABASE")?;
                format!(
                    "postgres://{}:{}@{}:{}/{}",
                    percent_encode(&user),
                    percent_encode(&password),
                    host,
                    port,
                    database
                )
            }
        };

        let redis_host = required("REDIS_HOST")?;
        let redis_port = var("REDIS_PORT").unwrap_or_else(|| "6379".into());
        let redis_password = var("REDIS_PASSWORD").filter(|v| !v.is_empty());
        let redis_url = match redis_password {
            Some(password) => format!(
                "redis://:{}@{}:{}/0",
                percent_encode(&password),
                redis_host,
                redis_port
            ),
            None => format!("redis://{redis_host}:{redis_port}/0"),
        };

        let mut trusted_proxies = Vec::new();
        if let Some(value) = var("AUTH_RATE_LIMIT_TRUSTED_PROXY_CIDRS") {
            for item in value.split(',').map(str::trim).filter(|i| !i.is_empty()) {
                match item.parse::<IpNet>() {
                    Ok(net) => trusted_proxies.push(net),
                    Err(_) => tracing::warn!("Ignoring invalid trusted proxy CIDR"),
                }
            }
        }

        let mail = match (
            var("AWS_EMAIL_SENDER").filter(|v| !v.is_empty()),
            var("AWS_ACCESS_KEY").filter(|v| !v.is_empty()),
            var("AWS_ACCESS_SECRET").filter(|v| !v.is_empty()),
        ) {
            (Some(sender), Some(access_key), Some(secret)) => Some(MailConfig {
                sender,
                access_key,
                secret,
                endpoint: var("AWS_SES_ENDPOINT").filter(|v| !v.is_empty()),
            }),
            _ => None,
        };

        Ok(Config {
            bind: var("BIND_ADDRESS").unwrap_or_else(|| "0.0.0.0".into()),
            port: var("PORT").and_then(|v| v.parse().ok()).unwrap_or(8080),
            database_url,
            database_max_connections: var("DATABASE_MAX_CONNECTIONS")
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            redis_url,
            r2_url: required("R2_URL")?,
            r2_access_key: required("R2_ACCESS_KEY")?,
            r2_secret: required("R2_ACCESS_TOKEN")?,
            r2_bucket: required("R2_BUCKET_NAME")?,
            r2_file_path: required("R2_FILE_PATH")?.trim_end_matches('/').to_string(),
            hash_key: required("HASH_KEY").context("HASH_KEY is required")?,
            origin: var("ORIGIN").filter(|v| !v.is_empty()),
            allowed_auth_origins: allowed_auth_origins_from_env(),
            cookie_secure: env_bool("AUTH_COOKIE_SECURE", true),
            email_subject_prefix: var("EMAIL_SUBJECT_PREFIX"),
            rate_limit: RateLimitConfig {
                subject_limit: positive_int("AUTH_RATE_LIMIT_REQUESTS", 10),
                global_limit: positive_int("AUTH_RATE_LIMIT_GLOBAL_REQUESTS", 300),
                window_seconds: positive_int("AUTH_RATE_LIMIT_WINDOW_SECONDS", 60),
                // The reference only lowercases this flag (no trim).
                trust_proxy: var("AUTH_RATE_LIMIT_TRUST_PROXY_HEADERS")
                    .map(|v| trust_proxy_flag_value(&v))
                    .unwrap_or(false),
                trusted_proxies,
            },
            google_client_id: var("GOOGLE_SERVICE_CLIENT_ID").filter(|v| !v.is_empty()),
            google_client_secret: var("GOOGLE_SERVICE_SERCET").filter(|v| !v.is_empty()),
            google_token_url: var("GOOGLE_TOKEN_URL")
                .unwrap_or_else(|| "https://oauth2.googleapis.com/token".into()),
            google_userinfo_url: var("GOOGLE_USERINFO_URL")
                .unwrap_or_else(|| "https://www.googleapis.com/oauth2/v1/userinfo".into()),
            lms_base_url: var("LMS_BASE_URL")
                .unwrap_or_else(|| "https://cof.ntpu.edu.tw".into())
                .trim_end_matches('/')
                .to_string(),
            mail,
            log_tail_source_key: var("LOG_TAIL_SOURCE_KEY").filter(|v| !v.is_empty()),
            log_tail_host: var("LOG_TAIL_HOST")
                .unwrap_or_else(|| "https://in.logs.betterstack.com".into()),
            service_name: var("SERVICE_NAME").unwrap_or_else(|| "local".into()),
            commit_sha: var("COMMIT_SHA"),
            schema_check: var("SCHEMA_CHECK").unwrap_or_else(|| "strict".into()),
            cache_prefix: var("CACHE_PREFIX").unwrap_or_else(|| "pe-cache:v3".into()),
            cache_clear_legacy: env_bool("CACHE_CLEAR_LEGACY_PREFIX", true),
            max_request_body: var("MAX_REQUEST_BODY_BYTES")
                .and_then(|v| v.parse().ok())
                .unwrap_or(60 * 1024 * 1024),
        })
    }

    pub fn origin_or_empty(&self) -> String {
        self.origin.clone().unwrap_or_default()
    }
}

/// Production names look like `PROD`, `production`, `service-production`.
pub(crate) fn is_production(service_name: &str) -> bool {
    service_name
        .split(|character: char| !character.is_alphanumeric())
        .any(|token| token.eq_ignore_ascii_case("prod") || token.eq_ignore_ascii_case("production"))
}

fn percent_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
