//! /posts* endpoints (04-api.md §4.7).

use past_exam_contract_tests::*;
use reqwest::multipart::{Form, Part};
use serde_json::{json, Value};

const MIB: usize = 1024 * 1024;

fn post_form(title: &str, course: &Course) -> Form {
    Form::new()
        .text("title", title.to_string())
        .text("course_id", course.id.clone())
        .text("department_id", course.department_id.clone())
}

fn pdf(bytes: Vec<u8>) -> Part {
    file_part(bytes, "exam.pdf", "application/pdf")
}

async fn post_count(ctx: &Ctx, title: &str) -> i64 {
    ctx.scalar_i64("SELECT count(*) FROM posts WHERE title = $1", &[&title])
        .await
}

async fn list(ctx: &Ctx, user: &User, query: &[(&str, &str)]) -> Value {
    let resp = ctx.get("/posts").auth(user).query(query).send().await;
    resp.expect(200);
    resp.json()
}

#[tokio::test]
async fn create_in_public_department_is_auto_approved() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;

    let resp = ctx
        .post("/posts")
        .auth(&user)
        .form(&[
            ("title", "期末考"),
            ("course_id", &course.id),
            ("department_id", &dept.id),
        ])
        .send()
        .await;
    resp.expect(200);
    let body = resp.json();
    assert_keys(&body, &["status", "post_id"]);
    assert_eq!(body["status"], json!("success"));
    let id = body["post_id"].as_str().unwrap().to_string();

    let row = ctx
        .row(
            "SELECT status, owner_id, content, is_migrate, is_anonymous, department_id, course_id \
             FROM posts WHERE id = $1",
            &[&id],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "APPROVED");
    assert_eq!(row.get::<_, String>(1), user.id);
    assert_eq!(row.get::<_, String>(2), "");
    assert_eq!(row.get::<_, Option<bool>>(3), Some(false));
    assert_eq!(row.get::<_, Option<bool>>(4), Some(false));
    assert_eq!(row.get::<_, String>(5), dept.id);
    assert_eq!(row.get::<_, String>(6), course.id);
}

#[tokio::test]
async fn create_in_private_department_is_pending_and_keeps_flags() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(false).await;
    ctx.member(&user, &dept).await;
    let course = ctx.course(&dept).await;

    let resp = ctx
        .post("/posts")
        .auth(&user)
        .form(&[
            ("title", "x"),
            ("course_id", &course.id),
            ("department_id", &dept.id),
            ("content", "說明"),
            ("is_anonymous", "true"),
            ("is_migrate", "true"),
        ])
        .send()
        .await;
    resp.expect(200);
    let id = resp.json()["post_id"].as_str().unwrap().to_string();
    let row = ctx
        .row(
            "SELECT status, content, is_anonymous, is_migrate FROM posts WHERE id = $1",
            &[&id],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "PENDING");
    assert_eq!(row.get::<_, String>(1), "說明");
    assert_eq!(row.get::<_, Option<bool>>(2), Some(true));
    assert_eq!(row.get::<_, Option<bool>>(3), Some(true));
}

#[tokio::test]
async fn uploaded_pdfs_are_stored_and_listed() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let first = pdf_bytes(1000);
    let mut second = pdf_bytes(2000);
    second[1999] = b'Z';

    let resp = ctx
        .post("/posts")
        .auth(&user)
        .multipart(
            post_form("with files", &course)
                .part("files", pdf(first.clone()))
                .part("files", pdf(second.clone())),
        )
        .send()
        .await;
    resp.expect(200);
    let post_id = resp.json()["post_id"].as_str().unwrap().to_string();

    let detail = ctx
        .get(&format!("/posts/{post_id}"))
        .auth(&user)
        .send()
        .await
        .json();
    let files: Vec<String> = detail["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_str().unwrap().to_string())
        .collect();
    assert_eq!(files.len(), 2);
    let prefix = format!("{}/{}/{}/", config().file_base, user.id, post_id);
    let mut stored = Vec::new();
    for url in &files {
        let key = url
            .strip_prefix(&prefix)
            .unwrap_or_else(|| panic!("{url} must start with {prefix}"));
        assert!(
            uuid::Uuid::parse_str(key).is_ok(),
            "object key ends with a uuid4: {url}"
        );
        let object = ctx.fetch_object(url).await;
        object.expect(200);
        stored.push(object.body);
    }
    stored.sort_by_key(|b| b.len());
    assert_eq!(stored, vec![first, second]);
    assert_eq!(
        ctx.scalar_i64(
            "SELECT count(*) FROM posts_files WHERE post_id = $1",
            &[&post_id]
        )
        .await,
        2
    );
}

