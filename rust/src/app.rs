use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::header::{
    ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS,
    ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_MAX_AGE, ACCESS_CONTROL_REQUEST_HEADERS,
    ACCESS_CONTROL_REQUEST_METHOD, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, ORIGIN, VARY,
};
use axum::http::{HeaderValue, Method, StatusCode};
use axum::middleware::{from_fn, from_fn_with_state, Next};
use axum::response::{IntoResponse, Response};
use axum::Router;
use redis::aio::ConnectionManager;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tower_http::catch_panic::CatchPanicLayer;

use crate::cache::Cache;
use crate::config::Config;
use crate::error::ApiError;
use crate::mail::Mailer;
use crate::routes;
use crate::schema::{self, SchemaStatus};
use crate::storage::Storage;

const STATIC_CORS_ORIGINS: [&str; 6] = [
    "https://past-exam.zeabur.app",
    "https://past-exam.ntpu.cc",
    "https://past-exam.ntpu.xyz",
    "http://localhost",
    "http://localhost:5173",
    "http://localhost:8080",
];

pub struct AppState {
    pub config: Arc<Config>,
    pub db: PgPool,
    pub redis: ConnectionManager,
    pub cache: Cache,
    pub storage: Storage,
    pub mailer: Mailer,
    pub http: reqwest::Client,
}

impl AppState {
    pub async fn connect(config: Config) -> anyhow::Result<Arc<AppState>> {
        let db = PgPoolOptions::new()
            .max_connections(config.database_max_connections)
            .acquire_timeout(Duration::from_secs(10))
            .connect(&config.database_url)
            .await
            .context("connect to PostgreSQL")?;
        let redis_client = redis::Client::open(config.redis_url.as_str())
            .context("invalid REDIS configuration")?;
        let redis = redis_client
            .get_connection_manager()
            .await
            .context("connect to Redis")?;
        let cache = Cache::new(
            redis.clone(),
            config.cache_prefix.clone(),
            config.cache_clear_legacy,
        );
        let storage = Storage::new(&config).context("configure object storage")?;
        let mailer = Mailer::new(&config);
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .context("build HTTP client")?;
        Ok(Arc::new(AppState {
            config: Arc::new(config),
            db,
            redis,
            cache,
            storage,
            mailer,
            http,
        }))
    }

