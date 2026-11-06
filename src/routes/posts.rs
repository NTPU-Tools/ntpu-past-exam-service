use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::Uri;
use axum::routing::{get, put};
use axum::{Json, Router};
use serde_json::{json, Value};
use sqlx::{Postgres, QueryBuilder};
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::extractor::AuthUser;
use crate::authz;
use crate::db::{self, PostListRow, PostRow, PostWithUserRow, UserRow, POST_COLUMNS, USER_COLUMNS};
use crate::dto::{CreatePostDto, PostBaseDto, PostDetailDto, PostListDto, StatusDto};
use crate::error::{ApiError, ApiResult};
use crate::extract::{clamped_pagination, query_map, Fields, RawForm, UploadedFile};

const MAX_POST_FILES: usize = 10;
const MAX_POST_FILE_BYTES: usize = 20 * 1024 * 1024;
const MAX_POST_TOTAL_BYTES: usize = 50 * 1024 * 1024;
const ALLOWED_PDF_TYPES: [&str; 2] = ["application/pdf", "application/x-pdf"];
const USER_ALIASES: &str = "u.id AS owner_user_id, u.email AS owner_email, u.username AS owner_username, \
    u.readable_name AS owner_readable_name, u.note AS owner_note, u.hashed_password AS owner_hashed_password, \
    u.school_department AS owner_school_department, u.is_super_user AS owner_is_super_user, \
    u.create_time AS owner_create_time, u.updated_time AS owner_updated_time";

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/posts", get(read_all_post).post(create_post))
        .route(
            "/posts/status/{department_id}/{post_id}",
            put(update_post_status),
        )
        .route("/posts/{post_id}", get(get_single_post).delete(delete_post))
}

async fn read_all_post(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    uri: Uri,
) -> ApiResult<Json<Value>> {
    let query = query_map(&uri);
    let status = query.get("status").cloned().unwrap_or_default();
    let user_id = query.get("user_id").cloned().unwrap_or_default();
    let course_id = query.get("course_id").cloned().unwrap_or_default();
    let department_id = query.get("department_id").cloned().unwrap_or_default();
    let (skip, limit) = clamped_pagination(&uri)?;

    let visible = authz::visible_department_ids(&state, &user).await?;
    if visible.is_empty() {
        return Ok(Json(json!([])));
    }
    let admin_ids = authz::admin_department_ids(&state, &user).await?;

    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT p.id, p.title, p.content, p.owner_id, p.course_id, p.is_migrate, p.is_anonymous, \
         p.status, p.department_id, p.create_time, p.updated_time, c.name AS course_name \
         FROM posts p JOIN courses c ON p.course_id = c.id \
         WHERE p.department_id = ANY(",
    );
    builder.push_bind(&visible);
    builder.push(")");
    if !user.is_super() {
        builder
            .push(" AND (p.status = 'APPROVED' OR p.owner_id = ")
            .push_bind(&user.id)
            .push(" OR p.department_id = ANY(")
            .push_bind(&admin_ids)
            .push("))");
    }
    match status.as_str() {
        "" => {}
        "PENDING" => {
            builder.push(" AND p.status IS DISTINCT FROM 'APPROVED'");
        }
        "APPROVED" => {
            builder.push(" AND p.status = 'APPROVED'");
        }
        "REJECTED" => {
            builder.push(" AND p.status = 'REJECTED'");
        }
        _ => return Err(ApiError::bad_request("Invalid post status")),
    }
    if !course_id.is_empty() {
        builder.push(" AND p.course_id = ").push_bind(&course_id);
    }
    if !user_id.is_empty() {
        builder.push(" AND p.owner_id = ").push_bind(&user_id);
        // F-17: anonymous authorship must not be discoverable by filtering.
        if user_id != user.id && !user.is_super() {
            builder.push(" AND p.is_anonymous IS NOT TRUE");
        }
    }
    if !department_id.is_empty() {
        builder
            .push(" AND p.department_id = ")
            .push_bind(&department_id);
    }
    builder
        .push(" ORDER BY p.create_time DESC, p.id DESC OFFSET ")
        .push_bind(skip)
        .push(" LIMIT ")
        .push_bind(limit);

    let rows: Vec<PostListRow> = builder.build_query_as().fetch_all(&state.db).await?;
    let items: Vec<PostListDto> = rows
        .into_iter()
        .map(|row| PostListDto {
            post: PostBaseDto::from(&row.post()),
            course_name: row.course_name,
        })
        .collect();
    Ok(Json(
        serde_json::to_value(items).map_err(ApiError::internal)?,
    ))
}

async fn get_single_post(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(post_id): Path<String>,
    uri: Uri,
) -> ApiResult<Json<Value>> {
    if let Some(cached) = state
        .cache
        .get("posts", &user.id, "get_single_post", &uri)
        .await
    {
        return Ok(Json(cached));
    }
    let value = load_post_detail(&state, &post_id, &user)
        .await?
        .ok_or_else(|| ApiError::not_found("Not Found"))?;
    state
        .cache
        .set("posts", 60, &user.id, "get_single_post", &uri, &value)
        .await;
    Ok(Json(value))
}