#[tokio::test]
async fn uploaded_pdf_has_pdf_content_type() {
    rust_only!(); // F-08
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let resp = ctx
        .post("/posts")
        .auth(&user)
        .multipart(post_form("typed", &course).part("files", pdf(pdf_bytes(100))))
        .send()
        .await;
    let post_id = resp.json()["post_id"].as_str().unwrap().to_string();
    let detail = ctx
        .get(&format!("/posts/{post_id}"))
        .auth(&user)
        .send()
        .await
        .json();
    let object = ctx.fetch_object(detail["files"][0].as_str().unwrap()).await;
    assert_eq!(
        object.header("content-type").as_deref(),
        Some("application/pdf")
    );
}

#[tokio::test]
async fn create_target_validation() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let public = ctx.department(true).await;
    let private = ctx.department(false).await;
    let public_course = ctx.course(&public).await;
    let private_course = ctx.course(&private).await;

    let send = |course_id: String, department_id: Option<String>| {
        let ctx = &ctx;
        let user = user.clone();
        async move {
            let mut fields = vec![
                ("title".to_string(), "t".to_string()),
                ("course_id".to_string(), course_id),
            ];
            if let Some(d) = department_id {
                fields.push(("department_id".to_string(), d));
            }
            let fields: Vec<(&str, &str)> = fields
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            ctx.post("/posts").auth(&user).form(&fields).send().await
        }
    };

    send("no-such-course".into(), Some(public.id.clone()))
        .await
        .expect_detail(404, "Course not found");
    send(public_course.id.clone(), Some(private.id.clone()))
        .await
        .expect_detail(400, "Course does not belong to the selected department");
    send(public_course.id.clone(), None)
        .await
        .expect_detail(400, "Course does not belong to the selected department");
    send(private_course.id.clone(), Some(private.id.clone()))
        .await
        .expect_detail(404, "Department not found");
}

#[tokio::test]
async fn create_field_validation() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let long_title = "a".repeat(257);
    let long_content = "a".repeat(10001);
    let max_title = "考".repeat(256);

    ctx.post("/posts")
        .auth(&user)
        .form(&[("course_id", &course.id), ("department_id", &dept.id)])
        .send()
        .await
        .expect_validation(&["body", "title"], Some("missing"));
    ctx.post("/posts")
        .auth(&user)
        .form(&[
            ("title", &long_title),
            ("course_id", &course.id),
            ("department_id", &dept.id),
        ])
        .send()
        .await
        .expect_validation(&["body", "title"], Some("string_too_long"));
    ctx.post("/posts")
        .auth(&user)
        .form(&[
            ("title", "t"),
            ("course_id", &course.id),
            ("department_id", &dept.id),
            ("content", &long_content),
        ])
        .send()
        .await
        .expect_validation(&["body", "content"], Some("string_too_long"));
    ctx.post("/posts")
        .auth(&user)
        .form(&[
            ("title", &max_title),
            ("course_id", &course.id),
            ("department_id", &dept.id),
        ])
        .send()
        .await
        .expect(200);
    ctx.post("/posts")
        .form(&[
            ("title", "t"),
            ("course_id", &course.id),
            ("department_id", &dept.id),
        ])
        .send()
        .await
        .expect(401);
}

