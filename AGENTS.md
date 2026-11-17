# AWS Lambda Web Adapter

## Build & Test

- Build: `cargo build`
- Run all tests: `cargo nextest run`
- Run a single test: `cargo nextest run <test_name>`
- Format check: `cargo fmt --all -- --check`
- Lint: `cargo clippy --all-targets -- -Dwarnings` (CI fails on warnings)
- Docs: `cargo doc --no-deps`
- Release/layer builds use `cargo lambda` with musl targets — see `Makefile` and `docs/development.md`

## Project Structure

- `src/lib.rs` — Core adapter logic (`Adapter`, `AdapterOptions`, `Protocol`, `LambdaInvokeMode`). The crate is `publish = false`; the lib target is internal to the binary and tests, with no API-stability contract
- `src/main.rs` — Binary entrypoint
- `src/readiness.rs` — Readiness check (HTTP/TCP) and wait loop with checkpoint progress logging
- `src/snapstart.rs` — SnapStart hooks: notifies the app before checkpoint / after restore and refreshes the HTTP client
- `tests/integ_tests/` — Integration tests
- `tests/e2e_tests/` — End-to-end tests (marked `#[ignore]`; require a deployed endpoint via `API_ENDPOINT`, `API_AUTH_TYPE`, and AWS credentials)
- `benches/` — Criterion benchmarks (run in CI by `benchmark.yaml`)

## Key Conventions

- Environment variables use `AWS_LWA_` prefix. Unprefixed names (`HOST`, `READINESS_CHECK_*`, `REMOVE_BASE_PATH`, `ASYNC_INIT`) are deprecated fallbacks, read via `get_env_with_deprecation`; `PORT` is a supported, non-deprecated fallback for `AWS_LWA_PORT`. Do not add new unprefixed variables
- Tests that mutate env vars rely on nextest's per-test process isolation
- nextest retries failed tests up to 3 times (`.config/nextest.toml`); check the run summary for tests reported as flaky
- Commit messages follow Conventional Commits (`commitlint.config.js`): allowed types are `feat`, `fix`, `docs`, `example`, `examples`, `chore`, `refactor`, `perf`, `test`, `ci`, `revert`; header ≤ 120 chars
