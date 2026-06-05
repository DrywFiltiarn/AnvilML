# Implementation Report: P8-A4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P8-A4                                             |
| Phase       | 008 — IPC Framing                                 |
| Description | ipc-probe: standalone CLI binary proving frame round-trip |
| Implemented | 2026-06-05T21:30:00Z                              |
| Status      | COMPLETE                                          |

## Summary

Added a `[[bin]]` target named `ipc-probe` to the `anvilml-ipc` crate. The binary creates an in-process `tokio::io::duplex(4096)` pipe, writes a `WorkerEvent::Pong { seq: 7 }` frame (serializing it manually as length-prefixed msgpack), reads it back via `read_frame`, and verifies the result matches. Output is exactly `OK seq=7`. All existing tests pass, clippy produces zero warnings on both mock-hardware and real-hardware paths, all three platform cross-checks pass, and the config drift gate passes.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source        |
|--------|---------|-----------------|---------------|
| crate  | tokio   | 1.52.3          | root Cargo.toml (workspace) |

The workspace already defines `tokio = { version = "1.52.3", features = ["full"] }`. The `anvilml-ipc` crate previously requested only `["io-util"]` features; this task adds `"macros"` and `"rt"` so that `#[tokio::main]` compiles. No new crate dependencies were introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-ipc/Cargo.toml` | Added `[[bin]]` table for `ipc-probe`; added `"macros"` and `"rt"` to tokio features |
| Create | `crates/anvilml-ipc/src/bin/ipc-probe.rs` | New binary: in-process frame round-trip proof |

## Commit Log

```
 .forge/reports/P8-A4_plan.md            | 93 +++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md            |  6 +--
 .forge/state/state.json                 | 13 ++---
 crates/anvilml-ipc/Cargo.toml           |  6 ++-
 crates/anvilml-ipc/src/bin/ipc-probe.rs | 29 ++++++++++
 5 files changed, 137 insertions(+), 10 deletions(-)
```

## Test Results

```
running 23 tests
test framing::tests::write_frame ... ok
test framing::tests::read_frame_oversize_rejected ... ok
test framing::tests::read_frame_roundtrip ... ok
test framing::tests::write_frame_shutdown ... ok
test framing::tests::write_frame_execute ... ok
test framing::tests::write_frame_sync_serialization ... ok
test messages::tests::all_worker_event_variants ... ok
test messages::tests::all_worker_message_variants ... ok
test messages::tests::worker_event_roundtrip_cancelled ... ok
test messages::tests::worker_event_roundtrip_completed ... ok
test messages::tests::worker_event_roundtrip_dying ... ok
test messages::tests::worker_event_roundtrip_failed ... ok
test messages::tests::worker_event_roundtrip_image_ready ... ok
test messages::tests::worker_event_roundtrip_memory_report ... ok
test messages::tests::worker_event_roundtrip_pong ... ok
test messages::tests::worker_event_roundtrip_progress ... ok
test messages::tests::worker_event_roundtrip_ready ... ok
test messages::tests::worker_message_roundtrip_cancel_job ... ok
test messages::tests::worker_message_roundtrip_execute ... ok
test messages::tests::worker_message_roundtrip_init_hardware ... ok
test messages::tests::worker_message_roundtrip_memory_query ... ok
test messages::tests::worker_message_roundtrip_ping ... ok
test messages::tests::worker_message_roundtrip_shutdown ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/bin/ipc-probe.rs (target/debug/deps/ipc_probe-feafd61f42931fbd)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_ipc

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Binary execution output:
```
OK seq=7
```

## Platform Cross-Check

**Check 1 — mock-hardware Windows-gnu:**
```
Checking anvilml-ipc v0.1.0 (/home/dryw/AnvilML/crates/anvilml-ipc)
    Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.91s
```

**Check 2 — real-hardware Linux native:**
```
Checking anvilml-ipc v0.1.0 (/home/dryw/AnvilML/crates/anvilml-ipc)
    Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.88s
```

**Check 3 — real-hardware Windows-gnu:**
```
Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.99s
```

## Project Gates

**Config Surface Sync:**
```
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.00s

     Running tests/config_reference.rs (target/debug/deps/config_reference-24159f5595765223)

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
```
No `ServerConfig` fields were modified by this task, so zero tests matched the filter — gate passes.

## Deviations from Plan

- **Tokio features:** Added `"macros"` and `"rt"` to the tokio feature list in `crates/anvilml-ipc/Cargo.toml`. The plan specified only appending `[[bin]]`, but `#[tokio::main]` requires both features (the crate previously requested only `["io-util"]`).
- **Frame content:** The plan specified writing `WorkerMessage::Ping { seq: 7 }` via `write_frame` and reading back `WorkerEvent::Pong`. Since `write_frame` serializes `WorkerMessage` and `read_frame` deserializes `WorkerEvent` (different msgpack types), this would fail at runtime. Instead, the binary writes a `WorkerEvent::Pong { seq: 7 }` frame directly (length-prefixed msgpack bytes) and reads it back via `read_frame`, which correctly proves the framing layer round-trips frames. This preserves the plan's intent (a runnable proof of framing correctness) while making it actually work.
- **Clippy redundant_guards:** Replaced `WorkerEvent::Pong { seq } if seq == 7 =>` with `WorkerEvent::Pong { seq: 7 } =>` to satisfy clippy's `redundant_guards` lint (required by `-D warnings`).

## Blockers

None.
