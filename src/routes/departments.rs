use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::Uri;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::extractor::AuthUser;
use crate::authz;
use crate::db::{
    self, CourseRow, MembershipRow, UserRow, COURSE_COLUMNS, DEPARTMENT_COLUMNS, MEMBERSHIP_COLUMNS,
};
use crate::dto::{
    DepartmentDto, DeptCourseDto, JoinRequestDto, MemberDto, MembershipDto, StatusDto,
};
use crate::error::{ApiError, ApiResult};
use crate::extract::{Fields, RawForm};

type JoinRequestRow = (
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

type MemberRow = (
    Option<bool>,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/departments", get(read_all_departments))
        .route("/departments/status", get(read_user_departments_status))
        .route("/departments/visible", get(read_user_viewable_departments))
        .route(
            "/departments/join-request/{department_id}/approve/{request_id}",
            put(approve_join_request),
        )
        .route(
            "/departments/{department_id}/courses",
            get(get_department_courses),
        )
        .route(
            "/departments/{department_id}/bulletins",
            get(get_department_bulletins),
        )
        .route(
            "/departments/{department_id}/visible-check",
            get(check_user_can_view),
        )
        .route(
            "/departments/{department_id}/admin-check",
            get(check_user_is_department_admin),
        )
        .route(
            "/departments/{department_id}/join-request/send",
            post(send_join_request),
        )
        .route(
            "/departments/{department_id}/join-request",
            get(get_join_request),
        )
        .route(
            "/departments/{department_id}/members",
            get(get_department_members),
        )
        .route(
            "/departments/{department_id}/admin",
            put(update_member_admin),
        )
        .route(
            "/departments/{department_id}",
            get(get_department_information),
        )
}

async fn all_departments(state: &AppState) -> ApiResult<Vec<DepartmentDto>> {
    let rows = sqlx::query_as::<_, db::DepartmentRow>(&format!(
        "SELECT {DEPARTMENT_COLUMNS} FROM departments ORDER BY sort_order ASC, id ASC"
    ))
    .fetch_all(&state.db)
    .await?;
    Ok(rows.iter().map(DepartmentDto::from).collect())
}

async fn read_all_departments(
    State(state): State<Arc<AppState>>,
    AuthUser(_user): AuthUser,
) -> ApiResult<Json<Vec<DepartmentDto>>> {
    Ok(Json(all_departments(&state).await?))
}

async fn membership_statuses(
    state: &AppState,
    user_id: &str,
) -> ApiResult<HashMap<String, Option<String>>> {
    let rows: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT department_id, status FROM users_departments WHERE user_id = $1")
            .bind(user_id)
            .fetch_all(&state.db)
            .await?;
    Ok(rows.into_iter().collect())
}

async fn read_user_departments_status(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    uri: Uri,
) -> ApiResult<Json<Value>> {
    if let Some(cached) = state
        .cache
        .get(
            "dept-status",
            &user.id,
            "read_user_departments_status",
            &uri,
        )
        .await
    {
        return Ok(Json(cached));
    }
    let departments = all_departments(&state).await?;
    let value = if user.is_super() {
        json!({ "visible": departments, "pending": [] })
    } else {
        let statuses = membership_statuses(&state, &user.id).await?;
        let approved: HashSet<&String> = statuses
            .iter()
            .filter(|(_, status)| status.as_deref() == Some("APPROVED"))
            .map(|(department, _)| department)
            .collect();
        let pending: HashSet<&String> = statuses
            .iter()
            .filter(|(_, status)| status.as_deref() == Some("PENDING"))
            .map(|(department, _)| department)
            .collect();
        let visible: Vec<&DepartmentDto> = departments
            .iter()
            .filter(|department| {
                department.is_public == Some(true) || approved.contains(&department.id)
            })
            .collect();
        let pending_departments: Vec<&DepartmentDto> = departments
            .iter()
            .filter(|department| pending.contains(&department.id))
            .collect();
        json!({ "visible": visible, "pending": pending_departments })
    };
    state
        .cache
        .set(
            "dept-status",
            60,
            &user.id,
            "read_user_departments_status",
            &uri,
            &value,
        )
        .await;
    Ok(Json(value))
}

async fn read_user_viewable_departments(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    uri: Uri,
) -> ApiResult<Json<Value>> {
    if let Some(cached) = state
        .cache
        .get(
            "dept-visible",
            &user.id,
            "read_user_viewable_departments",
            &uri,
        )
        .await
    {
        return Ok(Json(cached));
    }
    let departments = all_departments(&state).await?;
    let visible: Vec<DepartmentDto> = if user.is_super() {
        departments
    } else {
        let statuses = membership_statuses(&state, &user.id).await?;
        departments
            .into_iter()
            .filter(|department| {
                department.is_public == Some(true)
                    || statuses
                        .get(&department.id)
                        .and_then(|status| status.as_deref())
                        == Some("APPROVED")
            })
            .collect()
    };
    let value = serde_json::to_value(visible).map_err(ApiError::internal)?;
    state
        .cache
        .set(
            "dept-visible",
            60,
            &user.id,
            "read_user_viewable_departments",
            &uri,
            &value,
        )
        .await;
    Ok(Json(value))
}

async fn get_department_information(
    State(state): State<Arc<AppState>>,
    AuthUser(_user): AuthUser,
    Path(department_id): Path<String>,
) -> ApiResult<Json<DepartmentDto>> {
    let department = db::get_department(&state.db, &department_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Not Found"))?;
    Ok(Json(DepartmentDto::from(&department)))
}

async fn get_department_courses(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(department_id): Path<String>,
    uri: Uri,
) -> ApiResult<Json<Value>> {
    if let Some(cached) = state
        .cache
        .get("dept-courses", &user.id, "get_department_courses", &uri)
        .await
    {
        return Ok(Json(cached));
    }
    authz::require_viewable_department(&state, &user, &department_id).await?;
    let courses = sqlx::query_as::<_, CourseRow>(&format!(
        "SELECT {COURSE_COLUMNS} FROM courses WHERE department_id = $1"
    ))
    .bind(&department_id)
    .fetch_all(&state.db)
    .await?;
    let ids: Vec<String> = courses.iter().map(|course| course.id.clone()).collect();
    let approved: HashSet<String> = if ids.is_empty() {
        HashSet::new()
    } else {
        let rows: Vec<Option<String>> = sqlx::query_scalar(
            "SELECT DISTINCT course_id FROM posts WHERE course_id = ANY($1) AND status = 'APPROVED'",
        )
        .bind(&ids)
        .fetch_all(&state.db)
        .await?;
        rows.into_iter().flatten().collect()
    };
    let items: Vec<DeptCourseDto> = courses
        .iter()
        .map(|course| DeptCourseDto {
            id: course.id.clone(),
            name: course.name.clone(),
            category: course.category.clone(),
            department_id: course.department_id.clone(),
            create_time: course.create_time.into(),
            updated_time: course.updated_time.into(),
            has_posts: approved.contains(&course.id),
        })
        .collect();
    let value = serde_json::to_value(items).map_err(ApiError::internal)?;
    state
        .cache
        .set(
            "dept-courses",
            60,
            &user.id,
            "get_department_courses",
            &uri,
            &value,
        )
        .await;
    Ok(Json(value))
}

async fn get_department_bulletins(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(department_id): Path<String>,
    uri: Uri,
) -> ApiResult<Json<Value>> {
    if let Some(cached) = state
        .cache
        .get("dept-bulletins", &user.id, "get_department_bulletins", &uri)
        .await
    {
        return Ok(Json(cached));
    }
    authz::require_viewable_department(&state, &user, &department_id).await?;
    let rows = sqlx::query_as::<_, db::BulletinRow>(
        "SELECT id, title, content, department_id, create_time, updated_time \
         FROM bulletins WHERE department_id = $1",
    )
    .bind(&department_id)
    .fetch_all(&state.db)
    .await?;
    let value = serde_json::to_value(
        rows.iter()
            .map(crate::dto::BulletinDto::from)
            .collect::<Vec<_>>(),
    )
    .map_err(ApiError::internal)?;
    state
        .cache
        .set(
            "dept-bulletins",
            60,
            &user.id,
            "get_department_bulletins",
            &uri,
            &value,
        )
        .await;
    Ok(Json(value))
}

async fn check_user_can_view(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(department_id): Path<String>,
) -> ApiResult<Json<Value>> {
    let can_view = authz::is_approved(&state, &user.id, &department_id).await?;
    Ok(Json(json!({ "can_view": can_view })))
}

async fn check_user_is_department_admin(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(department_id): Path<String>,
) -> ApiResult<Json<Value>> {
    let is_admin = authz::is_department_admin(&state, &user.id, &department_id).await?;
    Ok(Json(json!({ "is_admin": is_admin })))
}

async fn send_join_request(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(department_id): Path<String>,
) -> ApiResult<Json<MembershipDto>> {
    let department = db::get_department(&state.db, &department_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Not Found"))?;

    let mut transaction = state.db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1)::bigint)")
        .bind(format!("{}:{}", user.id, department_id))
        .execute(&mut *transaction)
        .await?;
    let existing = sqlx::query_as::<_, MembershipRow>(&format!(
        "SELECT {MEMBERSHIP_COLUMNS} FROM users_departments \
         WHERE department_id = $1 AND user_id = $2 LIMIT 1"
    ))
    .bind(&department_id)
    .bind(&user.id)
    .fetch_optional(&mut *transaction)
    .await?;
    if let Some(existing) = existing {
        if existing.status.as_deref() == Some("APPROVED") {
            return Err(ApiError::bad_request("request has been approved"));
        }
        return Err(ApiError::bad_request("duplicate request"));
    }
    let membership = sqlx::query_as::<_, MembershipRow>(&format!(
        "INSERT INTO users_departments (id, user_id, department_id, status, is_department_admin) \
         VALUES ($1, $2, $3, 'PENDING', false) RETURNING {MEMBERSHIP_COLUMNS}"
    ))
    .bind(Uuid::new_v4().to_string())
    .bind(&user.id)
    .bind(&department_id)
    .fetch_one(&mut *transaction)
    .await?;
    transaction.commit().await?;

    let recipients = authz::admin_recipient_emails(&state, &department_id).await?;
    state.mailer.notify(
        "請求審核",
        &format!(
            "有人申請瀏覽您所管理的 {} 社群，請儘速審核。",
            department.display_name()
        ),
        &recipients,
        Some((
            "前往審核".to_string(),
            format!(
                "{}/admin/{}?open_edit_member_dialog=true",
                state.config.origin_or_empty(),
                department.id
            ),
        )),
    );
    state.cache.invalidate("dept-status").await;
    Ok(Json(MembershipDto::from(&membership)))
}

async fn get_join_request(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(department_id): Path<String>,
) -> ApiResult<Json<Vec<JoinRequestDto>>> {
    authz::require_admin(&state, &user, Some(&department_id)).await?;
    let rows: Vec<JoinRequestRow> = sqlx::query_as(
        "SELECT ud.id, u.id, u.email, u.username, u.readable_name, u.school_department, u.note \
         FROM users_departments ud JOIN users u ON u.id = ud.user_id \
         WHERE ud.department_id = $1 AND ud.status = 'PENDING'",
    )
    .bind(&department_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(
                |(id, user_id, email, username, readable_name, school_department, note)| {
                    JoinRequestDto {
                        id,
                        status: "PENDING".to_string(),
                        user_id: Some(user_id),
                        email,
                        username,
                        readable_name,
                        school_department,
                        note,
                    }
                },
            )
            .collect(),
    ))
}

