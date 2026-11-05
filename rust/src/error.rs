use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use serde_json::{json, Value};
use thiserror::Error;

#[derive(Debug, Clone, Serialize)]
pub struct ValidationError {
    #[serde(rename = "type")]
    pub kind: String,
    pub loc: Vec<Value>,
    pub msg: String,
    pub input: Value,
}

impl ValidationError {
    pub fn new(kind: &str, loc: &[&str], msg: String, input: Value) -> Self {
        ValidationError {
            kind: kind.to_string(),
            loc: loc.iter().map(|s| json!(s)).collect(),
            msg,
            input,
        }
    }

    pub fn missing(loc: &[&str]) -> Self {
        Self::new("missing", loc, "Field required".into(), Value::Null)
    }

    pub fn string_too_short(loc: &[&str], min: usize, input: Value) -> Self {
        let plural = if min == 1 { "character" } else { "characters" };
        Self::new(
            "string_too_short",
            loc,
            format!("String should have at least {min} {plural}"),
            input,
        )
    }

    pub fn string_too_long(loc: &[&str], max: usize, input: Value) -> Self {
        let plural = if max == 1 { "character" } else { "characters" };
        Self::new(
            "string_too_long",
            loc,
            format!("String should have at most {max} {plural}"),
            input,
        )
    }

    pub fn int_parsing(loc: &[&str], input: Value) -> Self {
        Self::new(
            "int_parsing",
            loc,
            "Input should be a valid integer, unable to parse string as an integer".into(),
            input,
        )
    }

    pub fn bool_parsing(loc: &[&str], input: Value) -> Self {
        Self::new(
            "bool_parsing",
            loc,
            "Input should be a valid boolean, unable to interpret input".into(),
            input,
        )
    }

    pub fn greater_than_equal(loc: &[&str], min: i64, input: Value) -> Self {
        Self::new(
            "greater_than_equal",
            loc,
            format!("Input should be greater than or equal to {min}"),
            input,
        )
    }

    pub fn less_than_equal(loc: &[&str], max: i64, input: Value) -> Self {
        Self::new(
            "less_than_equal",
            loc,
            format!("Input should be less than or equal to {max}"),
            input,
        )
    }

    pub fn string_pattern_mismatch(loc: &[&str], pattern: &str, input: Value) -> Self {
        Self::new(
            "string_pattern_mismatch",
            loc,
            format!("String should match pattern '{pattern}'"),
            input,
        )
    }
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("{detail}")]
    Detail { status: StatusCode, detail: String },
    #[error("{detail}")]
    DetailHeaders {
        status: StatusCode,
        detail: String,
        headers: Vec<(&'static str, String)>,
    },
    #[error("request validation failed")]
    Validation(Vec<ValidationError>),
    /// Unhandled failure: log it and return a plain-text 500 without CORS headers,
    /// matching Starlette's ServerErrorMiddleware.
    #[error("{0}")]
    Internal(String),
}

pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
    pub fn new(status: StatusCode, detail: impl Into<String>) -> Self {
        ApiError::Detail {
            status,
            detail: detail.into(),
        }
    }

    pub fn bad_request(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, detail)
    }

    pub fn unauthorized(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, detail)
    }

    pub fn forbidden(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, detail)
    }

    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, detail)
    }

    pub fn gone(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::GONE, detail)
    }

    pub fn too_large(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::PAYLOAD_TOO_LARGE, detail)
    }

    pub fn unavailable(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, detail)
    }

    pub fn with_headers(
        status: StatusCode,
        detail: impl Into<String>,
        headers: Vec<(&'static str, String)>,
    ) -> Self {
        ApiError::DetailHeaders {
            status,
            detail: detail.into(),
            headers,
        }
    }

    pub fn validation(error: ValidationError) -> Self {
        ApiError::Validation(vec![error])
    }

    pub fn internal(error: impl std::fmt::Display) -> Self {
        ApiError::Internal(error.to_string())
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        ApiError::internal(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            ApiError::Detail { status, detail } => {
                (status, Json(json!({ "detail": detail }))).into_response()
            }
            ApiError::DetailHeaders {
                status,
                detail,
                headers,
            } => {
                let mut response = (status, Json(json!({ "detail": detail }))).into_response();
                for (name, value) in headers {
                    if let Ok(value) = value.parse() {
                        response.headers_mut().insert(name, value);
                    }
                }
                response
            }
            ApiError::Validation(errors) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({ "detail": errors })),
            )
                .into_response(),
            ApiError::Internal(message) => {
                tracing::error!(error = %message, "unhandled error");
                let mut response = (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [("content-type", "text/plain; charset=utf-8")],
                    "Internal Server Error",
                )
                    .into_response();
                response
                    .headers_mut()
                    .insert("x-fatal", HeaderValue::from_static("1"));
                response
            }
        }
    }
}
