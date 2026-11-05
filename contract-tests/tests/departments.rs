//! /departments* endpoints (04-api.md §4.4).

use past_exam_contract_tests::*;
use serde_json::json;

#[tokio::test]
async fn list_includes_private_departments_sorted_by_sort_order() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let late = ctx.create_department(true, 900).await;
    let early = ctx.create_department(false, 3).await;

    let resp = ctx.get("/departments").auth(&user).send().await;
    resp.expect(200);
    let body = resp.json();
    let all = ids(&body);
    let pos = |id: &str| all.iter().position(|v| v == id).expect("listed");
    assert!(pos(&early.id) < pos(&late.id));
    let item = body.as_array().unwrap().iter().find(|d| d["id"] == json!(early.id)).unwrap();
    assert_keys(item, DEPARTMENT_KEYS);
    assert_eq!(item["is_public"], json!(false));
    assert_eq!(item["sort_order"], json!(3));
    assert_eq!(item["name"], json!(early.name));
    assert_ts_offset(&item["create_time"]);

    let sort_orders: Vec<i64> = body.as_array().unwrap().iter().map(|d| d["sort_order"].as_i64().unwrap()).collect();
    assert!(sort_orders.windows(2).all(|w| w[0] <= w[1]), "sorted by sort_order");
}

#[tokio::test]
async fn single_department_is_readable_by_any_authenticated_user() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let private = ctx.department(false).await;
    let resp = ctx.get(&format!("/departments/{}", private.id)).auth(&user).send().await;
    resp.expect(200);
    assert_keys(&resp.json(), DEPARTMENT_KEYS);
    ctx.get("/departments/no-such-department")
        .auth(&user)
        .send()
        .await
        .expect_detail(404, "Not Found");
    ctx.get(&format!("/departments/{}", private.id)).send().await.expect(401);
}

#[tokio::test]
async fn visible_departments() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let public = ctx.create_department(true, 10).await;
    let approved = ctx.create_department(false, 11).await;
    let pending = ctx.create_department(false, 12).await;
    let hidden = ctx.create_department(false, 13).await;
    ctx.member(&user, &approved).await;
    ctx.membership(&user, &pending, "PENDING", false).await;

    let resp = ctx.get("/departments/visible").auth(&user).send().await;
    resp.expect(200);
    let body = resp.json();
    let visible = ids(&body);
    assert!(visible.contains(&public.id));
    assert!(visible.contains(&approved.id));
    assert!(!visible.contains(&pending.id));
    assert!(!visible.contains(&hidden.id));
    let pos = |id: &str| visible.iter().position(|v| v == id).unwrap();
    assert!(pos(&public.id) < pos(&approved.id));
    assert_keys(&body[0], DEPARTMENT_KEYS);

    let admin = ctx.super_user().await;
    let all = ids(&ctx.get("/departments/visible").auth(&admin).send().await.json());
    assert!(all.contains(&hidden.id));
}

#[tokio::test]
async fn status_lists_visible_and_pending() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let public = ctx.department(true).await;
    let approved = ctx.department(false).await;
    let pending = ctx.department(false).await;
    ctx.member(&user, &approved).await;
    ctx.membership(&user, &pending, "PENDING", false).await;

    let resp = ctx.get("/departments/status").auth(&user).send().await;
    resp.expect(200);
    let body = resp.json();
    assert_keys(&body, &["visible", "pending"]);
    let visible = ids(&body["visible"]);
    assert!(visible.contains(&public.id) && visible.contains(&approved.id));
    assert!(!visible.contains(&pending.id));
    assert_eq!(ids(&body["pending"]), vec![pending.id.clone()]);
    assert_keys(&body["pending"][0], DEPARTMENT_KEYS);

    let admin = ctx.super_user().await;
    let body = ctx.get("/departments/status").auth(&admin).send().await.json();
    assert!(ids(&body["visible"]).contains(&pending.id));
    assert_eq!(body["pending"], json!([]));
}

#[tokio::test]
async fn visible_check_only_counts_approved_membership() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let admin = ctx.super_user().await;
    let public = ctx.department(true).await;
    let approved = ctx.department(false).await;
    ctx.member(&user, &approved).await;

    let check = |u: User, d: Dept| {
        let ctx = &ctx;
        async move {
            let resp = ctx.get(&format!("/departments/{}/visible-check", d.id)).auth(&u).send().await;
            resp.expect(200);
            resp.json()
        }
    };
    assert_eq!(check(user.clone(), approved.clone()).await, json!({"can_view": true}));
    assert_eq!(check(user.clone(), public.clone()).await, json!({"can_view": false}));
    assert_eq!(check(admin.clone(), approved.clone()).await, json!({"can_view": false}));
}