#[tokio::test]
async fn upload_validation_rejects_without_side_effects() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;

    let cases: Vec<(Form, u16, &str)> = vec![
        (
            post_form("bad-type", &course)
                .part("files", file_part(pdf_bytes(100), "a.pdf", "text/plain")),
            400,
            "Only PDF files are allowed",
        ),
        (
            post_form("bad-type-param", &course).part(
                "files",
                file_part(pdf_bytes(100), "a.pdf", "application/pdf; charset=binary"),
            ),
            400,
            "Only PDF files are allowed",
        ),
        (
            post_form("spoofed", &course).part("files", pdf(b"not a pdf".to_vec())),
            400,
            "Invalid PDF content",
        ),
        (
            (0..11).fold(post_form("too-many", &course), |f, _| {
                f.part("files", pdf(pdf_bytes(10)))
            }),
            413,
            "Too many files; maximum is 10",
        ),
        (
            post_form("too-big", &course).part("files", pdf(pdf_bytes(20 * MIB + 1))),
            413,
            "A PDF exceeds the per-file size limit",
        ),
        (
            post_form("too-much", &course)
                .part("files", pdf(pdf_bytes(17 * MIB)))
                .part("files", pdf(pdf_bytes(17 * MIB)))
                .part("files", pdf(pdf_bytes(17 * MIB))),
            413,
            "PDF uploads exceed the total size limit",
        ),
    ];
    for (form, status, detail) in cases {
        ctx.post("/posts")
            .auth(&user)
            .multipart(form)
            .send()
            .await
            .expect_detail(status, detail);
    }
    for title in [
        "bad-type",
        "bad-type-param",
        "spoofed",
        "too-many",
        "too-big",
        "too-much",
    ] {
        assert_eq!(
            post_count(&ctx, title).await,
            0,
            "{title} must not create a post"
        );
    }
    assert_eq!(
        ctx.scalar_i64(
            "SELECT count(*) FROM posts WHERE owner_id = $1",
            &[&user.id]
        )
        .await,
        0
    );

    // Exactly at the limits is accepted: 10 files, 20 MiB single file.
    let ten = (0..10).fold(post_form("ten-files", &course), |f, _| {
        f.part("files", pdf(pdf_bytes(10)))
    });
    ctx.post("/posts")
        .auth(&user)
        .multipart(ten)
        .send()
        .await
        .expect(200);
    let max = post_form("max-size", &course).part("files", pdf(pdf_bytes(20 * MIB)));
    ctx.post("/posts")
        .auth(&user)
        .multipart(max)
        .send()
        .await
        .expect(200);
    let alt = post_form("x-pdf", &course).part(
        "files",
        file_part(pdf_bytes(10), "a.pdf", "application/x-pdf"),
    );
    ctx.post("/posts")
        .auth(&user)
        .multipart(alt)
        .send()
        .await
        .expect(200);
}

#[tokio::test]
async fn list_visibility_rules() {
    let ctx = Ctx::new().await;
    let viewer = ctx.user().await;
    let other = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let approved = ctx.seed_post(&course, &other, "APPROVED").await;
    let others_pending = ctx.seed_post(&course, &other, "PENDING").await;
    let own_pending = ctx.seed_post(&course, &viewer, "PENDING").await;
    let others_rejected = ctx.seed_post(&course, &other, "REJECTED").await;

    let got = ids(&list(&ctx, &viewer, &[("department_id", &dept.id)]).await);
    assert!(got.contains(&approved.id));
    assert!(got.contains(&own_pending.id));
    assert!(!got.contains(&others_pending.id));
    assert!(!got.contains(&others_rejected.id));

    // A department admin sees every post of that department.
    let admin = ctx.user().await;
    ctx.admin_of(&admin, &dept).await;
    let got = ids(&list(&ctx, &admin, &[("department_id", &dept.id)]).await);
    assert_eq!(got.len(), 4);

    let root = ctx.super_user().await;
    assert_eq!(
        ids(&list(&ctx, &root, &[("course_id", &course.id)]).await).len(),
        4
    );
}

