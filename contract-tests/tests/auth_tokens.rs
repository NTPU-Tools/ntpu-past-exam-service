//! Access-token verification and current-user resolution (03-auth.md §3.1–3.3).

use base64::Engine;
use past_exam_contract_tests::*;
use serde_json::json;

const ROUTER_AUTH_PATH: &str = "/departments"; // router-level auth (mode A)

fn thread_list_path(course: &Course) -> String {
    format!("/threads/{}", course.id) // handler-level auth (mode B)
}

#[tokio::test]
async fn missing_or_malformed_authorization_header() {
    let ctx = Ctx::new().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    for path in [
        ROUTER_AUTH_PATH.to_string(),
        thread_list_path(&course),
        "/verify-token".into(),
    ] {
        ctx.get(&path)
            .send()
            .await
            .expect_detail(401, "Invalid or missing authorization header");
        for header in [
            "Token abc",
            "bearer abc",
            "Bearer",
            "Bearer  abc",
            "Bearer a b",
        ] {
            ctx.get(&path)
                .header("Authorization", header)
                .send()
                .await
                .expect_detail(401, "Invalid or missing authorization header");
        }
        // Browsers/proxies strip trailing OWS from header values, so "Bearer " can
        // arrive as "Bearer" (invalid header) or with the space (invalid token).
        let resp = ctx
            .get(&path)
            .header("Authorization", "Bearer ")
            .send()
            .await;
        resp.expect(401);
        assert!(
            matches!(
                resp.detail().as_str(),
                "Invalid or expired token" | "Invalid or missing authorization header"
            ),
            "unexpected detail: {}",
            resp.detail()
        );
    }
}

#[tokio::test]
async fn invalid_signatures_and_claims_are_rejected() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let id = user.id.as_str();
    let exp = now() + 600;

    let b64 = |v: serde_json::Value| {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(v.to_string())
    };
    let alg_none = format!(
        "{}.{}.",
        b64(json!({"alg": "none", "typ": "JWT"})),
        b64(json!({"sub": id, "id": id, "type": "access", "exp": exp}))
    );

    let cases = [
        (
            "expired",
            sign(&json!({"sub": id, "id": id, "type": "access", "exp": now() - 5})),
        ),
        (
            "wrong key",
            sign_with_key(
                &json!({"sub": id, "id": id, "type": "access", "exp": exp}),
                "other-key",
            ),
        ),
        (
            "missing exp",
            sign(&json!({"sub": id, "id": id, "type": "access"})),
        ),
        (
            "missing sub",
            sign(&json!({"id": id, "type": "access", "exp": exp})),
        ),
        (
            "non-string sub",
            sign(&json!({"sub": 12, "id": id, "type": "access", "exp": exp})),
        ),
        (
            "audience",
            sign(&json!({"sub": id, "id": id, "type": "access", "exp": exp, "aud": "x"})),
        ),
        (
            "not yet valid",
            sign(&json!({"sub": id, "id": id, "type": "access", "exp": exp, "nbf": now() + 300})),
        ),
        ("alg none", alg_none),
        ("garbage", "not-a-jwt".to_string()),
    ];
    for (name, token) in cases {
        let resp = ctx.get(ROUTER_AUTH_PATH).bearer(&token).send().await;
        assert_eq!(resp.status, 401, "{name}: {}", resp.text());
        assert_eq!(resp.detail(), "Invalid or expired token", "{name}");
    }
}

#[tokio::test]
async fn token_type_must_be_access() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let exp = now() + 600;
    for claims in [
        json!({"sub": user.id, "id": user.id, "exp": exp}),
        json!({"sub": user.id, "id": user.id, "type": "refresh", "exp": exp}),
    ] {
        ctx.get(ROUTER_AUTH_PATH)
            .bearer(&sign(&claims))
            .send()
            .await
            .expect_detail(401, "Invalid token type");
    }
}

#[tokio::test]
async fn deleted_user_is_rejected() {
    let ctx = Ctx::new().await;
    let token = access_token("no-such-user");
    ctx.get(ROUTER_AUTH_PATH)
        .bearer(&token)
        .send()
        .await
        .expect_detail(401, "Invalid credentials");
    ctx.get("/verify-token")
        .bearer(&token)
        .send()
        .await
        .expect_detail(401, "Invalid credentials");
}

#[tokio::test]
async fn early_tokens_resolve_by_sub_as_id_or_username() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let exp = now() + 600;
    for sub in [&user.id, &user.username] {
        ctx.get(ROUTER_AUTH_PATH)
            .bearer(&sign(&json!({"sub": sub, "type": "access", "exp": exp})))
            .send()
            .await
            .expect(200);
    }
}

#[tokio::test]
async fn unknown_id_claim_falls_back_to_sub() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let token = sign(
        &json!({"sub": user.username, "id": "no-such-id", "type": "access", "exp": now() + 600}),
    );
    ctx.get(ROUTER_AUTH_PATH)
        .bearer(&token)
        .send()
        .await
        .expect(200);
    let token = sign(
        &json!({"sub": "nobody-at-all", "id": "no-such-id", "type": "access", "exp": now() + 600}),
    );
    ctx.get(ROUTER_AUTH_PATH)
        .bearer(&token)
        .send()
        .await
        .expect_detail(401, "Invalid credentials");
}