#[tokio::test]
async fn admin_check_requires_approved_admin_membership() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let admin = ctx.super_user().await;
    let dept = ctx.department(false).await;
    let pending_dept = ctx.department(false).await;
    ctx.admin_of(&user, &dept).await;
    ctx.membership(&user, &pending_dept, "PENDING", true).await;

    let check = |u: &User, d: &Dept| ctx.get(&format!("/departments/{}/admin-check", d.id)).auth(u).send();
    assert_eq!(check(&user, &dept).await.json(), json!({"is_admin": true}));
    assert_eq!(check(&user, &pending_dept).await.json(), json!({"is_admin": false}));
    assert_eq!(check(&admin, &dept).await.json(), json!({"is_admin": false}));
}

#[tokio::test]
async fn department_courses_with_has_posts() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(true).await;
    let with_approved = ctx.course(&dept).await;
    let with_pending = ctx.course(&dept).await;
    let empty = ctx.course(&dept).await;
    ctx.seed_post(&with_approved, &user, "APPROVED").await;
    ctx.seed_post(&with_pending, &user, "PENDING").await;

    let resp = ctx.get(&format!("/departments/{}/courses", dept.id)).auth(&user).send().await;
    resp.expect(200);
    let body = resp.json();
    let courses = body.as_array().unwrap();
    assert_eq!(courses.len(), 3);
    for c in courses {
        assert_keys(c, &with_keys(COURSE_KEYS, &["has_posts"]));
        assert_ts_offset(&c["create_time"]);
        let expected = c["id"] == json!(with_approved.id);
        assert_eq!(c["has_posts"], json!(expected), "{c}");
    }
    assert!(ids(&body).contains(&empty.id));
}

#[tokio::test]
async fn department_courses_and_bulletins_hide_private_departments() {
    let ctx = Ctx::new().await;
    let outsider = ctx.user().await;
    let member = ctx.user().await;
    let admin = ctx.super_user().await;
    let private = ctx.department(false).await;
    ctx.member(&member, &private).await;
    ctx.course(&private).await;
    ctx.bulletin(&private).await;

    for suffix in ["courses", "bulletins"] {
        let path = format!("/departments/{}/{suffix}", private.id);
        ctx.get(&path).auth(&outsider).send().await.expect_detail(404, "Department not found");
        assert_eq!(ctx.get(&path).auth(&member).send().await.json().as_array().unwrap().len(), 1);
        ctx.get(&path).auth(&admin).send().await.expect(200);
        ctx.get(&format!("/departments/no-such-dept/{suffix}"))
            .auth(&member)
            .send()
            .await
            .expect_detail(404, "Department not found");
    }
}

#[tokio::test]
async fn department_bulletins_shape() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(true).await;
    let bulletin = ctx.bulletin(&dept).await;
    let body = ctx.get(&format!("/departments/{}/bulletins", dept.id)).auth(&user).send().await.json();
    assert_eq!(ids(&body), vec![bulletin]);
    assert_keys(&body[0], BULLETIN_KEYS);
}

#[tokio::test]
async fn send_join_request() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(false).await;
    let path = format!("/departments/{}/join-request/send", dept.id);

    let resp = ctx.post(&path).auth(&user).send().await;
    resp.expect(200);
    let body = resp.json();
    assert_keys(&body, MEMBERSHIP_KEYS);
    assert_eq!(body["user_id"], json!(user.id));
    assert_eq!(body["department_id"], json!(dept.id));
    assert_eq!(body["status"], json!("PENDING"));
    assert_eq!(body["is_department_admin"], json!(false));
    assert_ts_offset(&body["create_time"]);

    ctx.post(&path).auth(&user).send().await.expect_detail(400, "duplicate request");
    assert_eq!(
        ctx.scalar_i64(
            "SELECT count(*) FROM users_departments WHERE user_id = $1 AND department_id = $2",
            &[&user.id, &dept.id]
        )
        .await,
        1
    );

    let approved_user = ctx.user().await;
    ctx.member(&approved_user, &dept).await;
    ctx.post(&path)
        .auth(&approved_user)
        .send()
        .await
        .expect_detail(400, "request has been approved");

    ctx.post("/departments/no-such-dept/join-request/send")
        .auth(&user)
        .send()
        .await
        .expect_detail(404, "Not Found");
}

#[tokio::test]
async fn join_request_to_public_department_is_allowed() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(true).await;
    ctx.post(&format!("/departments/{}/join-request/send", dept.id))
        .auth(&user)
        .send()
        .await
        .expect(200);
}

