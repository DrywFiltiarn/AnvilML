# Implementation Report: P9-B1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P9-B1                                              |
| Phase       | 009 — Worker Spawn & Handshake                     |
| Description | ci: add Python venv setup to rust-linux and rust-windows jobs for worker subprocess tests |
| Implemented | 2026-06-06T15:45:00Z                               |
| Status      | COMPLETE                                           |

## Summary

Added a "Setup Python for worker tests" step in both the `rust-linux` and `rust-windows` CI jobs, placed immediately before the existing "Run tests" step. Each setup step creates a `.ci-venv` virtual environment and installs `msgpack` and `pillow`. Updated the "Run tests" step in both jobs to include `ANVILML_VENV_PATH: .ci-venv` and `ANVILML_WORKER_MOCK: "1"` as step-level environment variables. All pre-existing CI steps (format check, clippy lint, compile checks) remain unchanged and in the same order. No source code changes were made — only the CI workflow file was modified.

## Resolved Dependencies

No new dependencies added or modified. This task modifies only a CI workflow file.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `.github/workflows/ci.yml` | Add "Setup Python for worker tests" step and `env:` block to both `rust-linux` and `rust-windows` jobs |

## Commit Log

```
 .forge/reports/P9-B1_plan.md | 95 ++++++++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md |  6 +--
 .forge/state/state.json      | 13 +++---
 .github/workflows/ci.yml     | 12 ++++++
 4 files changed, 117 insertions(+), 9 deletions(-)
```

## Test Results

```
running 74 tests (anvilml-core lib) — ok. 74 passed; 0 failed; 0 ignored
running 56 tests (anvilml-hardware lib) — ok. 56 passed; 0 failed; 0 ignored
running 23 tests (anvilml-ipc lib) — ok. 23 passed; 0 failed; 0 ignored
running 0 tests (anvilml-ipc bin) — ok. 0 passed; 0 failed; 0 ignored
running 0 tests (anvilml-openapi bin) — ok. 0 passed; 0 failed; 0 ignored
running 19 tests (anvilml-registry lib) — ok. 19 passed; 0 failed; 0 ignored
running 1 test (anvilml-registry db) — ok. 1 passed; 0 failed; 0 ignored
running 4 tests (anvilml-registry device_store) — ok. 4 passed; 0 failed; 0 ignored
running 2 tests (anvilml-registry rescan) — ok. 2 passed; 0 failed; 0 ignored
running 1 test (anvilml-registry scanner) — ok. 1 passed; 0 failed; 0 ignored
running 7 tests (anvilml-registry seed_loader) — ok. 7 passed; 0 failed; 0 ignored
running 2 tests (anvilml-registry store_get) — ok. 2 passed; 0 failed; 0 ignored
running 3 tests (anvilml-registry store_list) — ok. 3 passed; 0 failed; 0 ignored
running 0 tests (anvilml-scheduler lib) — ok. 0 passed; 0 failed; 0 ignored
running 9 tests (anvilml-server lib) — ok. 9 passed; 0 failed; 0 ignored
running 3 tests (anvilml-server api_models) — ok. 3 passed; 0 failed; 0 ignored
running 1 test (anvilml-server api_ws_events) — ok. 1 passed; 0 failed; 0 ignored
running 10 tests (anvilml-worker lib) — ok. 8 passed; 0 failed; 2 ignored
running 8 tests (anvilml bin) — ok. 8 passed; 0 failed; 0 ignored
running 1 test (backend config_reference) — ok. 1 passed; 0 failed; 0 ignored
Doc-tests anvilml-core: ok. 0 passed
Doc-tests anvilml-hardware: ok. 2 passed; 0 failed; 0 ignored
Doc-tests anvilml-ipc: ok. 0 passed
Doc-tests anvilml-registry: ok. 0 passed
Doc-tests anvilml-scheduler: ok. 0 passed
Doc-tests anvilml-server: ok. 0 passed
Doc-tests anvilml-worker: ok. 0 passed
```

## Format Gate

```
cargo fmt --all -- --check
exit 0 — no formatting drift
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s

# 2. Mock-hardware Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.95s

# 3. Real-hardware Linux check
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.24s

# 4. Real-hardware Windows cross-check
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.95s

All four checks exited 0.
```

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p backend --features mock-hardware -- config_reference
Running tests/config_reference.rs (target/debug/deps/config_reference-a60e61c361bf4108)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out, finished in 0.00s
```

Note: The config_reference test was already compiled and ran successfully in the main test suite (`Running tests/config_reference.rs — ok. 1 passed`). The filtered run above shows 0 because the filter `config_reference` does not match the test name `test_toml_key_set_matches_default`. The gate passes via the full suite run.

## Deviations from Plan

None. Implementation matches the approved plan exactly.

## Blockers

None.
