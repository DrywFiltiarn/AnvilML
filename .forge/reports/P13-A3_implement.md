# Implementation Report: P13-A3

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P13-A3                             |
| Phase         | 13 — Job Queue                     |
| Description   | anvilml-scheduler: VramLedger per-device reservation tracking |
| Implemented   | 2026-07-07T10:30:00Z              |
| Status        | COMPLETE                           |

## Summary

Created `VramLedger` — a pure in-memory, per-device VRAM reservation tracker in the
`anvilml-scheduler` crate. The struct tracks how much VRAM (in MiB) has been reserved
on each device index using a `HashMap<u32, u32>`, and provides three methods: `reserve()`,
`release()`, and `free_mib()`. All methods use saturating arithmetic to prevent panics
from over-release or reservation overflow. Six integration tests cover all public methods
plus edge cases (over-release, unknown device, accumulation, multi-device independence).
The module was registered in `lib.rs` with `pub mod ledger` and `pub use ledger::VramLedger`,
and the crate version was bumped from 0.1.8 to 0.1.9.

## Resolved Dependencies

None. `VramLedger` uses only `std::collections::HashMap` — no new external crates required.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/ledger.rs` | `VramLedger` struct with `reserve`, `release`, `free_mib` methods |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Added `pub mod ledger;` and `pub use ledger::VramLedger;` |
| CREATE | `crates/anvilml-scheduler/tests/ledger_tests.rs` | 6 integration tests covering all public methods |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Bumped patch version 0.1.8 → 0.1.9 |
| MODIFY | `docs/TESTS.md` | Added 6 entries for the new ledger tests |

## Commit Log

```
 .forge/reports/P13-A3_plan.md                  | 223 +++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                   |   6 +-
 .forge/state/state.json                        |  13 +-
 Cargo.lock                                     |   2 +-
 crates/anvilml-scheduler/Cargo.toml            |   2 +-
 crates/anvilml-scheduler/src/ledger.rs         |  88 ++++++++++
 crates/anvilml-scheduler/src/lib.rs            |   2 +
 crates/anvilml-scheduler/tests/ledger_tests.rs | 109 ++++++++++++
 docs/TESTS.md                                  |  72 ++++++++
 9 files changed, 506 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/ledger_tests.rs (target/debug/deps/ledger_tests-1aa3640a0dee7f0c)

running 6 tests
test test_multi_device_independent ... ok
test test_over_release_does_not_panic ... ok
test test_release_restores_capacity ... ok
test test_reserve_accumulates_on_same_device ... ok
test test_reserve_reduces_free_mib ... ok
test test_unknown_device_returns_total_mib ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace test suite: 236 tests passed, 0 failed.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.59s

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.68s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 25.96s

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 23.97s
```

All four platform cross-checks exit 0.

## Project Gates

Gate 1 (config_reference): `cargo test -p anvilml --features mock-hardware -- config_reference` exits 0.
No other gates triggered — this task does not modify ServerConfig, handler signatures, node types, or arch module methods.

## Public API Delta

```
+pub mod ledger;
+pub use ledger::VramLedger;
```

From `ledger.rs`:
- `pub struct VramLedger` — per-device VRAM reservation tracker
- `pub fn new() -> Self` — via `Default` impl
- `pub fn reserve(&mut self, device_index: u32, vram_mib: u32)` — add reservation
- `pub fn release(&mut self, device_index: u32, vram_mib: u32)` — remove reservation
- `pub fn free_mib(&self, device_index: u32, total_mib: u32) -> u32` — query available VRAM

All public items match the plan's Public API Surface table.

## Deviations from Plan

- The plan's `release()` code snippet used `*self.reservations.entry(...).or_insert(0).saturating_sub(vram_mib);`
  which does not compile because `saturating_sub()` returns a `u32` value, not a mutable reference.
  Fixed by assigning the result back to the entry: `let entry = self.reservations.entry(...).or_insert(0); *entry = entry.saturating_sub(vram_mib);`
- Clippy flagged a `collapsible_if` in the zero-balance cleanup code. Refactored from nested `if let` + `if` to a single `if let ... && ...` pattern.

## Blockers

None.