#[tokio::test]
async fn list_excludes_invisible_departments() {
    let ctx = Ctx::new().await;
    let viewer = ctx.user().await;
    let hidden = ctx.department(false).await;
    let course = ctx.course(&hidden).await;
    ctx.seed_post(&course, &viewer, "APPROVED").await;
    assert_eq!(
        list(&ctx, &viewer, &[("department_id", &hidden.id)]).await,
        json!([])
    );
    // Own posts in a department that is no longer visible are hidden too.
    assert_eq!(
        list(&ctx, &viewer, &[("user_id", &viewer.id)]).await,
        json!([])
    );
}

#[tokio::test]
async fn list_status_filters() {
    let ctx = Ctx::new().await;
    let admin = ctx.user().await;
    let dept = ctx.department(false).await;
    ctx.admin_of(&admin, &dept).await;
    let course = ctx.course(&dept).await;
    let approved = ctx.seed_post(&course, &admin, "APPROVED").await;
    let pending = ctx.seed_post(&course, &admin, "PENDING").await;
    let rejected = ctx.seed_post(&course, &admin, "REJECTED").await;
    let d = dept.id.as_str();

    let mut got = ids(&list(&ctx, &admin, &[("department_id", d), ("status", "PENDING")]).await);
    got.sort();
    let mut want = vec![pending.id.clone(), rejected.id.clone()];
    want.sort();
    assert_eq!(got, want, "PENDING includes REJECTED (P-07)");
    assert_eq!(
        ids(&list(
            &ctx,
            &admin,
            &[("department_id", d), ("status", "APPROVED")]
        )
        .await),
        vec![approved.id]
    );
    assert_eq!(
        ids(&list(
            &ctx,
            &admin,
            &[("department_id", d), ("status", "REJECTED")]
        )
        .await),
        vec![rejected.id]
    );
    ctx.get("/posts")
        .auth(&admin)
        .query(&[("status", "approved")])
        .send()
        .await
        .expect_detail(400, "Invalid post status");
}

#[tokio::test]
async fn list_filters_ordering_and_shape() {
    let ctx = Ctx::new().await;
    let viewer = ctx.user().await;
    let author = ctx.user().await;
    let dept = ctx.department(true).await;
    let course_a = ctx.course(&dept).await;
    let course_b = ctx.course(&dept).await;
    let old = ctx
        .create_post(
            &course_a,
            &author,
            NewPost {
                create_time: Some("2025-01-01T00:00:00Z".into()),
                ..Default::default()
            },
        )
        .await;
    let new = ctx
        .create_post(
            &course_a,
            &author,
            NewPost {
                create_time: Some("2026-01-01T00:00:00Z".into()),
                anonymous: true,
                ..Default::default()
            },
        )
        .await;
    let other_course = ctx.seed_post(&course_b, &viewer, "APPROVED").await;

    let body = list(&ctx, &viewer, &[("department_id", &dept.id)]).await;
    assert_eq!(
        ids(&body),
        vec![other_course.id.clone(), new.id.clone(), old.id.clone()],
        "create_time DESC"
    );
    for item in body.as_array().unwrap() {
        assert_keys(item, &with_keys(POST_ROW_KEYS, &["course_name"]));
        assert_ts_offset(&item["create_time"]);
    }
    let anon = &body[1];
    assert_eq!(anon["owner_id"], json!("anonymous"));
    assert_eq!(anon["is_anonymous"], json!(true));
    assert_eq!(anon["course_name"], json!(course_a.name));
    assert_eq!(body[2]["owner_id"], json!(author.id));

    assert_eq!(
        ids(&list(&ctx, &viewer, &[("course_id", &course_b.id)]).await),
        vec![other_course.id.clone()]
    );
    let by_author = ids(&list(
        &ctx,
        &author,
        &[("department_id", &dept.id), ("user_id", &author.id)],
    )
    .await);
    assert_eq!(by_author, vec![new.id.clone(), old.id.clone()]);

    let page = ids(&list(
        &ctx,
        &viewer,
        &[("department_id", &dept.id), ("skip", "1"), ("limit", "1")],
    )
    .await);
    assert_eq!(page, vec![new.id.clone()]);
    let clamped = ids(&list(
        &ctx,
        &viewer,
        &[("department_id", &dept.id), ("limit", "0")],
    )
    .await);
    assert_eq!(clamped.len(), 1);
}