async fn load_post_detail(
    state: &AppState,
    post_id: &str,
    user: &UserRow,
) -> ApiResult<Option<Value>> {
    let row: Option<PostWithUserRow> = sqlx::query_as(&format!(
        "SELECT p.id, p.title, p.content, p.owner_id, p.course_id, p.is_migrate, p.is_anonymous, \
         p.status, p.department_id, p.create_time, p.updated_time, {USER_ALIASES} \
         FROM posts p JOIN users u ON p.owner_id = u.id WHERE p.id = $1"
    ))
    .bind(post_id)
    .fetch_optional(&state.db)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let post = row.post();
    let owner = row.owner();
    let visible = authz::visible_department_ids(state, user).await?;
    let visible_here = post
        .department_id
        .as_ref()
        .map(|department| visible.contains(department))
        .unwrap_or(false);
    if !visible_here {
        return Err(ApiError::not_found("Not Found"));
    }
    if post.status.as_deref() != Some("APPROVED")
        && !authz::can_view_unapproved_post(state, user, &post).await?
    {
        return Err(ApiError::not_found("Not Found"));
    }
    let files: Vec<String> = sqlx::query_scalar("SELECT url FROM posts_files WHERE post_id = $1")
        .bind(post_id)
        .fetch_all(&state.db)
        .await?;
    let detail = PostDetailDto {
        post: PostBaseDto::from(&post),
        files,
        owner_name: if post.is_anonymous() {
            "匿名用戶".to_string()
        } else {
            owner.display_name()
        },
    };
    Ok(Some(
        serde_json::to_value(detail).map_err(ApiError::internal)?,
    ))
}