#[tokio::test]
async fn thread_endpoints_require_the_id_claim() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(true).await;
    let course = ctx.course(&dept).await;
    let token = sign(&json!({"sub": user.id, "type": "access", "exp": now() + 600}));
    ctx.get(&thread_list_path(&course))
        .bearer(&token)
        .send()
        .await
        .expect_detail(401, "Invalid token: missing user id");

    ctx.get(&thread_list_path(&course))
        .bearer(&access_token("no-such-user"))
        .send()
        .await
        .expect_detail(401, "User not found");
}

#[tokio::test]
async fn forged_privilege_claims_are_ignored() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(false).await;
    let forged = sign(&json!({
        "sub": user.id, "id": user.id, "type": "access", "exp": now() + 600,
        "isu": true, "adm": format!("[\"{}\"]", dept.id),
    }));

    ctx.get("/users")
        .bearer(&forged)
        .send()
        .await
        .expect_detail(403, "Forbidden");
    ctx.get(&format!("/departments/{}/join-request", dept.id))
        .bearer(&forged)
        .send()
        .await
        .expect_detail(403, "Forbidden");
    ctx.get(&format!("/departments/{}/courses", dept.id))
        .bearer(&forged)
        .send()
        .await
        .expect_detail(404, "Department not found");
}

#[tokio::test]
async fn live_super_user_flag_is_used_without_claims() {
    let ctx = Ctx::new().await;
    let admin = ctx.super_user().await;
    let token = sign(&json!({
        "sub": admin.id, "id": admin.id, "type": "access", "exp": now() + 600,
        "isu": false, "adm": "[]",
    }));
    ctx.get("/users").bearer(&token).send().await.expect(200);
}

#[tokio::test]
async fn revoked_privileges_take_effect_immediately() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(false).await;
    let membership = ctx.admin_of(&user, &dept).await;
    let path = format!("/departments/{}/members", dept.id);

    ctx.get(&path).auth(&user).send().await.expect(200);
    ctx.db
        .execute(
            "UPDATE users_departments SET is_department_admin = false WHERE id = $1",
            &[&membership],
        )
        .await
        .unwrap();
    ctx.get(&path)
        .auth(&user)
        .send()
        .await
        .expect_detail(403, "Forbidden");
}

#[tokio::test]
async fn pending_admin_membership_grants_nothing() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let dept = ctx.department(false).await;
    ctx.membership(&user, &dept, "PENDING", true).await;
    ctx.get(&format!("/departments/{}/members", dept.id))
        .auth(&user)
        .send()
        .await
        .expect_detail(403, "Forbidden");
}

#[tokio::test]
async fn super_user_passes_admin_guard_for_any_department() {
    let ctx = Ctx::new().await;
    let admin = ctx.super_user().await;
    let dept = ctx.department(false).await;
    ctx.get(&format!("/departments/{}/members", dept.id))
        .auth(&admin)
        .send()
        .await
        .expect(200);
}

#[tokio::test]
async fn authentication_precedes_validation_on_router_auth_endpoints() {
    let ctx = Ctx::new().await;
    // Invalid query and no credentials: 401 wins on router-level auth routes.
    ctx.get("/users")
        .query(&[("limit", "0")])
        .send()
        .await
        .expect(401);
    // Admin guard (403) precedes form validation (422).
    let user = ctx.user().await;
    let dept = ctx.department(true).await;
    ctx.post(&format!("/courses/{}", dept.id))
        .auth(&user)
        .send()
        .await
        .expect_detail(403, "Forbidden");
}

#[tokio::test]
async fn verify_token_reports_live_scopes() {
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    let public = ctx.create_department(true, 1).await;
    let private_member = ctx.create_department(false, 2).await;
    let private_admin = ctx.create_department(false, 3).await;
    let hidden = ctx.department(false).await;
    ctx.member(&user, &private_member).await;
    ctx.admin_of(&user, &private_admin).await;

    let resp = ctx.get("/verify-token").auth(&user).send().await;
    resp.expect(200);
    assert_eq!(resp.header("cache-control").as_deref(), Some("no-store"));
    assert_eq!(resp.header("pragma").as_deref(), Some("no-cache"));
    let body = resp.json();
    assert_keys(&body, &["admin", "visible_departments"]);
    assert_eq!(ids(&body["admin"]), vec![private_admin.id.clone()]);
    let visible = ids(&body["visible_departments"]);
    for d in [&public, &private_member, &private_admin] {
        assert!(visible.contains(&d.id));
    }
    assert!(!visible.contains(&hidden.id));
    // ordered by sort_order
    let pos = |id: &str| visible.iter().position(|v| v == id).unwrap();
    assert!(pos(&public.id) < pos(&private_member.id));
    assert!(pos(&private_member.id) < pos(&private_admin.id));
}

#[tokio::test]
async fn verify_token_for_super_user_lists_everything() {
    let ctx = Ctx::new().await;
    let admin = ctx.super_user().await;
    let hidden = ctx.department(false).await;
    let body = ctx.get("/verify-token").auth(&admin).send().await.json();
    assert!(ids(&body["admin"]).contains(&hidden.id));
    assert!(ids(&body["visible_departments"]).contains(&hidden.id));
}
