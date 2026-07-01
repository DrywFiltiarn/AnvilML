# Implementation Report: P8-C1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P8-C1                           |
| Phase         | 8 — anvilml-worker              |
| Description   | anvilml-worker: demux.rs register/deregister pair (mandatory) |
| Implemented   | 2026-07-01T10:45:00Z           |
| Status        | COMPLETE                        |

## Summary

Implemented the `Demux` struct in `crates/anvilml-worker/src/demux.rs` — a mutex-protected
map from worker ID to `tokio::sync::mpsc::Sender<WorkerEvent>` that demultiplexes incoming
events to per-worker channels. Added `register()`, `deregister()`, and async `route()`
methods, plus 5 integration tests covering delivery, not-found, deregistration,
double-deregistration safety, and idempotent overwrite behavior. Updated `Cargo.toml` to
add the `sync` feature to tokio, re-exported `Demux` from `lib.rs`, bumped the crate
version to 0.1.3, and updated `docs/TESTS.md` with all 5 test entries.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| crate  | tokio     | 1.52.3           | rust-docs MCP  |

No new dependencies added. Only the existing `tokio` dependency's features were extended
from `["process"]` to `["process", "sync"]` to enable `tokio::sync::mpsc::Sender`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | crates/anvilml-worker/Cargo.toml | Added `sync` feature to tokio; bumped version 0.1.2 → 0.1.3 |
| Create | crates/anvilml-worker/src/demux.rs | New `Demux` struct with `new()`, `register()`, `deregister()`, `route()` |
| Modify | crates/anvilml-worker/src/lib.rs | Added `mod demux;` and `pub use demux::Demux;` |
| Create | crates/anvilml-worker/tests/demux_tests.rs | 5 integration tests for Demux |
| Modify | docs/TESTS.md | Added 5 entries for new demux tests |

## Commit Log

```
 .forge/reports/P8-C1_plan.md               | 128 ++++++++++++++++++
 .forge/state/CURRENT_TASK.md               |   6 +-
 .forge/state/state.json                    |  13 +-
 Cargo.lock                                 |   2 +-
 crates/anvilml-worker/Cargo.toml           |   4 +-
 crates/anvilml-worker/src/demux.rs         | 129 ++++++++++++++++++
 crates/anvilml-worker/src/lib.rs           |   3 +
 crates/anvilml-worker/tests/demux_tests.rs | 203 +++++++++++++++++++++++++++++
 docs/TESTS.md                              |  60 +++++++++
 9 files changed, 536 insertions(+), 12 deletions(-)
```

## Test Results

```
running 5 tests
test test_deregister_removes_entry ... ok
test test_double_deregister_is_safe ... ok
test test_register_and_route_delivers ... ok
test test_route_worker_not_found ... ok
test test_register_overwrites ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace test suite: 170 tests passed, 0 failed.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.69s

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 26.46s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.38s

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.38s
```

All four checks exited 0.

## Project Gates

```
Gate 1 — Config Surface Sync:
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Public API Delta

```
+pub use demux::Demux;
```

New public items:
- `pub struct Demux` — in `crates/anvilml-worker/src/demux.rs`
- `pub use demux::Demux` — re-exported in `crates/anvilml-worker/src/lib.rs`

Both match the plan's `## Public API Surface` table.

## Deviations from Plan

- Added `impl Default for Demux` (clippy `new_without_default` lint required it). The
  `new()` method delegates to `Self::default()`. This is additive — it does not change
  any behavior.
- Removed unused `use tokio::sync::mpsc::Sender` import from test file (clippy warning).
  The `Sender` type is inferred from `tokio::sync::mpsc::channel::<WorkerEvent>(16)`
  return type, so the explicit import was unnecessary.
- Used a block scope `{ let map = ...; ... }` in `route()` to satisfy clippy's
  `await_holding_lock` lint. The `MutexGuard` is dropped when the block ends, before
  the `.await` on `tx.send(event)`.

## Blockers

None.
