use crate::app::AppState;
use crate::db::{self, DepartmentRow, PostRow, UserRow};
use crate::error::{ApiError, ApiResult};

pub fn require_super(user: &UserRow) -> ApiResult<()> {
    if user.is_super() {
        Ok(())
    } else {
        Err(ApiError::forbidden("Forbidden"))
    }
}

pub async fn admin_department_ids(state: &AppState, user: &UserRow) -> ApiResult<Vec<String>> {
    if user.is_super() {
        let rows: Vec<String> =
            sqlx::query_scalar("SELECT id FROM departments ORDER BY sort_order ASC, id ASC")
                .fetch_all(&state.db)
                .await?;
        return Ok(rows);
    }
    let rows: Vec<Option<String>> = sqlx::query_scalar(
        "SELECT department_id FROM users_departments \
         WHERE user_id = $1 AND status = 'APPROVED' AND is_department_admin IS TRUE",
    )
    .bind(&user.id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows.into_iter().flatten().collect())
}

pub async fn visible_department_ids(state: &AppState, user: &UserRow) -> ApiResult<Vec<String>> {
    if user.is_super() {
        let rows: Vec<String> =
            sqlx::query_scalar("SELECT id FROM departments ORDER BY sort_order ASC, id ASC")
                .fetch_all(&state.db)
                .await?;
        return Ok(rows);
    }
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT d.id FROM departments d \
         WHERE d.is_public IS TRUE OR EXISTS ( \
             SELECT 1 FROM users_departments ud \
             WHERE ud.department_id = d.id AND ud.user_id = $1 AND ud.status = 'APPROVED' \
         ) \
         ORDER BY d.sort_order ASC, d.id ASC",
    )
    .bind(&user.id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

pub async fn is_approved(state: &AppState, user_id: &str, department_id: &str) -> ApiResult<bool> {
    Ok(sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM users_departments \
         WHERE user_id = $1 AND department_id = $2 AND status = 'APPROVED')",
    )
    .bind(user_id)
    .bind(department_id)
    .fetch_one(&state.db)
    .await?)
}

pub async fn is_department_admin(
    state: &AppState,
    user_id: &str,
    department_id: &str,
) -> ApiResult<bool> {
    Ok(sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM users_departments \
         WHERE user_id = $1 AND department_id = $2 AND status = 'APPROVED' \
         AND is_department_admin IS TRUE)",
    )
    .bind(user_id)
    .bind(department_id)
    .fetch_one(&state.db)
    .await?)
}

pub async fn can_view_department(
    state: &AppState,
    user: &UserRow,
    department: &DepartmentRow,
) -> ApiResult<bool> {
    if user.is_super() || department.is_public() {
        return Ok(true);
    }
    is_approved(state, &user.id, &department.id).await
}

pub async fn require_viewable_department(
    state: &AppState,
    user: &UserRow,
    department_id: &str,
) -> ApiResult<DepartmentRow> {
    let department = db::get_department(&state.db, department_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Department not found"))?;
    if !can_view_department(state, user, &department).await? {
        return Err(ApiError::not_found("Department not found"));
    }
    Ok(department)
}

pub async fn require_admin(
    state: &AppState,
    user: &UserRow,
    department_id: Option<&str>,
) -> ApiResult<()> {
    if user.is_super() {
        return Ok(());
    }
    let Some(department_id) = department_id else {
        return Err(ApiError::forbidden("Forbidden"));
    };
    if is_department_admin(state, &user.id, department_id).await? {
        Ok(())
    } else {
        Err(ApiError::forbidden("Forbidden"))
    }
}

pub async fn can_view_user_details(
    state: &AppState,
    viewer: &UserRow,
    target_id: &str,
) -> ApiResult<bool> {
    if viewer.id == target_id || viewer.is_super() {
        return Ok(true);
    }
    let admin_ids = admin_department_ids(state, viewer).await?;
    if admin_ids.is_empty() {
        return Ok(false);
    }
    Ok(sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM users_departments \
         WHERE user_id = $1 AND department_id = ANY($2))",
    )
    .bind(target_id)
    .bind(&admin_ids)
    .fetch_one(&state.db)
    .await?)
}

pub async fn can_view_unapproved_post(
    state: &AppState,
    user: &UserRow,
    post: &PostRow,
) -> ApiResult<bool> {
    if user.is_super() || post.owner_id.as_deref() == Some(user.id.as_str()) {
        return Ok(true);
    }
    let department_id = post.department_id.clone().unwrap_or_default();
    is_department_admin(state, &user.id, &department_id).await
}

pub async fn admin_recipient_emails(
    state: &AppState,
    department_id: &str,
) -> ApiResult<Vec<String>> {
    let rows: Vec<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT u.email, u.username FROM users u \
         JOIN users_departments ud ON ud.user_id = u.id \
         WHERE ud.department_id = $1 AND ud.is_department_admin IS TRUE",
    )
    .bind(department_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(email, username)| {
            email.unwrap_or_else(|| format!("s{}@gm.ntpu.edu.tw", username.unwrap_or_default()))
        })
        .collect())
}
