# 北大考古題 NTPU Past Exam Backend

Rust backend for the NTPU Past Exam service: Axum + Tokio, SQLx/PostgreSQL,
Redis, Cloudflare R2, and Amazon SES.

## Layout

| Path | Purpose |
| --- | --- |
| `src/` | HTTP routes, auth, cache, storage, mail, and database code |
| `migrations/` | SQLx migrations (schema owner) |
| `assets/` | Notification email templates |
| `testdata/` | Regression vectors and captured integration fixtures |
| `docker-compose.yml` | One-shot migration + API stack |

## Run

```bash
# Provide DATABASE_*, REDIS_*, R2_*, HASH_KEY and the optional AWS/Google
# settings; see .env.example.
cargo run -- serve
```

The full stack can also run through Compose. It joins the external network named
by `BACKEND_NETWORK` (default `backend`):

```bash
docker compose --env-file .env up -d --build
```

Health check: `GET /ping`.

## Test

```bash
cargo test
cargo clippy --all-targets
```

## Migrations

SQLx owns the schema. `cargo run -- migrate` applies the files in `migrations/`;
`serve` refuses to start when `_sqlx_migrations` is missing or behind the binary.
Add a new numbered SQL file for each change; never edit an applied migration.
