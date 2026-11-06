//! Shared harness for the black-box contract suite.
//!
//! Every test creates its own uniquely named rows directly in PostgreSQL, talks
//! to the server under test over HTTP only, and inspects Redis / object storage
//! where the contract includes shared state (refresh sessions, rate limits,
//! uploaded files).

use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rand::Rng;
use regex::Regex;
use reqwest::header::HeaderMap;
use reqwest::Method;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub use serde_json;

pub const REFRESH_COOKIE: &str = "ntpu-past-exam-refresh-token-v2";
pub const CSRF_HEADER: &str = "X-CSRF-Protection";

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct Config {
    pub base_url: String,
    pub database_url: String,
    pub redis_url: String,
    pub hash_key: String,
    pub file_base: String,
    pub allowed_origin: String,
    pub commit_sha: String,
    pub rate_limit_subject: u32,
    pub rate_limit_global: u32,
    pub rate_limit_window: u64,
}

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

pub fn config() -> &'static Config {
    static CONFIG: OnceLock<Config> = OnceLock::new();
    CONFIG.get_or_init(|| Config {
        base_url: env_or("CONTRACT_BASE_URL", "http://127.0.0.1:58081"),
        database_url: env_or(
            "CONTRACT_DATABASE_URL",
            "postgres://contract:contract@127.0.0.1:55432/past_exam_contract",
        ),
        redis_url: env_or("CONTRACT_REDIS_URL", "redis://127.0.0.1:56379"),
        hash_key: env_or("CONTRACT_HASH_KEY", "contract-test-secret-key"),
        file_base: env_or(
            "CONTRACT_FILE_BASE",
            "http://127.0.0.1:59000/past-exam-contract",
        ),
        allowed_origin: env_or("CONTRACT_ALLOWED_ORIGIN", "http://contract.test"),
        commit_sha: env_or("CONTRACT_COMMIT_SHA", "contract-sha"),
        rate_limit_subject: env_or("CONTRACT_RATE_LIMIT_REQUESTS", "5").parse().unwrap(),
        rate_limit_global: env_or("CONTRACT_RATE_LIMIT_GLOBAL_REQUESTS", "40")
            .parse()
            .unwrap(),
        rate_limit_window: env_or("CONTRACT_RATE_LIMIT_WINDOW_SECONDS", "60")
            .parse()
            .unwrap(),
    })
}

/// Kept so the deliberate-fix tests read the same as before. Every test in this
/// repository targets the Rust implementation.
#[macro_export]
macro_rules! rust_only {
    () => {};
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

pub fn uid(prefix: &str) -> String {
    format!(
        "{prefix}-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..16]
    )
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

pub fn sha256_hex(input: &str) -> String {
    hex::encode(Sha256::digest(input.as_bytes()))
}

/// A unique client address from 198.18.0.0/15 (benchmarking range), sent via
/// X-Forwarded-For so rate-limit buckets never collide between tests.
pub fn random_client_ip() -> String {
    let mut rng = rand::thread_rng();
    format!(
        "198.{}.{}.{}",
        rng.gen_range(18..=19),
        rng.gen_range(0..=255),
        rng.gen_range(1..=254)
    )
}

pub fn ts_offset_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{6})?\+00:00$").unwrap()
    })
}

pub fn ts_z_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{6})?Z$").unwrap())
}

#[track_caller]
pub fn assert_ts_offset(value: &Value) {
    let s = value
        .as_str()
        .unwrap_or_else(|| panic!("expected timestamp string, got {value}"));
    assert!(
        ts_offset_re().is_match(s),
        "expected isoformat '+00:00' timestamp, got {s}"
    );
}

#[track_caller]
pub fn assert_ts_z(value: &Value) {
    let s = value
        .as_str()
        .unwrap_or_else(|| panic!("expected timestamp string, got {value}"));
    assert!(
        ts_z_re().is_match(s),
        "expected pydantic 'Z' timestamp, got {s}"
    );
}

