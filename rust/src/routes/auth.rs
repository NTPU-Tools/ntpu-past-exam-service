use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{ConnectInfo, State};
use axum::http::header::{CACHE_CONTROL, PRAGMA, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde_json::{json, Value};
use unicode_general_category::get_general_category;
use unicode_general_category::GeneralCategory;
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::extractor::AuthUser;
use crate::auth::{cookie, google, jwt, lms, password, rate_limit, session};
use crate::authz;
use crate::config::{REFRESH_COOKIE_NAME, REFRESH_TOKEN_EXPIRE_DAYS};
use crate::db::{self, UserRow};
use crate::dto::{AuthTokenDto, CreateUserDto, StatusDto, VerifyTokenDto};
use crate::error::{ApiError, ApiResult};
use crate::extract::{Fields, RawForm};
use crate::routes::mark_no_store;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/login", post(login))
        .route("/exchange", post(exchange))
        .route("/verify-token", get(verify_token))
        .route("/refresh", post(refresh))
        .route("/refresh/logout", post(refresh_logout))
        .route("/logout", post(logout))
        .route("/create_user", post(create_user))
}

fn session_ttl() -> i64 {
    REFRESH_TOKEN_EXPIRE_DAYS * 24 * 60 * 60
}

async fn login(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    form: RawForm,
) -> ApiResult<Response> {
    cookie::validate_request_origin(&state.config, &headers, false)?;
    let subject = form
        .0
        .last("username")
        .filter(|username| !username.is_empty())
        .map(|username| format!("login:{}", username.trim()));
    rate_limit::enforce(&state, peer.ip(), &headers, subject.as_deref()).await?;

    let mut fields = Fields::new(&form.0);
    let username = fields.string("username", 0, None, true, "");
    let password = fields.string("password", 0, None, true, "");
    fields.grant_type();
    fields.finish(())?;

    let user = login_user(&state, &username, &password).await?;
    let (access, refresh) = issue_session(&state, &user).await?;
    Ok(token_response(&state, access, refresh))
}

async fn exchange(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    form: RawForm,
) -> ApiResult<Response> {
    cookie::validate_request_origin(&state.config, &headers, false)?;
    rate_limit::enforce(&state, peer.ip(), &headers, None).await?;

    let mut fields = Fields::new(&form.0);
    let code = fields.string("code", 0, None, true, "");
    let redirect_uri = fields.string("redirect_uri", 0, None, true, "");
    fields.finish(())?;

    let user = google_login(&state, &code, &redirect_uri).await?;
    let (access, refresh) = issue_session(&state, &user).await?;
    Ok(token_response(&state, access, refresh))
}

async fn verify_token(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> ApiResult<Response> {
    let admin = authz::admin_department_ids(&state, &user).await?;
    let visible_departments = authz::visible_department_ids(&state, &user).await?;
    let mut response = Json(VerifyTokenDto {
        admin,
        visible_departments,
    })
    .into_response();
    mark_no_store(&mut response);
    Ok(response)
}

async fn refresh(State(state): State<Arc<AppState>>, headers: HeaderMap) -> ApiResult<Response> {
    let cookie_token = cookie::request_cookie(&headers, REFRESH_COOKIE_NAME);
    let token = match &cookie_token {
        Some(token) => {
            cookie::validate_browser_mutation(&state.config, &headers)?;
            token.clone()
        }
        None => crate::auth::extractor::bearer_token_credentials(&headers)?,
    };

    let payload = jwt::decode(&state.config, &token, "refresh")?;
    let managed = session::is_managed(&payload);
    let legacy = session::is_legacy(&payload);

    if !managed {
        if !legacy {
            return Err(ApiError::unauthorized("Unknown refresh session"));
        }
        if cookie_token.is_some() || !state.config.legacy_refresh_bearer_enabled(Utc::now()) {
            return Err(ApiError::unauthorized(
                "Legacy refresh bearer migration is disabled",
            ));
        }
    }

    let user = crate::auth::extractor::resolve_from_claims(&state, &payload).await?;
    let existing_session = if managed {
        payload
            .get("sid")
            .and_then(Value::as_str)
            .map(str::to_string)
    } else {
        None
    };
    let admin_ids = authz::admin_department_ids(&state, &user).await?;
    let tokens = jwt::build_tokens(&state.config, &user, &admin_ids, existing_session)?;

    if managed {
        session::rotate(
            &state.redis,
            &tokens.session_id,
            payload
                .get("jti")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            &tokens.refresh_jti,
            session_ttl(),
        )
        .await?;
    } else {
        let legacy_ttl = session::token_ttl(&payload)?;
        session::migrate_legacy(
            &state.redis,
            &token,
            legacy_ttl,
            &tokens.session_id,
            &tokens.refresh_jti,
            session_ttl(),
        )
        .await?;
    }

    Ok(token_response(
        &state,
        tokens.access_token,
        tokens.refresh_token,
    ))
}

async fn refresh_logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    cookie::validate_browser_mutation(&state.config, &headers)?;
    if let Some(token) = cookie::request_cookie(&headers, REFRESH_COOKIE_NAME) {
        if let Ok(payload) = jwt::decode(&state.config, &token, "refresh") {
            session::revoke(&state.redis, &payload, &token).await?;
        }
    }
    Ok(deleted_session_response(&state))
}

