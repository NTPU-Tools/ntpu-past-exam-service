# past-exam-service (Rust)

Drop-in replacement for the FastAPI backend. The contract it implements is
specified in [`../docs/rust-rewrite/`](../docs/rust-rewrite/).

## Stack

Axum 0.8 + Tokio, SQLx (PostgreSQL), Redis (`redis`), `jsonwebtoken` (HS256),
`bcrypt`, `opendal` (R2/S3), `aws-sdk-ses`, `scraper` (LMS pages), `tracing`.
Only three things are implemented by hand because no crate matches the
contract: the FastAPI-compatible form/multipart extractor, the
Starlette-compatible CORS preflight response, and the Redis Lua scripts that
share refresh-session and rate-limit keys with the Python implementation.

## Run

```bash
# from the repository root: start the isolated stack (PostgreSQL, Redis, MinIO)
docker compose -f contract-tests/docker-compose.yml up -d --wait

# run the service against it
cd rust
set -a; . ../contract-tests/contract.env; set +a
cargo run -- serve
```

Health check: `GET /ping`. The binary also supports `past-exam-service
healthcheck` (used by the Docker healthcheck).

## Test

```bash
cargo test                        # unit tests (15)
cd ../contract-tests && cargo test # full black-box contract suite (161)
```

The contract suite asserts the documented behaviour against this implementation
(`CONTRACT_BASE_URL=http://127.0.0.1:58081 CONTRACT_TARGET=rust` when it is
already running).

## Environment

All legacy variables are honoured (see `docs/rust-rewrite/02-runtime.md`).
Additional settings: `PORT`, `BIND_ADDRESS`, `DATABASE_URL`,
`DATABASE_MAX_CONNECTIONS`, `LMS_BASE_URL`, `GOOGLE_TOKEN_URL`,
`GOOGLE_USERINFO_URL`, `AWS_SES_ENDPOINT`, `LOG_TAIL_HOST`, `SCHEMA_CHECK`,
`CACHE_PREFIX`, `CACHE_CLEAR_LEGACY_PREFIX`, `MAX_REQUEST_BODY_BYTES`.

Alembic owns the schema; this service never migrates. `SCHEMA_CHECK=strict`
(default) refuses to start only when `alembic_version` is an *older* revision
listed in `schema_revisions.txt` (or missing). An unknown, newer revision logs a
warning and starts, so an older build can restart after an expand-only
migration. After adding a migration, run
`python scripts/sync_schema_revisions.py`; `cargo test` fails until you do.

Production deploys use `docker-compose.coolify.yml`: a Python `migrate`
service runs `alembic upgrade head` and the API starts only after it exits 0.
See `docs/rust-rewrite/09-deployment.md`.

## Deliberate fixes validated against production

- F-16: notification subjects no longer get the 「測試環境 - 」 prefix when
  `SERVICE_NAME` is `oracle-production-coolify`; override with
  `EMAIL_SUBJECT_PREFIX`.
- F-17: `GET /posts?user_id=X` no longer exposes X's anonymous posts to other
  users.
- F-18: the legacy refresh-token migration path (window closed 2026-11-03) is
  gone; legacy tokens are rejected and logout only clears the cookie.

## Known gaps

Failure-path tests for SES, S3 and Redis outages (documented as L3 items in
`docs/rust-rewrite/08-testing.md`) are not implemented yet; the happy paths and
the Redis-shared session/rate-limit semantics are covered by the contract
suite.
