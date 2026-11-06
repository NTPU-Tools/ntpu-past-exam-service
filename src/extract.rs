use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{FromRequest, Multipart, Request};
use axum::http::{header::CONTENT_TYPE, Uri};
use serde_json::{json, Value};
use url::form_urlencoded;

use crate::app::AppState;
use crate::error::{ApiError, ApiResult, ValidationError};

const MAX_TEXT_FIELD_BYTES: usize = 1024 * 1024;
const MAX_PARTS: usize = 1000;

#[derive(Debug, Clone)]
pub struct UploadedFile {
    pub field: String,
    pub content_type: Option<String>,
    pub data: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct FormData {
    pub fields: Vec<(String, String)>,
    pub files: Vec<UploadedFile>,
}

impl FormData {
    pub fn last(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    pub fn files_named(&self, name: &str) -> Vec<&UploadedFile> {
        self.files
            .iter()
            .filter(|file| file.field == name)
            .collect()
    }
}

/// FastAPI-compatible form extraction: urlencoded and multipart are both
/// accepted; other content types are treated as an empty form.
pub struct RawForm(pub FormData);

impl FromRequest<Arc<AppState>> for RawForm {
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &Arc<AppState>) -> Result<Self, Self::Rejection> {
        let state = state.as_ref();
        let content_type = req
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
        let max_body = state.config.max_request_body;

        if content_type.starts_with("multipart/form-data") {
            let multipart = Multipart::from_request(req, state)
                .await
                .map_err(|_| ApiError::bad_request("Invalid multipart body"))?;
            return parse_multipart(multipart, max_body).await.map(RawForm);
        }

        if content_type.starts_with("application/x-www-form-urlencoded") {
            let bytes = axum::body::to_bytes(req.into_body(), max_body)
                .await
                .map_err(|_| ApiError::too_large("Request body too large"))?;
            let fields = form_urlencoded::parse(&bytes)
                .map(|(key, value)| (key.into_owned(), value.into_owned()))
                .collect();
            return Ok(RawForm(FormData {
                fields,
                files: Vec::new(),
            }));
        }

        Ok(RawForm(FormData::default()))
    }
}

async fn parse_multipart(mut multipart: Multipart, max_body: usize) -> ApiResult<FormData> {
    let mut form = FormData::default();
    let mut total_bytes = 0usize;
    let mut parts = 0usize;
    while let Some(mut field) = multipart.next_field().await.map_err(|error| {
        tracing::warn!(%error, "multipart next_field failed");
        ApiError::bad_request("Invalid multipart body")
    })? {
        parts += 1;
        if parts > MAX_PARTS {
            return Err(ApiError::bad_request("Too many form parts"));
        }
        let name = field.name().unwrap_or_default().to_string();
        let filename = field.file_name().map(str::to_string);
        let content_type = field.content_type().map(str::to_string);
        let is_file = filename.is_some();
        let mut data = Vec::new();
        while let Some(chunk) = field.chunk().await.map_err(|error| {
            tracing::warn!(%error, "multipart chunk failed");
            ApiError::bad_request("Invalid multipart body")
        })? {
            total_bytes += chunk.len();
            if total_bytes > max_body {
                return Err(ApiError::too_large("Request body too large"));
            }
            if !is_file && data.len() + chunk.len() > MAX_TEXT_FIELD_BYTES {
                return Err(ApiError::bad_request(
                    "Part exceeded the maximum field size",
                ));
            }
            data.extend_from_slice(&chunk);
        }
        if is_file {
            form.files.push(UploadedFile {
                field: name,
                content_type,
                data,
            });
        } else {
            form.fields
                .push((name, String::from_utf8_lossy(&data).into_owned()));
        }
    }
    Ok(form)
}

/// JSON body extractor for `PUT /users/me/preferences`.
pub struct JsonBody(pub Value);

impl FromRequest<Arc<AppState>> for JsonBody {
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &Arc<AppState>) -> Result<Self, Self::Rejection> {
        let bytes = axum::body::to_bytes(req.into_body(), state.config.max_request_body)
            .await
            .map_err(|_| ApiError::too_large("Request body too large"))?;
        match serde_json::from_slice::<Value>(&bytes) {
            Ok(value) => Ok(JsonBody(value)),
            Err(error) => Err(ApiError::Validation(vec![ValidationError {
                kind: "json_invalid".into(),
                loc: vec![json!("body"), json!(error.line())],
                msg: "JSON decode error".into(),
                input: json!({}),
            }])),
        }
    }
}

