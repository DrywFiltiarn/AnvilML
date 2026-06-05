# Plan Report: P900-A7

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P900-A7                                     |
| Phase       | 900 — Logging Retrofit                      |
| Description | anvilml-ipc: retrofit WARN/ERROR logging to framing.rs error paths |
| Depends on  | none (P900-A6 is independent)               |
| Project     | anvilml                                     |
| Planned at  | 2026-06-06T00:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Add `tracing` dependency to the `anvilml-ipc` crate and instrument its two public framing functions (`read_frame`, `write_frame`) with WARN/ERROR log calls at every error-returning code path, per FORGE_AGENT_RULES §11.1 and §11.2. No logic changes — logging only.

## Scope

### In Scope
- Add `tracing = { workspace = true }` to `crates/anvilml-ipc/Cargo.toml` `[dependencies]`
- In `read_frame`: add `tracing::warn!` before returning `AnvilError::PayloadTooLarge`, with `payload_mib` and `limit_mib` structured fields
- In `read_frame`: add `tracing::error!` on `rmp_serde::from_slice` deserialization failure, with `error=%e` structured field
- In `write_frame`: add `tracing::error!` on any write error (header or payload), with `error=%e` structured field
- Verify `cargo test -p anvilml-ipc` exits 0

### Out of Scope
- Per-frame DEBUG logging for every sent/received frame (belongs in Phase 009, P9-A4)
- Any changes to `anvilml-ipc/src/messages.rs`
- Changes to other crates' error paths
- Adding new tests (none required by this task)
- CI workflow file changes
- Platform cross-checks beyond the minimal compile check specified in acceptance criteria

## Approach

1. **Add tracing dependency.** Append `tracing = { workspace = true }` to `[dependencies]` in `crates/anvilml-ipc/Cargo.toml`. The workspace defines `tracing = "0.1.44"` in `[workspace.dependencies]` (confirmed at line 37 of root `Cargo.toml`).

2. **Instrument `read_frame` PayloadTooLarge path.** In `framing.rs`, before the `return Err(AnvilError::PayloadTooLarge(...))` on line 46, insert:
   ```rust
   tracing::warn!(payload_mib = payload_len / 1024 / 1024, limit_mib = max_mib, "IPC frame rejected: payload too large");
   ```
   where `payload_len` is the variable holding `len as u64` (already in scope on line 45).

3. **Instrument `read_frame` deserialize path.** On line 57–58, change the `.map_err(|e| AnvilError::Json(e.to_string()))?` to first log then error:
   ```rust
   let event = rmp_serde::from_slice::<WorkerEvent>(&payload).map_err(|e| {
       tracing::error!(error = %e, "IPC frame deserialize failed");
       AnvilError::Json(e.to_string())
   })?;
   ```

4. **Instrument `write_frame` error paths.** On lines 15 and 18–19 of `write_frame`, add logging before each `.map_err(...)`. Specifically:
   - Line 15 (msgpack serialization): the error is already caught by `.map_err`; wrap with a log:
     ```rust
     let payload = rmp_serde::to_vec_named(msg).map_err(|e| {
         tracing::error!(error = %e, "IPC frame write failed");
         AnvilError::Json(e.to_string())
     })?;
     ```
   - Lines 18–19 (write_all header and payload): similarly wrap:
     ```rust
     w.write_all(&header).await.map_err(|e| {
         tracing::error!(error = %e, "IPC frame write failed");
         AnvilError::Io(e)
     })?;
     w.write_all(&payload).await.map_err(|e| {
         tracing::error!(error = %e, "IPC frame write failed");
         AnvilError::Io(e)
     })?;
     ```

5. **Verify tests.** Run `cargo test -p anvilml-ipc` and confirm exit code 0 with no regressions. The existing test `read_frame_oversize_rejected` asserts on the error message string but does not assert log output, so it will continue to pass.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-ipc/Cargo.toml` | Add `tracing = { workspace = true }` to `[dependencies]` |
| Modify | `crates/anvilml-ipc/src/framing.rs` | Add WARN on PayloadTooLarge, ERROR on deserialize failure, ERROR on write failures |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-ipc/src/framing.rs` (mod tests) | `write_frame` | Frame serialization + header roundtrip still works |
| `crates/anvilml-ipc/src/framing.rs` (mod tests) | `write_frame_sync_serialization` | Msgpack serialization outside async context |
| `crates/anvilml-ipc/src/framing.rs` (mod tests) | `write_frame_shutdown` | Shutdown message frame layout |
| `crates/anvilml-ipc/src/framing.rs` (mod tests) | `write_frame_execute` | Execute message with job_id/graph/settings frame layout |
| `crates/anvilml-ipc/src/framing.rs` (mod tests) | `read_frame_roundtrip` | Pong event roundtrip through duplex pipe |
| `crates/anvilml-ipc/src/framing.rs` (mod tests) | `read_frame_oversize_rejected` | Oversized header rejected with PayloadTooLarge error |

No new test files are required — the task adds logging only, and all existing tests continue to pass without modification.

## CI Impact

No CI workflow files are modified. The change is purely additive (a dependency + log calls). All existing CI gates for `anvilml-ipc` will pass since no logic or test behavior changes. The workspace-level `cargo clippy --workspace --features mock-hardware` and `cargo test --workspace --features mock-hardware` commands will also pass because `tracing` is already a well-established dependency in the workspace used by other crates.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Log calls alter test output (e.g. tracing subscriber captures logs in tests) | Low | Medium | The `anvilml-ipc` crate has no `tracing` subscriber configured in its tests; `tracing::warn!`/`tracing::error!` calls are no-ops when no subscriber is active. If any test output changes, verify the test still asserts correctly. |
| `write_frame` error wrapping duplicates error messages (e.g. "IPC frame write failed" + existing `AnvilError::Io(e).to_string()`) | Medium | Low | The task description explicitly specifies this pattern. The error message is human-readable and the structured `error=` field provides the raw value for aggregators. Acceptable per §11.2 ERROR convention. |
| `payload_mib` integer division truncation produces 0 for small payloads near the limit boundary | Low | Negligible | `payload_len` at this point is guaranteed > `max_bytes`, so `payload_len / 1024 / 1024` will always be ≥ `max_mib`. The warn message correctly signals an oversized frame. |

## Acceptance Criteria

- [ ] `tracing = { workspace = true }` present in `crates/anvilml-ipc/Cargo.toml` `[dependencies]`
- [ ] `tracing::warn!` with `payload_mib=` and `limit_mib=` emitted before PayloadTooLarge return in `read_frame`
- [ ] `tracing::error!` with `error=%e` emitted on deserialize failure in `read_frame`
- [ ] `tracing::error!` with `error=%e` emitted on write errors in `write_frame`
- [ ] `cargo test -p anvilml-ipc` exits 0
