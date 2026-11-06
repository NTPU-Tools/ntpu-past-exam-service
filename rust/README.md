# past-exam-service (Rust)

Drop-in replacement for the FastAPI backend.

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

The repository-root `docker-compose.yml` runs a one-shot SQLx migration and
then starts the API:

```bash
docker compose --env-file .env up -d --build
```

## Test

```bash
cargo test                        # unit tests (14)
cd ../contract-tests && cargo test # full black-box contract suite (161)
```

The contract suite asserts the documented behaviour against this implementation
(`CONTRACT_BASE_URL=http://127.0.0.1:58081 CONTRACT_TARGET=rust` when it is
already running).

## Environment

All legacy variables are honoured. Additional settings: `PORT`, `BIND_ADDRESS`,
`DATABASE_URL`, `DATABASE_MAX_CONNECTIONS`, `LMS_BASE_URL`, `GOOGLE_TOKEN_URL`,
`GOOGLE_USERINFO_URL`, `AWS_SES_ENDPOINT`, `LOG_TAIL_HOST`, `SCHEMA_CHECK`,
`CACHE_PREFIX`, `CACHE_CLEAR_LEGACY_PREFIX`, `MAX_REQUEST_BODY_BYTES`.

SQLx owns the schema. `past-exam-service migrate` applies `rust/migrations/`;
`serve` refuses to start when `_sqlx_migrations` is missing or behind (unless
`SCHEMA_CHECK=warn` or `off`). Applied versions newer than this build only log a
warning, so an older build can restart after an expand-only migration.

## Behaviour notes

- F-16: notification subjects no longer get the 「測試環境 - 」 prefix when
  `SERVICE_NAME` contains `prod` or `production`; override with
  `EMAIL_SUBJECT_PREFIX`.
- F-17: `GET /posts?user_id=X` no longer exposes X's anonymous posts to other
  users.
- F-18: the legacy refresh-token migration path (window closed 2026-11-03) is
  gone; legacy tokens are rejected and logout only clears the cookie.

## Known gaps

Failure-path tests for SES, S3 and Redis outages are not implemented yet; the
happy paths and the Redis-shared session/rate-limit semantics are covered by the
contract suite.