/// Assert that a JSON object has exactly the given keys.
#[track_caller]
pub fn assert_keys(value: &Value, expected: &[&str]) {
    let obj = value
        .as_object()
        .unwrap_or_else(|| panic!("expected JSON object, got {value}"));
    let mut actual: Vec<&str> = obj.keys().map(String::as_str).collect();
    actual.sort_unstable();
    let mut wanted: Vec<&str> = expected.to_vec();
    wanted.sort_unstable();
    assert_eq!(actual, wanted, "unexpected key set in {value}");
}

pub fn ids(values: &Value) -> Vec<String> {
    values
        .as_array()
        .unwrap_or_else(|| panic!("expected JSON array, got {values}"))
        .iter()
        .map(|v| {
            if let Some(s) = v.as_str() {
                s.to_string()
            } else {
                v["id"].as_str().expect("id").to_string()
            }
        })
        .collect()
}

pub const DEPARTMENT_KEYS: &[&str] = &[
    "id",
    "create_time",
    "updated_time",
    "key",
    "name",
    "is_public",
    "sort_order",
];
pub const COURSE_KEYS: &[&str] = &[
    "id",
    "create_time",
    "updated_time",
    "name",
    "category",
    "department_id",
];
pub const BULLETIN_KEYS: &[&str] = &[
    "id",
    "create_time",
    "updated_time",
    "title",
    "content",
    "department_id",
];
pub const USER_KEYS: &[&str] = &[
    "id",
    "create_time",
    "updated_time",
    "email",
    "username",
    "readable_name",
    "note",
    "school_department",
    "is_super_user",
];
pub const USER_REDACTED_KEYS: &[&str] = &[
    "id",
    "create_time",
    "updated_time",
    "username",
    "readable_name",
    "note",
    "school_department",
];
pub const MEMBERSHIP_KEYS: &[&str] = &[
    "id",
    "create_time",
    "updated_time",
    "user_id",
    "department_id",
    "status",
    "is_department_admin",
];
pub const POST_ROW_KEYS: &[&str] = &[
    "id",
    "create_time",
    "updated_time",
    "title",
    "content",
    "owner_id",
    "course_id",
    "is_migrate",
    "is_anonymous",
    "status",
    "department_id",
];
pub const THREAD_KEYS: &[&str] = &[
    "id",
    "title",
    "content",
    "image_url",
    "course_id",
    "is_anonymous",
    "like_count",
    "create_time",
    "updated_time",
    "owner_id",
    "owner_name",
    "is_owner",
    "liked",
];
pub const COMMENT_KEYS: &[&str] = &[
    "id",
    "thread_id",
    "parent_comment_id",
    "content",
    "is_anonymous",
    "like_count",
    "create_time",
    "updated_time",
    "owner_id",
    "owner_name",
    "is_owner",
    "liked",
    "reply_count",
    "replies",
];

pub fn with_keys(base: &[&'static str], extra: &[&'static str]) -> Vec<&'static str> {
    base.iter().chain(extra.iter()).copied().collect()
}

// ---------------------------------------------------------------------------
// JWT
// ---------------------------------------------------------------------------

pub fn sign(claims: &Value) -> String {
    jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
        claims,
        &jsonwebtoken::EncodingKey::from_secret(config().hash_key.as_bytes()),
    )
    .unwrap()
}

pub fn sign_with_key(claims: &Value, key: &str) -> String {
    jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
        claims,
        &jsonwebtoken::EncodingKey::from_secret(key.as_bytes()),
    )
    .unwrap()
}

/// Access token in the exact shape the backend issues (sub/id/type/exp).
pub fn access_token(user_id: &str) -> String {
    sign(&json!({"sub": user_id, "id": user_id, "type": "access", "exp": now() + 900}))
}

