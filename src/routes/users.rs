use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::Uri;
use axum::routing::{get, put};
use axum::{Json, Router};
use serde_json::{json, Value};

use crate::app::AppState;
use crate::auth::extractor::AuthUser;
use crate::authz;
use crate::db::{self, DepartmentRow, UserRow, DEPARTMENT_COLUMNS, USER_COLUMNS};
use crate::dto::{DepartmentDto, PreferencesDto, UserDto, UserMeDto, UserRedactedDto};
use crate::error::{ApiError, ApiResult, ValidationError};
use crate::extract::{
    json_bool, query_bool, query_get, strict_pagination, Fields, JsonBody, RawForm,
};
use crate::routes::preferences;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/users", get(read_users))
        .route("/users/me/departments-admin", get(read_user_admin_scopes))
        .route("/users/update/me", put(update_me))
        .route("/users/me/preferences", put(update_preferences))
        .route(
            "/users/status/{department_id}/{user_id}",
            put(update_user_status),
        )
        .route(
            "/users/admin/{department_id}/{user_id}",
            put(update_user_admin),
        )
        .route("/users/{user_id}", get(read_user))
}

async fn read_users(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    uri: Uri,
) -> ApiResult<Json<Vec<UserDto>>> {
    authz::require_super(&user)?;
    if query_get(&uri, "is_active").is_some() {
        let _ = query_bool(&uri, "is_active")?;
        return Err(ApiError::gone(
            "The is_active user field is no longer supported",
        ));
    }
    let (skip, limit) = strict_pagination(&uri)?;
    let rows = sqlx::query_as::<_, UserRow>(&format!(
        "SELECT {USER_COLUMNS} FROM users OFFSET $1 LIMIT $2"
    ))
    .bind(skip)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows.iter().map(UserDto::from).collect()))
}

async fn read_user(
    State(state): State<Arc<AppState>>,
    AuthUser(viewer): AuthUser,
    Path(user_id): Path<String>,
) -> ApiResult<Json<Value>> {
    let is_me = user_id == "me";
    let target_id = if is_me { viewer.id.clone() } else { user_id };

    if !authz::can_view_user_details(&state, &viewer, &target_id).await? {
        return Err(ApiError::forbidden("Not allowed to view this user"));
    }
    let Some(target) = db::get_user(&state.db, &target_id).await? else {
        return Err(ApiError::not_found("User not found"));
    };

    let value = if is_me {
        let (show_empty_courses, default_is_anonymous) = preferences(&state, &target.id).await?;
        serde_json::to_value(UserMeDto {
            user: UserDto::from(&target),
            show_empty_courses,
            default_is_anonymous,
        })
    } else {
        serde_json::to_value(UserRedactedDto::from(&target))
    }
    .map_err(ApiError::internal)?;
    Ok(Json(value))
}

async fn read_user_admin_scopes(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    uri: Uri,
) -> ApiResult<Json<Value>> {
    if let Some(cached) = state
        .cache
        .get("user-admin", &user.id, "read_user_admin_scopes", &uri)
        .await
    {
        return Ok(Json(cached));
    }
    let departments = if user.is_super() {
        sqlx::query_as::<_, DepartmentRow>(&format!(
            "SELECT {DEPARTMENT_COLUMNS} FROM departments ORDER BY sort_order ASC, id ASC"
        ))
        .fetch_all(&state.db)
        .await?
    } else {
        let ids = authz::admin_department_ids(&state, &user).await?;
        sqlx::query_as::<_, DepartmentRow>(&format!(
            "SELECT {DEPARTMENT_COLUMNS} FROM departments WHERE id = ANY($1) \
             ORDER BY sort_order ASC, id ASC"
        ))
        .bind(&ids)
        .fetch_all(&state.db)
        .await?
    };
    let value = serde_json::to_value(
        departments
            .iter()
            .map(DepartmentDto::from)
            .collect::<Vec<_>>(),
    )
    .map_err(ApiError::internal)?;
    state
        .cache
        .set(
            "user-admin",
            30,
            &user.id,
            "read_user_admin_scopes",
            &uri,
            &value,
        )
        .await;
    Ok(Json(value))
}

