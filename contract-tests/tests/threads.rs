//! /threads* thread endpoints (04-api.md §4.8).

use past_exam_contract_tests::*;
use reqwest::multipart::Form;
use serde_json::json;

const MIB: usize = 1024 * 1024;

fn thread_form(title: &str) -> Form {
    Form::new()
        .text("title", title.to_string())
        .text("content", "body".to_string())
}

#[tokio::test]
async fn create_thread_without_image() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;

    let resp = ctx
        .post(&format!("/threads/{}", course.id))
        .auth(&user)
        .form(&[
            ("title", "請問"),
            ("content", "第一題怎麼解"),
            ("is_anonymous", "true"),
        ])
        .send()
        .await;
    resp.expect(200);
    let body = resp.json();
    assert_keys(&body, &["status", "thread_id"]);
    assert_eq!(body["status"], json!("success"));
    let id = body["thread_id"].as_str().unwrap().to_string();
    let row = ctx
        .row("SELECT title, content, owner_id, course_id, is_anonymous, like_count, image_url FROM threads WHERE id = $1", &[&id])
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "請問");
    assert_eq!(row.get::<_, String>(1), "第一題怎麼解");
    assert_eq!(row.get::<_, String>(2), user.id);
    assert_eq!(row.get::<_, String>(3), course.id);
    assert_eq!(row.get::<_, Option<bool>>(4), Some(true));
    assert_eq!(row.get::<_, Option<i32>>(5), Some(0));
    assert_eq!(row.get::<_, Option<String>>(6), None);
}

#[tokio::test]
async fn create_thread_with_image_stores_object() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let image = png_bytes(4096);

    let resp = ctx
        .post(&format!("/threads/{}", course.id))
        .auth(&user)
        .multipart(
            thread_form("with image").part("image", file_part(image.clone(), "a.png", "image/png")),
        )
        .send()
        .await;
    resp.expect(200);
    let id = resp.json()["thread_id"].as_str().unwrap().to_string();
    let detail = ctx
        .get(&format!("/threads/detail/{id}"))
        .auth(&user)
        .send()
        .await
        .json();
    let url = detail["image_url"].as_str().unwrap();
    let key = url
        .strip_prefix(&format!("{}/threads/", config().file_base))
        .unwrap_or_else(|| panic!("unexpected image url {url}"));
    assert!(uuid::Uuid::parse_str(key).is_ok());
    let object = ctx.fetch_object(url).await;
    object.expect(200);
    assert_eq!(object.body, image);
}

#[tokio::test]
async fn image_validation() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let path = format!("/threads/{}", course.id);

    let mut webp = b"RIFF\0\0\0\0WEBPVP8 ".to_vec();
    webp.resize(64, 0);
    let mut avi = b"RIFF\0\0\0\0AVI LIST".to_vec();
    avi.resize(64, 0);
    let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0];
    jpeg.resize(64, 0);

    let ok_cases = vec![
        (png_bytes(64), "image/png"),
        (jpeg.clone(), "image/jpeg"),
        (b"GIF87a----".to_vec(), "image/gif"),
        (b"GIF89a----".to_vec(), "image/gif"),
        (webp, "image/webp"),
        (jpeg, "image/png"), // magic need not match the declared type
        (png_bytes(5 * MIB), "image/png"),
    ];
    for (bytes, ty) in ok_cases {
        ctx.post(&path)
            .auth(&user)
            .multipart(thread_form("ok").part("image", file_part(bytes, "i", ty)))
            .send()
            .await
            .expect(200);
    }

    let bad_cases = vec![
        (
            png_bytes(64),
            "image/svg+xml",
            400,
            "Unsupported image type",
        ),
        (
            png_bytes(64),
            "application/octet-stream",
            400,
            "Unsupported image type",
        ),
        (
            b"plain text".to_vec(),
            "image/png",
            400,
            "Invalid image content",
        ),
        (avi, "image/webp", 400, "Invalid image content"),
        (b"RIFF".to_vec(), "image/webp", 400, "Invalid image content"),
        (png_bytes(5 * MIB + 1), "image/png", 413, "Image too large"),
    ];
    let before = ctx
        .scalar_i64(
            "SELECT count(*) FROM threads WHERE owner_id = $1",
            &[&user.id],
        )
        .await;
    for (bytes, ty, status, detail) in bad_cases {
        ctx.post(&path)
            .auth(&user)
            .multipart(thread_form("bad").part("image", file_part(bytes, "i", ty)))
            .send()
            .await
            .expect_detail(status, detail);
    }
    let after = ctx
        .scalar_i64(
            "SELECT count(*) FROM threads WHERE owner_id = $1",
            &[&user.id],
        )
        .await;
    assert_eq!(before, after, "rejected uploads must not create threads");
}