#[tokio::test]
async fn list_requires_an_existing_course() {
    let ctx = Ctx::new().await;
    let viewer = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let post = ctx.seed_post(&course, &viewer, "APPROVED").await;
    ctx.db
        .execute("DELETE FROM courses WHERE id = $1", &[&course.id])
        .await
        .unwrap();
    assert!(!ids(&list(&ctx, &viewer, &[("department_id", &dept.id)]).await).contains(&post.id));
    ctx.get(&format!("/posts/{}", post.id))
        .auth(&viewer)
        .send()
        .await
        .expect(200);
}

#[tokio::test]
async fn detail_shape_and_owner_name() {
    let ctx = Ctx::new().await;
    let viewer = ctx.user().await;
    let named = ctx
        .create_user(NewUser {
            readable_name: Some("陳同學".into()),
            ..Default::default()
        })
        .await;
    let unnamed = ctx.create_user(NewUser::default()).await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let p1 = ctx.seed_post(&course, &named, "APPROVED").await;
    let p2 = ctx.seed_post(&course, &unnamed, "APPROVED").await;
    let p3 = ctx
        .create_post(
            &course,
            &named,
            NewPost {
                anonymous: true,
                ..Default::default()
            },
        )
        .await;
    ctx.post_file(&p1, "http://files.example/a.pdf").await;

    let body = ctx
        .get(&format!("/posts/{}", p1.id))
        .auth(&viewer)
        .send()
        .await
        .json();
    assert_keys(&body, &with_keys(POST_ROW_KEYS, &["files", "owner_name"]));
    assert_eq!(body["owner_name"], json!("陳同學"));
    assert_eq!(body["owner_id"], json!(named.id));
    assert_eq!(body["files"], json!(["http://files.example/a.pdf"]));
    assert_eq!(body["status"], json!("APPROVED"));
    assert_ts_offset(&body["create_time"]);

    let body = ctx
        .get(&format!("/posts/{}", p2.id))
        .auth(&viewer)
        .send()
        .await
        .json();
    assert_eq!(body["owner_name"], json!(unnamed.username));
    assert_eq!(body["files"], json!([]));

    let body = ctx
        .get(&format!("/posts/{}", p3.id))
        .auth(&viewer)
        .send()
        .await
        .json();
    assert_eq!(body["owner_name"], json!("匿名用戶"));
    assert_eq!(body["owner_id"], json!("anonymous"));
}