    pub async fn check_schema(&self) -> anyhow::Result<()> {
        if self.config.schema_check == "off" {
            return Ok(());
        }
        let mut attempts = 0;
        loop {
            match sqlx::query_scalar::<_, String>("SELECT version_num FROM alembic_version LIMIT 1")
                .fetch_optional(&self.db)
                .await
            {
                Ok(version) => {
                    let message = match schema::classify(version.as_deref()) {
                        SchemaStatus::Current => return Ok(()),
                        SchemaStatus::Ahead => {
                            tracing::warn!(
                                version = version.as_deref().unwrap_or_default(),
                                head = schema::head(),
                                "database schema is newer than this build; continuing"
                            );
                            return Ok(());
                        }
                        SchemaStatus::Behind => format!(
                            "database schema {} is behind {}; run alembic upgrade head first",
                            version.as_deref().unwrap_or_default(),
                            schema::head()
                        ),
                        SchemaStatus::Missing => "alembic_version table is empty".to_string(),
                    };
                    if self.config.schema_check == "warn" {
                        tracing::warn!("{message}");
                        return Ok(());
                    }
                    bail!("{message}");
                }
                Err(error) => {
                    attempts += 1;
                    if attempts >= 30 {
                        return Err(error).context("check database schema version");
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .merge(routes::health::router())
        .merge(routes::auth::router())
        .merge(routes::users::router())
        .merge(routes::departments::router())
        .merge(routes::courses::router())
        .merge(routes::bulletins::router())
        .merge(routes::posts::router())
        .merge(routes::threads::router())
        .fallback(not_found)
        .with_state(state.clone())
        .layer(DefaultBodyLimit::disable())
        .layer(from_fn(fix_method_not_allowed))
        .layer(from_fn_with_state(state.clone(), request_log))
        .layer(from_fn(no_store))
        .layer(from_fn_with_state(state.clone(), cors))
        .layer(CatchPanicLayer::custom(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [("content-type", "text/plain; charset=utf-8")],
                "Internal Server Error",
            )
                .into_response()
        }))
}

async fn not_found() -> ApiError {
    ApiError::not_found("Not Found")
}

async fn fix_method_not_allowed(req: Request, next: Next) -> Response {
    let response = next.run(req).await;
    if response.status() == StatusCode::METHOD_NOT_ALLOWED {
        let (mut parts, _) = response.into_parts();
        parts.headers.remove(CONTENT_LENGTH);
        parts
            .headers
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        return Response::from_parts(parts, Body::from(r#"{"detail":"Method Not Allowed"}"#));
    }
    response
}

async fn request_log(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let start = Instant::now();
    let response = next.run(req).await;
    tracing::info!(
        target: "request",
        env = %state.config.service_name,
        method = %method,
        path = %path,
        status = response.status().as_u16(),
        elapsed_ms = start.elapsed().as_secs_f64() * 1000.0,
        "request"
    );
    response
}

async fn no_store(req: Request, next: Next) -> Response {
    let is_get = req.method() == Method::GET;
    let mut response = next.run(req).await;
    if is_get && response.status().is_success() && !response.headers().contains_key("x-fatal") {
        response
            .headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
}

fn origin_allowed(state: &AppState, origin: &str) -> bool {
    STATIC_CORS_ORIGINS.contains(&origin) || state.config.allowed_auth_origins.contains(origin)
}

/// Starlette-compatible CORS: disallowed preflights answer 400 with a body
/// (tower-http's CorsLayer silently answers 200) and 500s never get CORS headers.
async fn cors(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let origin = req
        .headers()
        .get(ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let is_preflight = req.method() == Method::OPTIONS
        && origin.is_some()
        && req.headers().contains_key(ACCESS_CONTROL_REQUEST_METHOD);

    if is_preflight {
        let origin = origin.unwrap_or_default();
        let allowed = origin_allowed(&state, &origin);
        let mut response = if allowed {
            (StatusCode::OK, "OK").into_response()
        } else {
            (StatusCode::BAD_REQUEST, "Disallowed CORS origin").into_response()
        };
        let headers = response.headers_mut();
        headers.insert(
            ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static("DELETE, GET, HEAD, OPTIONS, PATCH, POST, PUT"),
        );
        headers.insert(ACCESS_CONTROL_MAX_AGE, HeaderValue::from_static("600"));
        headers.insert(
            ACCESS_CONTROL_ALLOW_CREDENTIALS,
            HeaderValue::from_static("true"),
        );
        if allowed {
            if let Ok(value) = HeaderValue::from_str(&origin) {
                headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, value);
            }
        }
        if let Some(requested) = req.headers().get(ACCESS_CONTROL_REQUEST_HEADERS) {
            headers.insert(ACCESS_CONTROL_ALLOW_HEADERS, requested.clone());
        }
        headers.insert(VARY, HeaderValue::from_static("Origin"));
        return response;
    }

    let mut response = next.run(req).await;
    if let Some(origin) = origin {
        if origin_allowed(&state, &origin) && !response.headers().contains_key("x-fatal") {
            if let Ok(value) = HeaderValue::from_str(&origin) {
                response
                    .headers_mut()
                    .insert(ACCESS_CONTROL_ALLOW_ORIGIN, value);
            }
            response.headers_mut().insert(
                ACCESS_CONTROL_ALLOW_CREDENTIALS,
                HeaderValue::from_static("true"),
            );
            response
                .headers_mut()
                .insert(VARY, HeaderValue::from_static("Origin"));
        }
    }
    response.headers_mut().remove("x-fatal");
    response
}