#[tokio::test]
async fn create_thread_validation_and_visibility() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let hidden = ctx.department(false).await;
    let hidden_course = ctx.course(&hidden).await;
    let public = ctx.department(true).await;
    let course = ctx.course(&public).await;
    let path = format!("/threads/{}", course.id);

    ctx.post(&path)
        .auth(&user)
        .form(&[("title", &"a".repeat(201)), ("content", "x")])
        .send()
        .await
        .expect_validation(&["body", "title"], Some("string_too_long"));
    ctx.post(&path)
        .auth(&user)
        .form(&[("title", "x"), ("content", &"a".repeat(2001))])
        .send()
        .await
        .expect_validation(&["body", "content"], Some("string_too_long"));
    ctx.post(&path)
        .auth(&user)
        .form(&[("title", "x"), ("content", "")])
        .send()
        .await
        .expect_validation(&["body", "content"], Some("string_too_short"));
    ctx.post(&path)
        .form(&[("title", "x"), ("content", "y")])
        .send()
        .await
        .expect(401);
    ctx.post(&format!("/threads/{}", hidden_course.id))
        .auth(&user)
        .form(&[("title", "x"), ("content", "y")])
        .send()
        .await
        .expect_detail(404, "Department not found");
    ctx.post("/threads/no-such-course")
        .auth(&user)
        .form(&[("title", "x"), ("content", "y")])
        .send()
        .await
        .expect_detail(404, "Course not found");
}

#[tokio::test]
async fn list_threads() {
    let ctx = Ctx::new().await;
    let viewer = ctx.user().await;
    let author = ctx
        .create_user(NewUser {
            readable_name: Some("作者".into()),
            ..Default::default()
        })
        .await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let old = ctx
        .create_thread(&course, &author, false, Some("2025-01-01T00:00:00Z"))
        .await;
    let anon = ctx
        .create_thread(&course, &author, true, Some("2025-06-01T00:00:00Z"))
        .await;
    let mine = ctx
        .create_thread(&course, &viewer, false, Some("2026-01-01T00:00:00Z"))
        .await;
    ctx.post(&format!("/threads/{}/like", old.id))
        .auth(&viewer)
        .send()
        .await
        .expect(200);

    let resp = ctx
        .get(&format!("/threads/{}", course.id))
        .auth(&viewer)
        .send()
        .await;
    resp.expect(200);
    let body = resp.json();
    assert_keys(&body, &["threads", "total"]);
    assert_eq!(body["total"], json!(3));
    assert_eq!(
        ids(&body["threads"]),
        vec![mine.id.clone(), anon.id.clone(), old.id.clone()]
    );
    for t in body["threads"].as_array().unwrap() {
        assert_keys(t, THREAD_KEYS);
        assert_ts_z(&t["create_time"]);
    }
    let [t_mine, t_anon, t_old] = [
        &body["threads"][0],
        &body["threads"][1],
        &body["threads"][2],
    ];
    assert_eq!(t_mine["is_owner"], json!(true));
    assert_eq!(t_mine["liked"], json!(false));
    assert_eq!(t_anon["owner_id"], json!("anonymous"));
    assert_eq!(t_anon["owner_name"], json!("匿名用戶"));
    assert_eq!(t_anon["is_owner"], json!(false));
    assert_eq!(t_old["owner_id"], json!(author.id));
    assert_eq!(t_old["owner_name"], json!("作者"));
    assert_eq!(t_old["liked"], json!(true));
    assert_eq!(t_old["like_count"], json!(1));
    assert_eq!(t_old["image_url"], json!(null));

    let page = ctx
        .get(&format!("/threads/{}", course.id))
        .auth(&viewer)
        .query(&[("skip", "1"), ("limit", "1")])
        .send()
        .await
        .json();
    assert_eq!(ids(&page["threads"]), vec![anon.id.clone()]);
    assert_eq!(page["total"], json!(3));
    let clamped = ctx
        .get(&format!("/threads/{}", course.id))
        .auth(&viewer)
        .query(&[("limit", "0"), ("skip", "-3")])
        .send()
        .await
        .json();
    assert_eq!(ids(&clamped["threads"]), vec![mine.id]);
}

#[tokio::test]
async fn owner_is_masked_for_anonymous_threads_even_for_the_owner() {
    let ctx = Ctx::new().await;
    let owner = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let t = ctx.create_thread(&course, &owner, true, None).await;
    let body = ctx
        .get(&format!("/threads/detail/{}", t.id))
        .auth(&owner)
        .send()
        .await
        .json();
    assert_eq!(body["owner_id"], json!("anonymous"));
    assert_eq!(body["is_owner"], json!(true));
}