#[tokio::test]
async fn detail_visibility() {
    let ctx = Ctx::new().await;
    let viewer = ctx.user().await;
    let other = ctx.user().await;
    let public = ctx.department(true).await;
    let hidden = ctx.department(false).await;
    let course = ctx.course(&public).await;
    let hidden_course = ctx.course(&hidden).await;
    let others_pending = ctx.seed_post(&course, &other, "PENDING").await;
    let own_pending = ctx.seed_post(&course, &viewer, "PENDING").await;
    let hidden_post = ctx.seed_post(&hidden_course, &other, "APPROVED").await;
    let orphan_owner = ctx
        .seed_post(
            &course,
            &User {
                id: "ghost".into(),
                username: "ghost".into(),
                password: None,
            },
            "APPROVED",
        )
        .await;

    let get = |id: &str| ctx.get(&format!("/posts/{id}")).auth(&viewer).send();
    get(&others_pending.id)
        .await
        .expect_detail(404, "Not Found");
    get(&own_pending.id).await.expect(200);
    get(&hidden_post.id).await.expect_detail(404, "Not Found");
    get("no-such-post").await.expect_detail(404, "Not Found");
    get(&orphan_owner.id).await.expect_detail(404, "Not Found");

    let admin = ctx.user().await;
    ctx.admin_of(&admin, &public).await;
    ctx.get(&format!("/posts/{}", others_pending.id))
        .auth(&admin)
        .send()
        .await
        .expect(200);
    let root = ctx.super_user().await;
    ctx.get(&format!("/posts/{}", hidden_post.id))
        .auth(&root)
        .send()
        .await
        .expect(200);
}

#[tokio::test]
async fn status_update_by_department_admin() {
    let ctx = Ctx::new().await;
    let admin = ctx.user().await;
    let author = ctx.user().await;
    let dept = ctx.department(false).await;
    ctx.admin_of(&admin, &dept).await;
    ctx.member(&author, &dept).await;
    let course = ctx.course(&dept).await;
    let post = ctx.seed_post(&course, &author, "PENDING").await;

    // Warm the author's and the course caches.
    ctx.get(&format!("/posts/{}", post.id))
        .auth(&author)
        .send()
        .await
        .expect(200);
    assert_eq!(
        ctx.get(&format!("/courses/{}", course.id))
            .auth(&author)
            .send()
            .await
            .json()["posts"],
        json!([])
    );
    let dept_courses = ctx
        .get(&format!("/departments/{}/courses", dept.id))
        .auth(&author)
        .send()
        .await
        .json();
    assert_eq!(dept_courses[0]["has_posts"], json!(false));

    let path = format!("/posts/status/{}/{}", dept.id, post.id);
    let resp = ctx
        .put(&path)
        .auth(&admin)
        .form(&[("status", "APPROVED")])
        .send()
        .await;
    resp.expect(200);
    let body = resp.json();
    assert_keys(&body, &with_keys(POST_ROW_KEYS, &["files", "owner_name"]));
    assert_eq!(body["status"], json!("APPROVED"));
    assert_eq!(body["id"], json!(post.id));
    assert_ts_offset(&body["updated_time"]);

    // Invalidated caches.
    assert_eq!(
        ctx.get(&format!("/posts/{}", post.id))
            .auth(&author)
            .send()
            .await
            .json()["status"],
        json!("APPROVED")
    );
    assert_eq!(
        ids(&ctx
            .get(&format!("/courses/{}", course.id))
            .auth(&author)
            .send()
            .await
            .json()["posts"]),
        vec![post.id.clone()]
    );
    let dept_courses = ctx
        .get(&format!("/departments/{}/courses", dept.id))
        .auth(&author)
        .send()
        .await
        .json();
    assert_eq!(dept_courses[0]["has_posts"], json!(true));

    ctx.put(&path)
        .auth(&admin)
        .form(&[("status", "REJECTED")])
        .send()
        .await
        .expect(200);
    let status: String = ctx
        .row("SELECT status FROM posts WHERE id = $1", &[&post.id])
        .await
        .unwrap()
        .get(0);
    assert_eq!(status, "REJECTED");
}

