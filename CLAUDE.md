# Project

Rust backend for the NTPU Past Exam service. The API is built with Axum/Tokio,
SQLx/PostgreSQL, Redis, `opendal` (R2/S3), `aws-sdk-ses`, and `scraper`.

## Commands

```bash
# from rust/
cargo fmt --check
cargo test
cargo clippy --all-targets

# from the repository root
docker compose -f contract-tests/docker-compose.yml up -d --build --wait
cd contract-tests && cargo test
```

## Layout

- `rust/src/` — HTTP routes, auth, cache, storage, mail, and database code
- `rust/migrations/` — SQLx migrations; this is the schema source of truth
- `contract-tests/` — black-box HTTP contract tests and regression vectors
- `docker-compose.yml` — one-shot `migrate` service followed by the API

## Migrations

Add a new SQL file under `rust/migrations/` (next number in sequence), then run:

```bash
cd rust
cargo run -- migrate
```

`serve` refuses to start when `_sqlx_migrations` is missing or behind the
binary. Migrations must be expand/contract compatible: add nullable columns,
backfill, then tighten or remove in a later release. Never edit an applied
migration.

## Contract tests

The suite creates its own rows directly in PostgreSQL and talks to the service
over HTTP. Start the isolated stack and the service as described in
`contract-tests/README.md`, then run `cargo test` from `contract-tests/`.