#[tokio::test]
async fn thread_visibility() {
    let ctx = Ctx::new().await;
    let outsider = ctx.user().await;
    let member = ctx.user().await;
    let dept = ctx.department(false).await;
    ctx.member(&member, &dept).await;
    let course = ctx.course(&dept).await;
    let thread = ctx.thread(&course, &member).await;

    ctx.get(&format!("/threads/{}", course.id))
        .auth(&outsider)
        .send()
        .await
        .expect_detail(404, "Department not found");
    ctx.get(&format!("/threads/detail/{}", thread.id))
        .auth(&outsider)
        .send()
        .await
        .expect(404);
    ctx.post(&format!("/threads/{}/like", thread.id))
        .auth(&outsider)
        .send()
        .await
        .expect(404);
    ctx.get(&format!("/threads/{}/comments", thread.id))
        .auth(&outsider)
        .send()
        .await
        .expect(404);
    ctx.get(&format!("/threads/{}", course.id))
        .send()
        .await
        .expect(401);
    ctx.get("/threads/no-such-course")
        .auth(&member)
        .send()
        .await
        .expect_detail(404, "Course not found");
    ctx.get("/threads/detail/no-such-thread")
        .auth(&member)
        .send()
        .await
        .expect_detail(404, "Thread not found");

    let body = ctx
        .get(&format!("/threads/detail/{}", thread.id))
        .auth(&member)
        .send()
        .await
        .json();
    assert_keys(&body, THREAD_KEYS);
    assert_eq!(body["liked"], json!(false));
    assert_ts_z(&body["create_time"]);
}

#[tokio::test]
async fn update_thread_by_owner_only() {
    let ctx = Ctx::new().await;
    let owner = ctx.user().await;
    let admin = ctx.user().await;
    let dept = ctx.department(true).await;
    ctx.admin_of(&admin, &dept).await;
    let course = ctx.course(&dept).await;
    let thread = ctx.thread(&course, &owner).await;
    let path = format!("/threads/{}", thread.id);

    let resp = ctx
        .put(&path)
        .auth(&owner)
        .form(&[("title", "新標題")])
        .send()
        .await;
    resp.expect(200);
    assert_eq!(
        resp.json(),
        json!({"status": "success", "thread_id": thread.id})
    );
    let body = ctx
        .get(&format!("/threads/detail/{}", thread.id))
        .auth(&owner)
        .send()
        .await
        .json();
    assert_eq!(body["title"], json!("新標題"));
    assert_eq!(body["content"], json!("thread body"));
    assert_ts_z(&body["updated_time"]);

    ctx.put(&path)
        .auth(&owner)
        .form(&[("content", "新內容")])
        .send()
        .await
        .expect(200);
    let body = ctx
        .get(&format!("/threads/detail/{}", thread.id))
        .auth(&owner)
        .send()
        .await
        .json();
    assert_eq!(body["title"], json!("新標題"));
    assert_eq!(body["content"], json!("新內容"));

    ctx.put(&path)
        .auth(&admin)
        .form(&[("title", "x")])
        .send()
        .await
        .expect_detail(403, "Permission denied");
    ctx.put(&path)
        .auth(&owner)
        .form(&[("title", "")])
        .send()
        .await
        .expect_validation(&["body", "title"], Some("string_too_short"));
    ctx.put("/threads/no-such-thread")
        .auth(&owner)
        .form(&[("title", "x")])
        .send()
        .await
        .expect_detail(404, "Thread not found");

    // No fields: success, nothing changes.
    ctx.put(&path)
        .auth(&owner)
        .form(&[("unrelated", "x")])
        .send()
        .await
        .expect(200);
    let body = ctx
        .get(&format!("/threads/detail/{}", thread.id))
        .auth(&owner)
        .send()
        .await
        .json();
    assert_eq!(body["title"], json!("新標題"));
    assert_eq!(body["content"], json!("新內容"));
}

