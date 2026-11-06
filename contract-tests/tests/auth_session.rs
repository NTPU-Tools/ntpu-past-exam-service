//! Login, refresh sessions, cookies, CSRF/Origin, logout and /create_user
//! (03-auth.md §3.4–3.7, 04-api.md §4.2).
//!
//! The contract environment runs with AUTH_COOKIE_SECURE=false and
//! AUTH_ALLOW_LEGACY_REFRESH_BEARER=true (see contract.env).

use past_exam_contract_tests::*;
use serde_json::{json, Value};

const YEAR: i64 = 31_536_000;

fn session_key(sid: &str) -> String {
    format!("auth:refresh-session:{}", sha256_hex(sid))
}

fn legacy_refresh_token(user: &User) -> String {
    sign(&json!({"sub": user.id, "id": user.id, "type": "refresh", "exp": now() + YEAR}))
}

async fn login_resp(ctx: &Ctx, user: &User) -> Resp {
    ctx.post("/login")
        .origin(&config().allowed_origin)
        .form(&[
            ("username", &user.username),
            ("password", user.password.as_deref().unwrap()),
        ])
        .send()
        .await
}

#[track_caller]
fn assert_token_response(resp: &Resp) {
    resp.expect(200);
    let body = resp.json();
    assert_keys(&body, &["access_token", "token_type"]);
    assert_eq!(body["token_type"], json!("bearer"));
    assert_eq!(resp.header("cache-control").as_deref(), Some("no-store"));
    assert_eq!(resp.header("pragma").as_deref(), Some("no-cache"));
}

#[track_caller]
fn assert_refresh_cookie_attributes(set_cookie: &str) {
    let lower = set_cookie.to_lowercase();
    assert!(lower.contains("httponly"), "{set_cookie}");
    assert!(lower.contains("samesite=lax"), "{set_cookie}");
    assert!(lower.contains("path=/refresh"), "{set_cookie}");
    assert!(lower.contains("max-age=31536000"), "{set_cookie}");
    assert!(lower.contains("expires="), "{set_cookie}");
    assert!(!lower.contains("domain="), "{set_cookie}");
    assert!(
        !lower.contains("secure"),
        "AUTH_COOKIE_SECURE=false: {set_cookie}"
    );
}

#[track_caller]
fn assert_deleted_refresh_cookie(resp: &Resp) {
    let line = resp.refresh_set_cookie().expect("refresh cookie deletion");
    let lower = line.to_lowercase();
    assert!(lower.contains("max-age=0"), "{line}");
    assert!(lower.contains("path=/refresh"), "{line}");
    assert!(lower.contains("httponly"), "{line}");
    assert!(lower.contains("samesite=lax"), "{line}");
    assert!(
        !lower.contains("secure"),
        "AUTH_COOKIE_SECURE=false: {line}"
    );
    assert!(resp.refresh_cookie_value().is_none(), "{line}");
}

#[tokio::test]
async fn login_returns_access_token_and_sets_refresh_cookie() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let resp = login_resp(&ctx, &user).await;
    assert_token_response(&resp);
    assert_refresh_cookie_attributes(&resp.refresh_set_cookie().unwrap());
}

#[tokio::test]
async fn issued_tokens_have_the_documented_claims() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let admin_dept = ctx.department(false).await;
    ctx.admin_of(&user, &admin_dept).await;

    let resp = login_resp(&ctx, &user).await;
    resp.expect(200);
    let access = decode(resp.json()["access_token"].as_str().unwrap());
    assert_eq!(access["sub"], json!(user.id));
    assert_eq!(access["id"], json!(user.id));
    assert_eq!(access["type"], json!("access"));
    assert_eq!(access["isu"], json!(false));
    let adm: Value =
        serde_json::from_str(access["adm"].as_str().expect("adm is a string")).unwrap();
    assert_eq!(adm, json!([admin_dept.id]));
    let lifetime = access["exp"].as_i64().unwrap() - now();
    assert!(
        (890..=900).contains(&lifetime),
        "access lifetime {lifetime}"
    );
    assert!(access.get("sid").is_none() && access.get("jti").is_none());

    let refresh = decode(&resp.refresh_cookie_value().unwrap());
    assert_eq!(refresh["type"], json!("refresh"));
    assert_eq!(refresh["session_version"], json!(1));
    assert_eq!(refresh["sub"], json!(user.id));
    let lifetime = refresh["exp"].as_i64().unwrap() - now();
    assert!(
        (YEAR - 10..=YEAR).contains(&lifetime),
        "refresh lifetime {lifetime}"
    );

    let sid = refresh["sid"].as_str().unwrap();
    let jti = refresh["jti"].as_str().unwrap();
    assert!(uuid::Uuid::parse_str(sid).is_ok() && uuid::Uuid::parse_str(jti).is_ok());
    assert_eq!(ctx.redis_get(&session_key(sid)).await.as_deref(), Some(jti));
    let ttl = ctx.redis_ttl(&session_key(sid)).await;
    assert!((YEAR - 10..=YEAR).contains(&ttl), "session ttl {ttl}");
}

