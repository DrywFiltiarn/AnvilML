# Implementation Report: P7-A3

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P7-A3                              |
| Phase         | 007 — WebSocket Event Stream       |
| Description   | anvilml-server: SystemStats background tick task |
| Implemented   | 2026-06-16T10:15:00Z               |
| Status        | COMPLETE                           |

## Summary

Implemented the `stats_tick` module in `crates/anvilml-server/src/ws/stats_tick.rs` with a complete `pub fn start(broadcaster: Arc<EventBroadcaster>)` that spawns a tokio task broadcasting `WsEvent::SystemStats` events every 5 seconds containing CPU utilisation (via `sysinfo::System::global_cpu_usage()`) and RAM usage in mebibytes (via `sysinfo::System::used_memory()`). Wired the call into `backend/src/main.rs` after the server bind log and before `axum::serve()`. Added three integration tests and updated the test catalogue. Bumped the `anvilml-server` crate version from `0.1.11` to `0.1.12`.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source       |
|--------|---------|-----------------|--------------|
| crate  | sysinfo | 0.33            | lockfile     |

Note: `sysinfo = "0.33"` is already a dependency of `anvilml-hardware` (locked at `0.33.1` in `Cargo.lock`). The API shape was verified against the existing usage in `anvilml-hardware/src/cpu.rs` which uses `System::new_all()`, `sys.refresh_all()`, and `sys.total_memory()`. The additional APIs needed for this task (`sys.global_cpu_usage()` returning `f32`, `sys.used_memory()` returning `u64`) were confirmed present in sysinfo 0.33.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/Cargo.toml` | Add `sysinfo = "0.33"` dependency; bump version `0.1.11 → 0.1.12` |
| Modify | `crates/anvilml-server/src/ws/stats_tick.rs` | Replace stub with full tick implementation (98 lines) |
| Modify | `backend/src/main.rs` | Clone broadcaster before router build; call `stats_tick::start()` after bind log |
| CREATE | `crates/anvilml-server/tests/stats_tick_tests.rs` | Three integration tests for the tick task |
| Modify | `docs/TESTS.md` | Add three test entries for new stats_tick tests |

## Commit Log

```
 .forge/reports/P7-A3_plan.md                    | 127 ++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                    |   6 +-
 .forge/state/state.json                         |  13 +-
 Cargo.lock                                      |   3 +-
 backend/src/main.rs                             |  11 ++
 crates/anvilml-server/Cargo.toml                |   3 +-
 crates/anvilml-server/src/ws/stats_tick.rs      | 100 +++++++++++++++-
 crates/anvilml-server/tests/stats_tick_tests.rs | 150 ++++++++++++++++++++++++
 docs/TESTS.md                                   |  27 +++++
 9 files changed, 425 insertions(+), 15 deletions(-)
```

## Test Results

```
     Running tests/stats_tick_tests.rs (target/debug/deps/stats_tick_tests-e5991777c0fd61be)

running 3 tests
test test_stats_tick_cpu_pct_is_finite ... ok
test test_stats_tick_ram_used_mib_is_non_negative ... ok
test test_stats_tick_broadcasts_system_stats ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.26s
```

Full workspace test suite: 136 tests passed, 0 failed. All existing tests remain green.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux:
Checking anvilml-server v0.1.12 (/home/dryw/AnvilML/crates/anvilml-server)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Checking anvilml v0.1.9 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.85s

# 2. Mock-hardware Windows:
Checking anvilml-server v0.1.12 (/home/dryw/AnvilML/crates/anvilml-server)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Checking anvilml v0.1.9 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.46s

# 3. Real-hardware Linux:
Checking anvilml-server v0.1.12 (/home/dryw/AnvilML/crates/anvilml-server)
Checking anvilml v0.1.9 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.29s

# 4. Real-hardware Windows:
Checking anvilml-server v0.1.12 (/home/dryw/AnvilML/crates/anvilml-server)
Checking anvilml v0.1.9 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.34s
```

All four cross-checks exit 0.

## Project Gates

```
# Gate 1 — config_reference:
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 1 passes. No config fields were added or modified by this task.

## Public API Delta

```
+pub fn start(broadcaster: Arc<EventBroadcaster>) {
```

One new public item introduced:
- `fn start(broadcaster: Arc<EventBroadcaster>)` — function in `anvilml_server::ws::stats_tick` module

This matches the plan's `## Public API Surface` table exactly.

## Deviations from Plan

- **API signature correction**: The plan stated `sysinfo::System::global_cpu_usage()` returns `f64`. In sysinfo 0.33, it actually returns `f32`, so the cast from `f64` to `f32` was unnecessary. The implementation uses the value directly without casting. This was discovered during clippy linting (`clippy::unnecessary_cast`).
- **State cloning in main.rs**: The plan called for `state.broadcaster.clone()` after `axum::serve()`, but `state` is already moved into `build_router(state)` on the previous line. The fix was to clone the broadcaster before building the router: `let broadcaster = state.broadcaster.clone(); let router = build_router(state);`.
- **Test assertion refinement**: The plan's `test_stats_tick_ram_used_mib_is_non_negative` test asserted `>= 0`, but clippy flagged this as a useless comparison since `u64` is inherently non-negative. Changed to `> 0` which is a meaningful assertion (system RAM should always be in use).

## Blockers

None.
