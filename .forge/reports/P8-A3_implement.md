# Implementation Report: P8-A3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P8-A3                                             |
| Phase       | 008 — IPC Framing                                 |
| Description | anvilml-ipc: read_frame with size cap and read-fully loop |
| Implemented | 2026-06-05T19:07:20Z                              |
| Status      | COMPLETE                                          |

## Summary

Implemented `read_frame` in `crates/anvilml-ipc/src/framing.rs`: an async function that reads a length-prefixed msgpack frame from any `AsyncRead + Unpin` source, enforces a configurable size cap (`max_mib`) before allocating the payload buffer, and deserialises the payload into a `WorkerEvent`. Added two unit tests: `read_frame_roundtrip` (full duplex round-trip) and `read_frame_oversize_rejected` (malicious header rejection). All gates pass.

## Resolved Dependencies

No new dependencies added. All crates (`tokio`, `rmp_serde`, `anvilml-core`) already present in the crate's `Cargo.toml`.

| Type   | Name      | Version resolved | Source        |
|--------|-----------|-----------------|---------------|
| crate  | tokio     | (existing)      | Lockfile      |
| crate  | rmp-serde | (existing)      | Lockfile      |

## Files Changed

| Action   | Path                                      | Description |
|----------|-------------------------------------------|-------------|
| Edit     | `crates/anvilml-ipc/src/framing.rs`       | Add `AsyncRead`/`AsyncReadExt` imports; add `WorkerEvent` import; implement `read_frame` function; add two unit tests (`read_frame_roundtrip`, `read_frame_oversize_rejected`) |

## Commit Log

```
 .forge/reports/P8-A3_plan.md      | 122 ++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md      |   6 +-
 .forge/state/state.json           |  13 ++--
 crates/anvilml-ipc/src/framing.rs |  96 ++++++++++++++++++++++++++++--
 4 files changed, 223 insertions(+), 14 deletions(-)
```

## Test Results

### `cargo test -p anvilml-ipc -- read_frame`

```
running 2 tests
test framing::tests::read_frame_oversize_rejected ... ok
test framing::tests::read_frame_roundtrip ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 21 filtered out
```

### Full crate test suite (`cargo test -p anvilml-ipc`)

```
running 23 tests
test framing::tests::read_frame_oversize_rejected ... ok
test framing::tests::write_frame ... ok
test framing::tests::write_frame_execute ... ok
test framing::tests::read_frame_roundtrip ... ok
test framing::tests::write_frame_sync_serialization ... ok
test framing::tests::write_frame_shutdown ... ok
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

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Platform Cross-Check

### 1. `cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware`

```
Checking anvilml-ipc v0.1.0 (/home/dryw/AnvilML/crates/anvilml-ipc)
Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.29s
```

### 2. `cargo check --bin anvilml`

```
Checking anvilml-ipc v0.1.0 (/home/dryw/AnvilML/crates/anvilml-ipc)
Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.95s
```

### 3. `cargo check --bin anvilml --target x86_64-pc-windows-gnu`

```
Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.90s
```

All three checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync (`cargo test -p backend --features mock-hardware -- test_toml_key_set_matches_default`)

```
running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out
```

## Deviations from Plan

- **Test adjustment**: The plan specified writing `WorkerMessage::Ping { seq: 7 }` via `write_frame` and asserting `WorkerEvent::Pong { seq: 7 }`. Since the test uses a raw duplex pipe (no actual worker process to convert Ping→Pong), the test was adjusted to write the `WorkerEvent::Pong { seq: 7 }` directly through the duplex and read it back, verifying the framing layer round-trip correctly.
- **Cursor type**: The plan referenced `tokio::io::Cursor::new`; corrected to `std::io::Cursor::new` which is the actual standard library type.
- **Formatting side-effect**: `cargo fmt --all` reformatted existing test code (splitting `.await.expect(...)` across multiple lines). No functional changes were made.

## Blockers

None.