#[tokio::test]
async fn super_user_tokens_carry_isu_and_all_departments() {
    let ctx = Ctx::new().await;
    let admin = ctx.super_user().await;
    let dept = ctx.department(false).await;
    let resp = login_resp(&ctx, &admin).await;
    let access = decode(resp.json()["access_token"].as_str().unwrap());
    assert_eq!(access["isu"], json!(true));
    let adm: Vec<String> = serde_json::from_str(access["adm"].as_str().unwrap()).unwrap();
    assert!(adm.contains(&dept.id));
}

#[tokio::test]
async fn login_failures() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let no_password = ctx.create_user(NewUser::default()).await;

    for (username, password) in [
        (user.username.as_str(), "wrong"),
        ("no-such-user-xyz", "whatever"),
        (no_password.username.as_str(), "anything"),
    ] {
        ctx.post("/login")
            .form(&[("username", username), ("password", password)])
            .send()
            .await
            .expect_detail(400, "incorrect username or password");
    }

    ctx.post("/login")
        .form(&[("username", &user.username)])
        .send()
        .await
        .expect_validation(&["body", "password"], Some("missing"));
    ctx.post("/login")
        .form(&[
            ("username", &user.username),
            ("password", "correct horse"),
            ("grant_type", "client_credentials"),
        ])
        .send()
        .await
        .expect_validation(&["body", "grant_type"], None);
    ctx.post("/login")
        .form(&[
            ("username", &user.username),
            ("password", "correct horse"),
            ("grant_type", "password"),
        ])
        .send()
        .await
        .expect(200);
}

#[tokio::test]
async fn login_origin_checks() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    for origin in ["https://evil.example", "null", "https://past-exam.ntpu.cc"] {
        // Only AUTH_ALLOWED_ORIGINS/ORIGIN count; the static CORS list does not.
        ctx.post("/login")
            .origin(origin)
            .form(&[("username", &user.username), ("password", "correct horse")])
            .send()
            .await
            .expect_detail(403, "Origin is not allowed");
    }
    // Non-browser clients without Origin are allowed.
    ctx.post("/login")
        .form(&[("username", &user.username), ("password", "correct horse")])
        .send()
        .await
        .expect(200);
    // Every comma-separated AUTH_ALLOWED_ORIGINS entry is accepted.
    ctx.post("/login")
        .origin("http://localhost:5173")
        .form(&[("username", &user.username), ("password", "correct horse")])
        .send()
        .await
        .expect(200);
}

#[tokio::test]
async fn cookie_refresh_rotates_the_session() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let session = ctx.login(&user).await;
    let old = decode(&session.refresh);

    let resp = ctx
        .post("/refresh")
        .csrf()
        .refresh_cookie(&session.refresh)
        .send()
        .await;
    assert_token_response(&resp);
    let new_cookie = resp.refresh_cookie_value().unwrap();
    assert_refresh_cookie_attributes(&resp.refresh_set_cookie().unwrap());
    assert_ne!(new_cookie, session.refresh);

    let new = decode(&new_cookie);
    assert_eq!(new["sid"], old["sid"], "session family id is preserved");
    assert_ne!(new["jti"], old["jti"]);
    assert_eq!(
        ctx.redis_get(&session_key(new["sid"].as_str().unwrap()))
            .await,
        new["jti"].as_str().map(str::to_string)
    );
    let access = decode(resp.json()["access_token"].as_str().unwrap());
    assert_eq!(access["type"], json!("access"));
}

#[tokio::test]
async fn rotated_refresh_token_cannot_be_replayed() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let session = ctx.login(&user).await;
    ctx.post("/refresh")
        .csrf()
        .refresh_cookie(&session.refresh)
        .send()
        .await
        .expect(200);
    ctx.post("/refresh")
        .csrf()
        .refresh_cookie(&session.refresh)
        .send()
        .await
        .expect_detail(401, "Refresh session is no longer active");
}

