use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::http::Uri;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::extractor::authenticate_thread;
use crate::authz;
use crate::db::{
    self, CommentRow, CommentWithUserRow, CourseRow, ThreadRow, ThreadWithUserRow, UserRow,
    COMMENT_COLUMNS, THREAD_COLUMNS,
};
use crate::dto::{
    comment_dto, thread_dto, CommentDto, CreateCommentDto, CreateThreadDto, LikeCommentDto,
    LikeThreadDto, ThreadDto, UpdateThreadDto,
};
use crate::error::{ApiError, ApiResult};
use crate::extract::{clamped_pagination, Fields, RawForm};

const MAX_THREAD_IMAGE_BYTES: usize = 5 * 1024 * 1024;
const ALLOWED_THREAD_IMAGE_TYPES: [&str; 4] =
    ["image/jpeg", "image/png", "image/webp", "image/gif"];
const MAX_REPLY_DEPTH: usize = 10;
const MAX_TOTAL_REPLIES: usize = 500;

const THREAD_SELECT: &str = "t.id, t.title, t.content, t.image_url, t.owner_id, t.course_id, \
    t.is_anonymous, t.like_count, t.create_time, t.updated_time";
const USER_SELECT: &str = "u.id AS owner_user_id, u.email AS owner_email, u.username AS owner_username, u.readable_name AS owner_readable_name, u.note AS owner_note, u.hashed_password AS owner_hashed_password, u.school_department AS owner_school_department, u.is_super_user AS owner_is_super_user, u.create_time AS owner_create_time, u.updated_time AS owner_updated_time";
const COMMENT_SELECT: &str = "c.id, c.thread_id, c.parent_comment_id, c.content, c.owner_id, \
    c.is_anonymous, c.like_count, c.create_time, c.updated_time";

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/threads/{thread_id}",
            get(get_threads)
                .post(create_thread)
                .put(update_thread)
                .delete(delete_thread),
        )
        .route("/threads/detail/{thread_id}", get(get_thread_detail))
        .route("/threads/{thread_id}/like", post(like_thread))
        .route(
            "/threads/{thread_id}/comments",
            get(get_comments).post(create_comment),
        )
        .route(
            "/threads/comments/{comment_id}",
            get(get_comment_detail)
                .put(update_comment)
                .delete(delete_comment),
        )
        .route("/threads/comments/{comment_id}/like", post(comment_like))
}

