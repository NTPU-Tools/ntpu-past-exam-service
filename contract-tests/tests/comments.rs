//! /threads/{id}/comments and /threads/comments/* endpoints (04-api.md §4.8).

use past_exam_contract_tests::*;
use serde_json::{json, Value};

struct Fixture {
    ctx: Ctx,
    owner: User,
    thread: Thread,
}

async fn fixture() -> Fixture {
    let ctx = Ctx::new().await;
    let owner = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let thread = ctx.thread(&course, &owner).await;
    Fixture { ctx, owner, thread }
}

fn find<'a>(list: &'a Value, id: &str) -> &'a Value {
    list.as_array().unwrap().iter().find(|c| c["id"] == json!(id)).unwrap()
}

#[tokio::test]
async fn create_top_level_and_reply() {
    let Fixture { ctx, owner, thread } = fixture().await;
    let path = format!("/threads/{}/comments", thread.id);

    let resp = ctx.post(&path).auth(&owner).form(&[("content", "第一樓")]).send().await;
    resp.expect(200);
    let body = resp.json();
    assert_keys(&body, &["status", "comment_id"]);
    let root = body["comment_id"].as_str().unwrap().to_string();

    let reply = ctx
        .post(&path)
        .auth(&owner)
        .form(&[("content", "回覆"), ("parent_comment_id", &root), ("is_anonymous", "true")])
        .send()
        .await
        .json()["comment_id"]
        .as_str()
        .unwrap()
        .to_string();
    let row = ctx
        .row("SELECT thread_id, parent_comment_id, content, owner_id, is_anonymous, like_count FROM thread_comments WHERE id = $1", &[&reply])
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), thread.id);
    assert_eq!(row.get::<_, Option<String>>(1).as_deref(), Some(root.as_str()));
    assert_eq!(row.get::<_, String>(2), "回覆");
    assert_eq!(row.get::<_, String>(3), owner.id);
    assert_eq!(row.get::<_, Option<bool>>(4), Some(true));
    assert_eq!(row.get::<_, Option<i32>>(5), Some(0));
}

#[tokio::test]
async fn create_comment_errors() {
    let Fixture { ctx, owner, thread } = fixture().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let other_thread = ctx.thread(&course, &owner).await;
    let foreign_parent = ctx.comment(&other_thread, &owner, None).await;
    let path = format!("/threads/{}/comments", thread.id);

    ctx.post(&path)
        .auth(&owner)
        .form(&[("content", "x"), ("parent_comment_id", "no-such-comment")])
        .send()
        .await
        .expect_detail(404, "Parent comment not found");
    ctx.post(&path)
        .auth(&owner)
        .form(&[("content", "x"), ("parent_comment_id", &foreign_parent)])
        .send()
        .await
        .expect_detail(400, "Parent comment does not belong to this thread");
    ctx.post(&path)
        .auth(&owner)
        .form(&[("content", &"a".repeat(2001))])
        .send()
        .await
        .expect_validation(&["body", "content"], Some("string_too_long"));
    ctx.post(&path)
        .auth(&owner)
        .form(&[("content", "")])
        .send()
        .await
        .expect_validation(&["body", "content"], Some("string_too_short"));
    ctx.post("/threads/no-such-thread/comments")
        .auth(&owner)
        .form(&[("content", "x")])
        .send()
        .await
        .expect_detail(404, "Thread not found");
}

#[tokio::test]
async fn empty_parent_comment_id_creates_top_level_comment() {
    rust_only!(); // F-01: the reference implementation answers 500 here.
    let Fixture { ctx, owner, thread } = fixture().await;
    let resp = ctx
        .post(&format!("/threads/{}/comments", thread.id))
        .auth(&owner)
        .form(&[("content", "x"), ("parent_comment_id", "")])
        .send()
        .await;
    resp.expect(200);
    let id = resp.json()["comment_id"].as_str().unwrap().to_string();
    let parent: Option<String> = ctx
        .row("SELECT parent_comment_id FROM thread_comments WHERE id = $1", &[&id])
        .await
        .unwrap()
        .get(0);
    assert_eq!(parent, None);
}