#[tokio::test]
async fn refresh_reissues_claims_from_live_data() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let session = ctx.login(&user).await;
    let dept = ctx.department(false).await;
    ctx.admin_of(&user, &dept).await;

    let resp = ctx
        .post("/refresh")
        .csrf()
        .refresh_cookie(&session.refresh)
        .send()
        .await;
    let access = decode(resp.json()["access_token"].as_str().unwrap());
    let adm: Vec<String> = serde_json::from_str(access["adm"].as_str().unwrap()).unwrap();
    assert_eq!(adm, vec![dept.id]);
}

#[tokio::test]
async fn refresh_via_bearer_header_with_managed_token() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let session = ctx.login(&user).await;
    let resp = ctx.post("/refresh").bearer(&session.refresh).send().await;
    assert_token_response(&resp);
    assert!(resp.refresh_cookie_value().is_some());
}

#[tokio::test]
async fn refresh_rejections() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let exp = now() + YEAR;

    // No cookie, no bearer.
    ctx.post("/refresh")
        .send()
        .await
        .expect_detail(401, "Invalid credentials");
    ctx.post("/refresh")
        .header("Authorization", "Bearer ")
        .send()
        .await
        .expect_detail(401, "Invalid credentials");
    // An access token is not a refresh token.
    ctx.post("/refresh")
        .bearer(&access_token(&user.id))
        .send()
        .await
        .expect_detail(401, "Invalid token type");
    // Managed shape but unknown to Redis.
    let unknown = sign(&json!({
        "sub": user.id, "id": user.id, "type": "refresh", "exp": exp,
        "session_version": 1, "sid": uid("sid"), "jti": uid("jti"),
    }));
    ctx.post("/refresh")
        .csrf()
        .refresh_cookie(&unknown)
        .send()
        .await
        .expect_detail(401, "Refresh session is no longer active");
    // Neither managed nor legacy (jti without sid).
    let untracked =
        sign(&json!({"sub": user.id, "id": user.id, "type": "refresh", "exp": exp, "jti": "x"}));
    ctx.post("/refresh")
        .csrf()
        .refresh_cookie(&untracked)
        .send()
        .await
        .expect_detail(401, "Unknown refresh session");
    // Legacy tokens are never accepted from the cookie.
    ctx.post("/refresh")
        .csrf()
        .refresh_cookie(&legacy_refresh_token(&user))
        .send()
        .await
        .expect_detail(401, "Legacy refresh bearer migration is disabled");
    // Expired refresh token.
    let expired =
        sign(&json!({"sub": user.id, "id": user.id, "type": "refresh", "exp": now() - 5}));
    ctx.post("/refresh")
        .bearer(&expired)
        .send()
        .await
        .expect_detail(401, "Invalid or expired token");
}

#[tokio::test]
async fn refresh_for_deleted_user_is_rejected_without_rotating() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let session = ctx.login(&user).await;
    let claims = decode(&session.refresh);
    ctx.db
        .execute("DELETE FROM users WHERE id = $1", &[&user.id])
        .await
        .unwrap();

    ctx.post("/refresh")
        .csrf()
        .refresh_cookie(&session.refresh)
        .send()
        .await
        .expect_detail(401, "Invalid credentials");
    assert_eq!(
        ctx.redis_get(&session_key(claims["sid"].as_str().unwrap()))
            .await,
        claims["jti"].as_str().map(str::to_string)
    );
}

#[tokio::test]
async fn cookie_refresh_requires_origin_and_csrf_header() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let session = ctx.login(&user).await;
    let allowed = config().allowed_origin.clone();
    let cases: Vec<(Vec<(&str, &str)>, &str)> = vec![
        (vec![], "Origin header is required"),
        (vec![(CSRF_HEADER, "1")], "Origin header is required"),
        (
            vec![("Origin", &allowed)],
            "X-CSRF-Protection header is required",
        ),
        (
            vec![("Origin", &allowed), (CSRF_HEADER, "true")],
            "X-CSRF-Protection header is required",
        ),
        (
            vec![("Origin", "https://evil.example"), (CSRF_HEADER, "1")],
            "Origin is not allowed",
        ),
        (
            vec![("Origin", "null"), (CSRF_HEADER, "1")],
            "Origin is not allowed",
        ),
    ];
    for (headers, detail) in cases {
        let mut req = ctx.post("/refresh").refresh_cookie(&session.refresh);
        for (k, v) in &headers {
            req = req.header(k, v);
        }
        req.send().await.expect_detail(403, detail);
    }
    // The session was not consumed by the rejected attempts.
    ctx.post("/refresh")
        .csrf()
        .refresh_cookie(&session.refresh)
        .send()
        .await
        .expect(200);
}