async fn create_post(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    form: RawForm,
) -> ApiResult<Json<CreatePostDto>> {
    if user.id.is_empty() {
        return Err(ApiError::unauthorized("Invalid token: missing user id"));
    }
    let mut fields = Fields::new(&form.0);
    let title = fields.string("title", 1, Some(256), true, "");
    let course_id = fields.string("course_id", 1, Some(256), true, "");
    let department_id = fields.string("department_id", 0, Some(256), false, "");
    let content = fields.string("content", 0, Some(10000), false, "");
    let is_migrate = fields.boolean("is_migrate", false);
    let is_anonymous = fields.boolean("is_anonymous", false);
    fields.finish(())?;

    let course = db::get_course(&state.db, &course_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Course not found"))?;
    if course.department_id.as_deref() != Some(department_id.as_str()) {
        return Err(ApiError::bad_request(
            "Course does not belong to the selected department",
        ));
    }
    let department = authz::require_viewable_department(&state, &user, &department_id).await?;
    let files = validate_pdf_uploads(form.0.files_named("files"))?;

    let post_id = Uuid::new_v4().to_string();
    let status = if department.is_public() {
        "APPROVED"
    } else {
        "PENDING"
    };
    let mut uploaded: Vec<String> = Vec::new();
    let result: ApiResult<()> = async {
        let mut transaction = state.db.begin().await?;
        sqlx::query(
            "INSERT INTO posts (id, title, content, owner_id, course_id, is_migrate, is_anonymous, \
             status, department_id) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(&post_id)
        .bind(&title)
        .bind(&content)
        .bind(&user.id)
        .bind(&course_id)
        .bind(is_migrate)
        .bind(is_anonymous)
        .bind(status)
        .bind(&department_id)
        .execute(&mut *transaction)
        .await?;
        for file in files {
            let key = format!("{}/{}/{}", user.id, post_id, Uuid::new_v4());
            uploaded.push(key.clone());
            state
                .storage
                .put(&key, file.data, "application/pdf")
                .await
                .map_err(ApiError::internal)?;
            sqlx::query("INSERT INTO posts_files (id, url, post_id) VALUES ($1, $2, $3)")
                .bind(Uuid::new_v4().to_string())
                .bind(state.storage.url(&key))
                .bind(&post_id)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(())
    }
    .await;
    if let Err(error) = result {
        for key in &uploaded {
            state.storage.delete(key).await;
        }
        return Err(error);
    }

    if !department.is_public() {
        let recipients = authz::admin_recipient_emails(&state, &department_id).await?;
        state.mailer.notify(
            "考古題審核",
            &format!(
                "有人在您所管理的 {} 社群發布考古題，請儘速審核。",
                department.display_name()
            ),
            &recipients,
            Some((
                "前往審核".to_string(),
                format!(
                    "{}/admin/{}?open_edit_post_dialog=true&edit_post_detail_id={}",
                    state.config.origin_or_empty(),
                    department.id,
                    post_id
                ),
            )),
        );
    }

    state.cache.invalidate("course-detail").await;
    state.cache.invalidate("posts").await;
    state.cache.invalidate("dept-courses").await;
    Ok(Json(CreatePostDto {
        status: "success",
        post_id,
    }))
}

fn validate_pdf_uploads(files: Vec<&UploadedFile>) -> ApiResult<Vec<UploadedFile>> {
    if files.len() > MAX_POST_FILES {
        return Err(ApiError::too_large(format!(
            "Too many files; maximum is {MAX_POST_FILES}"
        )));
    }
    let mut total = 0usize;
    let mut accepted = Vec::new();
    for file in files {
        let content_type = file.content_type.as_deref().unwrap_or("");
        if !ALLOWED_PDF_TYPES.contains(&content_type) {
            return Err(ApiError::bad_request("Only PDF files are allowed"));
        }
        if file.data.len() > MAX_POST_FILE_BYTES {
            return Err(ApiError::too_large("A PDF exceeds the per-file size limit"));
        }
        if total + file.data.len() > MAX_POST_TOTAL_BYTES {
            return Err(ApiError::too_large(
                "PDF uploads exceed the total size limit",
            ));
        }
        if !file.data.starts_with(b"%PDF-") {
            return Err(ApiError::bad_request("Invalid PDF content"));
        }
        total += file.data.len();
        accepted.push(file.clone());
    }
    Ok(accepted)
}

async fn update_post_status(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path((department_id, post_id)): Path<(String, String)>,
    form: RawForm,
) -> ApiResult<Json<Value>> {
    authz::require_admin(&state, &user, Some(&department_id)).await?;
    let mut fields = Fields::new(&form.0);
    let status = fields.string("status", 0, None, true, "");
    fields.finish(())?;
    if !matches!(status.as_str(), "PENDING" | "APPROVED" | "REJECTED") {
        return Err(ApiError::bad_request("Invalid post status"));
    }
    let post = sqlx::query_as::<_, PostRow>(&format!(
        "SELECT {POST_COLUMNS} FROM posts WHERE id = $1 AND department_id = $2"
    ))
    .bind(&post_id)
    .bind(&department_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::not_found("Post not found"))?;

    sqlx::query(
        "UPDATE posts SET status = $2, \
         updated_time = CASE WHEN status IS DISTINCT FROM $2 THEN now() ELSE updated_time END \
         WHERE id = $1",
    )
    .bind(&post.id)
    .bind(&status)
    .execute(&state.db)
    .await?;

    let department = db::get_department(&state.db, &department_id).await?;
    let Some(department) = department else {
        return Err(ApiError::not_found("Not Found"));
    };
    if status == "APPROVED" {
        if let Some(owner) =
            sqlx::query_as::<_, UserRow>(&format!("SELECT {USER_COLUMNS} FROM users WHERE id = $1"))
                .bind(post.owner_id.as_deref().unwrap_or(""))
                .fetch_optional(&state.db)
                .await?
        {
            state.mailer.notify(
                "考古題審核通過",
                &format!(
                    "您在『{}』發布標題為『{}』的考古題已通過管理員的審核。",
                    department.display_name(),
                    post.title.clone().unwrap_or_default()
                ),
                &[owner.mail_address()],
                Some((
                    "查看考古題".to_string(),
                    format!(
                        "{}/{}/{}/{}",
                        state.config.origin_or_empty(),
                        department.id,
                        post.course_id.clone().unwrap_or_default(),
                        post.id
                    ),
                )),
            );
        }
    }

    state.cache.invalidate("posts").await;
    state.cache.invalidate("course-detail").await;
    state.cache.invalidate("dept-courses").await;

    match load_post_detail(&state, &post_id, &user).await? {
        Some(value) => Ok(Json(value)),
        None => Ok(Json(Value::Null)),
    }
}

async fn delete_post(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(post_id): Path<String>,
) -> ApiResult<Json<StatusDto>> {
    if user.id.is_empty() {
        return Err(ApiError::unauthorized("Invalid token: missing user id"));
    }
    let post =
        sqlx::query_as::<_, PostRow>(&format!("SELECT {POST_COLUMNS} FROM posts WHERE id = $1"))
            .bind(&post_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| ApiError::not_found("Post not found"))?;

    if !user.is_super() {
        let department_id = post.department_id.clone().unwrap_or_default();
        if !authz::is_department_admin(&state, &user.id, &department_id).await? {
            return Err(ApiError::forbidden("Forbidden"));
        }
    }

    let urls: Vec<String> = sqlx::query_scalar("SELECT url FROM posts_files WHERE post_id = $1")
        .bind(&post_id)
        .fetch_all(&state.db)
        .await?;
    let mut transaction = state.db.begin().await?;
    sqlx::query("DELETE FROM posts_files WHERE post_id = $1")
        .bind(&post_id)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("DELETE FROM posts WHERE id = $1")
        .bind(&post_id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;

    let keys: Vec<String> = urls
        .iter()
        .map(|url| state.storage.key_from_url(url))
        .collect();
    for key in keys {
        state.storage.delete(&key).await;
    }

    state.cache.invalidate("posts").await;
    state.cache.invalidate("course-detail").await;
    state.cache.invalidate("dept-courses").await;
    Ok(Json(StatusDto { status: "success" }))
}

#[allow(dead_code)]
fn unique(values: Vec<String>) -> HashSet<String> {
    values.into_iter().collect()
}