#[tokio::test]
async fn join_request_listing_for_admins() {
    let ctx = Ctx::new().await;
    let admin = ctx.user().await;
    let other_admin = ctx.user().await;
    let applicant = ctx
        .create_user(NewUser {
            readable_name: Some("Applicant".into()),
            email: Some("applicant@example.test".into()),
            school_department: Some("Stat".into()),
            note: Some("please".into()),
            ..Default::default()
        })
        .await;
    let member = ctx.user().await;
    let dept = ctx.department(false).await;
    let other = ctx.department(false).await;
    ctx.admin_of(&admin, &dept).await;
    ctx.admin_of(&other_admin, &other).await;
    let request_id = ctx.membership(&applicant, &dept, "PENDING", false).await;
    ctx.member(&member, &dept).await;
    let path = format!("/departments/{}/join-request", dept.id);

    let resp = ctx.get(&path).auth(&admin).send().await;
    resp.expect(200);
    assert_eq!(
        resp.json(),
        json!([{
            "id": request_id,
            "status": "PENDING",
            "user_id": applicant.id,
            "email": "applicant@example.test",
            "username": applicant.username,
            "readable_name": "Applicant",
            "school_department": "Stat",
            "note": "please",
        }])
    );
    ctx.get(&path).auth(&other_admin).send().await.expect_detail(403, "Forbidden");
    ctx.get(&path).auth(&member).send().await.expect_detail(403, "Forbidden");
}

#[tokio::test]
async fn member_listing_for_admins() {
    let ctx = Ctx::new().await;
    let admin = ctx
        .create_user(NewUser { readable_name: Some("Admin".into()), ..Default::default() })
        .await;
    let member = ctx
        .create_user(NewUser {
            readable_name: Some("Member".into()),
            email: Some("m@example.test".into()),
            school_department: Some("Econ".into()),
            ..Default::default()
        })
        .await;
    let applicant = ctx.user().await;
    let dept = ctx.department(false).await;
    ctx.admin_of(&admin, &dept).await;
    ctx.member(&member, &dept).await;
    ctx.membership(&applicant, &dept, "PENDING", false).await;

    let resp = ctx.get(&format!("/departments/{}/members", dept.id)).auth(&admin).send().await;
    resp.expect(200);
    let body = resp.json();
    let mut members: Vec<_> = body.as_array().unwrap().clone();
    members.sort_by_key(|m| m["readable_name"].as_str().unwrap().to_string());
    assert_eq!(
        members,
        vec![
            json!({"id": admin.id, "user_id": admin.id, "email": null, "username": admin.username,
                   "readable_name": "Admin", "school_department": null, "is_department_admin": true}),
            json!({"id": member.id, "user_id": member.id, "email": "m@example.test", "username": member.username,
                   "readable_name": "Member", "school_department": "Econ", "is_department_admin": false}),
        ]
    );
}

#[tokio::test]
async fn set_member_admin_flag() {
    let ctx = Ctx::new().await;
    let admin = ctx.user().await;
    let member = ctx.user().await;
    let dept = ctx.department(false).await;
    ctx.admin_of(&admin, &dept).await;
    let membership = ctx.member(&member, &dept).await;
    let path = format!("/departments/{}/admin", dept.id);

    let resp = ctx.put(&path).auth(&admin).form(&[("user_id", &member.id), ("is_admin", "true")]).send().await;
    resp.expect(200);
    assert_eq!(resp.json(), json!({"status": "success"}));
    let row = ctx
        .row("SELECT is_department_admin, updated_time IS NOT NULL FROM users_departments WHERE id = $1", &[&membership])
        .await
        .unwrap();
    assert!(row.get::<_, bool>(0));
    assert!(row.get::<_, bool>(1), "updated_time is set");
    assert_eq!(
        ctx.get(&format!("/departments/{}/admin-check", dept.id)).auth(&member).send().await.json(),
        json!({"is_admin": true})
    );

    ctx.put(&path).auth(&admin).form(&[("user_id", &member.id), ("is_admin", "false")]).send().await.expect(200);
    ctx.get(&format!("/departments/{}/members", dept.id))
        .auth(&member)
        .send()
        .await
        .expect_detail(403, "Forbidden");
}

#[tokio::test]
async fn unchanged_admin_flag_does_not_touch_updated_time() {
    let ctx = Ctx::new().await;
    let root = ctx.super_user().await;
    let member = ctx.user().await;
    let dept = ctx.department(false).await;
    let membership = ctx.member(&member, &dept).await;
    ctx.put(&format!("/departments/{}/admin", dept.id))
        .auth(&root)
        .form(&[("user_id", &member.id), ("is_admin", "false")])
        .send()
        .await
        .expect(200);
    let touched: bool = ctx
        .row("SELECT updated_time IS NOT NULL FROM users_departments WHERE id = $1", &[&membership])
        .await
        .unwrap()
        .get(0);
    assert!(!touched, "no-op update must not set updated_time");
}