#[tokio::test]
async fn cookie_takes_precedence_over_bearer() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let session = ctx.login(&user).await;
    ctx.post("/refresh")
        .refresh_cookie(&session.refresh)
        .bearer(&legacy_refresh_token(&user))
        .send()
        .await
        .expect_detail(403, "Origin header is required");
}

#[tokio::test]
async fn legacy_refresh_bearer_is_rejected_after_removal() {
    rust_only!(); // F-18: the migration window closed on 2026-11-03.
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let legacy = legacy_refresh_token(&user);
    ctx.post("/refresh")
        .bearer(&legacy)
        .send()
        .await
        .expect_detail(401, "Legacy refresh bearer migration is disabled");
}

#[tokio::test]
async fn refresh_logout_with_a_legacy_cookie_is_idempotent() {
    rust_only!(); // F-18: nothing server-side remains to revoke for legacy tokens.
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let legacy = legacy_refresh_token(&user);
    ctx.post("/refresh/logout")
        .csrf()
        .refresh_cookie(&legacy)
        .send()
        .await
        .expect(200);
    assert_eq!(
        ctx.redis_get(&format!("auth:legacy-refresh-used:{}", sha256_hex(&legacy)))
            .await,
        None
    );
}

#[tokio::test]
async fn logout_endpoints_delete_the_cookie() {
    let ctx = Ctx::new().await;
    for endpoint in ["/logout", "/refresh/logout"] {
        let resp = ctx.post(endpoint).csrf().send().await;
        resp.expect(200);
        assert_eq!(resp.json(), json!({"status": "success"}));
        assert_eq!(resp.header("cache-control").as_deref(), Some("no-store"));
        assert_eq!(resp.header("pragma").as_deref(), Some("no-cache"));
        assert_deleted_refresh_cookie(&resp);
    }
}

#[tokio::test]
async fn logout_endpoints_require_origin_and_csrf() {
    let ctx = Ctx::new().await;
    let allowed = config().allowed_origin.clone();
    let cases: Vec<Vec<(&str, &str)>> = vec![
        vec![],
        vec![("Origin", &allowed)],
        vec![(CSRF_HEADER, "1")],
        vec![("Origin", &allowed), (CSRF_HEADER, "wrong")],
        vec![("Origin", "https://evil.example"), (CSRF_HEADER, "1")],
    ];
    for endpoint in ["/logout", "/refresh/logout"] {
        for headers in &cases {
            let mut req = ctx.post(endpoint);
            for (k, v) in headers {
                req = req.header(k, v);
            }
            req.send().await.expect(403);
        }
    }
}

#[tokio::test]
async fn refresh_logout_revokes_the_whole_session_family() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let session = ctx.login(&user).await;
    let sid = decode(&session.refresh)["sid"]
        .as_str()
        .unwrap()
        .to_string();
    let rotated = ctx
        .post("/refresh")
        .csrf()
        .refresh_cookie(&session.refresh)
        .send()
        .await;
    let current = rotated.refresh_cookie_value().unwrap();

    // Logging out with the stale (pre-rotation) cookie still revokes the family.
    let resp = ctx
        .post("/refresh/logout")
        .csrf()
        .refresh_cookie(&session.refresh)
        .send()
        .await;
    resp.expect(200);
    assert_deleted_refresh_cookie(&resp);
    assert_eq!(ctx.redis_get(&session_key(&sid)).await, None);
    ctx.post("/refresh")
        .csrf()
        .refresh_cookie(&current)
        .send()
        .await
        .expect_detail(401, "Refresh session is no longer active");
}

#[tokio::test]
async fn refresh_logout_ignores_invalid_cookies() {
    let ctx = Ctx::new().await;
    ctx.post("/refresh/logout")
        .csrf()
        .refresh_cookie("garbage")
        .send()
        .await
        .expect(200);
}

#[tokio::test]
async fn refresh_logout_with_unknown_session_shape_is_rejected() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let untracked = sign(
        &json!({"sub": user.id, "id": user.id, "type": "refresh", "exp": now() + YEAR, "jti": "x"}),
    );
    ctx.post("/refresh/logout")
        .csrf()
        .refresh_cookie(&untracked)
        .send()
        .await
        .expect_detail(401, "Unknown refresh session");
}

