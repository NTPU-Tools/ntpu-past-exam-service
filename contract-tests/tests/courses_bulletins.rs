//! /courses* and /bulletins* endpoints (04-api.md §4.5–4.6).

use past_exam_contract_tests::*;
use serde_json::json;

#[tokio::test]
async fn course_list_is_limited_to_visible_departments_and_brief() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let public = ctx.department(true).await;
    let member_of = ctx.department(false).await;
    let hidden = ctx.department(false).await;
    ctx.member(&user, &member_of).await;
    let c_public = ctx.course(&public).await;
    let c_member = ctx.course(&member_of).await;
    let c_hidden = ctx.course(&hidden).await;

    let all = ctx.get_all_pages("/courses", &user).await;
    let listed: Vec<String> = all
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect();
    assert!(listed.contains(&c_public.id));
    assert!(listed.contains(&c_member.id));
    assert!(!listed.contains(&c_hidden.id));
    let item = all.iter().find(|c| c["id"] == json!(c_member.id)).unwrap();
    assert_eq!(
        item,
        &json!({"id": c_member.id, "name": c_member.name, "category": "required"})
    );

    let admin = ctx.super_user().await;
    let all = ctx.get_all_pages("/courses", &admin).await;
    assert!(all.iter().any(|c| c["id"] == json!(c_hidden.id)));
}

#[tokio::test]
async fn list_pagination_is_clamped() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(true).await;
    ctx.course(&dept).await;
    ctx.course(&dept).await;
    ctx.bulletin(&dept).await;
    ctx.bulletin(&dept).await;

    for path in ["/courses", "/bulletins"] {
        let one = ctx
            .get(path)
            .auth(&user)
            .query(&[("limit", "0")])
            .send()
            .await;
        one.expect(200);
        assert_eq!(
            one.json().as_array().unwrap().len(),
            1,
            "{path} limit=0 clamps to 1"
        );
        let many = ctx
            .get(path)
            .auth(&user)
            .query(&[("limit", "1000"), ("skip", "-5")])
            .send()
            .await;
        many.expect(200);
        assert!(
            many.json().as_array().unwrap().len() <= 100,
            "{path} limit clamps to 100"
        );
        ctx.get(path)
            .auth(&user)
            .query(&[("skip", "99999999")])
            .send()
            .await
            .expect(200);
        ctx.get(path)
            .auth(&user)
            .query(&[("limit", "x")])
            .send()
            .await
            .expect_validation(&["query", "limit"], Some("int_parsing"));
    }
}

#[tokio::test]
async fn course_detail_lists_only_approved_posts_without_owner_or_status() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let approved = ctx.seed_post(&course, &user, "APPROVED").await;
    ctx.seed_post(&course, &user, "PENDING").await;
    ctx.seed_post(&course, &user, "REJECTED").await;

    let resp = ctx
        .get(&format!("/courses/{}", course.id))
        .auth(&user)
        .send()
        .await;
    resp.expect(200);
    let body = resp.json();
    assert_keys(&body, &["course", "posts"]);
    assert_keys(&body["course"], COURSE_KEYS);
    assert_eq!(body["course"]["id"], json!(course.id));
    assert_ts_offset(&body["course"]["create_time"]);
    assert_eq!(ids(&body["posts"]), vec![approved.id.clone()]);
    let expected: Vec<&str> = POST_ROW_KEYS
        .iter()
        .copied()
        .filter(|k| *k != "status" && *k != "owner_id")
        .collect();
    assert_keys(&body["posts"][0], &expected);
}

#[tokio::test]
async fn course_detail_visibility() {
    let ctx = Ctx::new().await;
    let outsider = ctx.user().await;
    let member = ctx.user().await;
    let private = ctx.department(false).await;
    ctx.member(&member, &private).await;
    let course = ctx.course(&private).await;

    let path = format!("/courses/{}", course.id);
    ctx.get(&path)
        .auth(&outsider)
        .send()
        .await
        .expect_detail(404, "Department not found");
    ctx.get(&path).auth(&member).send().await.expect(200);
    ctx.get("/courses/no-such-course")
        .auth(&member)
        .send()
        .await
        .expect_detail(404, "Course not found");
}

#[tokio::test]
async fn create_course_as_department_admin() {
    let ctx = Ctx::new().await;
    let admin = ctx.user().await;
    let dept = ctx.department(false).await;
    ctx.admin_of(&admin, &dept).await;

    // Warm caches first so the test also proves invalidation.
    assert_eq!(
        ctx.get(&format!("/departments/{}/courses", dept.id))
            .auth(&admin)
            .send()
            .await
            .json(),
        json!([])
    );
    let before = ctx.get_all_pages("/courses", &admin).await.len();

    let resp = ctx
        .post(&format!("/courses/{}", dept.id))
        .auth(&admin)
        .form(&[("name", "民法總則"), ("category", "必修")])
        .send()
        .await;
    resp.expect(200);
    let body = resp.json();
    assert_keys(&body, COURSE_KEYS);
    assert_eq!(body["name"], json!("民法總則"));
    assert_eq!(body["category"], json!("必修"));
    assert_eq!(body["department_id"], json!(dept.id));
    assert_ts_offset(&body["create_time"]);
    let id = body["id"].as_str().unwrap().to_string();
    assert!(uuid::Uuid::parse_str(&id).is_ok());

    let dept_courses = ctx
        .get(&format!("/departments/{}/courses", dept.id))
        .auth(&admin)
        .send()
        .await
        .json();
    assert_eq!(ids(&dept_courses), vec![id.clone()]);
    assert_eq!(dept_courses[0]["has_posts"], json!(false));
    let after = ctx.get_all_pages("/courses", &admin).await;
    assert!(after.len() > before && after.iter().any(|c| c["id"] == json!(id)));
}

