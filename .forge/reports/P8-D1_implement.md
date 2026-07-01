# Implementation Report: P8-D1

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P8-D1                                       |
| Phase         | 008 — IPC Stress Gate & Worker Pool         |
| Description   | anvilml-worker: respawn.rs RespawnPolicy backoff + max-attempt guard |
| Implemented   | 2026-07-01T12:30:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Implemented `RespawnPolicy` — a pure, zero-I/O struct in `crates/anvilml-worker/src/respawn.rs` that encodes the worker crash-recovery backoff policy per `ANVILML_DESIGN.md §19.4`. The struct holds three configurable parameters (delay, max attempts, window) and provides `should_respawn()` which counts attempts within a trailing window to decide whether a crashed worker may be respawned, and `next_delay()` which returns the constant delay as a `Duration`. Six integration tests cover all decision paths. The module is exported via `lib.rs` and the crate version was bumped from 0.1.4 to 0.1.5.

## Resolved Dependencies

| Type   | Name | Version resolved | Source         |
|--------|------|------------------|----------------|
| std    | std::time::Instant | 1.96.0 (Rust stdlib) | N/A (stdlib) |
| std    | std::time::Duration | 1.96.0 (Rust stdlib) | N/A (stdlib) |

No new external dependencies were introduced. The implementation uses only `std::time::Instant` and `std::time::Duration` from the Rust standard library.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-worker/src/respawn.rs` | `RespawnPolicy` struct with `Default` impl, `new()`, `should_respawn()`, `next_delay()` |
| MODIFY | `crates/anvilml-worker/src/lib.rs` | Added `mod respawn; pub use respawn::RespawnPolicy;` |
| CREATE | `crates/anvilml-worker/tests/respawn_tests.rs` | 6 integration tests for `RespawnPolicy` |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Bumped patch version 0.1.4 → 0.1.5 |
| MODIFY | `docs/TESTS.md` | Added 6 test catalogue entries for new tests |

## Commit Log

```
 .forge/reports/P8-D1_plan.md                 | 232 +++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  11 +-
 Cargo.lock                                   |   2 +-
 crates/anvilml-worker/Cargo.toml             |   2 +-
 crates/anvilml-worker/src/lib.rs             |   3 +
 crates/anvilml-worker/src/respawn.rs         |  92 +++++++++++
 crates/anvilml-worker/tests/respawn_tests.rs | 122 ++++++++++++++
 docs/TESTS.md                                |  72 +++++++++
 9 files changed, 532 insertions(+), 10 deletions(-)
```

## Test Results

```
     Running tests/respawn_tests.rs (target/debug/deps/respawn_tests-497b5e933234460f)

running 6 tests
test test_at_limit_blocks_respawn ... ok
test test_attempts_outside_window_dont_count ... ok
test test_defaults_match_documented_values ... ok
test test_empty_history_allows_respawn ... ok
test test_next_delay_returns_correct_duration ... ok
test test_under_limit_allows_respawn ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace test suite: 169 tests passed, 0 failed (including the 6 new respawn tests).

## Format Gate

```
cargo fmt --all -- --check
```
Exit 0 — no formatting drift.

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.69s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 28.64s

# 3. Real-hardware Linux
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 24.84s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.62s
```

All four platform cross-checks passed.

## Project Gates

**Gate 1 — Config Surface Sync:** Not triggered — this task does not modify `ServerConfig` or any nested config struct. The existing `config_reference` test passes.

**Gate 2 — OpenAPI Drift:** Not triggered — this task does not modify handler function signatures, `#[utoipa::path]` annotations, or `AppState` fields.

**Gate 3 — Node Parity:** Not triggered — this task does not add, remove, or rename node types in `worker/nodes/` or modify `node_registry.rs`.

**Gate 4 — Mock/Real Parity Markers:** Not triggered — `RespawnPolicy` is a pure computation type, not a node's `execute()` or an arch module's `load()`/`sample()`/`decode()`. The dual-mode parity marker convention (`ANVILML_DESIGN.md §10.6`) applies only to those function categories.

## Public API Delta

New public items introduced by this task:

| Item | Type | Module Path |
|------|------|-------------|
| `RespawnPolicy` | struct | `anvilml_worker::RespawnPolicy` (re-exported from `respawn`) |
| `RespawnPolicy::new()` | fn | `anvilml_worker::RespawnPolicy::new(respawn_delay_ms: u32, respawn_max_attempts: u32, respawn_window_s: u32) -> Self` |
| `RespawnPolicy::should_respawn()` | fn | `anvilml_worker::RespawnPolicy::should_respawn(&self, attempt_history: &[std::time::Instant]) -> bool` |
| `RespawnPolicy::next_delay()` | fn | `anvilml_worker::RespawnPolicy::next_delay(&self) -> std::time::Duration` |
| `impl Default for RespawnPolicy` | impl | `anvilml_worker::RespawnPolicy` |

All items match the plan's `## Public API Surface` table exactly.

## Deviations from Plan

None. Implementation matches the approved plan exactly:
- All three struct fields use `u32` as specified.
- Default values (2000ms, 5, 300s) match `ANVILML_DESIGN.md §19.4`.
- `should_respawn()` uses `Instant::now()` at call time, matching the plan's approach.
- 6 tests written (plan required >= 5), covering all decision paths.
- `lib.rs` remains 21 lines (well under the 80-line hard cap).

## Blockers

None.
