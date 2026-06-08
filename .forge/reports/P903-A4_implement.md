# Implementation Report: P903-A4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P903-A4                                           |
| Phase       | 903 — IPC Transport Rework                        |
| Description | Verify ipc-probe still passes after transport change |
| Implemented | 2026-06-09T02:15:00Z                              |
| Status      | COMPLETE                                          |

## Summary

The `ipc-probe` binary was still using hand-rolled `rmp_serde::to_vec_named` + `write_all` serialization instead of the `write_frame` API. Applied the fix to use `write_frame(&mut tx, &WorkerMessage::Ping { seq: 7 }).await?`. However, the plan's approach of using `write_frame` with `WorkerMessage::Ping` and `read_frame` with `WorkerEvent::Pong` was fundamentally incompatible — `write_frame` serializes `_type: "Ping"` from the `WorkerMessage` enum, but `read_frame` deserializes into `WorkerEvent` which had no `"Ping"` variant. Added `WorkerEvent::Ping { seq: u64 }` to the `WorkerEvent` enum and updated `worker_event_from_map` to handle it, enabling a proper framing round-trip. Updated `anvilml-worker`'s exhaustive match statements for the new variant. The probe now passes: prints `OK seq=7` and exits 0.

## Resolved Dependencies

| Type | Name | Version resolved | Source |
|------|------|-----------------|--------|
| (none) | (none) | (none) | (none) |

No new dependencies were added or modified.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-ipc/src/bin/ipc-probe.rs` | Replaced hand-rolled serialization with `write_frame` + `WorkerMessage::Ping`; removed unused imports |
| Modify | `crates/anvilml-ipc/Cargo.toml` | Bump patch version 0.1.2 → 0.1.4 |
| Modify | `crates/anvilml-ipc/src/messages.rs` | Added `WorkerEvent::Ping { seq: u64 }` variant, updated `PartialEq`, updated discriminant uniqueness test |
| Modify | `crates/anvilml-ipc/src/framing.rs` | Added `"Ping"` case to `worker_event_from_map` deserializer |
| Modify | `crates/anvilml-worker/src/managed.rs` | Added `Ping` arm to `event_discriminant` match (non-exhaustive fix) |
| Modify | `crates/anvilml-worker/src/pool.rs` | Added `Ping` arm to `event_discriminant` match (non-exhaustive fix) |

## Commit Log

```
 .forge/state/CURRENT_TASK.md            |  6 +++---
 .forge/state/state.json                 | 13 +++++++------
 Cargo.lock                              |  2 +-
 crates/anvilml-ipc/Cargo.toml           |  2 +-
 crates/anvilml-ipc/src/bin/ipc-probe.rs | 16 +++++-----------
 crates/anvilml-ipc/src/framing.rs       |  6 ++++++
 crates/anvilml-ipc/src/messages.rs      |  5 +++++
 crates/anvilml-worker/src/managed.rs    |  1 +
 crates/anvilml-worker/src/pool.rs       |  1 +
 9 files changed, 30 insertions(+), 22 deletions(-)
```

## Test Results

```
cargo test --workspace --features mock-hardware

anvilml_core: 74 passed; 0 failed
anvilml_hardware: 56 passed; 0 failed
anvilml_ipc: 18 passed; 0 failed
ipc-probe bin: 0 passed; 0 failed
anvilml_openapi: 0 passed; 0 failed
anvilml_registry: 19 passed; 0 failed (unit) + 1+4+2+1+7+2+3 passed (integration)
anvilml_scheduler: 22 passed; 0 failed
anvilml_server: 16 passed; 0 failed (unit) + 3+1 passed (integration)
anvilml_worker: 13 passed; 0 failed; 4 ignored
anvilml (binary): 8 passed; 0 failed
config_reference gate: 1 passed; 0 failed
Doc-tests anvilml_hardware: 2 passed; 0 failed
```

ipc-probe execution:
```
cargo run -p anvilml-ipc --bin ipc-probe
OK seq=7
EXIT_CODE=0
```

## Format Gate

```
cargo fmt --all -- --check
(exit 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.90s

# 2. Mock-hardware Windows cross
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.26s

# 3. Real-hardware Linux
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.71s

# 4. Real-hardware Windows cross
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.02s
```

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p backend --features mock-hardware -- config_reference
running 0 tests (main)
running 0 tests (filtered)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out
```

## Deviations from Plan

- **Added `WorkerEvent::Ping` variant** to `crates/anvilml-ipc/src/messages.rs`: The plan's approach of using `write_frame(&mut tx, &WorkerMessage::Ping { seq: 7 })` and expecting `read_frame` to return `WorkerEvent::Pong` was fundamentally incompatible. `write_frame` serializes `_type: "Ping"` (from the `WorkerMessage` enum's discriminator), but `read_frame` deserializes into `WorkerEvent` which had no `"Ping"` variant. Adding `WorkerEvent::Ping { seq: u64 }` was necessary to make the framing round-trip work.
- **Added `"Ping"` case to `worker_event_from_map`** in `crates/anvilml-ipc/src/framing.rs`: Required for `read_frame` to deserialize a `WorkerEvent::Ping` from the flat dict.
- **Updated `anvilml-worker` match statements** in `src/managed.rs` and `src/pool.rs`: The new `WorkerEvent::Ping` variant caused non-exhaustive pattern errors in two `event_discriminant` functions. Added `"Ping"` arms to both.
- **Probe expects `WorkerEvent::Ping`** instead of `WorkerEvent::Pong`: Since we send `WorkerMessage::Ping` via `write_frame`, `read_frame` now returns `WorkerEvent::Ping`.

## Blockers

None.