#[tokio::test]
async fn delete_thread_permissions_and_cascade() {
    let ctx = Ctx::new().await;
    let owner = ctx.user().await;
    let other = ctx.user().await;
    let admin = ctx.user().await;
    let root = ctx.super_user().await;
    let dept = ctx.department(true).await;
    ctx.admin_of(&admin, &dept).await;
    let course = ctx.course(&dept).await;

    let t1 = ctx.thread(&course, &owner).await;
    let c1 = ctx.comment(&t1, &other, None).await;
    ctx.comment(&t1, &other, Some(&c1)).await;
    ctx.post(&format!("/threads/{}/like", t1.id))
        .auth(&other)
        .send()
        .await
        .expect(200);
    ctx.post(&format!("/threads/comments/{c1}/like"))
        .auth(&owner)
        .send()
        .await
        .expect(200);

    ctx.delete(&format!("/threads/{}", t1.id))
        .auth(&other)
        .send()
        .await
        .expect_detail(403, "Permission denied");
    let resp = ctx
        .delete(&format!("/threads/{}", t1.id))
        .auth(&owner)
        .send()
        .await;
    resp.expect(200);
    assert_eq!(resp.json(), json!({"status": "success"}));
    assert_eq!(
        ctx.scalar_i64("SELECT count(*) FROM threads WHERE id = $1", &[&t1.id])
            .await,
        0
    );
    assert_eq!(
        ctx.scalar_i64(
            "SELECT count(*) FROM thread_comments WHERE thread_id = $1",
            &[&t1.id]
        )
        .await,
        0
    );
    assert_eq!(
        ctx.scalar_i64(
            "SELECT count(*) FROM thread_likes WHERE thread_id = $1",
            &[&t1.id]
        )
        .await,
        0
    );
    assert_eq!(
        ctx.scalar_i64(
            "SELECT count(*) FROM comment_likes WHERE comment_id = $1",
            &[&c1]
        )
        .await,
        0
    );

    let t2 = ctx.thread(&course, &owner).await;
    ctx.delete(&format!("/threads/{}", t2.id))
        .auth(&admin)
        .send()
        .await
        .expect(200);
    let t3 = ctx.thread(&course, &owner).await;
    ctx.delete(&format!("/threads/{}", t3.id))
        .auth(&root)
        .send()
        .await
        .expect(200);
    ctx.delete(&format!("/threads/{}", t3.id))
        .auth(&root)
        .send()
        .await
        .expect_detail(404, "Thread not found");
}

#[tokio::test]
async fn delete_thread_removes_its_image() {
    let ctx = Ctx::new().await;
    let owner = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let resp = ctx
        .post(&format!("/threads/{}", course.id))
        .auth(&owner)
        .multipart(
            thread_form("img").part("image", file_part(png_bytes(128), "a.png", "image/png")),
        )
        .send()
        .await;
    let id = resp.json()["thread_id"].as_str().unwrap().to_string();
    let url = ctx
        .get(&format!("/threads/detail/{id}"))
        .auth(&owner)
        .send()
        .await
        .json()["image_url"]
        .as_str()
        .unwrap()
        .to_string();
    ctx.fetch_object(&url).await.expect(200);
    ctx.delete(&format!("/threads/{id}"))
        .auth(&owner)
        .send()
        .await
        .expect(200);
    assert_eq!(ctx.fetch_object(&url).await.status, 404);
}

#[tokio::test]
async fn toggle_thread_like() {
    let ctx = Ctx::new().await;
    let a = ctx.user().await;
    let b = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let thread = ctx.thread(&course, &a).await;
    let path = format!("/threads/{}/like", thread.id);

    let resp = ctx.post(&path).auth(&a).send().await;
    resp.expect(200);
    let body = resp.json();
    assert_keys(&body, &["thread", "liked"]);
    assert_eq!(body["liked"], json!(true));
    assert_keys(&body["thread"], THREAD_KEYS);
    assert_eq!(body["thread"]["liked"], json!(true));
    assert_eq!(body["thread"]["like_count"], json!(1));
    assert_eq!(body["thread"]["is_owner"], json!(true));
    assert_ts_z(&body["thread"]["updated_time"]); // like_count update bumps updated_time (P-15)

    let body = ctx.post(&path).auth(&b).send().await.json();
    assert_eq!(body["thread"]["like_count"], json!(2));
    assert_eq!(body["thread"]["is_owner"], json!(false));

    let body = ctx.post(&path).auth(&a).send().await.json();
    assert_eq!(body["liked"], json!(false));
    assert_eq!(body["thread"]["liked"], json!(false));
    assert_eq!(body["thread"]["like_count"], json!(1));
    assert_eq!(
        ctx.scalar_i64(
            "SELECT count(*) FROM thread_likes WHERE thread_id = $1",
            &[&thread.id]
        )
        .await,
        1
    );

    ctx.post("/threads/no-such-thread/like")
        .auth(&a)
        .send()
        .await
        .expect_detail(404, "Thread not found");
}

#[tokio::test]
async fn concurrent_likes_are_counted_exactly() {
    let ctx = Ctx::new().await;
    let owner = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let thread = ctx.thread(&course, &owner).await;
    let mut users = Vec::new();
    for _ in 0..8 {
        users.push(ctx.user().await);
    }
    let path = format!("/threads/{}/like", thread.id);
    let handles: Vec<_> = users
        .iter()
        .map(|u| tokio::spawn(ctx.post(&path).auth(u).send()))
        .collect();
    for handle in handles {
        handle.await.unwrap().expect(200);
    }
    let count: Option<i32> = ctx
        .row(
            "SELECT like_count FROM threads WHERE id = $1",
            &[&thread.id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, Some(8));
    assert_eq!(
        ctx.scalar_i64(
            "SELECT count(*) FROM thread_likes WHERE thread_id = $1",
            &[&thread.id]
        )
        .await,
        8
    );
}
