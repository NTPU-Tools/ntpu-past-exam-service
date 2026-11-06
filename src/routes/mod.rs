pub mod auth;
pub mod bulletins;
pub mod courses;
pub mod departments;
pub mod health;
pub mod posts;
pub mod threads;
pub mod users;

use axum::http::header::{CACHE_CONTROL, PRAGMA};
use axum::http::HeaderValue;
use axum::response::Response;

use crate::app::AppState;
use crate::error::ApiResult;

pub fn mark_no_store(response: &mut Response) {
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .insert(PRAGMA, HeaderValue::from_static("no-cache"));
}

pub async fn preferences(state: &AppState, user_id: &str) -> ApiResult<(bool, bool)> {
    let row: Option<(bool, bool)> = sqlx::query_as(
        "SELECT show_empty_courses, default_is_anonymous FROM user_preferences WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;
    Ok(row.unwrap_or((true, false)))
}