#[tokio::test]
async fn list_top_level_comments() {
    let Fixture { ctx, owner, thread } = fixture().await;
    let viewer = ctx.create_user(NewUser { readable_name: Some("讀者".into()), ..Default::default() }).await;
    let first = ctx.create_comment(&thread, &viewer, None, None, Some("2025-01-01T00:00:00Z")).await;
    let second = ctx.create_comment(&thread, &owner, None, None, Some("2025-02-01T00:00:00Z")).await;
    ctx.comment(&thread, &owner, Some(&first)).await;
    ctx.comment(&thread, &owner, Some(&first)).await;
    ctx.post(&format!("/threads/comments/{second}/like")).auth(&viewer).send().await.expect(200);

    let resp = ctx.get(&format!("/threads/{}/comments", thread.id)).auth(&viewer).send().await;
    resp.expect(200);
    let body = resp.json();
    assert_eq!(ids(&body), vec![first.clone(), second.clone()], "top level only, create_time ASC");
    for c in body.as_array().unwrap() {
        assert_keys(c, COMMENT_KEYS);
        assert_ts_z(&c["create_time"]);
        assert_eq!(c["replies"], json!(null));
        assert_eq!(c["parent_comment_id"], json!(null));
    }
    let f = find(&body, &first);
    assert_eq!(f["reply_count"], json!(2));
    assert_eq!(f["is_owner"], json!(true));
    assert_eq!(f["owner_name"], json!("讀者"));
    assert_eq!(f["liked"], json!(false));
    let s = find(&body, &second);
    assert_eq!(s["reply_count"], json!(0));
    assert_eq!(s["liked"], json!(true));
    assert_eq!(s["like_count"], json!(1));
    assert_eq!(s["is_owner"], json!(false));

    let page = ctx
        .get(&format!("/threads/{}/comments", thread.id))
        .auth(&viewer)
        .query(&[("skip", "1"), ("limit", "1")])
        .send()
        .await
        .json();
    assert_eq!(ids(&page), vec![second]);
}

#[tokio::test]
async fn comment_detail_returns_nested_reply_tree() {
    let Fixture { ctx, owner, thread } = fixture().await;
    let other = ctx.user().await;
    let root = ctx.create_comment(&thread, &owner, None, None, Some("2025-01-01T00:00:00Z")).await;
    let a = ctx.create_comment(&thread, &other, Some(&root), None, Some("2025-01-02T00:00:00Z")).await;
    let b = ctx.create_comment(&thread, &other, Some(&root), None, Some("2025-01-03T00:00:00Z")).await;
    let a1 = ctx.create_comment(&thread, &owner, Some(&a), None, Some("2025-01-04T00:00:00Z")).await;
    ctx.post(&format!("/threads/comments/{a1}/like")).auth(&owner).send().await.expect(200);

    let resp = ctx.get(&format!("/threads/comments/{root}")).auth(&owner).send().await;
    resp.expect(200);
    let body = resp.json();
    assert_keys(&body, COMMENT_KEYS);
    assert_eq!(body["id"], json!(root));
    assert_eq!(body["reply_count"], json!(2));
    assert_eq!(ids(&body["replies"]), vec![a.clone(), b.clone()]);
    let node_a = &body["replies"][0];
    assert_keys(node_a, COMMENT_KEYS);
    assert_eq!(node_a["parent_comment_id"], json!(root));
    assert_eq!(node_a["reply_count"], json!(1));
    assert_eq!(ids(&node_a["replies"]), vec![a1.clone()]);
    let leaf = &node_a["replies"][0];
    assert_eq!(leaf["replies"], json!([]));
    assert_eq!(leaf["reply_count"], json!(0));
    assert_eq!(leaf["liked"], json!(true));
    assert_eq!(body["replies"][1]["replies"], json!([]));

    // A reply can be the root of a detail request too.
    let sub = ctx.get(&format!("/threads/comments/{a}")).auth(&owner).send().await.json();
    assert_eq!(ids(&sub["replies"]), vec![a1]);

    ctx.get("/threads/comments/no-such-comment").auth(&owner).send().await.expect_detail(404, "Comment not found");
}

