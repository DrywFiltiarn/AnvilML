# Plan Report: P2-B2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P2-B2                                       |
| Phase       | 002 — Core Types & IPC                      |
| Description | anvilml-ipc: length-prefixed msgpack framing |
| Depends on  | P2-B1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-05-30T08:14:52Z                        |
| Attempt     | 1                                           |

## Objective

Implement the async length-prefixed msgpack framing layer for the `anvilml-ipc` crate. This provides two functions — `write_frame` and `read_frame` — that wrap raw async I/O with the protocol defined in `ANVILML_DESIGN.md §7.1`: a 4-byte big-endian u32 length prefix followed by N bytes of msgpack-encoded payload. The framing enforces a configurable maximum payload size (in MiB) before any heap allocation, and uses `read_exact` / `write_all` to guarantee full reads/writes on all platforms including Windows where pipe reads are frequently partial.

## Scope

### In Scope
- Create `crates/anvilml-ipc/src/framing.rs` with two async functions:
  - `write_frame<W: AsyncWrite + Unpin>(writer: &mut W, msg: &WorkerMessage) -> Result<(), AnvilError>`
  - `read_frame<R: AsyncRead + Unpin>(reader: &mut R, max_mib: u32) -> Result<WorkerEvent, AnvilError>`
- Add `bytes` dependency to `anvilml-ipc/Cargo.toml`
- Add `tokio` (features: io-util) as a regular dependency of `anvilml-ipc/Cargo.toml` (dev-dependencies already include tokio)
- Update `crates/anvilml-ipc/src/lib.rs` to re-export `framing::{write_frame, read_frame}`
- Write two unit tests in `framing.rs` using `tokio::io::duplex`:
  - Round-trip test: write a `WorkerMessage::Ping { seq: 1 }`, read back as `WorkerEvent::Pong { seq: 1 }`
  - Oversize-rejection test: frame with length header encoding 65 MiB + 1, assert `AnvilError::PayloadTooLarge` without reading payload
- Both functions use `read_exact` / `write_all` (not `read` / `write`) to satisfy the cross-platform partial-read invariant

### Out of Scope
- Any worker spawn/supervise logic (`anvilml-worker` crate)
- TCP/UDS socket framing — only stdin/stdout pipe framing
- Serialization format changes (msgpack named-map already established in P2-B1)
- Any HTTP or WebSocket framing
- Changes to `anvilml-core`, `anvilml-hardware`, `anvilml-registry`, `anvilml-scheduler`, `anvilml-server`
- CI workflow modifications (no new jobs or steps needed)

## Approach

1. **Update `anvilml-ipc/Cargo.toml`**: Add `tokio = { version = "1", features = ["io-util"] }` as a regular dependency and `bytes = "1"`. The `bytes` crate is used for efficient buffer construction (e.g., `BytesMut`) in the framing functions. Tokio's `io-util` feature provides `AsyncReadExt::read_exact`, `AsyncWriteExt::write_all`, and the `AsyncRead` / `AsyncWrite` trait bounds.

2. **Create `crates/anvilml-ipc/src/framing.rs`**:
   - Import `tokio::io::{AsyncRead, AsyncWrite}` traits, `bytes::BytesMut`, and `rmp_serde`.
   - Implement `write_frame`: serialize `msg` to msgpack via `rmp_serde::to_vec_named`, compute the 4-byte big-endian length prefix using `u32::to_be_bytes()`, write the header then the payload via `write_all` (not `write`). Return `AnvilError::Io` on I/O failure.
   - Implement `read_frame`: read exactly 4 bytes via `read_exact`, decode as big-endian u32, check `payload_len <= max_mib * 1024 * 1024` before any allocation (return `AnvilError::PayloadTooLarge { size_mib: payload_len / (1024*1024), limit_mib: max_mib }` if exceeded), then read exactly `payload_len` bytes via `read_exact`, deserialize as `WorkerEvent` via `rmp_serde::from_slice` (return `AnvilError::Io` on I/O or deserialization failure).
   - Both functions return `Result<_, AnvilError>` to match the error handling convention of all AnvilML crates.