pub struct Fields<'a> {
    data: &'a FormData,
    errors: Vec<ValidationError>,
}

impl<'a> Fields<'a> {
    pub fn new(data: &'a FormData) -> Self {
        Fields {
            data,
            errors: Vec::new(),
        }
    }

    fn push(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    fn check_len(&mut self, name: &str, value: &str, min: usize, max: Option<usize>) {
        let loc = ["body", name];
        let length = value.chars().count();
        if length < min {
            self.push(ValidationError::string_too_short(&loc, min, json!(value)));
        }
        if let Some(max) = max {
            if length > max {
                self.push(ValidationError::string_too_long(&loc, max, json!(value)));
            }
        }
    }

    pub fn string(
        &mut self,
        name: &str,
        min: usize,
        max: Option<usize>,
        required: bool,
        default: &str,
    ) -> String {
        match self.data.last(name) {
            Some(value) => {
                let value = value.to_string();
                self.check_len(name, &value, min, max);
                value
            }
            None if required => {
                self.push(ValidationError::missing(&["body", name]));
                String::new()
            }
            None => default.to_string(),
        }
    }

    pub fn optional(&mut self, name: &str, min: usize, max: Option<usize>) -> Option<String> {
        self.data.last(name).map(|value| {
            let value = value.to_string();
            self.check_len(name, &value, min, max);
            value
        })
    }

    pub fn boolean(&mut self, name: &str, default: bool) -> bool {
        match self.data.last(name) {
            Some(raw) => match parse_bool(raw) {
                Some(value) => value,
                None => {
                    self.push(ValidationError::bool_parsing(&["body", name], json!(raw)));
                    default
                }
            },
            None => default,
        }
    }

    pub fn required_boolean(&mut self, name: &str) -> bool {
        match self.data.last(name) {
            Some(raw) => match parse_bool(raw) {
                Some(value) => value,
                None => {
                    self.push(ValidationError::bool_parsing(&["body", name], json!(raw)));
                    false
                }
            },
            None => {
                self.push(ValidationError::missing(&["body", name]));
                false
            }
        }
    }

    pub fn grant_type(&mut self) {
        if let Some(raw) = self.data.last("grant_type") {
            if raw != "password" {
                self.push(ValidationError::string_pattern_mismatch(
                    &["body", "grant_type"],
                    "^password$",
                    json!(raw),
                ));
            }
        }
    }

    pub fn finish<T>(self, value: T) -> ApiResult<T> {
        if self.errors.is_empty() {
            Ok(value)
        } else {
            Err(ApiError::Validation(self.errors))
        }
    }
}

pub fn parse_bool(raw: &str) -> Option<bool> {
    match raw.to_lowercase().as_str() {
        "1" | "on" | "t" | "true" | "y" | "yes" => Some(true),
        "0" | "off" | "f" | "false" | "n" | "no" => Some(false),
        _ => None,
    }
}

pub fn json_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(value) => Some(*value),
        Value::Number(number) => match number.as_i64() {
            Some(0) => Some(false),
            Some(1) => Some(true),
            _ => None,
        },
        Value::String(value) => parse_bool(value),
        _ => None,
    }
}

pub fn query_pairs(uri: &Uri) -> Vec<(String, String)> {
    uri.query()
        .map(|query| {
            form_urlencoded::parse(query.as_bytes())
                .map(|(key, value)| (key.into_owned(), value.into_owned()))
                .collect()
        })
        .unwrap_or_default()
}

pub fn query_get(uri: &Uri, name: &str) -> Option<String> {
    query_pairs(uri)
        .into_iter()
        .rev()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value)
}

pub fn query_map(uri: &Uri) -> HashMap<String, String> {
    query_pairs(uri).into_iter().collect()
}