#[tokio::test]
async fn sessions_created_by_another_implementation_can_be_rotated() {
    // Cut-over compatibility: a session written to Redis with the documented key
    // and a token signed with the shared key must be accepted.
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let sid = uuid::Uuid::new_v4().to_string();
    let jti = uuid::Uuid::new_v4().to_string();
    ctx.redis_set(&session_key(&sid), &jti, YEAR as u64).await;
    let token = sign(&json!({
        "sub": user.id, "id": user.id, "type": "refresh", "isu": false, "adm": "[]",
        "session_version": 1, "sid": sid, "jti": jti, "exp": now() + YEAR,
    }));
    let resp = ctx
        .post("/refresh")
        .csrf()
        .refresh_cookie(&token)
        .send()
        .await;
    resp.expect(200);
    assert_eq!(
        decode(&resp.refresh_cookie_value().unwrap())["sid"],
        json!(sid)
    );
}

#[tokio::test]
async fn create_user_by_super_user() {
    let ctx = Ctx::new().await;
    let admin = ctx.super_user().await;
    let username = uid("created");

    let resp = ctx
        .post("/create_user")
        .auth(&admin)
        .form(&[("username", &username), ("password", "s3cret pass")])
        .send()
        .await;
    resp.expect(200);
    assert_eq!(resp.header("cache-control").as_deref(), Some("no-store"));
    assert!(
        resp.set_cookies().is_empty(),
        "must not replace the caller's session"
    );
    let body = resp.json();
    assert_keys(&body, &["status", "user_id"]);
    assert_eq!(body["status"], json!("success"));
    let user_id = body["user_id"].as_str().unwrap().to_string();

    let row = ctx
        .row(
            "SELECT username, readable_name, email, school_department, is_super_user, hashed_password \
             FROM users WHERE id = $1",
            &[&user_id],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), username);
    assert_eq!(
        row.get::<_, Option<String>>(1).as_deref(),
        Some(username.as_str())
    );
    assert_eq!(row.get::<_, Option<String>>(2), None);
    assert_eq!(row.get::<_, Option<String>>(3), None);
    assert_eq!(row.get::<_, Option<bool>>(4), Some(false));
    let hash: String = row.get(5);
    assert!(hash.starts_with("$2b$12$"), "{hash}");
    let prefs = ctx
        .row(
            "SELECT show_empty_courses, default_is_anonymous FROM user_preferences WHERE user_id = $1",
            &[&user_id],
        )
        .await
        .expect("preferences row");
    assert!(prefs.get::<_, bool>(0));
    assert!(!prefs.get::<_, bool>(1));

    // The created account can log in with its password.
    let created = User {
        id: user_id,
        username: username.clone(),
        password: Some("s3cret pass".into()),
    };
    ctx.login(&created).await;

    ctx.post("/create_user")
        .auth(&admin)
        .form(&[("username", &username), ("password", "other")])
        .send()
        .await
        .expect_detail(400, "Same username already exist.");
}

#[tokio::test]
async fn create_user_input_and_permission_errors() {
    let ctx = Ctx::new().await;
    let admin = ctx.super_user().await;
    let regular = ctx.user().await;

    ctx.post("/create_user")
        .auth(&regular)
        .form(&[("username", "x"), ("password", "y")])
        .send()
        .await
        .expect_detail(403, "Forbidden");
    ctx.post("/create_user")
        .form(&[("username", "x"), ("password", "y")])
        .send()
        .await
        .expect(401);
    ctx.post("/create_user")
        .auth(&admin)
        .form(&[("username", "x")])
        .send()
        .await
        .expect_validation(&["body", "password"], Some("missing"));
    ctx.post("/create_user")
        .auth(&admin)
        .form(&[("username", ""), ("password", "y")])
        .send()
        .await
        .expect_detail(400, "Invalid input");
}

#[tokio::test]
async fn exchange_checks_origin_and_input_before_contacting_google() {
    // These requests never reach Google: Origin is rejected first, and missing
    // fields fail validation after the rate limiter.
    let ctx = Ctx::new().await;
    ctx.post("/exchange")
        .origin("https://evil.example")
        .form(&[("code", "c"), ("redirect_uri", "r")])
        .send()
        .await
        .expect_detail(403, "Origin is not allowed");
    ctx.post("/exchange")
        .origin(&config().allowed_origin)
        .form(&[("code", "c")])
        .send()
        .await
        .expect_validation(&["body", "redirect_uri"], Some("missing"));
}