/// Decode a token issued by the server, verifying HS256 with the shared key.
pub fn decode(token: &str) -> Value {
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.leeway = 0;
    validation.validate_aud = false;
    validation.required_spec_claims = ["exp", "sub"].iter().map(|s| s.to_string()).collect();
    jsonwebtoken::decode::<Value>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(config().hash_key.as_bytes()),
        &validation,
    )
    .unwrap_or_else(|e| panic!("server-issued token failed verification: {e}"))
    .claims
}

// ---------------------------------------------------------------------------
// HTTP
// ---------------------------------------------------------------------------

pub struct Req {
    rb: reqwest::RequestBuilder,
    forwarded_for: Option<String>,
}

impl Req {
    pub fn header(self, name: &str, value: impl AsRef<str>) -> Self {
        Req {
            rb: self.rb.header(name, value.as_ref()),
            forwarded_for: self.forwarded_for,
        }
    }

    pub fn bearer(self, token: &str) -> Self {
        self.header("Authorization", format!("Bearer {token}"))
    }

    pub fn auth(self, user: &User) -> Self {
        self.bearer(&access_token(&user.id))
    }

    pub fn origin(self, origin: &str) -> Self {
        self.header("Origin", origin)
    }

    /// Allowed Origin + the CSRF marker header required for cookie mutations.
    pub fn csrf(self) -> Self {
        self.origin(&config().allowed_origin)
            .header(CSRF_HEADER, "1")
    }

    pub fn refresh_cookie(self, token: &str) -> Self {
        self.header("Cookie", format!("{REFRESH_COOKIE}={token}"))
    }

    pub fn form(self, fields: &[(&str, &str)]) -> Self {
        Req {
            rb: self.rb.form(fields),
            forwarded_for: self.forwarded_for,
        }
    }

    pub fn multipart(self, form: reqwest::multipart::Form) -> Self {
        Req {
            rb: self.rb.multipart(form),
            forwarded_for: self.forwarded_for,
        }
    }

    pub fn json(self, body: &Value) -> Self {
        Req {
            rb: self.rb.json(body),
            forwarded_for: self.forwarded_for,
        }
    }

    pub fn raw_body(self, content_type: &str, body: impl Into<reqwest::Body>) -> Self {
        Req {
            rb: self.rb.header("Content-Type", content_type).body(body),
            forwarded_for: self.forwarded_for,
        }
    }

    pub fn query(self, params: &[(&str, &str)]) -> Self {
        Req {
            rb: self.rb.query(params),
            forwarded_for: self.forwarded_for,
        }
    }

    /// Override the X-Forwarded-For value presented by this request.
    pub fn forwarded_for(self, value: impl Into<String>) -> Self {
        Req {
            rb: self.rb,
            forwarded_for: Some(value.into()),
        }
    }

    pub async fn send(self) -> Resp {
        let rb = match self.forwarded_for {
            Some(value) => self.rb.header("X-Forwarded-For", value),
            None => self.rb,
        };
        let response = rb.send().await.expect("request failed to send");
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let body = response.bytes().await.expect("read body").to_vec();
        Resp {
            status,
            headers,
            body,
        }
    }
}

