# Plan Report: P8-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P8-A2                                              |
| Phase       | 008 — IPC Framing                                  |
| Description | anvilml-ipc: write_frame (length-prefixed msgpack) |
| Depends on  | P8-A1                                               |
| Project     | anvilml                                             |
| Planned at  | 2026-06-05T18:32:00Z                               |
| Attempt     | 1                                                   |

## Objective

Add `tokio` (io-util) as a dependency to the `anvilml-ipc` crate and create `src/framing.rs` containing an async `write_frame` function that serializes a `WorkerMessage` via `rmp_serde::to_vec_named`, prepends a 4-byte big-endian `u32` length prefix, and writes the combined header+payload to any `AsyncWrite + Unpin` sink. Ship a unit test exercising the function against a `Vec<u8>` buffer that asserts the first four bytes equal the payload length encoded as big-endian.

## Scope

### In Scope
- Add `tokio` dependency with `io-util` feature to `crates/anvilml-ipc/Cargo.toml`.
- Create `crates/anvilml-ipc/src/framing.rs` with:
  - `pub async fn write_frame<W: AsyncWrite + Unpin>(w: &mut W, msg: &WorkerMessage) -> Result<(), AnvilError>`
  - Serialization via `rmp_serde::to_vec_named(msg)`
  - 4-byte big-endian length prefix construction (`u32::to_be_bytes`)
  - Write header + payload via `AsyncWriteExt::write_all`
- Update `crates/anvilml-ipc/src/lib.rs` to declare `pub mod framing;`.
- Add a unit test module in `framing.rs`:
  - `#[tokio::test] async fn write_frame()` — serializes a `WorkerMessage::Ping { seq: 7 }`, writes via `write_frame` to a `Vec<u8>`, reads the first 4 bytes, decodes as big-endian `u32`, asserts it equals the remaining payload length.
- Update `.forge/state/CURRENT_TASK.md`.

### Out of Scope
- `read_frame` implementation (task P8-A3).
- `ipc-probe` binary (task P8-A4).
- Any changes to `anvilml-core`, `backend/`, or other crates.
- Integration tests involving actual worker processes.
- Size-cap enforcement on writes (the cap applies on the read side per §7.1).
- Feature-flag gating of tokio.

## Approach

1. **Add dependency** — In `crates/anvilml-ipc/Cargo.toml`, add a `[dependencies]` entry for `tokio = { workspace = true, features = ["io-util"] }`. The workspace already declares `tokio = { version = "1.52.3", features = ["full"] }` in `[workspace.dependencies]`; referencing it with `features = ["io-util"]` is additive and compatible (the workspace entry's `"full"` feature set subsumes `"io-util"`).

   **Dependency resolution note:** Per FORGE_AGENT_RULES §6, the workspace dependency for `tokio` already exists at version `1.52.3` with `features = ["full"]`. Since `"full"` includes `"io-util"`, we can safely reference `{ workspace = true }` without additional feature specification. The minimal explicit feature annotation `["io-util"]` documents intent but is functionally redundant given the workspace `"full"` baseline.

2. **Create `framing.rs`** — Write `crates/anvilml-ipc/src/framing.rs`:
   - Imports: `anvilml_core::error::AnvilError`, `async_trait` not needed (async fn in trait not used), `bytes::Bytes` not needed, `std::io::Cursor` for test, `tokio::io::AsyncWriteExt`, `rmp_serde::to_vec_named`.
   - `pub async fn write_frame<W>(w: &mut W, msg: &WorkerMessage) -> Result<(), AnvilError> where W: AsyncWrite + Unpin`:
     a. `let payload = rmp_serde::to_vec_named(msg).map_err(|e| AnvilError::Json(e.to_string()))?;`
     b. `let len = payload.len() as u32;`
     c. `let header = len.to_be_bytes();`
     d. `w.write_all(&header).await.map_err(AnvilError::Io)?;`
     e. `w.write_all(&payload).await.map_err(AnvilError::Io)?;`
     f. `Ok(())`

3. **Update `lib.rs`** — Add `pub mod framing;` to the existing module declarations in `crates/anvilml-ipc/src/lib.rs`. The existing file has:
   ```rust
   pub mod messages;
   pub use messages::{WorkerEvent, WorkerMessage};
   ```
   Append `pub mod framing;` after line 1.

4. **Write tests** — In `framing.rs`, add a `#[cfg(test)]` module with at least one test:
   - `write_frame`: Creates a `Vec<u8>` as the write target, calls `write_frame(&mut buf, &msg)`, verifies `buf.len() == 4 + payload_len`, asserts `buf[0..4] == (payload_len as u32).to_be_bytes()`.
   - Use `#[tokio::test]` for the async test harness.

5. **Verify** — Run `cargo test -p anvilml-ipc -- write_frame` and confirm exit 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-ipc/Cargo.toml` | Add `tokio` dependency with `io-util` feature |
| Create | `crates/anvilml-ipc/src/framing.rs` | New module: `write_frame` function + unit tests |
| Modify | `crates/anvilml-ipc/src/lib.rs` | Declare `pub mod framing;` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-ipc/src/framing.rs` (inline test module) | `write_frame` | Serializes a `WorkerMessage::Ping { seq: 7 }`, writes to `Vec<u8>`, asserts first 4 bytes = payload length as big-endian u32, total buffer size = 4 + payload_len. |

## CI Impact

No CI workflow files are modified. The task only adds a dependency and new source code within the existing `anvilml-ipc` crate. Standard CI gates (format, clippy, tests, cross-check) will apply automatically on merge. The added `tokio` dependency is already declared in `[workspace.dependencies]`, so no lockfile drift is expected beyond the standard dependency resolution.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `rmp_serde::to_vec_named` returns an error for certain `WorkerMessage` variants (e.g., if `JobSettings` lacks proper serde derives) | P8-A1 already verified all variants round-trip via rmp_serde in `messages.rs`; the same serialization path is used. No new risk introduced. |
| `tokio::io::AsyncWriteExt::write_all` on `Vec<u8>` is well-supported but if a future reader encounters an unfamiliar sink, partial writes could be an issue | The task scope is limited to `write_frame` (write-only); the read side with its read-fully loop is P8-A3. For Vec<u8>, `write_all` is trivially correct. |
| Adding tokio to anvilml-ipc introduces a transitive async runtime dependency for downstream crates that may not expect it | Only `anvilml-worker` depends on `anvilml-ipc`, and it already pulls in tokio directly for its pool management. No new top-level runtime dependency is introduced at the workspace level. |
| The 4-byte length prefix overflows if payload exceeds 4 GiB | Practically impossible within the 64 MiB IPC payload cap (enforced on read side, P8-A3). Not a concern for this write-side task. |

## Acceptance Criteria

- [ ] `crates/anvilml-ipc/src/framing.rs` exists and exports `pub async fn write_frame<W: AsyncWrite + Unpin>(w: &mut W, msg: &WorkerMessage) -> Result<(), AnvilError>`
- [ ] `cargo test -p anvilml-ipc -- write_frame` exits 0
- [ ] The test writes a known `WorkerMessage` to a `Vec<u8>`, and asserts the first 4 bytes equal the payload length encoded as big-endian u32
- [ ] `crates/anvilml-ipc/Cargo.toml` contains the `tokio` dependency with `io-util` feature
- [ ] `crates/anvilml-ipc/src/lib.rs` declares `pub mod framing;`
