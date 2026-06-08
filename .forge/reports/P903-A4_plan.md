# Plan Report: P903-A4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P903-A4                                           |
| Phase       | 903 — IPC Transport Rework                        |
| Description | Verify ipc-probe still passes after transport change |
| Depends on  | P902-A1 (prerequisite fix may or may not be applied) |
| Project     | anvilml                                           |
| Planned at  | 2026-06-09T01:15:00Z                              |
| Attempt     | 1                                                 |

## Objective

Ensure `crates/anvilml-ipc/src/bin/ipc-probe.rs` uses the proper `write_frame` API instead of hand-rolling `rmp_serde` serialization. If P902-A1 has already been applied, this is a pure verification task with no code changes. If P902-A1 has not been applied, apply the minimal fix so that `cargo run -p anvilml-ipc --bin ipc-probe` prints `OK seq=7` and exits 0.

## Scope

### In Scope
- Read `crates/anvilml-ipc/src/bin/ipc-probe.rs` to determine if P902-A1 fix is already applied.
- If the probe still uses hand-rolled `rmp_serde::to_vec_named` + manual `write_all` for framing, replace with `write_frame(&mut tx, &WorkerMessage::Ping { seq: 7 }).await?`.
- Ensure required imports are present (`WorkerMessage`, `write_frame`).
- No changes to `framing.rs`, `Cargo.toml`, or any other file.
- Verify the binary compiles and runs correctly.

### Out of Scope
- Any changes to `framing.rs` (transport-agnostic — no framing changes).
- Any changes to `Cargo.toml` (no dependency changes).
- Any changes to `anvilml-worker`, `worker/ipc.py`, or any other crate.
- New tests, new logic, or new public API.
- Socket transport wiring — the in-process `tokio::io::duplex` test is transport-agnostic.

## Approach

1. **Inspect current state.** Read `crates/anvilml-ipc/src/bin/ipc-probe.rs` to check whether it already uses `write_frame` (P902-A1 applied) or still hand-rolls `rmp_serde` serialization (P902-A1 not applied).

2. **Apply fix if needed.** If the probe still uses the old pattern:
   - Add `use anvilml_ipc::WorkerMessage;` to the imports.
   - Add `use anvilml_ipc::framing::write_frame;` to the imports.
   - Replace the four lines of manual serialization (lines 12–16 in the current file):
     ```rust
     // BEFORE:
     let pong = json!({ "_type": "Pong", "seq": 7u64 });
     let payload = rmp_serde::to_vec_named(&pong)?;
     let len = payload.len() as u32;
     tx.write_all(&len.to_be_bytes()).await?;
     tx.write_all(&payload).await?;

     // AFTER:
     write_frame(&mut tx, &WorkerMessage::Ping { seq: 7 }).await?;
     ```
   - Remove the now-unused `use serde_json::json;` import.
   - Remove the now-unused `use tokio::io::AsyncWriteExt;` import (only needed for `write_all`; `write_frame` handles its own writes).

3. **Verify compilation.** Run `cargo check -p anvilml-ipc --bin ipc-probe` to confirm the fix compiles.

4. **Verify execution.** Run `cargo run -p anvilml-ipc --bin ipc-probe` and confirm:
   - Stdout contains `OK seq=7`
   - Exit code is 0

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Read | `crates/anvilml-ipc/src/bin/ipc-probe.rs` | Inspect current state |
| Modify | `crates/anvilml-ipc/src/bin/ipc-probe.rs` | Replace hand-rolled serialization with `write_frame` (only if P902-A1 not yet applied) |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| (none written) | `cargo run -p anvilml-ipc --bin ipc-probe` | End-to-end: framing layer round-trip produces `OK seq=7` and exit 0 |

No new test files are created. The existing `ipc-probe` binary serves as the acceptance test.

## CI Impact

No CI changes required. The `ipc-probe` binary is a development utility, not part of the CI test suite. No CI workflow files, format configuration, or lint configuration are modified.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| P902-A1 already applied — no changes needed | Medium | Low | Plan accounts for this: inspection step determines state before any write. |
| `write_frame` API mismatch — returns `AnvilError` instead of generic error | Low | Medium | `AnvilError` implements `std::error::Error`, so `?` works with `Result<_, Box<dyn std::error::Error>>`. Verified via existing `?` usage pattern in the file. |
| Unused imports left after refactor | Low | Low | Import cleanup is part of the fix step. Compiler will catch unused imports via `cargo check`. |
| `WorkerMessage::Ping` vs `WorkerEvent::Pong` semantic gap | Low | Low | The probe tests framing, not semantics. `write_frame` serializes `Ping`, `read_frame` deserializes `Pong` — this is correct because the framing layer is transport-agnostic and the probe verifies the round-trip works. The existing test already does this (sends Pong, reads Pong). The new version sends Ping via `write_frame` and reads Pong — the framing layer doesn't care about message semantics. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-ipc --bin ipc-probe` exits 0
- [ ] `cargo run -p anvilml-ipc --bin ipc-probe` prints `OK seq=7` to stdout
- [ ] `cargo run -p anvilml-ipc --bin ipc-probe` exits with code 0
- [ ] No changes to any file other than `crates/anvilml-ipc/src/bin/ipc-probe.rs` (if P902-A1 not applied)
