//! Health endpoints, routing, CORS, Cache-Control, error envelopes, validation
//! envelope and timestamp formats (02-runtime.md).

use past_exam_contract_tests::*;
use serde_json::json;

#[tokio::test]
async fn ping_returns_json_pong_with_no_store() {
    let ctx = Ctx::new().await;
    let resp = ctx.get("/ping").send().await;
    resp.expect(200);
    assert_eq!(resp.json(), json!("pong"));
    assert!(resp.header("content-type").unwrap().starts_with("application/json"));
    assert_eq!(resp.header("cache-control").as_deref(), Some("no-store"));
}

#[tokio::test]
async fn system_version_reports_commit_sha() {
    let ctx = Ctx::new().await;
    let resp = ctx.get("/system-version").send().await;
    resp.expect(200);
    assert_eq!(resp.json(), json!({"GIT_SHA": config().commit_sha}));
}

#[tokio::test]
async fn unknown_route_is_json_404() {
    let ctx = Ctx::new().await;
    ctx.get("/definitely/not/a/route")
        .send()
        .await
        .expect_detail(404, "Not Found");
}

#[tokio::test]
async fn wrong_method_is_json_405_with_allow_header() {
    let ctx = Ctx::new().await;
    let resp = ctx.delete("/ping").send().await;
    resp.expect_detail(405, "Method Not Allowed");
    assert!(resp.header("allow").unwrap().contains("GET"));
}

#[tokio::test]
async fn cors_preflight_for_allowed_origin() {
    let ctx = Ctx::new().await;
    for origin in ["http://localhost:5173", "https://past-exam.ntpu.cc", config().allowed_origin.as_str()] {
        let resp = ctx
            .req(reqwest::Method::OPTIONS, "/posts")
            .origin(origin)
            .header("Access-Control-Request-Method", "POST")
            .header("Access-Control-Request-Headers", "authorization,x-csrf-protection")
            .send()
            .await;
        resp.expect(200);
        assert_eq!(resp.header("access-control-allow-origin").as_deref(), Some(origin));
        assert_eq!(resp.header("access-control-allow-credentials").as_deref(), Some("true"));
        let methods = resp.header("access-control-allow-methods").unwrap();
        for m in ["DELETE", "GET", "OPTIONS", "PATCH", "POST", "PUT"] {
            assert!(methods.contains(m), "missing {m} in {methods}");
        }
        let allowed_headers = resp.header("access-control-allow-headers").unwrap().to_lowercase();
        assert!(allowed_headers.contains("authorization"));
        assert!(allowed_headers.contains("x-csrf-protection"));
        assert_eq!(resp.header("access-control-max-age").as_deref(), Some("600"));
        assert!(resp.header_all("vary").join(",").to_lowercase().contains("origin"));
    }
}

#[tokio::test]
async fn cors_preflight_for_disallowed_origin_is_rejected() {
    let ctx = Ctx::new().await;
    let resp = ctx
        .req(reqwest::Method::OPTIONS, "/posts")
        .origin("https://evil.example")
        .header("Access-Control-Request-Method", "POST")
        .send()
        .await;
    resp.expect(400);
    assert!(resp.header("access-control-allow-origin").is_none());
    assert_eq!(resp.text(), "Disallowed CORS origin");
}

#[tokio::test]
async fn cors_simple_request_headers() {
    let ctx = Ctx::new().await;
    let allowed = ctx.get("/ping").origin("https://past-exam.ntpu.xyz").send().await;
    allowed.expect(200);
    assert_eq!(
        allowed.header("access-control-allow-origin").as_deref(),
        Some("https://past-exam.ntpu.xyz")
    );
    assert_eq!(allowed.header("access-control-allow-credentials").as_deref(), Some("true"));

    let denied = ctx.get("/ping").origin("https://evil.example").send().await;
    denied.expect(200);
    assert!(denied.header("access-control-allow-origin").is_none());
}

#[tokio::test]
async fn successful_get_responses_are_marked_no_store() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let resp = ctx.get("/departments").auth(&user).send().await;
    resp.expect(200);
    assert_eq!(resp.header("cache-control").as_deref(), Some("no-store"));
}

#[tokio::test]
async fn validation_errors_use_the_fastapi_envelope() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;

    ctx.get("/posts")
        .auth(&user)
        .query(&[("skip", "abc")])
        .send()
        .await
        .expect_validation(&["query", "skip"], Some("int_parsing"));

    let resp = ctx.post("/posts").auth(&user).form(&[("title", "")]).send().await;
    resp.expect_validation(&["body", "title"], Some("string_too_short"))
        .expect_validation(&["body", "course_id"], Some("missing"));
}

#[tokio::test]
async fn json_body_validation() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;

    ctx.put("/users/me/preferences")
        .auth(&user)
        .json(&json!({"show_empty_courses": "maybe"}))
        .send()
        .await
        .expect_validation(&["body", "show_empty_courses"], Some("bool_parsing"))
        .expect_validation(&["body", "default_is_anonymous"], Some("missing"));

    let resp = ctx
        .put("/users/me/preferences")
        .auth(&user)
        .raw_body("application/json", "{bad")
        .send()
        .await;
    resp.expect(422);
    let first = &resp.json()["detail"][0];
    assert_eq!(first["type"], json!("json_invalid"));
    assert_eq!(first["loc"][0], json!("body"));
}

