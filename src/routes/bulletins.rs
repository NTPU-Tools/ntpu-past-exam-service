use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::Uri;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::Value;
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::extractor::AuthUser;
use crate::authz;
use crate::db::{BulletinRow, BULLETIN_COLUMNS};
use crate::dto::BulletinDto;
use crate::error::{ApiError, ApiResult};
use crate::extract::{clamped_pagination, Fields, RawForm};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/bulletins", get(get_all_bulletins))
        .route(
            "/bulletins/{bulletin_id}",
            get(get_single_bulletin).post(create_bulletin),
        )
}

async fn get_all_bulletins(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    uri: Uri,
) -> ApiResult<Json<Value>> {
    if let Some(cached) = state
        .cache
        .get("bulletins", &user.id, "get_all_bulletins", &uri)
        .await
    {
        return Ok(Json(cached));
    }
    let (skip, limit) = clamped_pagination(&uri)?;
    let visible = authz::visible_department_ids(&state, &user).await?;
    let items: Vec<BulletinDto> = if visible.is_empty() {
        Vec::new()
    } else {
        let rows = sqlx::query_as::<_, BulletinRow>(&format!(
            "SELECT {BULLETIN_COLUMNS} FROM bulletins WHERE department_id = ANY($1) \
             ORDER BY create_time ASC, id ASC OFFSET $2 LIMIT $3"
        ))
        .bind(&visible)
        .bind(skip)
        .bind(limit)
        .fetch_all(&state.db)
        .await?;
        rows.iter().map(BulletinDto::from).collect()
    };
    let value = serde_json::to_value(items).map_err(ApiError::internal)?;
    state
        .cache
        .set("bulletins", 60, &user.id, "get_all_bulletins", &uri, &value)
        .await;
    Ok(Json(value))
}

async fn get_single_bulletin(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(bulletin_id): Path<String>,
    uri: Uri,
) -> ApiResult<Json<Value>> {
    if let Some(cached) = state
        .cache
        .get("bulletins", &user.id, "get_single_bulletin", &uri)
        .await
    {
        return Ok(Json(cached));
    }
    let bulletin = sqlx::query_as::<_, BulletinRow>(&format!(
        "SELECT {BULLETIN_COLUMNS} FROM bulletins WHERE id = $1"
    ))
    .bind(&bulletin_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::not_found("Bulletin not found"))?;
    authz::require_viewable_department(
        &state,
        &user,
        bulletin.department_id.as_deref().unwrap_or(""),
    )
    .await?;
    let value = serde_json::to_value(BulletinDto::from(&bulletin)).map_err(ApiError::internal)?;
    state
        .cache
        .set(
            "bulletins",
            60,
            &user.id,
            "get_single_bulletin",
            &uri,
            &value,
        )
        .await;
    Ok(Json(value))
}

async fn create_bulletin(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(department_id): Path<String>,
    form: RawForm,
) -> ApiResult<Json<BulletinDto>> {
    authz::require_admin(&state, &user, Some(&department_id)).await?;
    let mut fields = Fields::new(&form.0);
    let title = fields.string("title", 0, None, true, "");
    let content = fields.string("content", 0, None, true, "");
    fields.finish(())?;

    let bulletin = sqlx::query_as::<_, BulletinRow>(&format!(
        "INSERT INTO bulletins (id, title, content, department_id) VALUES ($1, $2, $3, $4) \
         RETURNING {BULLETIN_COLUMNS}"
    ))
    .bind(Uuid::new_v4().to_string())
    .bind(&title)
    .bind(&content)
    .bind(&department_id)
    .fetch_one(&state.db)
    .await?;

    state.cache.invalidate("bulletins").await;
    state.cache.invalidate("dept-bulletins").await;
    Ok(Json(BulletinDto::from(&bulletin)))
}
