use serde::Serialize;

use crate::db::{
    BulletinRow, CommentRow, CourseRow, DepartmentRow, MembershipRow, PostRow, ThreadRow, UserRow,
};
use crate::json_fmt::{TsOffset, TsZ};

#[derive(Debug, Serialize)]
pub struct DepartmentDto {
    pub id: String,
    pub create_time: TsOffset,
    pub updated_time: TsOffset,
    pub key: Option<String>,
    pub name: Option<String>,
    pub is_public: Option<bool>,
    pub sort_order: i32,
}

impl From<&DepartmentRow> for DepartmentDto {
    fn from(row: &DepartmentRow) -> Self {
        DepartmentDto {
            id: row.id.clone(),
            create_time: row.create_time.into(),
            updated_time: row.updated_time.into(),
            key: row.key.clone(),
            name: row.name.clone(),
            is_public: row.is_public,
            sort_order: row.sort_order,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CourseDto {
    pub id: String,
    pub create_time: TsOffset,
    pub updated_time: TsOffset,
    pub name: Option<String>,
    pub category: Option<String>,
    pub department_id: Option<String>,
}

impl From<&CourseRow> for CourseDto {
    fn from(row: &CourseRow) -> Self {
        CourseDto {
            id: row.id.clone(),
            create_time: row.create_time.into(),
            updated_time: row.updated_time.into(),
            name: row.name.clone(),
            category: row.category.clone(),
            department_id: row.department_id.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CourseBriefDto {
    pub id: String,
    pub name: Option<String>,
    pub category: Option<String>,
}

impl From<&CourseRow> for CourseBriefDto {
    fn from(row: &CourseRow) -> Self {
        CourseBriefDto {
            id: row.id.clone(),
            name: row.name.clone(),
            category: row.category.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct DeptCourseDto {
    pub id: String,
    pub name: Option<String>,
    pub category: Option<String>,
    pub department_id: Option<String>,
    pub create_time: TsOffset,
    pub updated_time: TsOffset,
    pub has_posts: bool,
}

#[derive(Debug, Serialize)]
pub struct BulletinDto {
    pub id: String,
    pub create_time: TsOffset,
    pub updated_time: TsOffset,
    pub title: Option<String>,
    pub content: Option<String>,
    pub department_id: Option<String>,
}

impl From<&BulletinRow> for BulletinDto {
    fn from(row: &BulletinRow) -> Self {
        BulletinDto {
            id: row.id.clone(),
            create_time: row.create_time.into(),
            updated_time: row.updated_time.into(),
            title: row.title.clone(),
            content: row.content.clone(),
            department_id: row.department_id.clone(),
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct UserDto {
    pub id: String,
    pub create_time: TsOffset,
    pub updated_time: TsOffset,
    pub email: Option<String>,
    pub username: Option<String>,
    pub readable_name: Option<String>,
    pub note: Option<String>,
    pub school_department: Option<String>,
    pub is_super_user: Option<bool>,
}

impl From<&UserRow> for UserDto {
    fn from(row: &UserRow) -> Self {
        UserDto {
            id: row.id.clone(),
            create_time: row.create_time.into(),
            updated_time: row.updated_time.into(),
            email: row.email.clone(),
            username: row.username.clone(),
            readable_name: row.readable_name.clone(),
            note: row.note.clone(),
            school_department: row.school_department.clone(),
            is_super_user: row.is_super_user,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct UserMeDto {
    #[serde(flatten)]
    pub user: UserDto,
    pub show_empty_courses: bool,
    pub default_is_anonymous: bool,
}

#[derive(Debug, Serialize)]
pub struct UserRedactedDto {
    pub id: String,
    pub create_time: TsOffset,
    pub updated_time: TsOffset,
    pub username: Option<String>,
    pub readable_name: Option<String>,
    pub note: Option<String>,
    pub school_department: Option<String>,
}

impl From<&UserRow> for UserRedactedDto {
    fn from(row: &UserRow) -> Self {
        UserRedactedDto {
            id: row.id.clone(),
            create_time: row.create_time.into(),
            updated_time: row.updated_time.into(),
            username: row.username.clone(),
            readable_name: row.readable_name.clone(),
            note: row.note.clone(),
            school_department: row.school_department.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct MembershipDto {
    pub id: String,
    pub create_time: TsOffset,
    pub updated_time: TsOffset,
    pub user_id: Option<String>,
    pub department_id: Option<String>,
    pub status: Option<String>,
    pub is_department_admin: Option<bool>,
}

impl From<&MembershipRow> for MembershipDto {
    fn from(row: &MembershipRow) -> Self {
        MembershipDto {
            id: row.id.clone(),
            create_time: row.create_time.into(),
            updated_time: row.updated_time.into(),
            user_id: row.user_id.clone(),
            department_id: row.department_id.clone(),
            status: row.status.clone(),
            is_department_admin: row.is_department_admin,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct JoinRequestDto {
    pub id: String,
    pub status: String,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub username: Option<String>,
    pub readable_name: Option<String>,
    pub school_department: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MemberDto {
    pub id: String,
    pub user_id: String,
    pub email: Option<String>,
    pub username: Option<String>,
    pub readable_name: Option<String>,
    pub school_department: Option<String>,
    pub is_department_admin: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct PostBaseDto {
    pub id: String,
    pub create_time: TsOffset,
    pub updated_time: TsOffset,
    pub title: Option<String>,
    pub content: Option<String>,
    pub owner_id: Option<String>,
    pub course_id: Option<String>,
    pub is_migrate: Option<bool>,
    pub is_anonymous: Option<bool>,
    pub status: Option<String>,
    pub department_id: Option<String>,
}

impl From<&PostRow> for PostBaseDto {
    fn from(row: &PostRow) -> Self {
        PostBaseDto {
            id: row.id.clone(),
            create_time: row.create_time.into(),
            updated_time: row.updated_time.into(),
            title: row.title.clone(),
            content: row.content.clone(),
            owner_id: row.masked_owner(),
            course_id: row.course_id.clone(),
            is_migrate: row.is_migrate,
            is_anonymous: row.is_anonymous,
            status: row.status.clone(),
            department_id: row.department_id.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PostListDto {
    #[serde(flatten)]
    pub post: PostBaseDto,
    pub course_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PostDetailDto {
    #[serde(flatten)]
    pub post: PostBaseDto,
    pub files: Vec<String>,
    pub owner_name: String,
}

#[derive(Debug, Serialize)]
pub struct CoursePostDto {
    pub id: String,
    pub create_time: TsOffset,
    pub updated_time: TsOffset,
    pub title: Option<String>,
    pub content: Option<String>,
    pub course_id: Option<String>,
    pub is_migrate: Option<bool>,
    pub is_anonymous: Option<bool>,
    pub department_id: Option<String>,
}

impl From<&PostRow> for CoursePostDto {
    fn from(row: &PostRow) -> Self {
        CoursePostDto {
            id: row.id.clone(),
            create_time: row.create_time.into(),
            updated_time: row.updated_time.into(),
            title: row.title.clone(),
            content: row.content.clone(),
            course_id: row.course_id.clone(),
            is_migrate: row.is_migrate,
            is_anonymous: row.is_anonymous,
            department_id: row.department_id.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ThreadDto {
    pub id: String,
    pub title: Option<String>,
    pub content: Option<String>,
    pub image_url: Option<String>,
    pub course_id: Option<String>,
    pub is_anonymous: bool,
    pub like_count: i64,
    pub create_time: TsZ,
    pub updated_time: TsZ,
    pub owner_id: Option<String>,
    pub owner_name: String,
    pub is_owner: bool,
    pub liked: bool,
}

pub fn thread_dto(
    row: &ThreadRow,
    owner: Option<&UserRow>,
    current_user_id: &str,
    liked: bool,
) -> ThreadDto {
    let anonymous = row.is_anonymous();
    ThreadDto {
        id: row.id.clone(),
        title: row.title.clone(),
        content: row.content.clone(),
        image_url: row.image_url.clone(),
        course_id: row.course_id.clone(),
        is_anonymous: anonymous,
        like_count: i64::from(row.like_count.unwrap_or(0)),
        create_time: row.create_time.into(),
        updated_time: row.updated_time.into(),
        owner_id: if anonymous {
            Some("anonymous".to_string())
        } else {
            row.owner_id.clone()
        },
        owner_name: if anonymous {
            "匿名用戶".to_string()
        } else {
            owner.map(UserRow::display_name).unwrap_or_default()
        },
        is_owner: row.owner_id.as_deref() == Some(current_user_id),
        liked,
    }
}

#[derive(Debug, Serialize)]
pub struct CommentDto {
    pub id: String,
    pub thread_id: Option<String>,
    pub parent_comment_id: Option<String>,
    pub content: Option<String>,
    pub is_anonymous: bool,
    pub like_count: i64,
    pub create_time: TsZ,
    pub updated_time: TsZ,
    pub owner_id: Option<String>,
    pub owner_name: String,
    pub is_owner: bool,
    pub liked: bool,
    pub reply_count: Option<i64>,
    pub replies: Option<Vec<CommentDto>>,
}

pub fn comment_dto(
    row: &CommentRow,
    owner: Option<&UserRow>,
    current_user_id: &str,
    liked: bool,
    reply_count: Option<i64>,
    replies: Option<Vec<CommentDto>>,
) -> CommentDto {
    let anonymous = row.is_anonymous();
    CommentDto {
        id: row.id.clone(),
        thread_id: row.thread_id.clone(),
        parent_comment_id: row.parent_comment_id.clone(),
        content: row.content.clone(),
        is_anonymous: anonymous,
        like_count: i64::from(row.like_count.unwrap_or(0)),
        create_time: row.create_time.into(),
        updated_time: row.updated_time.into(),
        owner_id: if anonymous {
            Some("anonymous".to_string())
        } else {
            row.owner_id.clone()
        },
        owner_name: if anonymous {
            "匿名用戶".to_string()
        } else {
            owner.map(UserRow::display_name).unwrap_or_default()
        },
        is_owner: row.owner_id.as_deref() == Some(current_user_id),
        liked,
        reply_count,
        replies,
    }
}

#[derive(Debug, Serialize)]
pub struct AuthTokenDto {
    pub access_token: String,
    pub token_type: &'static str,
}

impl AuthTokenDto {
    pub fn bearer(access_token: String) -> Self {
        AuthTokenDto {
            access_token,
            token_type: "bearer",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct StatusDto {
    pub status: &'static str,
}

#[derive(Debug, Serialize)]
pub struct VerifyTokenDto {
    pub admin: Vec<String>,
    pub visible_departments: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateUserDto {
    pub status: &'static str,
    pub user_id: String,
}

#[derive(Debug, Serialize)]
pub struct CreatePostDto {
    pub status: &'static str,
    pub post_id: String,
}

#[derive(Debug, Serialize)]
pub struct CreateThreadDto {
    pub status: &'static str,
    pub thread_id: String,
}

#[derive(Debug, Serialize)]
pub struct CreateCommentDto {
    pub status: &'static str,
    pub comment_id: String,
}

#[derive(Debug, Serialize)]
pub struct UpdateThreadDto {
    pub status: &'static str,
    pub thread_id: String,
}

#[derive(Debug, Serialize)]
pub struct LikeThreadDto {
    pub thread: ThreadDto,
    pub liked: bool,
}

#[derive(Debug, Serialize)]
pub struct LikeCommentDto {
    pub comment: CommentDto,
    pub liked: bool,
}

#[derive(Debug, Serialize)]
pub struct PreferencesDto {
    pub show_empty_courses: bool,
    pub default_is_anonymous: bool,
}