/// pydantic v2 lax integer parsing (see testdata/vectors/parsing.json).
pub fn parse_int_lax(raw: &str) -> Option<i64> {
    let bytes = raw.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let (negative, rest) = match bytes[0] {
        b'+' => (false, &bytes[1..]),
        b'-' => (true, &bytes[1..]),
        _ => (false, bytes),
    };
    if rest.is_empty() {
        return None;
    }
    let mut digits = String::new();
    let mut previous_was_digit = false;
    let mut index = 0;
    while index < rest.len() {
        let byte = rest[index];
        if byte.is_ascii_digit() {
            digits.push(byte as char);
            previous_was_digit = true;
            index += 1;
        } else if byte == b'_'
            && previous_was_digit
            && index + 1 < rest.len()
            && rest[index + 1].is_ascii_digit()
        {
            previous_was_digit = false;
            index += 1;
        } else if byte == b'.' && previous_was_digit {
            let fraction = &rest[index + 1..];
            if fraction.is_empty() || !fraction.iter().all(|b| *b == b'0') {
                return None;
            }
            index = rest.len();
        } else {
            return None;
        }
    }
    if digits.is_empty() {
        return None;
    }
    let mut magnitude: i128 = 0;
    for digit in digits.bytes() {
        magnitude = magnitude
            .saturating_mul(10)
            .saturating_add(i128::from(digit - b'0'));
    }
    let signed = if negative { -magnitude } else { magnitude };
    Some(signed.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64)
}

pub fn strict_pagination(uri: &Uri) -> ApiResult<(i64, i64)> {
    let skip = match query_get(uri, "skip") {
        Some(raw) => {
            let value = parse_int_lax(&raw).ok_or_else(|| {
                ApiError::validation(ValidationError::int_parsing(&["query", "skip"], json!(raw)))
            })?;
            if value < 0 {
                return Err(ApiError::validation(ValidationError::greater_than_equal(
                    &["query", "skip"],
                    0,
                    json!(value),
                )));
            }
            if value > 10000 {
                return Err(ApiError::validation(ValidationError::less_than_equal(
                    &["query", "skip"],
                    10000,
                    json!(value),
                )));
            }
            value
        }
        None => 0,
    };
    let limit = match query_get(uri, "limit") {
        Some(raw) => {
            let value = parse_int_lax(&raw).ok_or_else(|| {
                ApiError::validation(ValidationError::int_parsing(
                    &["query", "limit"],
                    json!(raw),
                ))
            })?;
            if value < 1 {
                return Err(ApiError::validation(ValidationError::greater_than_equal(
                    &["query", "limit"],
                    1,
                    json!(value),
                )));
            }
            if value > 100 {
                return Err(ApiError::validation(ValidationError::less_than_equal(
                    &["query", "limit"],
                    100,
                    json!(value),
                )));
            }
            value
        }
        None => 100,
    };
    Ok((skip, limit))
}

pub fn clamped_pagination(uri: &Uri) -> ApiResult<(i64, i64)> {
    let skip = match query_get(uri, "skip") {
        Some(raw) => parse_int_lax(&raw)
            .ok_or_else(|| {
                ApiError::validation(ValidationError::int_parsing(&["query", "skip"], json!(raw)))
            })?
            .clamp(0, 10000),
        None => 0,
    };
    let limit = match query_get(uri, "limit") {
        Some(raw) => parse_int_lax(&raw)
            .ok_or_else(|| {
                ApiError::validation(ValidationError::int_parsing(
                    &["query", "limit"],
                    json!(raw),
                ))
            })?
            .clamp(1, 100),
        None => 100,
    };
    Ok((skip, limit))
}

pub fn query_bool(uri: &Uri, name: &str) -> ApiResult<Option<bool>> {
    match query_get(uri, name) {
        Some(raw) => match parse_bool(&raw) {
            Some(value) => Ok(Some(value)),
            None => Err(ApiError::validation(ValidationError::bool_parsing(
                &["query", name],
                json!(raw),
            ))),
        },
        None => Ok(None),
    }
}
