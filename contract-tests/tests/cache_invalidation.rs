//! Cross-cutting cache behaviour (06-integrations.md §6.3): cached GETs must be
//! isolated per user and invalidated by every mutation in the matrix. Each test
//! warms the cache first, mutates through the API, then reads again.

use past_exam_contract_tests::*;
use serde_json::json;

#[tokio::test]
async fn cached_responses_are_never_shared_between_users() {
    let ctx = Ctx::new().await;
    let author = ctx.user().await;
    let stranger = ctx.user().await;
    let member = ctx.user().await;
    let public = ctx.department(true).await;
    let private = ctx.department(false).await;
    ctx.member(&member, &private).await;
    let course = ctx.course(&public).await;
    let private_course = ctx.course(&private).await;
    let pending = ctx.seed_post(&course, &author, "PENDING").await;
    let bulletin = ctx.bulletin(&private).await;

    ctx.get(&format!("/posts/{}", pending.id)).auth(&author).send().await.expect(200);
    ctx.get(&format!("/posts/{}", pending.id)).auth(&stranger).send().await.expect(404);

    ctx.get(&format!("/courses/{}", private_course.id)).auth(&member).send().await.expect(200);
    ctx.get(&format!("/courses/{}", private_course.id)).auth(&stranger).send().await.expect(404);

    ctx.get(&format!("/bulletins/{bulletin}")).auth(&member).send().await.expect(200);
    ctx.get(&format!("/bulletins/{bulletin}")).auth(&stranger).send().await.expect(404);

    ctx.get(&format!("/departments/{}/courses", private.id)).auth(&member).send().await.expect(200);
    ctx.get(&format!("/departments/{}/courses", private.id)).auth(&stranger).send().await.expect(404);

    let member_visible = ids(&ctx.get("/departments/visible").auth(&member).send().await.json());
    let stranger_visible = ids(&ctx.get("/departments/visible").auth(&stranger).send().await.json());
    assert!(member_visible.contains(&private.id));
    assert!(!stranger_visible.contains(&private.id));

    let admin = ctx.user().await;
    ctx.admin_of(&admin, &private).await;
    assert_eq!(ids(&ctx.get("/users/me/departments-admin").auth(&admin).send().await.json()), vec![private.id.clone()]);
    assert_eq!(ctx.get("/users/me/departments-admin").auth(&stranger).send().await.json(), json!([]));
}

#[tokio::test]
async fn admin_demotion_invalidates_cached_pending_post_detail() {
    let ctx = Ctx::new().await;
    let reviewer = ctx.user().await;
    let root = ctx.super_user().await;
    let author = ctx.user().await;
    let dept = ctx.department(true).await;
    ctx.admin_of(&reviewer, &dept).await;
    let course = ctx.course(&dept).await;
    let pending = ctx.seed_post(&course, &author, "PENDING").await;
    let path = format!("/posts/{}", pending.id);

    ctx.get(&path).auth(&reviewer).send().await.expect(200);
    ctx.put(&format!("/departments/{}/admin", dept.id))
        .auth(&root)
        .form(&[("user_id", &reviewer.id), ("is_admin", "false")])
        .send()
        .await
        .expect(200);
    ctx.get(&path).auth(&reviewer).send().await.expect(404);
}

#[tokio::test]
async fn admin_promotion_refreshes_departments_admin() {
    let ctx = Ctx::new().await;
    let root = ctx.super_user().await;
    let user = ctx.user().await;
    let dept = ctx.department(false).await;
    ctx.member(&user, &dept).await;
    assert_eq!(ctx.get("/users/me/departments-admin").auth(&user).send().await.json(), json!([]));

    ctx.put(&format!("/departments/{}/admin", dept.id))
        .auth(&root)
        .form(&[("user_id", &user.id), ("is_admin", "true")])
        .send()
        .await
        .expect(200);
    assert_eq!(ids(&ctx.get("/users/me/departments-admin").auth(&user).send().await.json()), vec![dept.id]);
}

#[tokio::test]
async fn approval_refreshes_departments_admin_cache() {
    rust_only!(); // F-05
    let ctx = Ctx::new().await;
    let root = ctx.super_user().await;
    let user = ctx.user().await;
    let dept = ctx.department(false).await;
    let request = ctx.membership(&user, &dept, "PENDING", true).await;
    assert_eq!(ctx.get("/users/me/departments-admin").auth(&user).send().await.json(), json!([]));
    ctx.put(&format!("/departments/join-request/{}/approve/{request}", dept.id))
        .auth(&root)
        .send()
        .await
        .expect(200);
    assert_eq!(ids(&ctx.get("/users/me/departments-admin").auth(&user).send().await.json()), vec![dept.id]);
}

#[tokio::test]
async fn new_post_is_visible_in_cached_course_views() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    assert_eq!(ctx.get(&format!("/courses/{}", course.id)).auth(&user).send().await.json()["posts"], json!([]));
    assert_eq!(
        ctx.get(&format!("/departments/{}/courses", dept.id)).auth(&user).send().await.json()[0]["has_posts"],
        json!(false)
    );

    let resp = ctx
        .post("/posts")
        .auth(&user)
        .form(&[("title", "new"), ("course_id", &course.id), ("department_id", &dept.id)])
        .send()
        .await;
    let id = resp.json()["post_id"].as_str().unwrap().to_string();

    assert_eq!(ids(&ctx.get(&format!("/courses/{}", course.id)).auth(&user).send().await.json()["posts"]), vec![id]);
    assert_eq!(
        ctx.get(&format!("/departments/{}/courses", dept.id)).auth(&user).send().await.json()[0]["has_posts"],
        json!(true)
    );
}

#[tokio::test]
async fn approval_refreshes_cached_course_and_bulletin_lists() {
    let ctx = Ctx::new().await;
    let root = ctx.super_user().await;
    let user = ctx.user().await;
    let dept = ctx.department(false).await;
    let course = ctx.course(&dept).await;
    let bulletin = ctx.bulletin(&dept).await;
    let request = ctx.membership(&user, &dept, "PENDING", false).await;

    let courses_before = ctx.get_all_pages("/courses", &user).await;
    assert!(!courses_before.iter().any(|c| c["id"] == json!(course.id)));
    let bulletins_before = ctx.get_all_pages("/bulletins", &user).await;
    assert!(!bulletins_before.iter().any(|b| b["id"] == json!(bulletin)));

    ctx.put(&format!("/departments/join-request/{}/approve/{request}", dept.id))
        .auth(&root)
        .send()
        .await
        .expect(200);

    assert!(ctx.get_all_pages("/courses", &user).await.iter().any(|c| c["id"] == json!(course.id)));
    assert!(ctx.get_all_pages("/bulletins", &user).await.iter().any(|b| b["id"] == json!(bulletin)));
    ctx.get(&format!("/departments/{}/bulletins", dept.id)).auth(&user).send().await.expect(200);
}