async fn update_me(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    form: RawForm,
) -> ApiResult<Json<Value>> {
    let mut fields = Fields::new(&form.0);
    let major = fields.string("major", 0, None, true, "");
    let note = fields.string("note", 0, None, false, "");
    fields.finish(())?;

    sqlx::query(
        "UPDATE users SET school_department = $2, note = $3, updated_time = now() WHERE id = $1",
    )
    .bind(&user.id)
    .bind(&major)
    .bind(&note)
    .execute(&state.db)
    .await?;
    let updated = db::get_user(&state.db, &user.id)
        .await?
        .ok_or_else(|| ApiError::not_found("User not found"))?;
    let (show_empty_courses, default_is_anonymous) = preferences(&state, &updated.id).await?;
    let value = serde_json::to_value(UserMeDto {
        user: UserDto::from(&updated),
        show_empty_courses,
        default_is_anonymous,
    })
    .map_err(ApiError::internal)?;
    Ok(Json(value))
}

async fn update_preferences(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    JsonBody(body): JsonBody,
) -> ApiResult<Json<PreferencesDto>> {
    let Some(object) = body.as_object() else {
        return Err(ApiError::validation(ValidationError::new(
            "model_attributes_type",
            &["body"],
            "Input should be a valid dictionary or object to extract fields from".into(),
            body,
        )));
    };
    let mut errors = Vec::new();
    let mut read = |name: &str| -> bool {
        match object.get(name) {
            Some(value) => match json_bool(value) {
                Some(value) => value,
                None => {
                    errors.push(ValidationError::bool_parsing(
                        &["body", name],
                        value.clone(),
                    ));
                    false
                }
            },
            None => {
                errors.push(ValidationError::missing(&["body", name]));
                false
            }
        }
    };
    let show_empty_courses = read("show_empty_courses");
    let default_is_anonymous = read("default_is_anonymous");
    if !errors.is_empty() {
        return Err(ApiError::Validation(errors));
    }

    sqlx::query(
        "INSERT INTO user_preferences (id, user_id, show_empty_courses, default_is_anonymous) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (user_id) DO UPDATE SET \
             show_empty_courses = EXCLUDED.show_empty_courses, \
             default_is_anonymous = EXCLUDED.default_is_anonymous, \
             updated_time = CASE WHEN \
                 user_preferences.show_empty_courses IS DISTINCT FROM EXCLUDED.show_empty_courses \
                 OR user_preferences.default_is_anonymous IS DISTINCT FROM EXCLUDED.default_is_anonymous \
             THEN now() ELSE user_preferences.updated_time END",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(&user.id)
    .bind(show_empty_courses)
    .bind(default_is_anonymous)
    .execute(&state.db)
    .await?;

    state.cache.invalidate("user-detail").await;
    state.cache.invalidate("verify-token").await;
    Ok(Json(PreferencesDto {
        show_empty_courses,
        default_is_anonymous,
    }))
}

async fn update_user_status(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path((department_id, _user_id)): Path<(String, String)>,
    form: RawForm,
) -> ApiResult<Json<Value>> {
    authz::require_admin(&state, &user, Some(&department_id)).await?;
    let mut fields = Fields::new(&form.0);
    let _ = fields.required_boolean("is_active");
    fields.finish(())?;
    Err(ApiError::gone("Global user status is no longer supported"))
}

async fn update_user_admin(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path((department_id, _user_id)): Path<(String, String)>,
    form: RawForm,
) -> ApiResult<Json<Value>> {
    authz::require_admin(&state, &user, Some(&department_id)).await?;
    let mut fields = Fields::new(&form.0);
    let _ = fields.required_boolean("is_admin");
    fields.finish(())?;
    Err(ApiError::gone(format!(
        "Use /departments/{department_id}/admin instead"
    )))
}

#[allow(dead_code)]
async fn deprecated_no_store() -> Value {
    json!({})
}
