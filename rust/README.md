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
cargo test                        # vector-driven unit tests (14)
cd ../contract-tests && cargo test # full black-box contract suite (160)
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

`schema_check=strict` (default) refuses to start unless `alembic_version`
equals the schema head the code was written for; Alembic still owns the schema
until the Python implementation is retired.

## Known gaps

Failure-path tests for SES, S3 and Redis outages (documented as L3 items in
`docs/rust-rewrite/08-testing.md`) are not implemented yet; the happy paths and
the Redis-shared session/rate-limit semantics are covered by the contract
suite.