3. **Update `crates/anvilml-ipc/src/lib.rs`**: Add `pub mod framing;` and `pub use framing::{read_frame, write_frame};` alongside the existing `messages` re-exports.

4. **Write tests in `framing.rs`** under `#[cfg(test)] mod tests`:  
   - Use `tokio::runtime::Builder::new_current_thread().build().unwrap()` to create a test runtime (or `#[tokio::test]` if the dev-dependency already includes `rt`).  
   - Use `tokio::io::duplex(4096)` to create an in-memory duplex pipe for both read and write sides.  
   - **Round-trip test**: construct a `WorkerMessage::Ping { seq: 1 }`, call `write_frame(&mut tx, &msg)`, then call `read_frame(&mut rx, 64)` and assert the result is `Ok(WorkerEvent::Pong { seq: 1 })`.  
   - **Oversize-rejection test**: construct a 4-byte header encoding `65 * 1024 * 1024 + 1` (exceeds default 64 MiB), write only the header to the duplex channel, then call `read_frame(&mut rx, 64)` and assert it returns `Err(AnvilError::PayloadTooLarge { .. })`. Crucially, the function must reject *before* attempting to allocate a buffer for the missing payload.

5. **Verify**: Run `cargo test -p anvilml-ipc -- framing` to confirm both tests pass with exit code 0. Also run `cargo test -p anvilml-ipc` to ensure no regressions in the existing `messages` module tests.

## Files Affected

| Action   | Path                              | Description                                      |
|----------|-----------------------------------|--------------------------------------------------|
| MODIFY   | crates/anvilml-ipc/Cargo.toml     | Add `tokio` (io-util) and `bytes` dependencies   |
| CREATE   | crates/anvilml-ipc/src/framing.rs | Async read_frame/write_frame + unit tests        |
| MODIFY   | crates/anvilml-ipc/src/lib.rs     | Re-export framing module and its two functions   |

## Tests

| Test ID / Name            | File                              | Validates                                      |
|---------------------------|-----------------------------------|------------------------------------------------|
| `framing_roundtrip_ping`  | crates/anvilml-ipc/src/framing.rs | write_frame + read_frame round-trip via duplex  |
| `framing_oversize_reject` | crates/anvilml-ipc/src/framing.rs | Oversize header rejected before payload alloc   |

## CI Impact

No CI changes required. The existing CI matrix already runs `cargo test -p anvilml-ipc --features mock-hardware` on both Linux and Windows, which will automatically include the new framing tests. No new jobs or steps need to be added.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| `bytes` crate adds unnecessary dependency weight | Low | Low | `bytes::BytesMut` is lightweight (~24 bytes per instance) and idiomatic for async I/O in the Tokio ecosystem. The overhead is negligible compared to msgpack payloads. |
| `read_exact` on duplex may panic on EOF instead of returning an error | Low | Medium | `tokio::io::AsyncReadExt::read_exact` returns `ErrorKind::UnexpectedEof` on short reads — this is caught by the `?` operator and converted to `AnvilError::Io`, which is the correct behavior. |
| Oversize check uses integer division for MiB conversion, potentially under-reporting size | Low | Low | The check compares raw byte count against `max_mib * 1024 * 1024` (exact multiplication), not MiB division. The error message uses division for display only. |
| Tokio version mismatch between regular and dev-dependencies | Low | Medium | Pin the same major version (`tokio = { version = "1", ... }`) in both sections to ensure compatibility. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-ipc -- framing` exits 0 with both `framing_roundtrip_ping` and `framing_oversize_reject` tests passing
- [ ] `cargo test -p anvilml-ipc` exits 0 (no regression in existing messages module tests)
- [ ] `framing.rs` uses `read_exact` for all reads and `write_all` for all writes (verified by code inspection)
- [ ] Oversize check (`payload_len <= max_mib * 1024 * 1024`) occurs before any heap allocation for the payload buffer
- [ ] `Cargo.toml` includes `tokio` (features: io-util) as a regular dependency and `bytes` as a dependency
- [ ] `lib.rs` re-exports `framing::{write_frame, read_frame}`
