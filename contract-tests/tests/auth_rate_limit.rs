//! Redis fixed-window auth rate limit (03-auth.md §3.6).
//!
//! The contract environment trusts X-Forwarded-For from private/loopback peers
//! and uses AUTH_RATE_LIMIT_REQUESTS=5, AUTH_RATE_LIMIT_GLOBAL_REQUESTS=40,
//! AUTH_RATE_LIMIT_WINDOW_SECONDS=60. Each Ctx presents its own client IP.

use past_exam_contract_tests::*;

fn subject_limit() -> u32 {
    config().rate_limit_subject
}

fn global_limit() -> u32 {
    config().rate_limit_global
}

async fn failed_login(ctx: &Ctx, username: &str) -> Resp {
    ctx.post("/login")
        .form(&[("username", username), ("password", "wrong")])
        .send()
        .await
}

#[track_caller]
fn assert_limited(resp: &Resp) {
    resp.expect_detail(429, "Too many authentication attempts");
    let retry: u64 = resp
        .header("retry-after")
        .expect("Retry-After")
        .parse()
        .unwrap();
    assert!(
        (1..=config().rate_limit_window).contains(&retry),
        "Retry-After {retry}"
    );
}

#[tokio::test]
async fn per_account_limit_applies_to_successful_and_failed_logins() {
    ensure_rate_window_headroom(20).await;
    let ctx = Ctx::new().await;
    let user = ctx.user().await;
    for _ in 0..subject_limit() {
        ctx.post("/login")
            .form(&[("username", &user.username), ("password", "correct horse")])
            .send()
            .await
            .expect(200);
    }
    assert_limited(
        &ctx.post("/login")
            .form(&[("username", &user.username), ("password", "correct horse")])
            .send()
            .await,
    );
}

#[tokio::test]
async fn per_account_limit_is_scoped_by_username() {
    ensure_rate_window_headroom(20).await;
    let ctx = Ctx::new().await;
    let a = uid("rl-a");
    let b = uid("rl-b");
    for _ in 0..subject_limit() {
        failed_login(&ctx, &a).await.expect(400);
    }
    failed_login(&ctx, &b).await.expect(400);
    assert_limited(&failed_login(&ctx, &a).await);
}

#[tokio::test]
async fn username_subject_is_trimmed_and_casefolded() {
    ensure_rate_window_headroom(20).await;
    let ctx = Ctx::new().await;
    let name = uid("rl-case");
    for i in 0..subject_limit() {
        let variant = if i % 2 == 0 {
            format!("  {name}  ")
        } else {
            name.to_uppercase()
        };
        failed_login(&ctx, &variant).await.expect(400);
    }
    assert_limited(&failed_login(&ctx, &name).await);
}

#[tokio::test]
async fn per_account_limit_is_scoped_by_client_ip() {
    ensure_rate_window_headroom(20).await;
    let first = Ctx::new().await;
    let second = Ctx::new().await;
    let name = uid("rl-ip");
    for _ in 0..subject_limit() {
        failed_login(&first, &name).await.expect(400);
    }
    assert_limited(&failed_login(&first, &name).await);
    failed_login(&second, &name).await.expect(400);
}

#[tokio::test]
async fn global_limit_per_ip_across_accounts() {
    ensure_rate_window_headroom(30).await;
    let ctx = Ctx::new().await;
    for _ in 0..global_limit() {
        failed_login(&ctx, &uid("rl-g")).await.expect(400);
    }
    assert_limited(&failed_login(&ctx, &uid("rl-g")).await);
}

#[tokio::test]
async fn exchange_shares_the_global_limit() {
    ensure_rate_window_headroom(30).await;
    let ctx = Ctx::new().await;
    for _ in 0..global_limit() {
        failed_login(&ctx, &uid("rl-x")).await.expect(400);
    }
    // Limited before any call to Google is attempted.
    assert_limited(
        &ctx.post("/exchange")
            .form(&[("code", "c"), ("redirect_uri", "r")])
            .send()
            .await,
    );
}

#[tokio::test]
async fn requests_with_missing_fields_still_count() {
    ensure_rate_window_headroom(30).await;
    let ctx = Ctx::new().await;
    for _ in 0..global_limit() {
        ctx.post("/exchange")
            .form(&[("code", "c")])
            .send()
            .await
            .expect(422);
    }
    assert_limited(&ctx.post("/exchange").form(&[("code", "c")]).send().await);
}

#[tokio::test]
async fn rejected_origins_do_not_consume_the_limit() {
    ensure_rate_window_headroom(20).await;
    let ctx = Ctx::new().await;
    let name = uid("rl-origin");
    for _ in 0..(subject_limit() + 3) {
        ctx.post("/login")
            .origin("https://evil.example")
            .form(&[("username", &name), ("password", "x")])
            .send()
            .await
            .expect(403);
    }
    for _ in 0..subject_limit() {
        failed_login(&ctx, &name).await.expect(400);
    }
    assert_limited(&failed_login(&ctx, &name).await);
}

#[tokio::test]
async fn forwarded_chain_uses_first_untrusted_hop_from_the_right() {
    ensure_rate_window_headroom(20).await;
    let ctx = Ctx::new().await;
    let client = random_client_ip();
    let name = uid("rl-xff");
    // 10.0.0.0/8 is trusted in the contract environment; 6.6.6.6 is a spoofed
    // left-most value that must be ignored.
    for _ in 0..subject_limit() {
        ctx.post("/login")
            .forwarded_for(format!("6.6.6.6, {client}, 10.1.2.3"))
            .form(&[("username", &name), ("password", "x")])
            .send()
            .await
            .expect(400);
    }
    let other = Ctx::new().await.with_ip(&client);
    assert_limited(&failed_login(&other, &name).await);
}

#[tokio::test]
async fn redis_counters_use_the_shared_key_format() {
    ensure_rate_window_headroom(20).await;
    let ctx = Ctx::new().await;
    let name = uid("rl-key");
    failed_login(&ctx, &name).await.expect(400);
    failed_login(&ctx, &name).await.expect(400);

    let global = rate_limit_key(&format!("global:{}", ctx.ip));
    let subject = rate_limit_key(&format!("subject:{}:login:{}", ctx.ip, name.to_lowercase()));
    assert_eq!(ctx.redis_get(&global).await.as_deref(), Some("2"));
    assert_eq!(ctx.redis_get(&subject).await.as_deref(), Some("2"));
    let ttl = ctx.redis_ttl(&global).await;
    assert!(
        ttl >= 1 && ttl as u64 <= config().rate_limit_window,
        "ttl {ttl}"
    );
}

#[tokio::test]
async fn other_auth_endpoints_are_not_rate_limited() {
    ensure_rate_window_headroom(20).await;
    let ctx = Ctx::new().await;
    for _ in 0..(global_limit() + 5) {
        ctx.post("/refresh").send().await.expect(401);
    }
    failed_login(&ctx, &uid("rl-other")).await.expect(400);
}
