# Implementation Report: P10-A1

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P10-A1                                        |
| Phase         | 010 — Worker Lifecycle                        |
| Description   | anvilml-worker: respawn.rs RespawnPolicy with backoff and max-attempt guard |
| Implemented   | 2026-06-17T08:15:00Z                          |
| Status        | COMPLETE                                      |

## Summary

Implemented the full `RespawnPolicy` in `crates/anvilml-worker/src/respawn.rs`: made all three struct fields (`delay_ms`, `max_attempts`, `window_s`) public, implemented `should_respawn()` (pure decision function that checks max-attempt guard), and implemented `next_delay_ms()` (exponential backoff `delay_ms * 2^attempt` capped at 30,000 ms). Updated module-level and struct-level doc comments to remove stub references. Created `crates/anvilml-worker/tests/respawn_tests.rs` with four integration tests. Bumped `anvilml-worker` version from 0.1.6 to 0.1.7. Updated `docs/TESTS.md` with entries for all four new tests.

## Resolved Dependencies

None. No new dependencies added or modified.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | crates/anvilml-worker/src/respawn.rs | Made fields pub, implemented should_respawn and next_delay_ms, removed stub references from doc comments |
| CREATE | crates/anvilml-worker/tests/respawn_tests.rs | Four integration tests for RespawnPolicy |
| MODIFY | crates/anvilml-worker/Cargo.toml | Bumped version 0.1.6 → 0.1.7 |
| MODIFY | docs/TESTS.md | Added entries for 4 new respawn tests |

## Commit Log

```
 .forge/reports/P10-A1_plan.md                | 184 +++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  13 +-
 Cargo.lock                                   |   2 +-
 crates/anvilml-worker/Cargo.toml             |   2 +-
 crates/anvilml-worker/src/respawn.rs         |  89 +++++++++----
 crates/anvilml-worker/tests/respawn_tests.rs | 100 +++++++++++++++
 docs/TESTS.md                                |  36 ++++++
 8 files changed, 399 insertions(+), 33 deletions(-)
```

## Test Results

```
     Running tests/respawn_tests.rs (target/debug/deps/respawn_tests-14a19f998e3124a7)

running 4 tests
test test_next_delay_ms_exponential_backoff_and_cap ... ok
test test_should_respawn_max_attempts_exceeded ... ok
test test_should_respawn_window_reset ... ok
test test_should_respawn_within_window ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Note: The workspace test suite includes a pre-existing failure in `backend/tests/cli_tests.rs::test_custom_port_health` (unrelated to this task — it is a test infrastructure issue where the binary is not correctly built for the integration test). This failure existed before this task and is not caused by any changes in this task.

## Format Gate

```
(no output — cargo fmt --all -- --check exited 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.70s

# 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.87s

# 3. Real-hardware Linux (bin anvilml)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.89s

# 4. Real-hardware Windows (bin anvilml, x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.37s
```

All four platform cross-checks exit 0.

## Project Gates

None applicable — this task does not modify config fields on `ServerConfig`, handler signatures, or node types. No gates triggered.

## Public API Delta

```
+    pub delay_ms: u64,
+    pub max_attempts: u32,
+    pub window_s: u32,
+    pub fn should_respawn(&self, crash_count: u32, _last_crash: Instant) -> bool {
+    pub fn next_delay_ms(&self, attempt: u32) -> u64 {
```

New public items:
- `RespawnPolicy::delay_ms: u64` (pub field, struct)
- `RespawnPolicy::max_attempts: u32` (pub field, struct)
- `RespawnPolicy::window_s: u32` (pub field, struct)
- `RespawnPolicy::should_respawn(&self, crash_count: u32, _last_crash: Instant) -> bool` (pub fn)
- `RespawnPolicy::next_delay_ms(&self, attempt: u32) -> u64` (pub fn)

All match the plan's Public API Surface table.

## Deviations from Plan

1. **`should_respawn` logic adjustment**: The plan's wording was ambiguous about whether the window expiry check gates the respawn decision. After implementing and running the test `test_should_respawn_within_window`, I verified that the method returns `true` whenever `crash_count < max_attempts`, regardless of elapsed time. The window expiry check is informational — it tells the caller whether to reset `crash_count` to 0, not whether to allow respawning. The `_last_crash` parameter is accepted but not used in the return value (hence the underscore prefix).

2. **Pre-existing test failure**: `test_custom_port_health` in `backend/tests/cli_tests.rs` fails with a pre-existing infrastructure issue (binary not correctly built for integration test). This is unrelated to this task's changes and is documented in Test Results.

## Blockers

None.
