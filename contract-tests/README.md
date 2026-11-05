# Contract tests

Black-box HTTP contract suite for the NTPU Past Exam backend. It runs against
any implementation — today the Python reference on `main`, later the Rust
rewrite — and is the acceptance gate described in
`docs/rust-rewrite/08-testing.md`.

## Run against the Python reference

```bash
docker compose -f contract-tests/docker-compose.yml up -d --build --wait
cd contract-tests && cargo test
```

The stack is isolated (project `pe-contract`, ports 55432/56379/59000/58080,
tmpfs storage) and does not touch the regular development stack.

## Run against another implementation

Start the implementation with `contract.env` (it reuses the stack's
PostgreSQL/Redis/MinIO), then:

```bash
CONTRACT_BASE_URL=http://127.0.0.1:58081 CONTRACT_TARGET=rust cargo test
```

`CONTRACT_TARGET=rust` additionally enables the tests for the deliberate fixes
listed in `docs/rust-rewrite/07-deviations.md` (they fail on the reference by
design).

| Variable | Default |
| --- | --- |
| `CONTRACT_BASE_URL` | `http://127.0.0.1:58080` |
| `CONTRACT_DATABASE_URL` | `postgres://contract:contract@127.0.0.1:55432/past_exam_contract` |
| `CONTRACT_REDIS_URL` | `redis://127.0.0.1:56379` |
| `CONTRACT_HASH_KEY` | `contract-test-secret-key` |
| `CONTRACT_FILE_BASE` | `http://127.0.0.1:59000/past-exam-contract` |
| `CONTRACT_ALLOWED_ORIGIN` | `http://contract.test` |
| `CONTRACT_COMMIT_SHA` | `contract-sha` |
| `CONTRACT_RATE_LIMIT_REQUESTS` / `_GLOBAL_REQUESTS` / `_WINDOW_SECONDS` | `5` / `40` / `60` |
| `CONTRACT_TARGET` | `reference` |

## Layout

| Path | Purpose |
| --- | --- |
| `src/lib.rs` | Harness: config, seeding helpers, HTTP wrapper, JWT helpers, assertions |
| `tests/*.rs` | One test binary per area |
| `vectors/*.json` | Unit-test vectors produced by the reference implementation |
| `fixtures/` | LMS HTML and Google JSON samples used by `vectors/lms.json` / `vectors/google.json` |
| `reference/create_schema.py` | Builds the schema the way production PostgreSQL was created |
| `reference/generate_vectors.py` | Regenerates `vectors/` from the reference code |
| `reference/export_routes.py` / `routes.json` | Route inventory of the reference app |
| `scripts/check_route_coverage.py` | Fails when a reference route has no test call site |

## Maintenance

```bash
# regenerate vectors after changing fixtures or the reference implementation
docker compose -f contract-tests/docker-compose.yml run --rm --no-deps --user "$(id -u)" \
  -v "$PWD/contract-tests:/contract" schema python /contract/reference/generate_vectors.py /contract

# refresh the route inventory and check coverage
docker compose -f contract-tests/docker-compose.yml run --rm --no-deps --user "$(id -u)" \
  -v "$PWD/contract-tests:/contract" schema python /contract/reference/export_routes.py /contract/reference/routes.json
python3 contract-tests/scripts/check_route_coverage.py

# tear down
docker compose -f contract-tests/docker-compose.yml down -v
```
