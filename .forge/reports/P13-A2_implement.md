# Implementation Report: P13-A2

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P13-A2                             |
| Phase         | 013 — Job Scheduling & Dispatch    |
| Description   | anvilml-scheduler: ledger.rs VramLedger per-device reservation |
| Implemented   | 2026-06-19T21:15:00Z              |
| Status        | COMPLETE                           |

## Summary

Implemented `VramLedger` — a per-device VRAM reservation tracking ledger in `crates/anvilml-scheduler/src/ledger.rs`. The ledger provides `new()`, `register_device()`, `would_fit()`, `reserve()`, and `release()` methods for tracking VRAM reservations per GPU device index. It is pure synchronous with no I/O or async. `reserve()` panics on over-reservation and `release()` panics on underflow, both intentional programming-error guards. The dispatch loop must call `would_fit` before `reserve`. Added `pub mod ledger;` to the scheduler's `lib.rs`, created 8 integration tests in `crates/anvilml-scheduler/tests/ledger_tests.rs`, bumped `anvilml-scheduler` version from 0.1.5 to 0.1.6, and updated `docs/TESTS.md` with entries for all 8 new tests.

## Resolved Dependencies

None. The ledger uses only `std::collections::HashMap` (stdlib) and `tracing` (already a workspace dependency declared in `anvilml-scheduler`'s `Cargo.toml`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | crates/anvilml-scheduler/src/ledger.rs | VramLedger struct with new(), register_device(), would_fit(), reserve(), release(), and Default impl |
| MODIFY | crates/anvilml-scheduler/src/lib.rs | Added `pub mod ledger;` after `pub mod queue;` |
| CREATE | crates/anvilml-scheduler/tests/ledger_tests.rs | 8 integration tests for VramLedger |
| MODIFY | crates/anvilml-scheduler/Cargo.toml | Bumped version 0.1.5 → 0.1.6 |
| MODIFY | docs/TESTS.md | Added 8 test entries for ledger tests |

## Commit Log

```
 .forge/reports/P13-A2_plan.md                  | 134 +++++++++++++++++++
 .forge/state/CURRENT_TASK.md                   |   6 +-
 .forge/state/state.json                        |  13 +-
 Cargo.lock                                     |   2 +-
 crates/anvilml-scheduler/Cargo.toml            |   2 +-
 crates/anvilml-scheduler/src/ledger.rs         | 178 +++++++++++++++++++++++++
 crates/anvilml-scheduler/src/lib.rs            |   1 +
 crates/anvilml-scheduler/tests/ledger_tests.rs | 164 +++++++++++++++++++++++
 docs/TESTS.md                                  |  72 ++++++++++
 9 files changed, 561 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/ledger_tests.rs (target/debug/deps/ledger_tests-c72ff8775f674d9e)

running 8 tests
test test_duplicate_registration_is_noop ... ok
test test_multiple_devices_independent ... ok
test test_register_device_and_would_fit ... ok
test test_release_restores_free_vram ... ok
test test_release_underflow_panics - should panic ... ok
test test_reserve_overflow_panics - should panic ... ok
test test_reserve_reduces_free_vram ... ok
test test_would_fit_unknown_device_returns_false ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace test suite: 170+ tests across all crates, 0 failures.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# Check 1: Mock-hardware Linux
    Checking anvilml-scheduler v0.1.6 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.21 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Checking anvilml v0.1.14 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.32s
--- CHECK 1 PASSED ---

# Check 2: Mock-hardware Windows
    Checking anvilml-worker v0.1.25 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.6 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.21 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml v0.1.14 (/home/dryw/AnvilML/backend)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.54s
--- CHECK 2 PASSED ---

# Check 3: Real-hardware Linux
    Checking anvilml-worker v0.1.25 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.6 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.21 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml v0.1.14 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.77s
--- CHECK 3 PASSED ---

# Check 4: Real-hardware Windows
    Checking anvilml-worker v0.1.25 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.6 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.21 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml v0.1.14 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.44s
--- CHECK 4 PASSED ---
```

## Project Gates

```
# Gate 1: config_reference
    Finished `test` profile [unoptimized + debuginfo] target(s) in 11.35s
     Running tests/config_reference.rs
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 1 passed. No config fields were added or modified — the ledger is pure code with no config surface.

## Public API Delta

```
+pub mod ledger;
```

The only new `pub` item introduced in the `lib.rs` file is `pub mod ledger;`. The `VramLedger` struct and its methods are declared in the new `ledger.rs` file and are accessible via `anvilml_scheduler::ledger::VramLedger`.

New public items:
- `pub struct VramLedger` — `anvilml_scheduler::ledger::VramLedger`
- `pub fn new() -> Self` — `anvilml_scheduler::ledger::VramLedger::new`
- `pub fn register_device(&mut self, index: u32, vram_total_mib: u32)` — `anvilml_scheduler::ledger::VramLedger::register_device`
- `pub fn would_fit(&self, index: u32, requested_mib: u32) -> bool` — `anvilml_scheduler::ledger::VramLedger::would_fit`
- `pub fn reserve(&mut self, index: u32, mib: u32)` — `anvilml_scheduler::ledger::VramLedger::reserve`
- `pub fn release(&mut self, index: u32, mib: u32)` — `anvilml_scheduler::ledger::VramLedger::release`
- `impl Default for VramLedger` — `anvilml_scheduler::ledger::VramLedger::default`

## Deviations from Plan

None. Implementation matches the approved plan exactly.

## Blockers

None.
