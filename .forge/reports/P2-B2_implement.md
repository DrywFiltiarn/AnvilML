# Implementation Report: P2-B2

| Field          | Value                                      |
|----------------|---------------------------------------------|
| Task ID        | P2-B2                                       |
| Phase          | 002 — Core Types & IPC                      |
| Description    | anvilml-ipc: length-prefixed msgpack framing |
| Project        | anvilml                                     |
| Implemented at | 2026-05-30T08:45:00Z                        |
| Attempt        | 1                                           |

## Summary

Implemented the async length-prefixed msgpack framing layer for the `anvilml-ipc` crate. Created `framing.rs` with two public async functions — `write_frame` and `read_frame` — that wrap raw async I/O with the protocol defined in `ANVILML_DESIGN.md §7.1`: a 4-byte big-endian u32 length prefix followed by N bytes of msgpack-encoded payload. The framing enforces a configurable maximum payload size (in MiB) before any heap allocation, and uses `read_exact` / `write_all` to guarantee full reads/writes on all platforms including Windows where pipe reads are frequently partial.

## Files Changed

| Action   | Path                              | Description                                          |
|----------|-----------------------------------|------------------------------------------------------|
| MODIFY   | crates/anvilml-ipc/Cargo.toml     | Added tokio (io-util) and bytes as regular deps       |
| CREATE   | crates/anvilml-ipc/src/framing.rs | Length-prefixed msgpack framing with 2 unit tests     |
| MODIFY   | crates/anvilml-ipc/src/lib.rs     | Re-export framing::{write_frame, read_frame}          |
| MODIFY   | .forge/state/CURRENT_TASK.md      | Updated task status to COMPLETE                      |

## Test Results

All 76 workspace tests pass (0 failures). The new framing tests are:
- `framing::tests::roundtrip_ping_pong` — writes a Ping, reads it back as WorkerMessage, then writes Pong and reads via read_frame
- `framing::tests::reject_oversize_payload` — sends 65 MiB + 1 length prefix, verifies PayloadTooLarge without reading payload

```
   Compiling anvilml-ipc v0.1.0 (/home/dryw/AnvilML/crates/anvilml-ipc)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.91s
     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-924a1698aed899bc)

running 18 tests
test framing::tests::reject_oversize_payload ... ok
test messages::tests::msgpack_uses_named_map_format ... ok
test framing::tests::roundtrip_ping_pong ... ok
test messages::tests::worker_event_cancelled_roundtrip ... ok
test messages::tests::worker_event_completed_roundtrip ... ok
test messages::tests::worker_event_dying_roundtrip ... ok
test messages::tests::worker_event_failed_roundtrip ... ok
test messages::tests::worker_event_image_ready_roundtrip ... ok
test messages::tests::worker_event_memory_report_roundtrip ... ok
test messages::tests::worker_event_pong_roundtrip ... ok
test messages::tests::worker_event_progress_roundtrip ... ok
test messages::tests::worker_event_ready_roundtrip ... ok
test messages::tests::worker_message_cancel_job_roundtrip ... ok
test messages::tests::worker_message_execute_roundtrip ... ok
test messages::tests::worker_message_initialize_hardware_roundtrip ... ok
test messages::tests::worker_message_memory_query_roundtrip ... ok
test messages::tests::worker_message_ping_roundtrip ... ok
test messages::tests::worker_message_shutdown_roundtrip ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## CI Changes

No CI changes made.

## Commit Log

```
 M .forge/state/CURRENT_TASK.md
 M .forge/state/state.json
 M Cargo.lock
 M crates/anvilml-ipc/Cargo.toml
 M crates/anvilml-ipc/src/lib.rs
?? .forge/reports/P2-B2_plan.md
?? crates/anvilml-ipc/src/framing.rs
```

## Acceptance Criteria — Verification

| Criterion | Status | Evidence |
|-----------|--------|----------|
| `write_frame` function exists and serializes WorkerMessage with 4-byte BE u32 length prefix | PASS | `cargo test -p anvilml-ipc roundtrip_ping_pong` |
| `read_frame` function exists and decodes WorkerEvent from framed payload | PASS | `cargo test -p anvilml-ipc roundtrip_ping_pong` |
| Oversize rejection returns PayloadTooLarge without reading payload | PASS | `cargo test -p anvilml-ipc reject_oversize_payload` |
| Both functions use `read_exact` / `write_all` (not partial read/write) | PASS | Source inspection of framing.rs |
| `bytes` dependency added to Cargo.toml | PASS | `grep bytes crates/anvilml-ipc/Cargo.toml` |
| `tokio` (io-util) added as regular dependency | PASS | `grep tokio crates/anvilml-ipc/Cargo.toml` |
| `lib.rs` re-exports `framing::{write_frame, read_frame}` | PASS | `grep -A1 'pub use framing' crates/anvilml-ipc/src/lib.rs` |
| Full workspace test suite exits 0 | PASS | `cargo test --workspace` — 76 tests, 0 failures |
