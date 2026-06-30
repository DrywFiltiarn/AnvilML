# Implementation Report: P900-A1

| Field         | Value                                                         |
|---------------|---------------------------------------------------------------|
| Task ID       | P900-A1                                                       |
| Phase         | 900 — Spec-Drift & Logging Retrofit                           |
| Description   | backend: wire tracing-subscriber, ANVILML_LOG/RUST_LOG never read |
| Implemented   | 2026-06-30T12:00:00Z                                          |
| Status        | COMPLETE                                                      |

## Summary

Registered a real `tracing-subscriber` in the `anvilml` binary so that all existing `tracing::info!`/`debug!` calls produce visible output. The subscriber reads its filter from `ANVILML_LOG` (primary) or `RUST_LOG` (fallback), defaulting to `"info"`. Output is written to stderr so it does not mix with `hw-probe` JSON on stdout. Two integration tests verify that setting either env var to `"debug"` yields non-empty stderr from the spawned binary.

## Resolved Dependencies

| Type   | Name              | Version resolved | Source          |
|--------|-------------------|-----------------|-----------------|
| crate  | tracing-subscriber| 0.3.23          | rust-docs MCP   |
| crate  | serial_test       | 3.5.0           | rust-docs MCP   |

`tracing-subscriber 0.3.23` — `EnvFilter::try_from_env()`, `EnvFilter::new()`, `tracing_subscriber::fmt()`, `.with_env_filter()`, `.with_writer()`, and `.init()` all confirmed present in version 0.3.23 via MCP lookup. The `env-filter` feature enables `EnvFilter::try_from_env()` and `EnvFilter::new()`.

`serial_test 3.5.0` — matches the version already used by `anvilml-core` and `anvilml-hardware` dev-dependencies.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `backend/Cargo.toml` | Added `tracing-subscriber` dependency with `env-filter` feature; added `serial_test` dev-dependency; bumped patch version 0.1.4 → 0.1.5 |
| MODIFY | `backend/src/main.rs` | Added `use tracing_subscriber::EnvFilter;` import; added tracing subscriber init as first statement in `main()` before CLI parsing, with `.with_writer(std::io::stderr)` to keep logs separate from stdout data |
| CREATE | `backend/tests/logging_tests.rs` | Two integration tests: `test_anvilml_log_debug_yields_stderr` and `test_rust_log_debug_yields_stderr`, both `#[serial]` with capture-and-restore env var isolation |
| MODIFY | `docs/TESTS.md` | Added test catalogue entries for both new integration tests |

## Commit Log

```
 .forge/reports/P900-A1_plan.md | 120 ++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md   |   6 +-
 .forge/state/state.json        |  13 +++--
 Cargo.lock                     | 108 +++++++++++++++++++++++++++++++++-
 backend/Cargo.toml             |   6 +-
 backend/src/main.rs            |  17 ++++++
 backend/tests/logging_tests.rs | 128 +++++++++++++++++++++++++++++++++++++++++
 docs/TESTS.md                  |  24 ++++++++
 8 files changed, 411 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/logging_tests.rs (target/debug/deps/logging_tests-473043148970f23e)

running 2 tests
test tests::test_anvilml_log_debug_yields_stderr ... ok
test tests::test_rust_log_debug_yields_stderr ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

Full workspace test suite: 136 tests passed, 0 failed across all crates.

## Format Gate

```
cargo fmt --all -- --check
```
(No output — all files already formatted.)

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.52s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.16s

# 3. Real-hardware Linux
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.44s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.47s
```

All four checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p anvilml --features mock-hardware -- config_reference
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored
```

Gate 1 passes. No config fields were added or modified by this task.

### Gate 2 — OpenAPI Drift
Not triggered — no handler function signatures, `#[utoipa::path]` annotations, or `AppState` fields were modified.

### Gate 3 — Node Parity
Not triggered — no nodes added, removed, or renamed.

### Gate 4 — Mock/Real Parity Markers
Not triggered — no node `execute()` or arch module `load()`/`sample()`/`decode()`/`compute_latent_shape()` functions were added or modified.

## Public API Delta

No new `pub` items introduced. The grep command returned zero results:
```
git diff HEAD -- backend/src/main.rs backend/Cargo.toml backend/tests/logging_tests.rs | grep '^+.*pub ' | head -40
```
(No output — all changes are internal to the binary's entry point and private test files.)

## Deviations from Plan

1. **Added `.with_writer(std::io::stderr)` to the subscriber configuration.** The approved plan specified a bare `.init()` call, which writes to stdout by default. Since `hw-probe` outputs JSON to stdout, tracing output would mix with the JSON data. Redirecting to stderr keeps logs separate from data output, which is the conventional behavior for logging. This is a necessary correction — without it, the tests would fail because stdout was non-empty but stderr was empty.

2. **Used `unsafe` blocks around `std::env::set_var`/`remove_var` calls.** Rust 1.96 (edition 2024) marks `std::env::set_var` and `std::env::remove_var` as `unsafe`. All env var mutations in the test file are wrapped in `unsafe { ... }` blocks with comments explaining that `#[serial]` guarantees no concurrent access.

3. **Added `serial_test` as a dev-dependency.** The approved plan assumed `#[serial]` would be available but did not note that the `backend` crate had no `serial_test` dependency (it was only present in `anvilml-core` and `anvilml-hardware`). Added `serial_test = "3.5.0"` to `backend/Cargo.toml` dev-dependencies.

## Blockers

None.
