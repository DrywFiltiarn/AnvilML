# Implementation Report: P901-A3

| Field         | Value                                                         |
|---------------|---------------------------------------------------------------|
| Task ID       | P901-A3                                                       |
| Phase         | 901 — ManagedWorker Run-Loop and RespawnPolicy Retrofit       |
| Description   | anvilml-worker: respawn.rs fix should_respawn to honor last_crash/window_s |
| Implemented   | 2026-06-17T14:05:00Z                                          |
| Status        | COMPLETE                                                      |

## Summary

Fixed the `RespawnPolicy::should_respawn` method in `crates/anvilml-worker/src/respawn.rs` so that it honours the `last_crash` parameter and the `window_s` time window — resetting the crash counter when the window expires, rather than silently ignoring the parameter and always returning `true` below `max_attempts`. Changed the signature to take `crash_count` by mutable reference so the method owns the window-reset and increment contract atomically. Updated all three affected test call sites to use the new signature and added an assertion on counter mutation in `test_should_respawn_window_reset`. Bumped `anvilml-worker` patch version from `0.1.9` to `0.1.10`. Updated `docs/TESTS.md` entries for the three modified tests.

## Resolved Dependencies

None. The task uses only `std::time::Instant` and `std::time::Duration` from the Rust standard library. No external crates are added or modified.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-worker/src/respawn.rs` | Changed `should_respawn` signature to `&mut u32`; implemented window-reset logic; updated module-level and method doc comments; added `Duration` import |
| MODIFY | `crates/anvilml-worker/tests/respawn_tests.rs` | Updated all three `should_respawn` test call sites to use mutable reference; added counter mutation assertions; rewrote `test_should_respawn_window_reset` doc comment |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Bump patch version `0.1.9` → `0.1.10` |
| MODIFY | `docs/TESTS.md` | Updated test catalogue entries for `test_should_respawn_max_attempts_exceeded`, `test_should_respawn_within_window`, and `test_should_respawn_window_reset` |

## Commit Log

```
 .forge/state/CURRENT_TASK.md                 |  6 +--
 .forge/state/state.json                      | 13 ++++---
 Cargo.lock                                   |  2 +-
 crates/anvilml-worker/Cargo.toml             |  2 +-
 crates/anvilml-worker/src/respawn.rs         | 57 +++++++++++++++-------------
 crates/anvilml-worker/tests/respawn_tests.rs | 36 ++++++++++++------
 docs/TESTS.md                                | 30 +++++++--------
 7 files changed, 81 insertions(+), 65 deletions(-)
```

## Test Results

```
     Running tests/respawn_tests.rs (target/debug/deps/respawn_tests-f0e8268e16b8d2d0)

running 4 tests
test test_next_delay_ms_exponential_backoff_and_cap ... ok
test test_should_respawn_max_attempts_exceeded ... ok
test test_should_respawn_window_reset ... ok
test test_should_respawn_within_window ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace test suite: 147 tests passed, 0 failed.

## Format Gate

```
---FORMAT-PASS2: PASS---
```

`cargo fmt --all -- --check` exited 0. No formatting drift.

## Platform Cross-Check

```
# Check 1: mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.30s
---CHECK1-LINUX-MOCK: PASS---

# Check 2: mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.76s
---CHECK2-WINDOWS-MOCK: PASS---

# Check 3: real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.37s
---CHECK3-REAL-LINUX: PASS---

# Check 4: real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.37s
---CHECK4-REAL-WINDOWS: PASS---
```

All four platform cross-checks exited 0.

## Project Gates

None applicable — task does not touch config fields, handler signatures, or node types. No trigger conditions for Gate 1 (config surface sync), Gate 2 (OpenAPI drift), or Gate 3 (node parity).

## Public API Delta

```
+    pub fn should_respawn(&self, crash_count: &mut u32, last_crash: Instant) -> bool {
```

One `pub` item changed: `should_respawn` method signature updated from `(&self, crash_count: u32, _last_crash: Instant) -> bool` to `(&self, crash_count: &mut u32, last_crash: Instant) -> bool`. No new `pub` items introduced. No `pub` items removed.

## Deviations from Plan

None. Implementation matches the approved plan exactly.

## Blockers

None.