#[tokio::test]
async fn status_update_errors() {
    let ctx = Ctx::new().await;
    let admin = ctx.user().await;
    let author = ctx.user().await;
    let dept = ctx.department(false).await;
    let other = ctx.department(false).await;
    ctx.admin_of(&admin, &dept).await;
    let course = ctx.course(&dept).await;
    let other_course = ctx.course(&other).await;
    let post = ctx.seed_post(&course, &author, "PENDING").await;
    let foreign = ctx.seed_post(&other_course, &author, "PENDING").await;

    ctx.put(&format!("/posts/status/{}/{}", dept.id, post.id))
        .auth(&admin)
        .form(&[("status", "DELETED")])
        .send()
        .await
        .expect_detail(400, "Invalid post status");
    ctx.put(&format!("/posts/status/{}/{}", dept.id, foreign.id))
        .auth(&admin)
        .form(&[("status", "APPROVED")])
        .send()
        .await
        .expect_detail(404, "Post not found");
    ctx.put(&format!("/posts/status/{}/{}", other.id, foreign.id))
        .auth(&admin)
        .form(&[("status", "APPROVED")])
        .send()
        .await
        .expect_detail(403, "Forbidden");
    ctx.put(&format!("/posts/status/{}/{}", dept.id, post.id))
        .auth(&admin)
        .send()
        .await
        .expect_validation(&["body", "status"], Some("missing"));
    let status: String = ctx
        .row("SELECT status FROM posts WHERE id = $1", &[&foreign.id])
        .await
        .unwrap()
        .get(0);
    assert_eq!(status, "PENDING");
}

#[tokio::test]
async fn status_update_without_a_department_row_commits_then_404s() {
    let ctx = Ctx::new().await;
    let root = ctx.super_user().await;
    let author = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let post = ctx.seed_post(&course, &author, "PENDING").await;
    ctx.db
        .execute("DELETE FROM departments WHERE id = $1", &[&dept.id])
        .await
        .unwrap();
    ctx.put(&format!("/posts/status/{}/{}", dept.id, post.id))
        .auth(&root)
        .form(&[("status", "APPROVED")])
        .send()
        .await
        .expect_detail(404, "Not Found");
    let status: String = ctx
        .row("SELECT status FROM posts WHERE id = $1", &[&post.id])
        .await
        .unwrap()
        .get(0);
    assert_eq!(status, "APPROVED");
}

#[tokio::test]
async fn unchanged_status_does_not_touch_updated_time() {
    let ctx = Ctx::new().await;
    let root = ctx.super_user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let post = ctx.seed_post(&course, &root, "APPROVED").await;
    ctx.put(&format!("/posts/status/{}/{}", dept.id, post.id))
        .auth(&root)
        .form(&[("status", "APPROVED")])
        .send()
        .await
        .expect(200);
    let touched: bool = ctx
        .row(
            "SELECT updated_time IS NOT NULL FROM posts WHERE id = $1",
            &[&post.id],
        )
        .await
        .unwrap()
        .get(0);
    assert!(!touched, "no-op update must not set updated_time");
}

#[tokio::test]
async fn status_update_returns_null_when_owner_is_missing() {
    let ctx = Ctx::new().await;
    let admin = ctx.super_user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let ghost = User {
        id: uid("ghost"),
        username: "ghost".into(),
        password: None,
    };
    let post = ctx.seed_post(&course, &ghost, "PENDING").await;
    let resp = ctx
        .put(&format!("/posts/status/{}/{}", dept.id, post.id))
        .auth(&admin)
        .form(&[("status", "APPROVED")])
        .send()
        .await;
    resp.expect(200);
    assert_eq!(resp.json(), json!(null));
}