#[tokio::test]
async fn comment_detail_depth_is_limited_to_ten_levels() {
    let Fixture { ctx, owner, thread } = fixture().await;
    let root = ctx.comment(&thread, &owner, None).await;
    let mut parent = root.clone();
    for _ in 0..12 {
        parent = ctx.comment(&thread, &owner, Some(&parent)).await;
    }
    let body = ctx.get(&format!("/threads/comments/{root}")).auth(&owner).send().await.json();
    let mut depth = 0;
    let mut node = &body;
    while let Some(child) = node["replies"].as_array().and_then(|r| r.first()) {
        depth += 1;
        node = child;
    }
    assert_eq!(depth, 10);
    assert_eq!(node["replies"], json!([]));
    assert_eq!(node["reply_count"], json!(0));
}

#[tokio::test]
async fn comment_detail_caps_replies_at_500_ordered_by_time_then_id() {
    let Fixture { ctx, owner, thread } = fixture().await;
    let root = ctx.comment(&thread, &owner, None).await;
    let prefix = uid("r");
    let rows: Vec<String> = (0..501).rev().map(|i| format!("{prefix}-{i:04}")).collect();
    ctx.db
        .execute(
            "INSERT INTO thread_comments (id, thread_id, parent_comment_id, content, owner_id, is_anonymous, like_count, create_time) \
             SELECT x, $2, $3, 'reply', $4, false, 0, '2026-01-01T00:00:00Z' FROM unnest($1::text[]) AS x",
            &[&rows, &thread.id, &root, &owner.id],
        )
        .await
        .unwrap();
    let body = ctx.get(&format!("/threads/comments/{root}")).auth(&owner).send().await.json();
    let got = ids(&body["replies"]);
    let want: Vec<String> = (0..500).map(|i| format!("{prefix}-{i:04}")).collect();
    assert_eq!(got, want);
    assert_eq!(body["reply_count"], json!(500));
}

#[tokio::test]
async fn update_comment() {
    let Fixture { ctx, owner, thread } = fixture().await;
    let other = ctx.user().await;
    let id = ctx.comment(&thread, &owner, None).await;
    let path = format!("/threads/comments/{id}");

    let resp = ctx.put(&path).auth(&owner).form(&[("content", "已修改")]).send().await;
    resp.expect(200);
    assert_eq!(resp.json(), json!({"status": "success", "comment_id": id}));
    let body = ctx.get(&path).auth(&owner).send().await.json();
    assert_eq!(body["content"], json!("已修改"));
    assert_ts_z(&body["updated_time"]);

    ctx.put(&path).auth(&other).form(&[("content", "x")]).send().await.expect_detail(403, "Permission denied");
    ctx.put(&path).auth(&owner).form(&[("other", "x")]).send().await.expect_detail(400, "Content is required");
    ctx.put(&path)
        .auth(&owner)
        .form(&[("content", "")])
        .send()
        .await
        .expect_validation(&["body", "content"], Some("string_too_short"));
    ctx.put("/threads/comments/no-such").auth(&owner).form(&[("content", "x")]).send().await.expect_detail(404, "Comment not found");
}

