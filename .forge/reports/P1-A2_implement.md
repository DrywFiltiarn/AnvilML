# Implementation Report: P1-A2

| Field          | Value                                      |
|----------------|---------------------------------------------|
| Task ID        | P1-A2                                       |
| Phase          | 001 — Workspace Scaffold                    |
| Description    | anvilml — Linux CI jobs (rust, python-worker, openapi-diff) |
| Project        | anvilml                                     |
| Implemented at | 2026-05-29T14:33:26Z                        |
| Attempt        | 1                                           |

## Summary

Created `.github/workflows/ci.yml` with three Linux CI jobs as specified in the approved plan:

1. **`rust`** — Quality gate running `cargo fmt --all --check`, `cargo clippy --workspace --features mock-hardware -- -D warnings`, and `cargo test --workspace --features mock-hardware`. Each step is a separate `run:` block for clear failure attribution.
2. **`python-worker`** — Depends on `rust` passing. Installs Python dependencies from `worker/requirements/base.txt` and runs pytest with `ANVILML_WORKER_MOCK=1` environment variable.
3. **`openapi-diff`** — Depends on `rust` passing. Regenerates `backend/openapi.json` via `cargo run -p anvilml-openapi` and verifies it matches the committed version using `git diff --exit-code`.

Cargo registry (`~/.cargo/registry`) and `target/` directory are cached with a key based on `Cargo.lock` hash in both the `rust` and `openapi-diff` jobs.

## Files Changed

| Action   | Path                              | Description                                            |
|----------|-----------------------------------|--------------------------------------------------------|
| CREATE   | `.github/workflows/ci.yml`        | GitHub Actions CI workflow with 3 Linux jobs           |

## Test Results

### `cargo fmt --all --check`
```
# Exit code 0 — no formatting changes needed
```

### `cargo clippy --workspace --features mock-hardware -- -D warnings`
```
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml-core v0.1.0 (/home/dryw/AnvilML/crates/anvilml-core)
    Checking anvilml-ipc v0.1.0 (/home/dryw/AnvilML/crates/anvilml-ipc)
    Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.30s
```

### `cargo test --workspace --features mock-hardware`
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-e21e4f507e95ce34)

running 1 test
test tests::it_works ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-e148295248438f7b)

running 1 test
test tests::it_works ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-b516fe4a58f81a30)

running 1 test
test tests::it_works ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/main.rs (target/debug/deps/anvilml_openapi-7d4156d460ab7961)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-a3e873874b6b89de)

running 1 test
test tests::it_works ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-c5ab94446f3e4109)

running 1 test
test tests::it_works ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/anvilml_server-2e16d22d2c454f2d)

running 1 test
test tests::it_works ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-eaa1d41160c21ba5)

running 1 test
test tests::it_works ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/main.rs (target/debug/deps/sindristudio-24eac98154dbcb8b)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_core

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_hardware

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_ipc

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_registry

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_scheduler

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_worker

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

All 7 unit tests passed across 8 crates with 0 failures.

## CI Changes

Created `.github/workflows/ci.yml` — the first CI workflow file for the repository. Defines three jobs:

1. **`rust`** — Quality gate with `cargo fmt --all --check`, `cargo clippy --workspace --features mock-hardware -- -D warnings`, and `cargo test --workspace --features mock-hardware`. Caches Cargo registry and target directory.
2. **`python-worker`** — Depends on `rust`. Installs deps from `worker/requirements/base.txt` and runs pytest with `ANVILML_WORKER_MOCK=1`.
3. **`openapi-diff`** — Depends on `rust`. Regenerates `backend/openapi.json` via `cargo run -p anvilml-openapi` and diffs against committed version.

## Commit Log

```
A  .forge/reports/P1-A2_plan.md
M  .forge/state/CURRENT_TASK.md
M  .forge/state/state.json
A  .github/workflows/ci.yml
```

## Acceptance Criteria — Verification

| Criterion                 | Status | Evidence                        |
|---------------------------|--------|---------------------------------|
| `.github/workflows/ci.yml` exists and is valid YAML | PASS | File created at `.github/workflows/ci.yml`, 67 lines, valid YAML structure with three jobs |
| Job `rust` runs on `ubuntu-latest` with 3 separate steps: fmt check, clippy lint, cargo test | PASS | Lines 10-31 define `rust` job with 5 steps (checkout, cache, fmt, clippy, test), each as separate `run:` blocks |
| All Rust jobs use `--features mock-hardware` | PASS | Clippy step: `cargo clippy --workspace --features mock-hardware -- -D warnings`; Test step: `cargo test --workspace --features mock-hardware` |
| Job `python-worker` runs on `ubuntu-latest`, declares `needs: [rust]`, installs deps from `worker/requirements/base.txt`, and runs pytest with `ANVILML_WORKER_MOCK=1` | PASS | Lines 33-46 define `python-worker` job with `needs: [rust]`, pip install, and pytest step with env var |
| Job `openapi-diff` runs on `ubuntu-latest`, declares `needs: [rust]`, runs `cargo run -p anvilml-openapi`, then `git diff --exit-code backend/openapi.json` | PASS | Lines 48-67 define `openapi-diff` job with `needs: [rust]`, cache, cargo run, and git diff steps |
| Cargo registry (`~/.cargo/registry`) and `target/` directory are cached with a key based on `Cargo.lock` hash | PASS | Both `rust` and `openapi-diff` jobs use `actions/cache@v4` with path including `~/.cargo/registry` and `target/`, keyed on `hashFiles('Cargo.lock')` |
| No other CI jobs (e.g. `rust-windows`) are included — those belong to P1-A3 | PASS | Only three jobs defined: `rust`, `python-worker`, `openapi-diff` |
