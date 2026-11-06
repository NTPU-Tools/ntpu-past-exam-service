# Contract tests

Black-box HTTP contract suite for the NTPU Past Exam backend. It creates its own
rows in PostgreSQL, talks to the service under test over HTTP only, and inspects
Redis / object storage where the contract includes shared state (refresh
sessions, rate limits, uploaded files).

## Run

```bash
# isolated stack: PostgreSQL, Redis, MinIO, and the Rust schema migrator
docker compose -p pe-contract -f contract-tests/docker-compose.yml up -d --build --wait

# start the service under test with the stack's environment
cd rust
set -a; . ../contract-tests/contract.env; set +a
cargo run --release -- serve

# in another shell
cd contract-tests && cargo test
```

The stack is isolated (project `pe-contract`, ports 55432/56379/59000) and does
not touch the regular development stack.

## Configuration

| Variable | Default |
| --- | --- |
| `CONTRACT_BASE_URL` | `http://127.0.0.1:58081` |
| `CONTRACT_DATABASE_URL` | `postgres://contract:contract@127.0.0.1:55432/past_exam_contract` |
| `CONTRACT_REDIS_URL` | `redis://127.0.0.1:56379` |
| `CONTRACT_HASH_KEY` | `contract-test-secret-key` |
| `CONTRACT_FILE_BASE` | `http://127.0.0.1:59000/past-exam-contract` |
| `CONTRACT_ALLOWED_ORIGIN` | `http://contract.test` |
| `CONTRACT_COMMIT_SHA` | `contract-sha` |
| `CONTRACT_RATE_LIMIT_REQUESTS` / `_GLOBAL_REQUESTS` / `_WINDOW_SECONDS` | `5` / `40` / `60` |

## Files

| Path | Purpose |
| --- | --- |
| `tests/*.rs` | HTTP contract tests |
| `src/lib.rs` | Test harness (HTTP, database, Redis, MinIO helpers) |
| `vectors/*.json` | Regression vectors for unit tests in `rust/` |
| `fixtures/` | Captured LMS/Google payloads for integration unit tests |
