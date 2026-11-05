use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::Uri;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::extractor::AuthUser;
use crate::authz;
use crate::db::{self, CourseRow, PostRow, COURSE_COLUMNS, POST_COLUMNS};
use crate::dto::{CourseBriefDto, CourseDto, CoursePostDto};
use crate::error::{ApiError, ApiResult};
use crate::extract::{clamped_pagination, Fields, RawForm};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/courses", get(read_courses))
        .route("/courses/{course_id}", get(get_course).post(create_course))
}

async fn read_courses(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    uri: Uri,
) -> ApiResult<Json<Value>> {
    if let Some(cached) = state
        .cache
        .get("courses", &user.id, "read_courses", &uri)
        .await
    {
        return Ok(Json(cached));
    }
    let (skip, limit) = clamped_pagination(&uri)?;
    let visible = authz::visible_department_ids(&state, &user).await?;
    let items: Vec<CourseBriefDto> = if visible.is_empty() {
        Vec::new()
    } else {
        let rows = sqlx::query_as::<_, CourseRow>(&format!(
            "SELECT {COURSE_COLUMNS} FROM courses WHERE department_id = ANY($1) \
             ORDER BY create_time ASC, id ASC OFFSET $2 LIMIT $3"
        ))
        .bind(&visible)
        .bind(skip)
        .bind(limit)
        .fetch_all(&state.db)
        .await?;
        rows.iter().map(CourseBriefDto::from).collect()
    };
    let value = serde_json::to_value(items).map_err(ApiError::internal)?;
    state
        .cache
        .set("courses", 60, &user.id, "read_courses", &uri, &value)
        .await;
    Ok(Json(value))
}

async fn get_course(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(course_id): Path<String>,
    uri: Uri,
) -> ApiResult<Json<Value>> {
    if let Some(cached) = state
        .cache
        .get("course-detail", &user.id, "get_course", &uri)
        .await
    {
        return Ok(Json(cached));
    }
    let course = db::get_course(&state.db, &course_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Course not found"))?;
    authz::require_viewable_department(
        &state,
        &user,
        course.department_id.as_deref().unwrap_or(""),
    )
    .await?;
    let posts = sqlx::query_as::<_, PostRow>(&format!(
        "SELECT {POST_COLUMNS} FROM posts WHERE course_id = $1 AND status = 'APPROVED' \
         ORDER BY create_time ASC, id ASC"
    ))
    .bind(&course_id)
    .fetch_all(&state.db)
    .await?;
    let value = json!({
        "course": CourseDto::from(&course),
        "posts": posts.iter().map(CoursePostDto::from).collect::<Vec<_>>(),
    });
    state
        .cache
        .set("course-detail", 30, &user.id, "get_course", &uri, &value)
        .await;
    Ok(Json(value))
}

async fn create_course(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(department_id): Path<String>,
    form: RawForm,
) -> ApiResult<Json<CourseDto>> {
    authz::require_admin(&state, &user, Some(&department_id)).await?;
    let mut fields = Fields::new(&form.0);
    let name = fields.string("name", 0, None, true, "");
    let category = fields.string("category", 0, None, true, "");
    fields.finish(())?;

    let course = sqlx::query_as::<_, CourseRow>(&format!(
        "INSERT INTO courses (id, name, category, department_id) VALUES ($1, $2, $3, $4) \
         RETURNING {COURSE_COLUMNS}"
    ))
    .bind(Uuid::new_v4().to_string())
    .bind(&name)
    .bind(&category)
    .bind(&department_id)
    .fetch_one(&state.db)
    .await?;

    state.cache.invalidate("courses").await;
    state.cache.invalidate("dept-courses").await;
    Ok(Json(CourseDto::from(&course)))
}
