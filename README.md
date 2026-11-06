# 北大考古題 NTPU Past Exam Backend

Rust backend for the NTPU Past Exam service: Axum + Tokio, SQLx/PostgreSQL,
Redis, Cloudflare R2, and Amazon SES.

## Layout

| Path | Purpose |
| --- | --- |
| `rust/` | Service implementation |
| `rust/migrations/` | SQLx migrations (schema owner) |
| `contract-tests/` | Black-box HTTP contract suite |
| `docker-compose.yml` | One-shot migration + API stack |

## Run

```bash
# isolated PostgreSQL / Redis / MinIO stack
docker compose -f contract-tests/docker-compose.yml up -d --build --wait

# run the service against it
cd rust
set -a; . ../contract-tests/contract.env; set +a
cargo run -- serve
```

For the full stack, copy `.env.example` to `.env`, fill in the values, then:

```bash
docker compose --env-file .env up -d --build
```

Health check: `GET /ping`.

## Test

```bash
cd rust
cargo test
cargo clippy --all-targets

cd ../contract-tests
cargo test
```

## Configuration

See `.env.example` for the database, Redis, object storage, mail, Google login,
and auth settings. `SERVICE_NAME`, `ORIGIN`, and `AUTH_COOKIE_SECURE` control
environment-specific behaviour.
