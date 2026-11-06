# Project

Rust backend for the NTPU Past Exam service. The API uses Axum/Tokio,
SQLx/PostgreSQL, Redis, `opendal` (R2/S3), `aws-sdk-ses`, and `scraper`.

## Commands

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets
cargo run -- migrate
cargo run -- serve
```

## Layout

- `src/` — HTTP routes, auth, cache, storage, mail, and database code
- `migrations/` — SQLx migrations; this is the schema source of truth
- `assets/` — notification email templates
- `testdata/` — regression vectors and captured integration fixtures
- `docker-compose.yml` — one-shot `migrate` service followed by the API

## Migrations

Add a new numbered SQL file under `migrations/`, then run:

```bash
cargo run -- migrate
```

`serve` refuses to start when `_sqlx_migrations` is missing or behind the
binary. Migrations must be expand/contract compatible: add nullable columns,
backfill, then tighten or remove in a later release. Never edit an applied
migration.