#[tokio::test]
async fn delete_comment_permissions_and_cascade() {
    let ctx = Ctx::new().await;
    let thread_owner = ctx.user().await;
    let author = ctx.user().await;
    let other = ctx.user().await;
    let admin = ctx.user().await;
    let root_user = ctx.super_user().await;
    let dept = ctx.department(true).await;
    ctx.admin_of(&admin, &dept).await;
    let course = ctx.course(&dept).await;
    let thread = ctx.thread(&course, &thread_owner).await;

    let c1 = ctx.comment(&thread, &author, None).await;
    let reply = ctx.comment(&thread, &other, Some(&c1)).await;
    ctx.post(&format!("/threads/comments/{reply}/like")).auth(&author).send().await.expect(200);

    // Neither the thread owner nor other users may delete someone's comment.
    ctx.delete(&format!("/threads/comments/{c1}")).auth(&thread_owner).send().await.expect_detail(403, "Permission denied");
    ctx.delete(&format!("/threads/comments/{c1}")).auth(&other).send().await.expect_detail(403, "Permission denied");

    let resp = ctx.delete(&format!("/threads/comments/{c1}")).auth(&author).send().await;
    resp.expect(200);
    assert_eq!(resp.json(), json!({"status": "success"}));
    assert_eq!(
        ctx.scalar_i64("SELECT count(*) FROM thread_comments WHERE id = ANY($1)", &[&vec![c1.clone(), reply.clone()]]).await,
        0
    );
    assert_eq!(ctx.scalar_i64("SELECT count(*) FROM comment_likes WHERE comment_id = $1", &[&reply]).await, 0);

    let c2 = ctx.comment(&thread, &author, None).await;
    ctx.delete(&format!("/threads/comments/{c2}")).auth(&admin).send().await.expect(200);
    let c3 = ctx.comment(&thread, &author, None).await;
    ctx.delete(&format!("/threads/comments/{c3}")).auth(&root_user).send().await.expect(200);
    ctx.delete(&format!("/threads/comments/{c3}")).auth(&root_user).send().await.expect_detail(404, "Comment not found");
}

#[tokio::test]
async fn toggle_comment_like() {
    let Fixture { ctx, owner, thread } = fixture().await;
    let other = ctx.user().await;
    let id = ctx.comment(&thread, &owner, None).await;
    ctx.comment(&thread, &owner, Some(&id)).await;
    let path = format!("/threads/comments/{id}/like");

    let resp = ctx.post(&path).auth(&other).send().await;
    resp.expect(200);
    let body = resp.json();
    assert_keys(&body, &["comment", "liked"]);
    assert_eq!(body["liked"], json!(true));
    let c = &body["comment"];
    assert_keys(c, COMMENT_KEYS);
    assert_eq!(c["liked"], json!(true));
    assert_eq!(c["like_count"], json!(1));
    assert_eq!(c["reply_count"], json!(null));
    assert_eq!(c["replies"], json!(null));
    assert_eq!(c["is_owner"], json!(false));
    assert_ts_z(&c["updated_time"]);

    let body = ctx.post(&path).auth(&other).send().await.json();
    assert_eq!(body["liked"], json!(false));
    assert_eq!(body["comment"]["like_count"], json!(0));
    ctx.post("/threads/comments/no-such/like").auth(&other).send().await.expect_detail(404, "Comment not found");
}

#[tokio::test]
async fn comment_endpoints_enforce_department_visibility() {
    let ctx = Ctx::new().await;
    let member = ctx.user().await;
    let outsider = ctx.user().await;
    let dept = ctx.department(false).await;
    ctx.member(&member, &dept).await;
    let course = ctx.course(&dept).await;
    let thread = ctx.thread(&course, &member).await;
    let comment = ctx.comment(&thread, &member, None).await;

    for resp in [
        ctx.get(&format!("/threads/comments/{comment}")).auth(&outsider).send().await,
        ctx.put(&format!("/threads/comments/{comment}")).auth(&outsider).form(&[("content", "x")]).send().await,
        ctx.delete(&format!("/threads/comments/{comment}")).auth(&outsider).send().await,
        ctx.post(&format!("/threads/comments/{comment}/like")).auth(&outsider).send().await,
        ctx.post(&format!("/threads/{}/comments", thread.id)).auth(&outsider).form(&[("content", "x")]).send().await,
    ] {
        resp.expect_detail(404, "Department not found");
    }
    ctx.get(&format!("/threads/comments/{comment}")).send().await.expect(401);
    ctx.get(&format!("/threads/comments/{comment}")).auth(&member).send().await.expect(200);
}