#[derive(Debug)]
pub struct Resp {
    pub status: u16,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Resp {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).to_string()
    }

    #[track_caller]
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|e| panic!("invalid JSON ({e}): {}", self.text()))
    }

    #[track_caller]
    pub fn detail(&self) -> String {
        self.json()["detail"]
            .as_str()
            .unwrap_or_else(|| panic!("no string detail in {}", self.text()))
            .to_string()
    }

    pub fn header(&self, name: &str) -> Option<String> {
        self.headers
            .get(name)
            .map(|v| v.to_str().unwrap_or_default().to_string())
    }

    pub fn header_all(&self, name: &str) -> Vec<String> {
        self.headers
            .get_all(name)
            .iter()
            .map(|v| v.to_str().unwrap_or_default().to_string())
            .collect()
    }

    pub fn set_cookies(&self) -> Vec<String> {
        self.header_all("set-cookie")
    }

    /// The Set-Cookie line for the refresh cookie, if any.
    pub fn refresh_set_cookie(&self) -> Option<String> {
        self.set_cookies()
            .into_iter()
            .find(|c| c.starts_with(&format!("{REFRESH_COOKIE}=")))
    }

    /// Value of the refresh cookie set by this response (None when deleted/absent).
    pub fn refresh_cookie_value(&self) -> Option<String> {
        let line = self.refresh_set_cookie()?;
        let value = line
            .split(';')
            .next()?
            .trim_start_matches(&format!("{REFRESH_COOKIE}="))
            .trim_matches('"')
            .to_string();
        (!value.is_empty()).then_some(value)
    }

    #[track_caller]
    pub fn expect(&self, status: u16) -> &Self {
        assert_eq!(
            self.status,
            status,
            "unexpected status, body: {}",
            self.text()
        );
        self
    }

    #[track_caller]
    pub fn expect_detail(&self, status: u16, detail: &str) -> &Self {
        self.expect(status);
        assert_eq!(self.detail(), detail, "unexpected detail");
        self
    }

    /// 422 whose error list contains an entry at `loc` (e.g. ["body","title"]).
    #[track_caller]
    pub fn expect_validation(&self, loc: &[&str], error_type: Option<&str>) -> &Self {
        self.expect(422);
        let errors = self.json()["detail"].clone();
        let errors = errors
            .as_array()
            .unwrap_or_else(|| panic!("422 detail must be an array: {}", self.text()));
        let wanted: Vec<Value> = loc.iter().map(|s| json!(s)).collect();
        let found = errors.iter().find(|e| {
            e["loc"].as_array().map(|l| l == &wanted).unwrap_or(false)
                && e["type"].is_string()
                && e["msg"].is_string()
        });
        let found = found.unwrap_or_else(|| panic!("no 422 entry for {loc:?}: {}", self.text()));
        if let Some(t) = error_type {
            assert_eq!(found["type"], json!(t), "422 type for {loc:?}");
        }
        self
    }
}

