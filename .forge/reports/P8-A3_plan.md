# Plan Report: P8-A3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P8-A3                                             |
| Phase       | 008 — IPC Framing                                 |
| Description | anvilml-ipc: read_frame with size cap and read-fully loop |
| Depends on  | P8-A1, P8-A2                                      |
| Project     | anvilml                                           |
| Planned at  | 2026-06-05T19:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add `read_frame` to `crates/anvilml-ipc/src/framing.rs`: an async function that reads a length-prefixed msgpack frame from any `AsyncRead + Unpin` source, enforces a configurable size cap (`max_mib`) before allocating the payload buffer, and deserialises the payload into a `WorkerEvent`. The implementation must use `tokio::io::AsyncReadExt::read_exact` for both the 4-byte header and the N-byte payload — which internally loops until all bytes are received, satisfying the Windows pipe partial-read requirement documented in ANVILML_DESIGN.md §7.1.

## Scope

### In Scope
- Add `async fn read_frame<R: AsyncRead + Unpin>(r: &mut R, max_mib: u32) -> Result<WorkerEvent, AnvilError>` to `framing.rs`
- Import `tokio::io::AsyncReadExt` (for `read_exact`) alongside the existing `tokio::io::AsyncWriteExt` import
- Size-cap check: if decoded length N > max_mib × 1024 × 1024, return `AnvilError::PayloadTooLarge(...)` before any payload allocation
- Use `read_exact` for the 4-byte header and again for the N-byte payload
- Deserialize via `rmp_serde::from_slice` on the fully-read buffer
- Add tests in `framing.rs`:
  - `read_frame_roundtrip`: write a `Ping{seq:7}` frame via `tokio::io::duplex`, read it back, verify `WorkerEvent::Pong { seq: 7 }`
  - `read_frame_oversize_rejected`: construct a buffer with a 4-byte header claiming an oversized length (e.g. 1 GiB) but no payload bytes; verify `PayloadTooLarge` is returned immediately without blocking on missing data
- No changes to any file outside `framing.rs`

### Out of Scope
- Modifying `messages.rs`, `lib.rs`, or any other crate file
- Adding a new binary target (that is P8-A4)
- Changing Cargo.toml dependencies (all needed crates — tokio, rmp-serde, anvilml-core — are already present)
- Windows cross-compilation changes beyond what the existing feature-flag setup already supports

## Approach

1. **Import `AsyncReadExt`** in `framing.rs`: add `use tokio::io::{AsyncRead, AsyncReadExt};` (keeping the existing `AsyncWrite`/`AsyncWriteExt` imports on their own line).

2. **Implement `read_frame`**:
   ```rust
   pub async fn read_frame<R>(r: &mut R, max_mib: u32) -> Result<WorkerEvent, AnvilError>
   where
       R: AsyncRead + Unpin,
   {
       // 1. Read exactly 4 bytes for the length header.
       let mut header = [0u8; 4];
       r.read_exact(&mut header).await?;

       // 2. Decode big-endian u32 payload length.
       let len = u32::from_be_bytes(header);

       // 3. Enforce size cap BEFORE allocating the payload buffer.
       let max_bytes = (max_mib as u64) * 1024 * 1024;
       if len as u64 > max_bytes {
           return Err(AnvilError::PayloadTooLarge(format!(
               "frame length {} exceeds limit {} MiB",
               len, max_mib
           )));
       }

       // 4. Allocate and read exactly N payload bytes.
       let mut payload = vec![0u8; len as usize];
       r.read_exact(&mut payload).await?;

       // 5. Deserialize msgpack → WorkerEvent.
       let event = rmp_serde::from_slice::<WorkerEvent>(&payload)
           .map_err(|e| AnvilError::Json(e.to_string()))?;

       Ok(event)
   }
   ```
   Key design decisions:
   - `read_exact` is used for both header and payload. Tokio's `read_exact` internally loops on short reads, which is the critical Windows pipe correctness guarantee (§7.1).
   - The size cap is checked **before** `vec![0u8; len]`, preventing a malicious tiny header from triggering gigabyte-scale allocation.
   - Error mapping: I/O errors propagate via `?` (auto-converted by `From<std::io::Error>`); deserialisation failures map to `AnvilError::Json`.

3. **Add test `read_frame_roundtrip`**:
   - Create a tokio duplex pair (`tokio::io::duplex(4096)`).
   - In one direction, use `write_frame` (already tested in P8-A2) to write a `WorkerMessage::Ping { seq: 7 }`.
   - In the other direction, call `read_frame` with `max_mib = 64`.
   - Assert the result is `Ok(WorkerEvent::Pong { seq: 7 })`.

4. **Add test `read_frame_oversize_rejected`**:
   - Construct a buffer containing only a 4-byte header claiming an oversized length (e.g. `0xFFFFFFFF` = ~4 GiB) with zero payload bytes.
   - Wrap it in `tokio::io::Cursor::new` and call `read_frame` with `max_mib = 64`.
   - Assert the result is `Err(AnvilError::PayloadTooLarge(_))`.

5. **Verify**: run `cargo test -p anvilml-ipc -- read_frame` — all tests must exit 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Edit | `crates/anvilml-ipc/src/framing.rs` | Add `AsyncReadExt` import; add `read_frame` function; add two unit tests |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-ipc/src/framing.rs` | `read_frame_roundtrip` | Full round-trip: write a `Ping{seq:7}` frame via duplex, read back `Pong{seq:7}`, confirming header decode, payload read, and msgpack deserialisation all work end-to-end. |
| `crates/anvilml-ipc/src/framing.rs` | `read_frame_oversize_rejected` | A header claiming ~4 GiB with no payload returns `PayloadTooLarge` immediately without blocking or allocating a large buffer. |

## CI Impact

No CI workflow files are modified. The existing CI gates (format, clippy, tests, platform cross-checks) already cover this crate. The task requires the additional cross-check `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` to pass, which is one of the documented platform cross-check commands in ENVIRONMENT.md §Platform Cross-Check.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `read_exact` on duplex might not behave as expected in tests | Tokio's `duplex` is a well-tested in-memory pipe; `read_exact` is the standard idiom used throughout the tokio ecosystem. The test uses a small buffer (4096 bytes) which is more than enough for a Ping frame (~80 bytes). |
| Oversize header test could hang if cap check is misplaced | The spec mandates checking the cap BEFORE allocating `vec![0u8; len]`. The implementation checks `len as u64 > max_bytes` and returns early, so no read of payload bytes occurs. The test constructs a zero-length payload buffer to confirm this. |
| rmp-serde deserialisation could fail on unexpected event type | `rmp_serde::from_slice::<WorkerEvent>` will return an error for malformed or unknown msgpack data, mapped to `AnvilError::Json`. This is the correct behaviour — a desynced stream should be caught and handled upstream (worker kill + respawn per §7.1). |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-ipc -- read_frame` exits 0 with all tests passing
- [ ] `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0
- [ ] `read_frame` rejects frames with header length > `max_mib * 1024 * 1024` before allocating the payload buffer
- [ ] `read_frame` successfully round-trips a `Ping{seq:7}` → `Pong{seq:7}` via `tokio::io::duplex`
- [ ] No files other than `crates/anvilml-ipc/src/framing.rs` are modified