async fn logout(State(state): State<Arc<AppState>>, headers: HeaderMap) -> ApiResult<Response> {
    cookie::validate_browser_mutation(&state.config, &headers)?;
    Ok(deleted_session_response(&state))
}

async fn create_user(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    form: RawForm,
) -> ApiResult<Response> {
    authz::require_super(&user)?;
    let mut fields = Fields::new(&form.0);
    let username = fields.string("username", 0, None, true, "");
    let password = fields.string("password", 0, None, true, "");
    fields.grant_type();
    fields.finish(())?;
    if username.is_empty() || password.is_empty() {
        return Err(ApiError::bad_request("Invalid input"));
    }
    if db::get_user_by_username(&state.db, &username)
        .await?
        .is_some()
    {
        return Err(ApiError::bad_request("Same username already exist."));
    }
    let hashed = password::hash(&password)?;
    let created = create_user_record(
        &state,
        &username,
        Some(&username),
        None,
        None,
        Some(&hashed),
    )
    .await?;
    let mut response = Json(CreateUserDto {
        status: "success",
        user_id: created.id,
    })
    .into_response();
    mark_no_store(&mut response);
    Ok(response)
}

pub fn token_response(state: &AppState, access_token: String, refresh_token: String) -> Response {
    let mut response = Json(AuthTokenDto::bearer(access_token)).into_response();
    if let Ok(value) = HeaderValue::from_str(&cookie::refresh_cookie(&state.config, &refresh_token))
    {
        response.headers_mut().insert(SET_COOKIE, value);
    }
    mark_no_store(&mut response);
    response
}

fn deleted_session_response(state: &AppState) -> Response {
    let mut response = Json(StatusDto { status: "success" }).into_response();
    if let Ok(value) = HeaderValue::from_str(&cookie::delete_cookie(&state.config)) {
        response.headers_mut().insert(SET_COOKIE, value);
    }
    // Keep headers explicit: the response is not a cacheable GET either way.
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .insert(PRAGMA, HeaderValue::from_static("no-cache"));
    response
}

async fn issue_session(state: &AppState, user: &UserRow) -> ApiResult<(String, String)> {
    let admin_ids = authz::admin_department_ids(state, user).await?;
    let tokens = jwt::build_tokens(&state.config, user, &admin_ids, None)?;
    session::register(
        &state.redis,
        &tokens.session_id,
        &tokens.refresh_jti,
        session_ttl(),
    )
    .await?;
    Ok((tokens.access_token, tokens.refresh_token))
}

async fn login_user(state: &AppState, username: &str, password: &str) -> ApiResult<UserRow> {
    let existing = db::get_user_by_username(&state.db, username).await?;

    if let Some(lms_username) = python_int_string(username) {
        let info = lms::get_user_info(
            &state.http,
            &state.config.lms_base_url,
            &lms_username,
            password,
        )
        .await?;
        return upsert_lms_user(state, username, &info).await;
    }

    let user = existing.ok_or_else(|| ApiError::bad_request("incorrect username or password"))?;
    let Some(hashed) = user.hashed_password.as_deref() else {
        return Err(ApiError::bad_request("incorrect username or password"));
    };
    if !password::verify(password, hashed) {
        return Err(ApiError::bad_request("incorrect username or password"));
    }
    Ok(user)
}