async fn visible_course(state: &AppState, user: &UserRow, course_id: &str) -> ApiResult<CourseRow> {
    let course = db::get_course(&state.db, course_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Course not found"))?;
    authz::require_viewable_department(state, user, course.department_id.as_deref().unwrap_or(""))
        .await?;
    Ok(course)
}

async fn visible_thread(
    state: &AppState,
    user: &UserRow,
    thread_id: &str,
) -> ApiResult<(ThreadRow, CourseRow)> {
    let thread = db::get_thread(&state.db, thread_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Thread not found"))?;
    let course = visible_course(state, user, thread.course_id.as_deref().unwrap_or("")).await?;
    Ok((thread, course))
}

async fn visible_comment(
    state: &AppState,
    user: &UserRow,
    comment_id: &str,
) -> ApiResult<(CommentRow, ThreadRow, CourseRow)> {
    let comment = db::get_comment(&state.db, comment_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Comment not found"))?;
    let (thread, course) =
        visible_thread(state, user, comment.thread_id.as_deref().unwrap_or("")).await?;
    Ok((comment, thread, course))
}

pub(crate) fn valid_image_magic(data: &[u8]) -> bool {
    data.starts_with(&[0xFF, 0xD8, 0xFF])
        || data.starts_with(b"\x89PNG\r\n\x1a\n")
        || data.starts_with(b"GIF87a")
        || data.starts_with(b"GIF89a")
        || (data.starts_with(b"RIFF") && data.len() >= 12 && &data[8..12] == b"WEBP")
}

async fn create_thread(
    State(state): State<Arc<AppState>>,
    Path(course_id): Path<String>,
    headers: HeaderMap,
    form: RawForm,
) -> ApiResult<Json<CreateThreadDto>> {
    let mut fields = Fields::new(&form.0);
    let title = fields.string("title", 1, Some(200), true, "");
    let content = fields.string("content", 1, Some(2000), true, "");
    let is_anonymous = fields.boolean("is_anonymous", false);
    fields.finish(())?;

    let user = authenticate_thread(&state, &headers).await?;
    visible_course(&state, &user, &course_id).await?;

    let mut image = None;
    if let Some(upload) = form.0.files_named("image").last() {
        let content_type = upload.content_type.clone().unwrap_or_default();
        if !ALLOWED_THREAD_IMAGE_TYPES.contains(&content_type.as_str()) {
            return Err(ApiError::bad_request("Unsupported image type"));
        }
        if upload.data.len() > MAX_THREAD_IMAGE_BYTES {
            return Err(ApiError::too_large("Image too large"));
        }
        if !valid_image_magic(&upload.data) {
            return Err(ApiError::bad_request("Invalid image content"));
        }
        image = Some((upload.data.clone(), content_type));
    }

    let thread_id = Uuid::new_v4().to_string();
    let mut image_key = None;
    if let Some((data, content_type)) = image {
        let key = format!("threads/{}", Uuid::new_v4());
        state
            .storage
            .put(&key, data, &content_type)
            .await
            .map_err(ApiError::internal)?;
        image_key = Some(key);
    }
    let image_url = image_key.as_ref().map(|key| state.storage.url(key));
    let inserted = sqlx::query(
        "INSERT INTO threads (id, title, content, image_url, owner_id, course_id, is_anonymous, like_count) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, 0)",
    )
    .bind(&thread_id)
    .bind(&title)
    .bind(&content)
    .bind(image_url)
    .bind(&user.id)
    .bind(&course_id)
    .bind(is_anonymous)
    .execute(&state.db)
    .await;
    if let Err(error) = inserted {
        if let Some(key) = &image_key {
            state.storage.delete(key).await;
        }
        return Err(ApiError::from(error));
    }
    Ok(Json(CreateThreadDto {
        status: "success",
        thread_id,
    }))
}

async fn get_threads(
    State(state): State<Arc<AppState>>,
    Path(course_id): Path<String>,
    headers: HeaderMap,
    uri: Uri,
) -> ApiResult<Json<Value>> {
    let (skip, limit) = clamped_pagination(&uri)?;
    let user = authenticate_thread(&state, &headers).await?;
    visible_course(&state, &user, &course_id).await?;

    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM threads WHERE course_id = $1")
        .bind(&course_id)
        .fetch_one(&state.db)
        .await?;
    let rows: Vec<ThreadWithUserRow> = sqlx::query_as(&format!(
        "SELECT {THREAD_SELECT}, {USER_SELECT} FROM threads t \
         JOIN users u ON t.owner_id = u.id WHERE t.course_id = $1 \
         ORDER BY t.create_time DESC, t.id DESC OFFSET $2 LIMIT $3"
    ))
    .bind(&course_id)
    .bind(skip)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    let ids: Vec<String> = rows.iter().map(|row| row.id.clone()).collect();
    let liked: HashSet<String> = if ids.is_empty() {
        HashSet::new()
    } else {
        sqlx::query_scalar::<_, String>(
            "SELECT thread_id FROM thread_likes WHERE thread_id = ANY($1) AND user_id = $2",
        )
        .bind(&ids)
        .bind(&user.id)
        .fetch_all(&state.db)
        .await?
        .into_iter()
        .collect()
    };
    let threads: Vec<ThreadDto> = rows
        .iter()
        .map(|row| {
            thread_dto(
                &row.thread(),
                Some(&row.owner()),
                &user.id,
                liked.contains(&row.id),
            )
        })
        .collect();
    Ok(Json(json!({ "threads": threads, "total": total })))
}

async fn get_thread_detail(
    State(state): State<Arc<AppState>>,
    Path(thread_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<ThreadDto>> {
    let user = authenticate_thread(&state, &headers).await?;
    let (_, course) = visible_thread(&state, &user, &thread_id).await?;
    let _ = course;
    let row: Option<ThreadWithUserRow> = sqlx::query_as(&format!(
        "SELECT {THREAD_SELECT}, {USER_SELECT} FROM threads t \
         JOIN users u ON t.owner_id = u.id WHERE t.id = $1"
    ))
    .bind(&thread_id)
    .fetch_optional(&state.db)
    .await?;
    let Some(row) = row else {
        return Err(ApiError::not_found("Thread not found"));
    };
    let thread = row.thread();
    let liked = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM thread_likes WHERE thread_id = $1 AND user_id = $2)",
    )
    .bind(&thread_id)
    .bind(&user.id)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(thread_dto(
        &thread,
        Some(&row.owner()),
        &user.id,
        liked,
    )))
}

async fn update_thread(
    State(state): State<Arc<AppState>>,
    Path(thread_id): Path<String>,
    headers: HeaderMap,
    form: RawForm,
) -> ApiResult<Json<UpdateThreadDto>> {
    let mut fields = Fields::new(&form.0);
    let title = fields.optional("title", 1, Some(200));
    let content = fields.optional("content", 1, Some(2000));
    fields.finish(())?;

    let user = authenticate_thread(&state, &headers).await?;
    let (thread, _) = visible_thread(&state, &user, &thread_id).await?;
    if thread.owner_id.as_deref() != Some(user.id.as_str()) {
        return Err(ApiError::forbidden("Permission denied"));
    }
    if title.is_some() || content.is_some() {
        sqlx::query(
            "UPDATE threads SET title = COALESCE($2, title), content = COALESCE($3, content), \
             updated_time = CASE WHEN ($2 IS NOT NULL AND title IS DISTINCT FROM $2) \
                 OR ($3 IS NOT NULL AND content IS DISTINCT FROM $3) THEN now() ELSE updated_time END \
             WHERE id = $1",
        )
        .bind(&thread_id)
        .bind(&title)
        .bind(&content)
        .execute(&state.db)
        .await?;
    }
    Ok(Json(UpdateThreadDto {
        status: "success",
        thread_id,
    }))
}

async fn delete_thread(
    State(state): State<Arc<AppState>>,
    Path(thread_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    let user = authenticate_thread(&state, &headers).await?;
    let (thread, course) = visible_thread(&state, &user, &thread_id).await?;

    let mut can_delete = user.is_super() || thread.owner_id.as_deref() == Some(user.id.as_str());
    if !can_delete {
        can_delete = authz::is_department_admin(
            &state,
            &user.id,
            course.department_id.as_deref().unwrap_or(""),
        )
        .await?;
    }
    if !can_delete {
        return Err(ApiError::forbidden("Permission denied"));
    }

    let image_key = thread
        .image_url
        .as_ref()
        .map(|url| state.storage.key_from_url(url));
    sqlx::query("DELETE FROM threads WHERE id = $1")
        .bind(&thread_id)
        .execute(&state.db)
        .await?;
    if let Some(key) = image_key {
        state.storage.delete(&key).await;
    }
    Ok(Json(json!({ "status": "success" })))
}

async fn like_thread(
    State(state): State<Arc<AppState>>,
    Path(thread_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<LikeThreadDto>> {
    let user = authenticate_thread(&state, &headers).await?;
    visible_thread(&state, &user, &thread_id).await?;

    let mut transaction = state.db.begin().await?;
    let thread = sqlx::query_as::<_, ThreadRow>(&format!(
        "SELECT {THREAD_COLUMNS} FROM threads WHERE id = $1 FOR UPDATE"
    ))
    .bind(&thread_id)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(thread) = thread else {
        return Err(ApiError::not_found("Thread not found"));
    };
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT id FROM thread_likes WHERE thread_id = $1 AND user_id = $2 FOR UPDATE",
    )
    .bind(&thread_id)
    .bind(&user.id)
    .fetch_optional(&mut *transaction)
    .await?;
    let current = i64::from(thread.like_count.unwrap_or(0));
    if existing.is_some() {
        sqlx::query("DELETE FROM thread_likes WHERE id = $1")
            .bind(existing)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("UPDATE threads SET like_count = $2, updated_time = now() WHERE id = $1")
            .bind(&thread_id)
            .bind((current - 1).max(0) as i32)
            .execute(&mut *transaction)
            .await?;
    } else {
        sqlx::query("INSERT INTO thread_likes (id, thread_id, user_id) VALUES ($1, $2, $3)")
            .bind(Uuid::new_v4().to_string())
            .bind(&thread_id)
            .bind(&user.id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("UPDATE threads SET like_count = $2, updated_time = now() WHERE id = $1")
            .bind(&thread_id)
            .bind((current + 1) as i32)
            .execute(&mut *transaction)
            .await?;
    }
    match transaction.commit().await {
        Ok(()) => {}
        Err(error) if is_unique_violation(&error) => {}
        Err(error) => return Err(ApiError::from(error)),
    }

    let thread = db::get_thread(&state.db, &thread_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Thread not found"))?;
    let owner = match thread.owner_id.as_deref() {
        Some(owner_id) => db::get_user(&state.db, owner_id).await?,
        None => None,
    };
    let liked = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM thread_likes WHERE thread_id = $1 AND user_id = $2)",
    )
    .bind(&thread_id)
    .bind(&user.id)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(LikeThreadDto {
        thread: thread_dto(&thread, owner.as_ref(), &user.id, liked),
        liked,
    }))
}

async fn create_comment(
    State(state): State<Arc<AppState>>,
    Path(thread_id): Path<String>,
    headers: HeaderMap,
    form: RawForm,
) -> ApiResult<Json<CreateCommentDto>> {
    let mut fields = Fields::new(&form.0);
    let content = fields.string("content", 1, Some(2000), true, "");
    let is_anonymous = fields.boolean("is_anonymous", false);
    let parent_comment_id = fields.string("parent_comment_id", 0, None, false, "");
    fields.finish(())?;
    let parent_comment_id = Some(parent_comment_id).filter(|value| !value.is_empty());

    let user = authenticate_thread(&state, &headers).await?;
    visible_thread(&state, &user, &thread_id).await?;

    if let Some(parent_id) = &parent_comment_id {
        let parent = db::get_comment(&state.db, parent_id)
            .await?
            .ok_or_else(|| ApiError::not_found("Parent comment not found"))?;
        if parent.thread_id.as_deref() != Some(thread_id.as_str()) {
            return Err(ApiError::bad_request(
                "Parent comment does not belong to this thread",
            ));
        }
    }

    let comment_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO thread_comments (id, thread_id, parent_comment_id, content, owner_id, is_anonymous, like_count) \
         VALUES ($1, $2, $3, $4, $5, $6, 0)",
    )
    .bind(&comment_id)
    .bind(&thread_id)
    .bind(&parent_comment_id)
    .bind(&content)
    .bind(&user.id)
    .bind(is_anonymous)
    .execute(&state.db)
    .await?;
    Ok(Json(CreateCommentDto {
        status: "success",
        comment_id,
    }))
}

async fn get_comments(
    State(state): State<Arc<AppState>>,
    Path(thread_id): Path<String>,
    headers: HeaderMap,
    uri: Uri,
) -> ApiResult<Json<Value>> {
    let (skip, limit) = clamped_pagination(&uri)?;
    let user = authenticate_thread(&state, &headers).await?;
    visible_thread(&state, &user, &thread_id).await?;

    let rows: Vec<CommentWithUserRow> = sqlx::query_as(&format!(
        "SELECT {COMMENT_SELECT}, {USER_SELECT} FROM thread_comments c \
         JOIN users u ON c.owner_id = u.id \
         WHERE c.thread_id = $1 AND c.parent_comment_id IS NULL \
         ORDER BY c.create_time ASC, c.id ASC OFFSET $2 LIMIT $3"
    ))
    .bind(&thread_id)
    .bind(skip)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    let ids: Vec<String> = rows.iter().map(|row| row.id.clone()).collect();
    if ids.is_empty() {
        return Ok(Json(json!([])));
    }
    let reply_rows: Vec<(Option<String>, i64)> = sqlx::query_as(
        "SELECT parent_comment_id, count(*) FROM thread_comments \
         WHERE parent_comment_id = ANY($1) GROUP BY parent_comment_id",
    )
    .bind(&ids)
    .fetch_all(&state.db)
    .await?;
    let reply_counts: HashMap<String, i64> = reply_rows
        .into_iter()
        .filter_map(|(parent, count)| parent.map(|parent| (parent, count)))
        .collect();
    let liked: HashSet<String> = sqlx::query_scalar::<_, String>(
        "SELECT comment_id FROM comment_likes WHERE comment_id = ANY($1) AND user_id = $2",
    )
    .bind(&ids)
    .bind(&user.id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .collect();
    let comments: Vec<CommentDto> = rows
        .iter()
        .map(|row| {
            comment_dto(
                &row.comment(),
                Some(&row.owner()),
                &user.id,
                liked.contains(&row.id),
                Some(*reply_counts.get(&row.id).unwrap_or(&0)),
                None,
            )
        })
        .collect();
    Ok(Json(
        serde_json::to_value(comments).map_err(ApiError::internal)?,
    ))
}

async fn get_comment_detail(
    State(state): State<Arc<AppState>>,
    Path(comment_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<CommentDto>> {
    let user = authenticate_thread(&state, &headers).await?;
    let (root, _, _) = visible_comment(&state, &user, &comment_id).await?;

    let Some(root_owner) = db::get_user(&state.db, root.owner_id.as_deref().unwrap_or("")).await?
    else {
        return Err(ApiError::not_found("Comment not found"));
    };

    let mut map: HashMap<String, CommentDto> = HashMap::new();
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    let mut order: Vec<String> = vec![comment_id.clone()];
    let mut seen: HashSet<String> = HashSet::from([comment_id.clone()]);
    map.insert(
        comment_id.clone(),
        comment_dto(
            &root,
            Some(&root_owner),
            &user.id,
            false,
            Some(0),
            Some(Vec::new()),
        ),
    );

    let mut current = vec![comment_id.clone()];
    let mut depth = 0usize;
    while !current.is_empty() && depth < MAX_REPLY_DEPTH && map.len() - 1 < MAX_TOTAL_REPLIES {
        let remaining = MAX_TOTAL_REPLIES.saturating_sub(map.len() - 1);
        let rows: Vec<CommentWithUserRow> = sqlx::query_as(&format!(
            "SELECT {COMMENT_SELECT}, {USER_SELECT} FROM thread_comments c \
             JOIN users u ON c.owner_id = u.id \
             WHERE c.parent_comment_id = ANY($1) \
             ORDER BY c.create_time ASC, c.id ASC LIMIT $2"
        ))
        .bind(&current)
        .bind(remaining as i64)
        .fetch_all(&state.db)
        .await?;
        if rows.is_empty() {
            break;
        }
        let mut next = Vec::new();
        for row in rows {
            if map.len() > MAX_TOTAL_REPLIES {
                break;
            }
            if !seen.insert(row.id.clone()) {
                continue;
            }
            if let Some(parent) = &row.parent_comment_id {
                children
                    .entry(parent.clone())
                    .or_default()
                    .push(row.id.clone());
            }
            order.push(row.id.clone());
            next.push(row.id.clone());
            map.insert(
                row.id.clone(),
                comment_dto(
                    &row.comment(),
                    Some(&row.owner()),
                    &user.id,
                    false,
                    Some(0),
                    Some(Vec::new()),
                ),
            );
        }
        current = next;
        depth += 1;
    }

    let ids: Vec<String> = map.keys().cloned().collect();
    let liked: HashSet<String> = sqlx::query_scalar::<_, String>(
        "SELECT comment_id FROM comment_likes WHERE comment_id = ANY($1) AND user_id = $2",
    )
    .bind(&ids)
    .bind(&user.id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .collect();
    for node in map.values_mut() {
        node.liked = liked.contains(&node.id);
    }

    for id in order.iter().rev() {
        let child_ids = children.remove(id).unwrap_or_default();
        let mut rebuilt = Vec::new();
        for child in child_ids {
            if let Some(node) = map.remove(&child) {
                rebuilt.push(node);
            }
        }
        if let Some(node) = map.get_mut(id) {
            node.reply_count = Some(rebuilt.len() as i64);
            node.replies = Some(rebuilt);
        }
    }

    let root = map
        .remove(&comment_id)
        .ok_or_else(|| ApiError::not_found("Comment not found"))?;
    Ok(Json(root))
}

async fn update_comment(
    State(state): State<Arc<AppState>>,
    Path(comment_id): Path<String>,
    headers: HeaderMap,
    form: RawForm,
) -> ApiResult<Json<Value>> {
    let mut fields = Fields::new(&form.0);
    let content = fields.optional("content", 1, Some(2000));
    fields.finish(())?;

    let user = authenticate_thread(&state, &headers).await?;
    let (comment, _, _) = visible_comment(&state, &user, &comment_id).await?;
    if comment.owner_id.as_deref() != Some(user.id.as_str()) {
        return Err(ApiError::forbidden("Permission denied"));
    }
    let Some(content) = content else {
        return Err(ApiError::bad_request("Content is required"));
    };
    sqlx::query(
        "UPDATE thread_comments SET content = $2, \
         updated_time = CASE WHEN content IS DISTINCT FROM $2 THEN now() ELSE updated_time END \
         WHERE id = $1",
    )
    .bind(&comment_id)
    .bind(&content)
    .execute(&state.db)
    .await?;
    Ok(Json(
        json!({ "status": "success", "comment_id": comment_id }),
    ))
}

async fn delete_comment(
    State(state): State<Arc<AppState>>,
    Path(comment_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    let user = authenticate_thread(&state, &headers).await?;
    let (comment, _, course) = visible_comment(&state, &user, &comment_id).await?;

    let mut can_delete = user.is_super() || comment.owner_id.as_deref() == Some(user.id.as_str());
    if !can_delete {
        can_delete = authz::is_department_admin(
            &state,
            &user.id,
            course.department_id.as_deref().unwrap_or(""),
        )
        .await?;
    }
    if !can_delete {
        return Err(ApiError::forbidden("Permission denied"));
    }

    let deleted = sqlx::query("DELETE FROM thread_comments WHERE id = $1")
        .bind(&comment_id)
        .execute(&state.db)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(ApiError::not_found("Comment not found"));
    }
    Ok(Json(json!({ "status": "success" })))
}

async fn comment_like(
    State(state): State<Arc<AppState>>,
    Path(comment_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<LikeCommentDto>> {
    let user = authenticate_thread(&state, &headers).await?;
    visible_comment(&state, &user, &comment_id).await?;

    let mut transaction = state.db.begin().await?;
    let comment = sqlx::query_as::<_, CommentRow>(&format!(
        "SELECT {COMMENT_COLUMNS} FROM thread_comments WHERE id = $1 FOR UPDATE"
    ))
    .bind(&comment_id)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(comment) = comment else {
        return Err(ApiError::not_found("Comment not found"));
    };
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT id FROM comment_likes WHERE comment_id = $1 AND user_id = $2 FOR UPDATE",
    )
    .bind(&comment_id)
    .bind(&user.id)
    .fetch_optional(&mut *transaction)
    .await?;
    let current = i64::from(comment.like_count.unwrap_or(0));
    if existing.is_some() {
        sqlx::query("DELETE FROM comment_likes WHERE id = $1")
            .bind(existing)
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            "UPDATE thread_comments SET like_count = $2, updated_time = now() WHERE id = $1",
        )
        .bind(&comment_id)
        .bind((current - 1).max(0) as i32)
        .execute(&mut *transaction)
        .await?;
    } else {
        sqlx::query("INSERT INTO comment_likes (id, comment_id, user_id) VALUES ($1, $2, $3)")
            .bind(Uuid::new_v4().to_string())
            .bind(&comment_id)
            .bind(&user.id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            "UPDATE thread_comments SET like_count = $2, updated_time = now() WHERE id = $1",
        )
        .bind(&comment_id)
        .bind((current + 1) as i32)
        .execute(&mut *transaction)
        .await?;
    }
    match transaction.commit().await {
        Ok(()) => {}
        Err(error) if is_unique_violation(&error) => {}
        Err(error) => return Err(ApiError::from(error)),
    }

    let comment = db::get_comment(&state.db, &comment_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Comment not found"))?;
    let owner = match comment.owner_id.as_deref() {
        Some(owner_id) => db::get_user(&state.db, owner_id).await?,
        None => None,
    };
    let liked = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM comment_likes WHERE comment_id = $1 AND user_id = $2)",
    )
    .bind(&comment_id)
    .bind(&user.id)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(LikeCommentDto {
        comment: comment_dto(&comment, owner.as_ref(), &user.id, liked, None, None),
        liked,
    }))
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(database) if database.code().as_deref() == Some("23505"))
}