#[tokio::test]
async fn delete_by_admin_removes_rows_objects_and_cached_views() {
    let ctx = Ctx::new().await;
    let author = ctx.user().await;
    let admin = ctx.user().await;
    let dept = ctx.department(true).await;
    ctx.admin_of(&admin, &dept).await;
    let course = ctx.course(&dept).await;
    let resp = ctx
        .post("/posts")
        .auth(&author)
        .multipart(post_form("to delete", &course).part("files", pdf(pdf_bytes(100))))
        .send()
        .await;
    let post_id = resp.json()["post_id"].as_str().unwrap().to_string();
    let detail = ctx
        .get(&format!("/posts/{post_id}"))
        .auth(&author)
        .send()
        .await
        .json();
    let url = detail["files"][0].as_str().unwrap().to_string();
    ctx.fetch_object(&url).await.expect(200);
    assert_eq!(
        ids(&ctx
            .get(&format!("/courses/{}", course.id))
            .auth(&author)
            .send()
            .await
            .json()["posts"]),
        vec![post_id.clone()]
    );
    let dept_courses = format!("/departments/{}/courses", dept.id);
    assert_eq!(
        ctx.get(&dept_courses).auth(&author).send().await.json()[0]["has_posts"],
        json!(true)
    );

    ctx.delete(&format!("/posts/{post_id}"))
        .auth(&author)
        .send()
        .await
        .expect_detail(403, "Forbidden");

    let resp = ctx
        .delete(&format!("/posts/{post_id}"))
        .auth(&admin)
        .send()
        .await;
    resp.expect(200);
    assert_eq!(resp.json(), json!({"status": "success"}));
    assert_eq!(
        ctx.scalar_i64("SELECT count(*) FROM posts WHERE id = $1", &[&post_id])
            .await,
        0
    );
    assert_eq!(
        ctx.scalar_i64(
            "SELECT count(*) FROM posts_files WHERE post_id = $1",
            &[&post_id]
        )
        .await,
        0
    );
    assert_eq!(ctx.fetch_object(&url).await.status, 404);
    ctx.get(&format!("/posts/{post_id}"))
        .auth(&author)
        .send()
        .await
        .expect(404);
    assert_eq!(
        ctx.get(&format!("/courses/{}", course.id))
            .auth(&author)
            .send()
            .await
            .json()["posts"],
        json!([])
    );
    assert_eq!(
        ctx.get(&dept_courses).auth(&author).send().await.json()[0]["has_posts"],
        json!(false)
    );

    ctx.delete(&format!("/posts/{post_id}"))
        .auth(&admin)
        .send()
        .await
        .expect_detail(404, "Post not found");
}

#[tokio::test]
async fn delete_by_super_user_and_permission_errors() {
    let ctx = Ctx::new().await;
    let author = ctx.user().await;
    let member_admin_elsewhere = ctx.user().await;
    let root = ctx.super_user().await;
    let dept = ctx.department(true).await;
    let elsewhere = ctx.department(true).await;
    ctx.admin_of(&member_admin_elsewhere, &elsewhere).await;
    let course = ctx.course(&dept).await;
    let post = ctx.seed_post(&course, &author, "APPROVED").await;

    ctx.delete(&format!("/posts/{}", post.id))
        .auth(&member_admin_elsewhere)
        .send()
        .await
        .expect_detail(403, "Forbidden");
    ctx.delete(&format!("/posts/{}", post.id))
        .auth(&root)
        .send()
        .await
        .expect(200);
}

#[tokio::test]
async fn anonymous_posts_are_not_discoverable_through_the_user_filter() {
    rust_only!(); // F-17: the reference returns anonymous posts for any user_id filter.
    let ctx = Ctx::new().await;
    let author = ctx.user().await;
    let stranger = ctx.user().await;
    let root = ctx.super_user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let named = ctx.seed_post(&course, &author, "APPROVED").await;
    let anonymous = ctx
        .create_post(
            &course,
            &author,
            NewPost {
                anonymous: true,
                ..Default::default()
            },
        )
        .await;
    let query = [
        ("department_id", dept.id.as_str()),
        ("user_id", author.id.as_str()),
    ];

    let seen_by_stranger = ids(&list(&ctx, &stranger, &query).await);
    assert_eq!(seen_by_stranger, vec![named.id.clone()]);

    let seen_by_author = ids(&list(&ctx, &author, &query).await);
    assert!(seen_by_author.contains(&named.id));
    assert!(seen_by_author.contains(&anonymous.id));

    let seen_by_super = ids(&list(&ctx, &root, &query).await);
    assert!(seen_by_super.contains(&anonymous.id));
}