async fn upsert_lms_user(
    state: &AppState,
    username: &str,
    info: &lms::LmsUser,
) -> ApiResult<UserRow> {
    match db::get_user_by_username(&state.db, username).await? {
        Some(user) => {
            sqlx::query(
                "UPDATE users SET readable_name = $2, school_department = $3, email = $4, \
                 updated_time = now() WHERE id = $1",
            )
            .bind(&user.id)
            .bind(&info.readable_name)
            .bind(&info.department)
            .bind(&info.email)
            .execute(&state.db)
            .await?;
            db::get_user(&state.db, &user.id)
                .await?
                .ok_or_else(|| ApiError::internal("user disappeared after update"))
        }
        None => {
            create_user_record(
                state,
                username,
                Some(&info.readable_name),
                Some(&info.department),
                info.email.as_deref(),
                None,
            )
            .await
        }
    }
}

async fn google_login(state: &AppState, code: &str, redirect_uri: &str) -> ApiResult<UserRow> {
    let data = google::exchange(&state.http, &state.config, code, redirect_uri).await?;
    let email = data
        .get("email")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::forbidden("Forbidden"))?;
    let name = data
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::forbidden("Forbidden"))?;
    let school_id = school_id_from_email(email);

    match db::get_user_by_username(&state.db, &school_id).await? {
        Some(user) => {
            sqlx::query(
                "UPDATE users SET readable_name = $2, email = $3, updated_time = now() WHERE id = $1",
            )
            .bind(&user.id)
            .bind(name)
            .bind(email)
            .execute(&state.db)
            .await?;
            db::get_user(&state.db, &user.id)
                .await?
                .ok_or_else(|| ApiError::internal("user disappeared after update"))
        }
        None => {
            create_user_record(state, &school_id, Some(name), Some(" "), Some(email), None).await
        }
    }
}

pub async fn create_user_record(
    state: &AppState,
    username: &str,
    readable_name: Option<&str>,
    school_department: Option<&str>,
    email: Option<&str>,
    hashed_password: Option<&str>,
) -> ApiResult<UserRow> {
    let id = Uuid::new_v4().to_string();
    let mut transaction = state.db.begin().await?;
    sqlx::query(
        "INSERT INTO users (id, username, readable_name, school_department, email, hashed_password, is_super_user) \
         VALUES ($1, $2, $3, $4, $5, $6, false)",
    )
    .bind(&id)
    .bind(username)
    .bind(readable_name)
    .bind(school_department)
    .bind(email)
    .bind(hashed_password)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "INSERT INTO user_preferences (id, user_id, show_empty_courses, default_is_anonymous) \
         VALUES ($1, $2, true, false)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    db::get_user(&state.db, &id)
        .await?
        .ok_or_else(|| ApiError::internal("user vanished after insert"))
}

pub(crate) fn school_id_from_email(email: &str) -> String {
    email.chars().skip(1).take(9).collect()
}

/// Python `int()` acceptance for the LMS login path, normalized like `str(int(x))`.
pub fn python_int_string(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let chars: Vec<char> = trimmed.chars().collect();
    let mut index = 0;
    let negative = match chars[0] {
        '+' => {
            index = 1;
            false
        }
        '-' => {
            index = 1;
            true
        }
        _ => false,
    };
    if index >= chars.len() {
        return None;
    }
    let mut digits = String::new();
    let mut previous_was_digit = false;
    while index < chars.len() {
        let character = chars[index];
        if let Some(value) = decimal_value(character) {
            digits.push(char::from_digit(value, 10)?);
            previous_was_digit = true;
            index += 1;
        } else if character == '_'
            && previous_was_digit
            && index + 1 < chars.len()
            && decimal_value(chars[index + 1]).is_some()
        {
            previous_was_digit = false;
            index += 1;
        } else {
            return None;
        }
    }
    if digits.is_empty() {
        return None;
    }
    let without_zeros = digits.trim_start_matches('0');
    let normalized = if without_zeros.is_empty() {
        "0"
    } else {
        without_zeros
    };
    Some(format!(
        "{}{}",
        if negative && normalized != "0" {
            "-"
        } else {
            ""
        },
        normalized
    ))
}

fn decimal_value(character: char) -> Option<u32> {
    if character.is_ascii_digit() {
        return character.to_digit(10);
    }
    if get_general_category(character) != GeneralCategory::DecimalNumber {
        return None;
    }
    let mut start = character as u32;
    while start > 0 {
        match char::from_u32(start - 1) {
            Some(previous) if get_general_category(previous) == GeneralCategory::DecimalNumber => {
                start -= 1
            }
            _ => break,
        }
    }
    Some(character as u32 - start)
}

#[allow(dead_code)]
fn debug_claims(value: &Value) -> String {
    json!(value).to_string()
}
