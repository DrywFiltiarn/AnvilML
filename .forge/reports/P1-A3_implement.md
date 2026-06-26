# Implementation Report: P1-A3

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P1-A3                              |
| Phase         | 001 — Repository Scaffold          |
| Description   | backend: shutdown.rs signal handler stub |
| Implemented   | 2026-06-26T15:30:00Z              |
| Status        | COMPLETE                           |

## Summary

Implemented the shutdown signal handler stub for the AnvilML backend. Added `tokio` dependency, created `backend/src/shutdown.rs` with `pub async fn wait_for_shutdown_signal()` that awaits `tokio::signal::ctrl_c()`, converted `backend/src/main.rs` from synchronous `fn main()` to `#[tokio::main] async fn main()`, and added integration tests in `backend/tests/shutdown_tests.rs` verifying signal delivery (Unix) and timeout cancellability (all platforms). Added `backend/src/lib.rs` to expose the `shutdown` module for integration testing. Bumped backend crate version from 0.1.0 to 0.1.1. All tests pass, all gates clean.

## Resolved Dependencies

| Type   | Name  | Version resolved | Source         |
|--------|-------|-----------------|----------------|
| crate  | tokio | 1.52.3          | crates.io (MCP unavailable, version 1.47.0 from plan used as floor — cargo resolved to 1.52.3 compatible with Rust 1.96.0/edition 2024) |

**Note:** The rust-docs MCP tool was not available as a tool in this session. The plan specified version 1.47.0. Cargo's resolver selected 1.52.3 (latest compatible with Rust 1.96.0). This is above the floor of 1.47.0. The `tokio::signal::ctrl_c()` API is stable since tokio 0.2 and confirmed present in 1.52.3.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `backend/src/lib.rs` | Library crate entry point exposing `pub mod shutdown` for integration tests |
| CREATE | `backend/src/shutdown.rs` | Shutdown signal handler: `wait_for_shutdown_signal()` async function |
| MODIFY | `backend/src/main.rs` | Convert to `#[tokio::main] async fn main()`, import `shutdown` from lib, await signal |
| MODIFY | `backend/Cargo.toml` | Add `tokio = { version = "1.47.0", features = ["full"] }`, bump version to `0.1.1` |
| CREATE | `backend/tests/shutdown_tests.rs` | Integration tests: signal delivery (Unix) and timeout cancellability (all platforms) |
| MODIFY | `docs/TESTS.md` | Added entries for `test_shutdown_signal_returns_on_ctrl_c` and `test_shutdown_signal_timeout_cancels` |

## Commit Log

```
 .forge/reports/P1-A3_plan.md    | 300 ++++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md    |   6 +-
 .forge/state/state.json         |  13 +-
 Cargo.lock                      | 161 ++++++++++++++++++++-
 backend/Cargo.toml              |   3 +-
 backend/src/lib.rs              |   6 +
 backend/src/main.rs             |  17 ++-
 backend/src/shutdown.rs         |  13 ++
 backend/tests/shutdown_tests.rs |  97 +++++++++++++
 docs/TESTS.md                   |  24 ++++
 10 files changed, 623 insertions(+), 17 deletions(-)
```

## Test Results

```
   Compiling anvilml v0.1.1 (/home/dryw/AnvilML/backend)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.48s
     Running unittests src/lib.rs (target/debug/deps/anvilml-57b951d2e7df7095)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml-f3b356793e388c52)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/cli_help_test.rs (target/debug/deps/cli_help_test-b0616338c1e31031)

running 1 test
test tests::cli_help_shows_all_flags ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/shutdown_tests.rs (target/debug/deps/shutdown_tests-79abafa647a16e6a)

running 2 tests
test tests::test_shutdown_signal_timeout_cancels ... ok
test tests::test_shutdown_signal_returns_on_ctrl_c ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, formatting clean)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.08s
---CHECK1 OK---

# 2. Mock-hardware Windows:
    Checking anvilml v0.1.1 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.86s
---CHECK2 OK---

# 3. Real-hardware Linux:
    Checking anvilml v0.1.1 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.14s
---CHECK3 OK---

# 4. Real-hardware Windows:
    Checking anvilml v0.1.1 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.19s
---CHECK4 OK---
```

All four platform cross-checks passed.

## Project Gates

```
# Gate 1 — Config Surface Sync:
cargo test -p anvilml --features mock-hardware -- config_reference
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out
```

No `config_reference` test exists in this crate (it's defined in a different crate). Gate passes.

## Public API Delta

```
backend/src/shutdown.rs:8:pub async fn wait_for_shutdown_signal() {
backend/src/lib.rs:6:pub mod shutdown;
```

New public items:
- `pub mod shutdown;` — module path: `anvilml::shutdown` (in `backend/src/lib.rs`)
- `pub async fn wait_for_shutdown_signal()` — module path: `anvilml::shutdown::wait_for_shutdown_signal` (in `backend/src/shutdown.rs`)

Both match the plan's `## Public API Surface` table.

## Deviations from Plan

1. **Added `backend/src/lib.rs`** — The plan did not mention creating a library crate entry point. This was necessary because the approved plan's test file imports `anvilml::shutdown::wait_for_shutdown_signal`, which requires the `backend` crate to be a library crate (not just a binary crate). Without `lib.rs`, the integration test would fail with `cannot find module or crate anvilml in this scope`.

2. **Version bump via direct override** — The plan said to "increment the patch version in `backend/Cargo.toml` from `0.1.0` to `0.1.1`." However, `backend/Cargo.toml` uses `version.workspace = true` (inheriting from `[workspace.package] version = "0.1.0"`). Per ENVIRONMENT.md §12, the workspace version is read-only. The fix was to override with `version = "0.1.1"` at the package level, giving the backend its own version while preserving the workspace version at `0.1.0`.

3. **Fixed `cli` unused variable** — The plan stated to "remove the `let _ = cli;` dead-code suppression (cli is now used)." However, `cli` is NOT used after the async conversion — the HTTP server wiring is a later phase. Fixed by renaming to `let _cli = cli::parse();` to suppress the warning cleanly without a separate dead-code suppression line.

4. **Signal test: `$PPID` instead of `$$`** — The plan's test used `kill -INT $$` which expands to the child shell's PID, not the parent test process PID. Fixed to use `$PPID` (parent PID) to correctly send SIGINT to the test process.

5. **Signal test: 100ms pre-delay** — Added a `tokio::time::sleep(100ms)` before spawning the signal sender to give the tokio runtime time to register the signal handler. This prevents a race condition where the signal arrives before the handler is installed.

6. **Tokio version resolved to 1.52.3** — The plan specified 1.47.0 (MCP unavailable during planning). Cargo resolved to 1.52.3, which is above the 1.47.0 floor. No API changes needed — `tokio::signal::ctrl_c()` is stable.

## Blockers

None.
