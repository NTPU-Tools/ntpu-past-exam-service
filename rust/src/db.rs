use chrono::{DateTime, Utc};
use sqlx::PgPool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserRow {
    pub id: String,
    pub email: Option<String>,
    pub username: Option<String>,
    pub readable_name: Option<String>,
    pub note: Option<String>,
    pub hashed_password: Option<String>,
    pub school_department: Option<String>,
    pub is_super_user: Option<bool>,
    pub create_time: Option<DateTime<Utc>>,
    pub updated_time: Option<DateTime<Utc>>,
}

impl UserRow {
    pub fn is_super(&self) -> bool {
        self.is_super_user == Some(true)
    }

    pub fn display_name(&self) -> String {
        self.readable_name
            .clone()
            .filter(|name| !name.is_empty())
            .or_else(|| self.username.clone())
            .unwrap_or_default()
    }

    pub fn mail_address(&self) -> String {
        self.email.clone().unwrap_or_else(|| {
            format!(
                "s{}@gm.ntpu.edu.tw",
                self.username.clone().unwrap_or_default()
            )
        })
    }
}

pub const USER_COLUMNS: &str =
    "id, email, username, readable_name, note, hashed_password, school_department, is_super_user, create_time, updated_time";

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DepartmentRow {
    pub id: String,
    #[sqlx(rename = "key")]
    pub key: Option<String>,
    pub name: Option<String>,
    pub is_public: Option<bool>,
    pub sort_order: i32,
    pub create_time: Option<DateTime<Utc>>,
    pub updated_time: Option<DateTime<Utc>>,
}

impl DepartmentRow {
    pub fn is_public(&self) -> bool {
        self.is_public == Some(true)
    }

    pub fn display_name(&self) -> String {
        self.name.clone().unwrap_or_default()
    }
}

pub const DEPARTMENT_COLUMNS: &str =
    "id, key, name, is_public, sort_order, create_time, updated_time";

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CourseRow {
    pub id: String,
    pub name: Option<String>,
    pub category: Option<String>,
    pub department_id: Option<String>,
    pub create_time: Option<DateTime<Utc>>,
    pub updated_time: Option<DateTime<Utc>>,
}

pub const COURSE_COLUMNS: &str = "id, name, category, department_id, create_time, updated_time";

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BulletinRow {
    pub id: String,
    pub title: Option<String>,
    pub content: Option<String>,
    pub department_id: Option<String>,
    pub create_time: Option<DateTime<Utc>>,
    pub updated_time: Option<DateTime<Utc>>,
}

pub const BULLETIN_COLUMNS: &str = "id, title, content, department_id, create_time, updated_time";

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PostRow {
    pub id: String,
    pub title: Option<String>,
    pub content: Option<String>,
    pub owner_id: Option<String>,
    pub course_id: Option<String>,
    pub is_migrate: Option<bool>,
    pub is_anonymous: Option<bool>,
    pub status: Option<String>,
    pub department_id: Option<String>,
    pub create_time: Option<DateTime<Utc>>,
    pub updated_time: Option<DateTime<Utc>>,
}

impl PostRow {
    pub fn is_anonymous(&self) -> bool {
        self.is_anonymous == Some(true)
    }

    pub fn masked_owner(&self) -> Option<String> {
        if self.is_anonymous() {
            Some("anonymous".to_string())
        } else {
            self.owner_id.clone()
        }
    }
}

pub const POST_COLUMNS: &str = "id, title, content, owner_id, course_id, is_migrate, is_anonymous, status, department_id, create_time, updated_time";

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ThreadRow {
    pub id: String,
    pub title: Option<String>,
    pub content: Option<String>,
    pub image_url: Option<String>,
    pub owner_id: Option<String>,
    pub course_id: Option<String>,
    pub is_anonymous: Option<bool>,
    pub like_count: Option<i32>,
    pub create_time: Option<DateTime<Utc>>,
    pub updated_time: Option<DateTime<Utc>>,
}

impl ThreadRow {
    pub fn is_anonymous(&self) -> bool {
        self.is_anonymous == Some(true)
    }
}

pub const THREAD_COLUMNS: &str =
    "id, title, content, image_url, owner_id, course_id, is_anonymous, like_count, create_time, updated_time";

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CommentRow {
    pub id: String,
    pub thread_id: Option<String>,
    pub parent_comment_id: Option<String>,
    pub content: Option<String>,
    pub owner_id: Option<String>,
    pub is_anonymous: Option<bool>,
    pub like_count: Option<i32>,
    pub create_time: Option<DateTime<Utc>>,
    pub updated_time: Option<DateTime<Utc>>,
}

impl CommentRow {
    pub fn is_anonymous(&self) -> bool {
        self.is_anonymous == Some(true)
    }
}