#[tokio::test]
async fn form_booleans_follow_pydantic_lax_parsing() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let thread = ctx.thread(&course, &user).await;
    let path = format!("/threads/{}/comments", thread.id);

    for (raw, expected) in [("YES", true), ("on", true), ("T", true), ("1", true), ("off", false), ("n", false), ("0", false)] {
        let resp = ctx
            .post(&path)
            .auth(&user)
            .form(&[("content", "x"), ("is_anonymous", raw)])
            .send()
            .await;
        resp.expect(200);
        let id = resp.json()["comment_id"].as_str().unwrap().to_string();
        let row = ctx
            .row("SELECT is_anonymous FROM thread_comments WHERE id = $1", &[&id])
            .await
            .unwrap();
        assert_eq!(row.get::<_, bool>(0), expected, "form boolean {raw}");
    }

    ctx.post(&path)
        .auth(&user)
        .form(&[("content", "x"), ("is_anonymous", "2")])
        .send()
        .await
        .expect_validation(&["body", "is_anonymous"], Some("bool_parsing"));
}

#[tokio::test]
async fn string_lengths_are_counted_in_code_points() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;

    // 200 CJK characters = 600 UTF-8 bytes, still within the 200-character limit.
    let title: String = "考".repeat(200);
    ctx.post(&format!("/threads/{}", course.id))
        .auth(&user)
        .form(&[("title", &title), ("content", "x")])
        .send()
        .await
        .expect(200);

    let too_long: String = "考".repeat(201);
    ctx.post(&format!("/threads/{}", course.id))
        .auth(&user)
        .form(&[("title", &too_long), ("content", "x")])
        .send()
        .await
        .expect_validation(&["body", "title"], Some("string_too_long"));
}

#[tokio::test]
async fn timestamps_use_isoformat_outside_threads_and_z_inside() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let with_micros = ctx
        .create_post(
            &course,
            &user,
            NewPost {
                create_time: Some("2026-01-02T03:04:05.123456Z".into()),
                ..Default::default()
            },
        )
        .await;
    let whole_second = ctx
        .create_post(
            &course,
            &user,
            NewPost {
                create_time: Some("2026-01-02T03:04:05Z".into()),
                ..Default::default()
            },
        )
        .await;

    let a = ctx.get(&format!("/posts/{}", with_micros.id)).auth(&user).send().await;
    assert_eq!(a.json()["create_time"], json!("2026-01-02T03:04:05.123456+00:00"));
    assert_eq!(a.json()["updated_time"], json!(null));
    let b = ctx.get(&format!("/posts/{}", whole_second.id)).auth(&user).send().await;
    assert_eq!(b.json()["create_time"], json!("2026-01-02T03:04:05+00:00"));

    let t1 = ctx
        .create_thread(&course, &user, false, Some("2026-01-02T03:04:05.12Z"))
        .await;
    let t2 = ctx
        .create_thread(&course, &user, false, Some("2026-01-02T03:04:05Z"))
        .await;
    let a = ctx.get(&format!("/threads/detail/{}", t1.id)).auth(&user).send().await;
    assert_eq!(a.json()["create_time"], json!("2026-01-02T03:04:05.120000Z"));
    let b = ctx.get(&format!("/threads/detail/{}", t2.id)).auth(&user).send().await;
    assert_eq!(b.json()["create_time"], json!("2026-01-02T03:04:05Z"));
}

#[tokio::test]
async fn non_ascii_is_emitted_as_raw_utf8() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let post = ctx
        .create_post(&course, &user, NewPost { anonymous: true, ..Default::default() })
        .await;
    let resp = ctx.get(&format!("/posts/{}", post.id)).auth(&user).send().await;
    resp.expect(200);
    assert!(resp.text().contains("匿名用戶"), "expected raw UTF-8: {}", resp.text());
}

#[tokio::test]
async fn repeated_form_fields_take_the_last_value() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let resp = ctx
        .post(&format!("/threads/{}", course.id))
        .auth(&user)
        .form(&[("title", "first"), ("title", "last"), ("content", "x")])
        .send()
        .await;
    resp.expect(200);
    let id = resp.json()["thread_id"].as_str().unwrap().to_string();
    let title: String = ctx.row("SELECT title FROM threads WHERE id = $1", &[&id]).await.unwrap().get(0);
    assert_eq!(title, "last");
}

#[tokio::test]
async fn form_endpoints_treat_other_content_types_as_an_empty_form() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    ctx.post(&format!("/threads/{}", course.id))
        .auth(&user)
        .json(&json!({"title": "x", "content": "y"}))
        .send()
        .await
        .expect_validation(&["body", "title"], Some("missing"));
}

#[tokio::test]
async fn openapi_schema_is_not_exposed() {
    rust_only!(); // F-14: the reference still serves FastAPI's default /openapi.json.
    let ctx = Ctx::new().await;
    ctx.get("/openapi.json").send().await.expect(404);
}