#[tokio::test]
async fn set_member_admin_errors() {
    let ctx = Ctx::new().await;
    let admin = ctx.user().await;
    let outsider = ctx.user().await;
    let stranger = ctx.user().await;
    let pending = ctx.user().await;
    let dept = ctx.department(false).await;
    ctx.admin_of(&admin, &dept).await;
    let pending_membership = ctx.membership(&pending, &dept, "PENDING", false).await;
    let path = format!("/departments/{}/admin", dept.id);

    ctx.put(&path)
        .auth(&outsider)
        .form(&[("user_id", &stranger.id), ("is_admin", "true")])
        .send()
        .await
        .expect_detail(403, "Forbidden");
    ctx.put(&path)
        .auth(&admin)
        .form(&[("user_id", &stranger.id), ("is_admin", "true")])
        .send()
        .await
        .expect_detail(404, "Not Found");
    ctx.put(&path)
        .auth(&admin)
        .form(&[("user_id", &stranger.id)])
        .send()
        .await
        .expect_validation(&["body", "is_admin"], Some("missing"));

    // Pending memberships can be flagged too (P-11), without granting access.
    ctx.put(&path)
        .auth(&admin)
        .form(&[("user_id", &pending.id), ("is_admin", "true")])
        .send()
        .await
        .expect(200);
    let row = ctx
        .row("SELECT is_department_admin, status FROM users_departments WHERE id = $1", &[&pending_membership])
        .await
        .unwrap();
    assert!(row.get::<_, bool>(0));
    assert_eq!(row.get::<_, String>(1), "PENDING");
}

#[tokio::test]
async fn approve_join_request() {
    let ctx = Ctx::new().await;
    let admin = ctx.user().await;
    let applicant = ctx.user().await;
    let dept = ctx.department(false).await;
    ctx.admin_of(&admin, &dept).await;
    ctx.course(&dept).await;
    let request_id = ctx.membership(&applicant, &dept, "PENDING", false).await;

    // Warm the applicant's caches while still pending.
    assert!(!ids(&ctx.get("/departments/visible").auth(&applicant).send().await.json()).contains(&dept.id));
    assert_eq!(
        ids(&ctx.get("/departments/status").auth(&applicant).send().await.json()["pending"]),
        vec![dept.id.clone()]
    );
    ctx.get(&format!("/departments/{}/courses", dept.id))
        .auth(&applicant)
        .send()
        .await
        .expect(404);

    let path = format!("/departments/join-request/{}/approve/{}", dept.id, request_id);
    let resp = ctx.put(&path).auth(&admin).send().await;
    resp.expect(200);
    assert_eq!(resp.json(), json!({"status": "success"}));
    let row = ctx
        .row("SELECT status, updated_time IS NOT NULL FROM users_departments WHERE id = $1", &[&request_id])
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "APPROVED");
    assert!(row.get::<_, bool>(1));

    // Caches are invalidated: the new membership is visible immediately.
    assert!(ids(&ctx.get("/departments/visible").auth(&applicant).send().await.json()).contains(&dept.id));
    let status = ctx.get("/departments/status").auth(&applicant).send().await.json();
    assert_eq!(status["pending"], json!([]));
    assert!(ids(&status["visible"]).contains(&dept.id));
    ctx.get(&format!("/departments/{}/courses", dept.id))
        .auth(&applicant)
        .send()
        .await
        .expect(200);
    assert!(ids(&ctx.get("/verify-token").auth(&applicant).send().await.json()["visible_departments"]).contains(&dept.id));

    ctx.put(&path).auth(&admin).send().await.expect_detail(400, "request is not pending");
}

#[tokio::test]
async fn approval_is_bound_to_the_authorized_department() {
    let ctx = Ctx::new().await;
    let admin = ctx.user().await;
    let applicant = ctx.user().await;
    let mine = ctx.department(false).await;
    let theirs = ctx.department(false).await;
    ctx.admin_of(&admin, &mine).await;
    let foreign_request = ctx.membership(&applicant, &theirs, "PENDING", false).await;

    ctx.put(&format!("/departments/join-request/{}/approve/{}", mine.id, foreign_request))
        .auth(&admin)
        .send()
        .await
        .expect_detail(404, "Not Found");
    ctx.put(&format!("/departments/join-request/{}/approve/{}", theirs.id, foreign_request))
        .auth(&admin)
        .send()
        .await
        .expect_detail(403, "Forbidden");
    let status: String = ctx
        .row("SELECT status FROM users_departments WHERE id = $1", &[&foreign_request])
        .await
        .unwrap()
        .get(0);
    assert_eq!(status, "PENDING");
}

#[tokio::test]
async fn join_request_send_refreshes_status_cache() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(false).await;
    assert_eq!(ctx.get("/departments/status").auth(&user).send().await.json()["pending"], json!([]));
    ctx.post(&format!("/departments/{}/join-request/send", dept.id)).auth(&user).send().await.expect(200);
    assert_eq!(
        ids(&ctx.get("/departments/status").auth(&user).send().await.json()["pending"]),
        vec![dept.id]
    );
}