pub const COMMENT_COLUMNS: &str = "id, thread_id, parent_comment_id, content, owner_id, is_anonymous, like_count, create_time, updated_time";

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MembershipRow {
    pub id: String,
    pub user_id: Option<String>,
    pub department_id: Option<String>,
    pub status: Option<String>,
    pub is_department_admin: Option<bool>,
    pub create_time: Option<DateTime<Utc>>,
    pub updated_time: Option<DateTime<Utc>>,
}

pub const MEMBERSHIP_COLUMNS: &str =
    "id, user_id, department_id, status, is_department_admin, create_time, updated_time";

pub async fn get_user(db: &PgPool, id: &str) -> sqlx::Result<Option<UserRow>> {
    sqlx::query_as::<_, UserRow>(&format!("SELECT {USER_COLUMNS} FROM users WHERE id = $1"))
        .bind(id)
        .fetch_optional(db)
        .await
}

pub async fn get_user_by_username(db: &PgPool, username: &str) -> sqlx::Result<Option<UserRow>> {
    sqlx::query_as::<_, UserRow>(&format!(
        "SELECT {USER_COLUMNS} FROM users WHERE username = $1"
    ))
    .bind(username)
    .fetch_optional(db)
    .await
}

pub async fn get_department(db: &PgPool, id: &str) -> sqlx::Result<Option<DepartmentRow>> {
    sqlx::query_as::<_, DepartmentRow>(&format!(
        "SELECT {DEPARTMENT_COLUMNS} FROM departments WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(db)
    .await
}

pub async fn get_course(db: &PgPool, id: &str) -> sqlx::Result<Option<CourseRow>> {
    sqlx::query_as::<_, CourseRow>(&format!(
        "SELECT {COURSE_COLUMNS} FROM courses WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(db)
    .await
}

pub async fn get_membership(
    db: &PgPool,
    user_id: &str,
    department_id: &str,
) -> sqlx::Result<Option<MembershipRow>> {
    sqlx::query_as::<_, MembershipRow>(&format!(
        "SELECT {MEMBERSHIP_COLUMNS} FROM users_departments WHERE user_id = $1 AND department_id = $2 LIMIT 1"
    ))
    .bind(user_id)
    .bind(department_id)
    .fetch_optional(db)
    .await
}

pub async fn get_thread(db: &PgPool, id: &str) -> sqlx::Result<Option<ThreadRow>> {
    sqlx::query_as::<_, ThreadRow>(&format!(
        "SELECT {THREAD_COLUMNS} FROM threads WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(db)
    .await
}

pub async fn get_comment(db: &PgPool, id: &str) -> sqlx::Result<Option<CommentRow>> {
    sqlx::query_as::<_, CommentRow>(&format!(
        "SELECT {COMMENT_COLUMNS} FROM thread_comments WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(db)
    .await
}

// ---------------------------------------------------------------------------
// Flat join rows (sqlx tuples only support scalar Decode, not nested FromRow).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PostListRow {
    pub id: String,
    pub title: Option<String>,
    pub content: Option<String>,
    pub owner_id: Option<String>,
    pub course_id: Option<String>,
    pub is_migrate: Option<bool>,
    pub is_anonymous: Option<bool>,
    pub status: Option<String>,
    pub department_id: Option<String>,
    pub create_time: Option<DateTime<Utc>>,
    pub updated_time: Option<DateTime<Utc>>,
    pub course_name: Option<String>,
}

impl PostListRow {
    pub fn post(&self) -> PostRow {
        PostRow {
            id: self.id.clone(),
            title: self.title.clone(),
            content: self.content.clone(),
            owner_id: self.owner_id.clone(),
            course_id: self.course_id.clone(),
            is_migrate: self.is_migrate,
            is_anonymous: self.is_anonymous,
            status: self.status.clone(),
            department_id: self.department_id.clone(),
            create_time: self.create_time,
            updated_time: self.updated_time,
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PostWithUserRow {
    pub id: String,
    pub title: Option<String>,
    pub content: Option<String>,
    pub owner_id: Option<String>,
    pub course_id: Option<String>,
    pub is_migrate: Option<bool>,
    pub is_anonymous: Option<bool>,
    pub status: Option<String>,
    pub department_id: Option<String>,
    pub create_time: Option<DateTime<Utc>>,
    pub updated_time: Option<DateTime<Utc>>,
    pub owner_user_id: String,
    pub owner_email: Option<String>,
    pub owner_username: Option<String>,
    pub owner_readable_name: Option<String>,
    pub owner_note: Option<String>,
    pub owner_hashed_password: Option<String>,
    pub owner_school_department: Option<String>,
    pub owner_is_super_user: Option<bool>,
    pub owner_create_time: Option<DateTime<Utc>>,
    pub owner_updated_time: Option<DateTime<Utc>>,
}

impl PostWithUserRow {
    pub fn post(&self) -> PostRow {
        PostRow {
            id: self.id.clone(),
            title: self.title.clone(),
            content: self.content.clone(),
            owner_id: self.owner_id.clone(),
            course_id: self.course_id.clone(),
            is_migrate: self.is_migrate,
            is_anonymous: self.is_anonymous,
            status: self.status.clone(),
            department_id: self.department_id.clone(),
            create_time: self.create_time,
            updated_time: self.updated_time,
        }
    }

    pub fn owner(&self) -> UserRow {
        UserRow {
            id: self.owner_user_id.clone(),
            email: self.owner_email.clone(),
            username: self.owner_username.clone(),
            readable_name: self.owner_readable_name.clone(),
            note: self.owner_note.clone(),
            hashed_password: self.owner_hashed_password.clone(),
            school_department: self.owner_school_department.clone(),
            is_super_user: self.owner_is_super_user,
            create_time: self.owner_create_time,
            updated_time: self.owner_updated_time,
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ThreadWithUserRow {
    pub id: String,
    pub title: Option<String>,
    pub content: Option<String>,
    pub image_url: Option<String>,
    pub owner_id: Option<String>,
    pub course_id: Option<String>,
    pub is_anonymous: Option<bool>,
    pub like_count: Option<i32>,
    pub create_time: Option<DateTime<Utc>>,
    pub updated_time: Option<DateTime<Utc>>,
    pub owner_user_id: String,
    pub owner_email: Option<String>,
    pub owner_username: Option<String>,
    pub owner_readable_name: Option<String>,
    pub owner_note: Option<String>,
    pub owner_hashed_password: Option<String>,
    pub owner_school_department: Option<String>,
    pub owner_is_super_user: Option<bool>,
    pub owner_create_time: Option<DateTime<Utc>>,
    pub owner_updated_time: Option<DateTime<Utc>>,
}

impl ThreadWithUserRow {
    pub fn thread(&self) -> ThreadRow {
        ThreadRow {
            id: self.id.clone(),
            title: self.title.clone(),
            content: self.content.clone(),
            image_url: self.image_url.clone(),
            owner_id: self.owner_id.clone(),
            course_id: self.course_id.clone(),
            is_anonymous: self.is_anonymous,
            like_count: self.like_count,
            create_time: self.create_time,
            updated_time: self.updated_time,
        }
    }

    pub fn owner(&self) -> UserRow {
        UserRow {
            id: self.owner_user_id.clone(),
            email: self.owner_email.clone(),
            username: self.owner_username.clone(),
            readable_name: self.owner_readable_name.clone(),
            note: self.owner_note.clone(),
            hashed_password: self.owner_hashed_password.clone(),
            school_department: self.owner_school_department.clone(),
            is_super_user: self.owner_is_super_user,
            create_time: self.owner_create_time,
            updated_time: self.owner_updated_time,
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CommentWithUserRow {
    pub id: String,
    pub thread_id: Option<String>,
    pub parent_comment_id: Option<String>,
    pub content: Option<String>,
    pub owner_id: Option<String>,
    pub is_anonymous: Option<bool>,
    pub like_count: Option<i32>,
    pub create_time: Option<DateTime<Utc>>,
    pub updated_time: Option<DateTime<Utc>>,
    pub owner_user_id: String,
    pub owner_email: Option<String>,
    pub owner_username: Option<String>,
    pub owner_readable_name: Option<String>,
    pub owner_note: Option<String>,
    pub owner_hashed_password: Option<String>,
    pub owner_school_department: Option<String>,
    pub owner_is_super_user: Option<bool>,
    pub owner_create_time: Option<DateTime<Utc>>,
    pub owner_updated_time: Option<DateTime<Utc>>,
}

impl CommentWithUserRow {
    pub fn comment(&self) -> CommentRow {
        CommentRow {
            id: self.id.clone(),
            thread_id: self.thread_id.clone(),
            parent_comment_id: self.parent_comment_id.clone(),
            content: self.content.clone(),
            owner_id: self.owner_id.clone(),
            is_anonymous: self.is_anonymous,
            like_count: self.like_count,
            create_time: self.create_time,
            updated_time: self.updated_time,
        }
    }

    pub fn owner(&self) -> UserRow {
        UserRow {
            id: self.owner_user_id.clone(),
            email: self.owner_email.clone(),
            username: self.owner_username.clone(),
            readable_name: self.owner_readable_name.clone(),
            note: self.owner_note.clone(),
            hashed_password: self.owner_hashed_password.clone(),
            school_department: self.owner_school_department.clone(),
            is_super_user: self.owner_is_super_user,
            create_time: self.owner_create_time,
            updated_time: self.owner_updated_time,
        }
    }
}