// ---------------------------------------------------------------------------
// Seeded rows
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct User {
    pub id: String,
    pub username: String,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct NewUser {
    pub username: Option<String>,
    pub readable_name: Option<String>,
    pub email: Option<String>,
    pub school_department: Option<String>,
    pub note: Option<String>,
    pub password: Option<String>,
    pub super_user: bool,
    /// (show_empty_courses, default_is_anonymous); None = no preferences row.
    pub preferences: Option<(bool, bool)>,
}

#[derive(Debug, Clone)]
pub struct Dept {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Course {
    pub id: String,
    pub name: String,
    pub department_id: String,
}

#[derive(Debug, Clone)]
pub struct Post {
    pub id: String,
    pub course_id: String,
    pub department_id: String,
}

#[derive(Debug, Clone)]
pub struct Thread {
    pub id: String,
    pub course_id: String,
}

#[derive(Debug, Clone, Default)]
pub struct NewPost {
    pub title: Option<String>,
    pub status: Option<String>,
    pub anonymous: bool,
    pub migrate: bool,
    /// Explicit create_time (RFC 3339) for ordering tests.
    pub create_time: Option<String>,
}

pub struct Session {
    pub access: String,
    pub refresh: String,
}

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

pub struct Ctx {
    pub db: tokio_postgres::Client,
    pub redis: redis::aio::MultiplexedConnection,
    http: reqwest::Client,
    /// Client address presented through X-Forwarded-For on every request.
    pub ip: String,
}

impl Ctx {
    pub async fn new() -> Ctx {
        let cfg = config();
        let (db, connection) = tokio_postgres::connect(&cfg.database_url, tokio_postgres::NoTls)
            .await
            .expect("connect to contract PostgreSQL (is the stack up?)");
        tokio::spawn(async move {
            let _ = connection.await;
        });
        let redis = redis::Client::open(cfg.redis_url.as_str())
            .unwrap()
            .get_multiplexed_async_connection()
            .await
            .expect("connect to contract Redis");
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap();
        Ctx {
            db,
            redis,
            http,
            ip: random_client_ip(),
        }
    }

    /// Same connections, different client address.
    pub fn with_ip(mut self, ip: &str) -> Ctx {
        self.ip = ip.to_string();
        self
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", config().base_url, path)
    }

    pub fn req(&self, method: Method, path: &str) -> Req {
        Req {
            rb: self.http.request(method, self.url(path)),
            forwarded_for: Some(self.ip.clone()),
        }
    }

    pub fn get(&self, path: &str) -> Req {
        self.req(Method::GET, path)
    }
    pub fn post(&self, path: &str) -> Req {
        self.req(Method::POST, path)
    }
    pub fn put(&self, path: &str) -> Req {
        self.req(Method::PUT, path)
    }
    pub fn delete(&self, path: &str) -> Req {
        self.req(Method::DELETE, path)
    }

    /// Follow skip/limit pagination (page size 100) and concatenate the pages.
    pub async fn get_all_pages(&self, path: &str, user: &User) -> Vec<Value> {
        let mut all = Vec::new();
        let mut skip = 0;
        loop {
            let page = self
                .get(path)
                .auth(user)
                .query(&[("skip", &skip.to_string()), ("limit", "100")])
                .send()
                .await;
            page.expect(200);
            let items = page.json().as_array().cloned().unwrap();
            let n = items.len();
            all.extend(items);
            if n < 100 || skip >= 10000 {
                break;
            }
            skip += 100;
        }
        all
    }

    /// Password login through the API.
    pub async fn login(&self, user: &User) -> Session {
        let resp = self
            .post("/login")
            .origin(&config().allowed_origin)
            .form(&[
                ("username", &user.username),
                (
                    "password",
                    user.password.as_deref().expect("user has a password"),
                ),
            ])
            .send()
            .await;
        resp.expect(200);
        Session {
            access: resp.json()["access_token"].as_str().unwrap().to_string(),
            refresh: resp.refresh_cookie_value().expect("refresh cookie"),
        }
    }

    // -- object storage ---------------------------------------------------

    pub async fn fetch_object(&self, url: &str) -> Resp {
        Req {
            rb: self.http.get(url),
            forwarded_for: None,
        }
        .send()
        .await
    }

    // -- redis ------------------------------------------------------------

    pub async fn redis_get(&self, key: &str) -> Option<String> {
        let mut con = self.redis.clone();
        redis::cmd("GET")
            .arg(key)
            .query_async(&mut con)
            .await
            .unwrap()
    }

    pub async fn redis_ttl(&self, key: &str) -> i64 {
        let mut con = self.redis.clone();
        redis::cmd("TTL")
            .arg(key)
            .query_async(&mut con)
            .await
            .unwrap()
    }

    pub async fn redis_set(&self, key: &str, value: &str, ttl: u64) {
        let mut con = self.redis.clone();
        let _: () = redis::cmd("SET")
            .arg(key)
            .arg(value)
            .arg("EX")
            .arg(ttl)
            .query_async(&mut con)
            .await
            .unwrap();
    }

    // -- seeding ----------------------------------------------------------

    pub async fn create_user(&self, new: NewUser) -> User {
        let id = uuid::Uuid::new_v4().to_string();
        let username = new.username.clone().unwrap_or_else(|| uid("user"));
        let hashed = new.password.as_ref().map(|p| bcrypt::hash(p, 4).unwrap());
        self.db
            .execute(
                "INSERT INTO users (id, username, readable_name, email, school_department, note, \
                 hashed_password, is_super_user) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
                &[
                    &id,
                    &username,
                    &new.readable_name,
                    &new.email,
                    &new.school_department,
                    &new.note,
                    &hashed,
                    &new.super_user,
                ],
            )
            .await
            .unwrap();
        if let Some((show_empty, anonymous)) = new.preferences {
            self.db
                .execute(
                    "INSERT INTO user_preferences (id, user_id, show_empty_courses, default_is_anonymous) \
                     VALUES ($1,$2,$3,$4)",
                    &[&uuid::Uuid::new_v4().to_string(), &id, &show_empty, &anonymous],
                )
                .await
                .unwrap();
        }
        User {
            id,
            username,
            password: new.password,
        }
    }

    /// Regular user with a password and profile data.
    pub async fn user(&self) -> User {
        let username = uid("user");
        self.create_user(NewUser {
            readable_name: Some(format!("Name {username}")),
            email: Some(format!("{username}@example.test")),
            school_department: Some("CSIE".into()),
            password: Some("correct horse".into()),
            username: Some(username),
            ..Default::default()
        })
        .await
    }

    pub async fn super_user(&self) -> User {
        let username = uid("super");
        self.create_user(NewUser {
            readable_name: Some(format!("Super {username}")),
            email: Some(format!("{username}@example.test")),
            password: Some("correct horse".into()),
            super_user: true,
            username: Some(username),
            ..Default::default()
        })
        .await
    }

    pub async fn create_department(&self, public: bool, sort_order: i32) -> Dept {
        let id = uuid::Uuid::new_v4().to_string();
        let name = uid("dept");
        self.db
            .execute(
                "INSERT INTO departments (id, key, name, is_public, sort_order) VALUES ($1,$2,$3,$4,$5)",
                &[&id, &name, &name, &public, &sort_order],
            )
            .await
            .unwrap();
        Dept { id, name }
    }

    pub async fn department(&self, public: bool) -> Dept {
        self.create_department(public, 500).await
    }

    pub async fn membership(&self, user: &User, dept: &Dept, status: &str, admin: bool) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.db
            .execute(
                "INSERT INTO users_departments (id, user_id, department_id, status, is_department_admin) \
                 VALUES ($1,$2,$3,$4,$5)",
                &[&id, &user.id, &dept.id, &status, &admin],
            )
            .await
            .unwrap();
        id
    }

    pub async fn member(&self, user: &User, dept: &Dept) -> String {
        self.membership(user, dept, "APPROVED", false).await
    }

    pub async fn admin_of(&self, user: &User, dept: &Dept) -> String {
        self.membership(user, dept, "APPROVED", true).await
    }

    pub async fn course(&self, dept: &Dept) -> Course {
        let id = uuid::Uuid::new_v4().to_string();
        let name = uid("course");
        self.db
            .execute(
                "INSERT INTO courses (id, name, category, department_id) VALUES ($1,$2,'required',$3)",
                &[&id, &name, &dept.id],
            )
            .await
            .unwrap();
        Course {
            id,
            name,
            department_id: dept.id.clone(),
        }
    }

    pub async fn create_post(&self, course: &Course, owner: &User, new: NewPost) -> Post {
        let id = uuid::Uuid::new_v4().to_string();
        let title = new.title.unwrap_or_else(|| uid("post"));
        let status = new.status.unwrap_or_else(|| "APPROVED".into());
        self.db
            .execute(
                "INSERT INTO posts (id, title, content, owner_id, course_id, is_migrate, is_anonymous, \
                 status, department_id, create_time) \
                 VALUES ($1,$2,'content',$3,$4,$5,$6,$7,$8, COALESCE(($9::text)::timestamptz, now()))",
                &[
                    &id,
                    &title,
                    &owner.id,
                    &course.id,
                    &new.migrate,
                    &new.anonymous,
                    &status,
                    &course.department_id,
                    &new.create_time,
                ],
            )
            .await
            .unwrap();
        Post {
            id,
            course_id: course.id.clone(),
            department_id: course.department_id.clone(),
        }
    }

    pub async fn seed_post(&self, course: &Course, owner: &User, status: &str) -> Post {
        self.create_post(
            course,
            owner,
            NewPost {
                status: Some(status.into()),
                ..Default::default()
            },
        )
        .await
    }

    pub async fn post_file(&self, post: &Post, url: &str) {
        self.db
            .execute(
                "INSERT INTO posts_files (id, url, post_id) VALUES ($1,$2,$3)",
                &[&uuid::Uuid::new_v4().to_string(), &url, &post.id],
            )
            .await
            .unwrap();
    }

    pub async fn bulletin(&self, dept: &Dept) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.db
            .execute(
                "INSERT INTO bulletins (id, title, content, department_id) VALUES ($1,$2,'body',$3)",
                &[&id, &uid("bulletin"), &dept.id],
            )
            .await
            .unwrap();
        id
    }

    pub async fn create_thread(
        &self,
        course: &Course,
        owner: &User,
        anonymous: bool,
        create_time: Option<&str>,
    ) -> Thread {
        let id = uuid::Uuid::new_v4().to_string();
        self.db
            .execute(
                "INSERT INTO threads (id, title, content, owner_id, course_id, is_anonymous, like_count, create_time) \
                 VALUES ($1,$2,'thread body',$3,$4,$5,0, COALESCE(($6::text)::timestamptz, now()))",
                &[&id, &uid("thread"), &owner.id, &course.id, &anonymous, &create_time],
            )
            .await
            .unwrap();
        Thread {
            id,
            course_id: course.id.clone(),
        }
    }

    pub async fn thread(&self, course: &Course, owner: &User) -> Thread {
        self.create_thread(course, owner, false, None).await
    }

    pub async fn create_comment(
        &self,
        thread: &Thread,
        owner: &User,
        parent: Option<&str>,
        id: Option<&str>,
        create_time: Option<&str>,
    ) -> String {
        let id = id
            .map(str::to_string)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        self.db
            .execute(
                "INSERT INTO thread_comments (id, thread_id, parent_comment_id, content, owner_id, \
                 is_anonymous, like_count, create_time) \
                 VALUES ($1,$2,$3,'comment body',$4,false,0, COALESCE(($5::text)::timestamptz, now()))",
                &[&id, &thread.id, &parent, &owner.id, &create_time],
            )
            .await
            .unwrap();
        id
    }

    pub async fn comment(&self, thread: &Thread, owner: &User, parent: Option<&str>) -> String {
        self.create_comment(thread, owner, parent, None, None).await
    }

    // -- generic queries ---------------------------------------------------

    pub async fn scalar_i64(
        &self,
        sql: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> i64 {
        self.db
            .query_one(sql, params)
            .await
            .unwrap()
            .get::<_, i64>(0)
    }

    pub async fn row(
        &self,
        sql: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> Option<tokio_postgres::Row> {
        self.db.query_opt(sql, params).await.unwrap()
    }
}

/// Sleep until the current fixed rate-limit window has at least `min_remaining`
/// seconds left, so a burst of requests cannot straddle a window boundary.
pub async fn ensure_rate_window_headroom(min_remaining: u64) {
    let window = config().rate_limit_window;
    let remaining = window - (now() as u64 % window);
    if remaining < min_remaining {
        tokio::time::sleep(Duration::from_secs(remaining + 1)).await;
    }
}

pub fn rate_limit_key(identifier: &str) -> String {
    let window = now() as u64 / config().rate_limit_window;
    format!("rate-limit:auth:{window}:{}", &sha256_hex(identifier)[..32])
}

pub fn pdf_bytes(len: usize) -> Vec<u8> {
    let mut data = b"%PDF-1.4\n".to_vec();
    data.resize(len.max(9), b'0');
    data
}

pub fn png_bytes(len: usize) -> Vec<u8> {
    let mut data = b"\x89PNG\r\n\x1a\n".to_vec();
    data.resize(len.max(8), 0);
    data
}

pub fn file_part(bytes: Vec<u8>, filename: &str, content_type: &str) -> reqwest::multipart::Part {
    reqwest::multipart::Part::bytes(bytes)
        .file_name(filename.to_string())
        .mime_str(content_type)
        .unwrap()
}