async fn get_department_members(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(department_id): Path<String>,
) -> ApiResult<Json<Vec<MemberDto>>> {
    authz::require_admin(&state, &user, Some(&department_id)).await?;
    let rows: Vec<MemberRow> = sqlx::query_as(
        "SELECT ud.is_department_admin, u.id, u.email, u.username, u.readable_name, u.school_department \
         FROM users_departments ud JOIN users u ON u.id = ud.user_id \
         WHERE ud.department_id = $1 AND ud.status = 'APPROVED'",
    )
    .bind(&department_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(
                |(
                    is_department_admin,
                    user_id,
                    email,
                    username,
                    readable_name,
                    school_department,
                )| {
                    MemberDto {
                        id: user_id.clone(),
                        user_id,
                        email,
                        username,
                        readable_name,
                        school_department,
                        is_department_admin,
                    }
                },
            )
            .collect(),
    ))
}

async fn update_member_admin(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(department_id): Path<String>,
    form: RawForm,
) -> ApiResult<Json<StatusDto>> {
    authz::require_admin(&state, &user, Some(&department_id)).await?;
    let mut fields = Fields::new(&form.0);
    let target_user_id = fields.string("user_id", 0, None, true, "");
    let is_admin = fields.required_boolean("is_admin");
    fields.finish(())?;

    let membership = db::get_membership(&state.db, &target_user_id, &department_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Not Found"))?;
    sqlx::query(
        "UPDATE users_departments SET is_department_admin = $2, \
         updated_time = CASE WHEN is_department_admin IS DISTINCT FROM $2 THEN now() ELSE updated_time END \
         WHERE id = $1",
    )
    .bind(&membership.id)
    .bind(is_admin)
    .execute(&state.db)
    .await?;

    let department = db::get_department(&state.db, &department_id).await?;
    let target = db::get_user(&state.db, &target_user_id).await?;
    if let (Some(department), Some(target)) = (department, target) {
        let content = if is_admin {
            format!(
                "您在 {} 的權限已被提升至『管理員』。",
                department.display_name()
            )
        } else {
            format!(
                "您在 {} 的權限已被變更至『一般用戶』。",
                department.display_name()
            )
        };
        state
            .mailer
            .notify("權限變更", &content, &[target.mail_address()], None);
    }

    for namespace in [
        "user-admin",
        "posts",
        "courses",
        "course-detail",
        "dept-courses",
        "bulletins",
        "dept-bulletins",
    ] {
        state.cache.invalidate(namespace).await;
    }
    Ok(Json(StatusDto { status: "success" }))
}

async fn approve_join_request(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path((department_id, request_id)): Path<(String, String)>,
) -> ApiResult<Json<StatusDto>> {
    authz::require_admin(&state, &user, Some(&department_id)).await?;
    let membership = sqlx::query_as::<_, MembershipRow>(&format!(
        "SELECT {MEMBERSHIP_COLUMNS} FROM users_departments \
         WHERE id = $1 AND department_id = $2"
    ))
    .bind(&request_id)
    .bind(&department_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::not_found("Not Found"))?;
    if membership.status.as_deref() != Some("PENDING") {
        return Err(ApiError::bad_request("request is not pending"));
    }
    sqlx::query(
        "UPDATE users_departments SET status = 'APPROVED', updated_time = now() WHERE id = $1",
    )
    .bind(&membership.id)
    .execute(&state.db)
    .await?;

    let applicant = match membership.user_id.as_deref() {
        Some(id) => db::get_user(&state.db, id).await?,
        None => None,
    };
    if let Some(applicant) = applicant {
        if let Some(department) = db::get_department(&state.db, &department_id).await? {
            state.mailer.notify(
                "審核通過",
                &format!("您申請加入 {} 的審核已通過。", department.display_name()),
                &[applicant.mail_address()],
                None,
            );
        }
    }

    for namespace in [
        "dept-status",
        "dept-visible",
        "user-admin",
        "posts",
        "courses",
        "course-detail",
        "dept-courses",
        "bulletins",
        "dept-bulletins",
    ] {
        state.cache.invalidate(namespace).await;
    }
    Ok(Json(StatusDto { status: "success" }))
}

#[allow(dead_code)]
async fn unused(user: &UserRow) -> &Option<String> {
    &user.email
}