#[tokio::test]
async fn create_course_errors() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let admin = ctx.user().await;
    let dept = ctx.department(true).await;
    ctx.member(&user, &dept).await;
    ctx.admin_of(&admin, &dept).await;
    let path = format!("/courses/{}", dept.id);
    ctx.post(&path)
        .auth(&user)
        .form(&[("name", "x"), ("category", "y")])
        .send()
        .await
        .expect_detail(403, "Forbidden");
    ctx.post(&path)
        .auth(&admin)
        .form(&[("name", "x")])
        .send()
        .await
        .expect_validation(&["body", "category"], Some("missing"));
}

#[tokio::test]
async fn bulletin_list_and_detail_visibility() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let public = ctx.department(true).await;
    let hidden = ctx.department(false).await;
    let b_public = ctx.bulletin(&public).await;
    let b_hidden = ctx.bulletin(&hidden).await;

    let all = ctx.get_all_pages("/bulletins", &user).await;
    assert!(all.iter().any(|b| b["id"] == json!(b_public)));
    assert!(!all.iter().any(|b| b["id"] == json!(b_hidden)));
    let item = all.iter().find(|b| b["id"] == json!(b_public)).unwrap();
    assert_keys(item, BULLETIN_KEYS);
    assert_ts_offset(&item["create_time"]);

    let resp = ctx
        .get(&format!("/bulletins/{b_public}"))
        .auth(&user)
        .send()
        .await;
    resp.expect(200);
    assert_keys(&resp.json(), BULLETIN_KEYS);
    ctx.get(&format!("/bulletins/{b_hidden}"))
        .auth(&user)
        .send()
        .await
        .expect_detail(404, "Department not found");
    ctx.get("/bulletins/no-such-bulletin")
        .auth(&user)
        .send()
        .await
        .expect_detail(404, "Bulletin not found");
}

#[tokio::test]
async fn create_bulletin_as_department_admin() {
    let ctx = Ctx::new().await;
    let admin = ctx.user().await;
    let member = ctx.user().await;
    let dept = ctx.department(false).await;
    ctx.admin_of(&admin, &dept).await;
    ctx.member(&member, &dept).await;
    let dept_path = format!("/departments/{}/bulletins", dept.id);
    assert_eq!(
        ctx.get(&dept_path).auth(&member).send().await.json(),
        json!([])
    );
    assert!(!ctx
        .get_all_pages("/bulletins", &member)
        .await
        .iter()
        .any(|b| b["department_id"] == json!(dept.id)));

    let resp = ctx
        .post(&format!("/bulletins/{}", dept.id))
        .auth(&admin)
        .form(&[("title", "期中考公告"), ("content", "第三週")])
        .send()
        .await;
    resp.expect(200);
    let body = resp.json();
    assert_keys(&body, BULLETIN_KEYS);
    assert_eq!(body["title"], json!("期中考公告"));
    assert_eq!(body["content"], json!("第三週"));
    assert_eq!(body["department_id"], json!(dept.id));
    let id = body["id"].as_str().unwrap().to_string();

    assert_eq!(
        ids(&ctx.get(&dept_path).auth(&member).send().await.json()),
        vec![id.clone()]
    );
    assert!(ctx
        .get_all_pages("/bulletins", &member)
        .await
        .iter()
        .any(|b| b["id"] == json!(id)));

    ctx.post(&format!("/bulletins/{}", dept.id))
        .auth(&member)
        .form(&[("title", "x"), ("content", "y")])
        .send()
        .await
        .expect_detail(403, "Forbidden");
    ctx.post(&format!("/bulletins/{}", dept.id))
        .auth(&admin)
        .form(&[("title", "x")])
        .send()
        .await
        .expect_validation(&["body", "content"], Some("missing"));
}

#[tokio::test]
async fn super_user_can_create_a_course_for_an_unknown_department() {
    // P-12: the department is not checked for existence.
    let ctx = Ctx::new().await;
    let root = ctx.super_user().await;
    let dept_id = uid("missing-dept");
    let resp = ctx
        .post(&format!("/courses/{dept_id}"))
        .auth(&root)
        .form(&[("name", "x"), ("category", "y")])
        .send()
        .await;
    resp.expect(200);
    assert_eq!(resp.json()["department_id"], json!(dept_id));

    let resp = ctx
        .post(&format!("/bulletins/{dept_id}"))
        .auth(&root)
        .form(&[("title", "x"), ("content", "y")])
        .send()
        .await;
    resp.expect(200);
    assert_eq!(resp.json()["department_id"], json!(dept_id));
}
