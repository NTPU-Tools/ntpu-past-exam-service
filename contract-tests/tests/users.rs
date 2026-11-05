//! /users* endpoints (04-api.md §4.3).

use past_exam_contract_tests::*;
use serde_json::json;

#[tokio::test]
async fn list_users_requires_super_user_and_hides_password_hashes() {
    let ctx = Ctx::new().await;
    let admin = ctx.super_user().await;
    let regular = ctx.user().await;

    ctx.get("/users").auth(&regular).send().await.expect_detail(403, "Forbidden");

    let resp = ctx.get("/users").auth(&admin).query(&[("limit", "100")]).send().await;
    resp.expect(200);
    let users = resp.json();
    let users = users.as_array().unwrap();
    assert!(!users.is_empty() && users.len() <= 100);
    for u in users {
        assert_keys(u, USER_KEYS);
        assert_ts_offset(&u["create_time"]);
    }

    let page = ctx.get("/users").auth(&admin).query(&[("limit", "1"), ("skip", "1")]).send().await;
    page.expect(200);
    assert_eq!(page.json().as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn list_users_pagination_is_strict() {
    let ctx = Ctx::new().await;
    let admin = ctx.super_user().await;
    for (key, value, ty) in [
        ("skip", "-1", "greater_than_equal"),
        ("skip", "10001", "less_than_equal"),
        ("limit", "0", "greater_than_equal"),
        ("limit", "101", "less_than_equal"),
        ("limit", "abc", "int_parsing"),
    ] {
        ctx.get("/users")
            .auth(&admin)
            .query(&[(key, value)])
            .send()
            .await
            .expect_validation(&["query", key], Some(ty));
    }
    ctx.get("/users")
        .auth(&admin)
        .query(&[("skip", "10000"), ("limit", "100")])
        .send()
        .await
        .expect(200);
}

#[tokio::test]
async fn removed_is_active_filter_returns_gone() {
    let ctx = Ctx::new().await;
    let admin = ctx.super_user().await;
    ctx.get("/users")
        .auth(&admin)
        .query(&[("is_active", "true")])
        .send()
        .await
        .expect_detail(410, "The is_active user field is no longer supported");
    ctx.get("/users")
        .auth(&admin)
        .query(&[("is_active", "maybe")])
        .send()
        .await
        .expect_validation(&["query", "is_active"], Some("bool_parsing"));
}

#[tokio::test]
async fn me_returns_full_profile_with_default_preferences() {
    let ctx = Ctx::new().await;
    let user = ctx
        .create_user(NewUser {
            readable_name: Some("王小明".into()),
            email: Some("me@example.test".into()),
            school_department: Some("Law".into()),
            note: Some("hello".into()),
            password: Some("pw".into()),
            ..Default::default()
        })
        .await;
    let resp = ctx.get("/users/me").auth(&user).send().await;
    resp.expect(200);
    let body = resp.json();
    assert_keys(&body, &with_keys(USER_KEYS, &["show_empty_courses", "default_is_anonymous"]));
    assert_eq!(body["id"], json!(user.id));
    assert_eq!(body["username"], json!(user.username));
    assert_eq!(body["readable_name"], json!("王小明"));
    assert_eq!(body["email"], json!("me@example.test"));
    assert_eq!(body["school_department"], json!("Law"));
    assert_eq!(body["note"], json!("hello"));
    assert_eq!(body["is_super_user"], json!(false));
    assert_eq!(body["updated_time"], json!(null));
    assert_eq!(body["show_empty_courses"], json!(true));
    assert_eq!(body["default_is_anonymous"], json!(false));
    assert_ts_offset(&body["create_time"]);
}

#[tokio::test]
async fn me_reflects_stored_preferences() {
    let ctx = Ctx::new().await;
    let user = ctx
        .create_user(NewUser { preferences: Some((false, true)), ..Default::default() })
        .await;
    let body = ctx.get("/users/me").auth(&user).send().await.json();
    assert_eq!(body["show_empty_courses"], json!(false));
    assert_eq!(body["default_is_anonymous"], json!(true));
}

#[tokio::test]
async fn viewing_own_id_returns_the_redacted_view() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let resp = ctx.get(&format!("/users/{}", user.id)).auth(&user).send().await;
    resp.expect(200);
    assert_keys(&resp.json(), USER_REDACTED_KEYS);
}

#[tokio::test]
async fn other_profiles_require_a_shared_admin_department() {
    let ctx = Ctx::new().await;
    let viewer = ctx.user().await;
    let target = ctx
        .create_user(NewUser { note: Some("join reason".into()), email: Some("t@x".into()), ..Default::default() })
        .await;
    let path = format!("/users/{}", target.id);

    ctx.get(&path).auth(&viewer).send().await.expect_detail(403, "Not allowed to view this user");

    // Admin of a department where the target only has a PENDING request.
    let dept = ctx.department(false).await;
    ctx.admin_of(&viewer, &dept).await;
    ctx.membership(&target, &dept, "PENDING", false).await;
    let resp = ctx.get(&path).auth(&viewer).send().await;
    resp.expect(200);
    let body = resp.json();
    assert_keys(&body, USER_REDACTED_KEYS);
    assert_eq!(body["note"], json!("join reason"));
}

#[tokio::test]
async fn admin_of_unrelated_department_cannot_view() {
    let ctx = Ctx::new().await;
    let viewer = ctx.user().await;
    let target = ctx.user().await;
    let mine = ctx.department(false).await;
    let theirs = ctx.department(false).await;
    ctx.admin_of(&viewer, &mine).await;
    ctx.member(&target, &theirs).await;
    // An unapproved admin flag does not count either.
    ctx.membership(&viewer, &theirs, "PENDING", true).await;
    ctx.get(&format!("/users/{}", target.id))
        .auth(&viewer)
        .send()
        .await
        .expect_detail(403, "Not allowed to view this user");
}

#[tokio::test]
async fn super_user_views_anyone_and_gets_404_for_missing_users() {
    let ctx = Ctx::new().await;
    let admin = ctx.super_user().await;
    let regular = ctx.user().await;
    let target = ctx.user().await;
    let resp = ctx.get(&format!("/users/{}", target.id)).auth(&admin).send().await;
    resp.expect(200);
    assert_keys(&resp.json(), USER_REDACTED_KEYS);

    ctx.get("/users/no-such-user").auth(&admin).send().await.expect_detail(404, "User not found");
    ctx.get("/users/no-such-user")
        .auth(&regular)
        .send()
        .await
        .expect_detail(403, "Not allowed to view this user");
}

#[tokio::test]
async fn departments_admin_lists_approved_admin_departments() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let admin_dept = ctx.department(false).await;
    let member_dept = ctx.department(false).await;
    let pending_admin = ctx.department(false).await;
    ctx.admin_of(&user, &admin_dept).await;
    ctx.member(&user, &member_dept).await;
    ctx.membership(&user, &pending_admin, "PENDING", true).await;

    let resp = ctx.get("/users/me/departments-admin").auth(&user).send().await;
    resp.expect(200);
    let body = resp.json();
    assert_eq!(ids(&body), vec![admin_dept.id.clone()]);
    assert_keys(&body[0], DEPARTMENT_KEYS);

    let nobody = ctx.user().await;
    assert_eq!(ctx.get("/users/me/departments-admin").auth(&nobody).send().await.json(), json!([]));
}

#[tokio::test]
async fn departments_admin_for_super_user_is_every_department() {
    let ctx = Ctx::new().await;
    let admin = ctx.super_user().await;
    let dept = ctx.department(false).await;
    let body = ctx.get("/users/me/departments-admin").auth(&admin).send().await.json();
    assert!(ids(&body).contains(&dept.id));
}

#[tokio::test]
async fn update_me_changes_major_and_note_only() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let before = ctx.get("/users/me").auth(&user).send().await.json();

    let resp = ctx
        .put("/users/update/me")
        .auth(&user)
        .form(&[("major", "經濟學系"), ("note", "請讓我加入")])
        .send()
        .await;
    resp.expect(200);
    let body = resp.json();
    assert_keys(&body, &with_keys(USER_KEYS, &["show_empty_courses", "default_is_anonymous"]));
    assert_eq!(body["school_department"], json!("經濟學系"));
    assert_eq!(body["note"], json!("請讓我加入"));
    for key in ["username", "readable_name", "email", "is_super_user", "create_time"] {
        assert_eq!(body[key], before[key], "{key} must not change");
    }
    assert_ts_offset(&body["updated_time"]);

    // Password still works (hashed_password untouched).
    ctx.login(&user).await;
}

#[tokio::test]
async fn update_me_note_defaults_to_empty_and_major_is_required() {
    let ctx = Ctx::new().await;
    let user = ctx
        .create_user(NewUser { note: Some("old".into()), ..Default::default() })
        .await;
    let body = ctx.put("/users/update/me").auth(&user).form(&[("major", "")]).send().await.json();
    assert_eq!(body["note"], json!(""));
    assert_eq!(body["school_department"], json!(""));

    ctx.put("/users/update/me")
        .auth(&user)
        .form(&[("note", "x")])
        .send()
        .await
        .expect_validation(&["body", "major"], Some("missing"));
}

#[tokio::test]
async fn preferences_upsert() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await; // no preferences row yet

    let resp = ctx
        .put("/users/me/preferences")
        .auth(&user)
        .json(&json!({"show_empty_courses": false, "default_is_anonymous": true}))
        .send()
        .await;
    resp.expect(200);
    assert_eq!(resp.json(), json!({"show_empty_courses": false, "default_is_anonymous": true}));

    let resp = ctx
        .put("/users/me/preferences")
        .auth(&user)
        .json(&json!({"show_empty_courses": true, "default_is_anonymous": true}))
        .send()
        .await;
    assert_eq!(resp.json(), json!({"show_empty_courses": true, "default_is_anonymous": true}));
    assert_eq!(
        ctx.scalar_i64("SELECT count(*) FROM user_preferences WHERE user_id = $1", &[&user.id]).await,
        1
    );
    let me = ctx.get("/users/me").auth(&user).send().await.json();
    assert_eq!(me["show_empty_courses"], json!(true));
    assert_eq!(me["default_is_anonymous"], json!(true));
}

#[tokio::test]
async fn removed_status_endpoints_return_gone_after_authorization() {
    let ctx = Ctx::new().await;
    let admin = ctx.user().await;
    let outsider = ctx.user().await;
    let target = ctx.user().await;
    let dept = ctx.department(false).await;
    ctx.admin_of(&admin, &dept).await;

    let status_path = format!("/users/status/{}/{}", dept.id, target.id);
    let admin_path = format!("/users/admin/{}/{}", dept.id, target.id);

    ctx.put(&status_path)
        .auth(&admin)
        .form(&[("is_active", "true")])
        .send()
        .await
        .expect_detail(410, "Global user status is no longer supported");
    ctx.put(&admin_path)
        .auth(&admin)
        .form(&[("is_admin", "true")])
        .send()
        .await
        .expect_detail(410, &format!("Use /departments/{}/admin instead", dept.id));

    ctx.put(&status_path)
        .auth(&outsider)
        .form(&[("is_active", "true")])
        .send()
        .await
        .expect_detail(403, "Forbidden");
    ctx.put(&status_path)
        .auth(&admin)
        .send()
        .await
        .expect_validation(&["body", "is_active"], Some("missing"));
}
