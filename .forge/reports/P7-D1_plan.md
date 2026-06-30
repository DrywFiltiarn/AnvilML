# Plan Report: P7-D1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-D1                                       |
| Phase       | 7 — IPC Foundations                         |
| Description | anvilml-ipc: lib.rs re-export pass, 80-line check |
| Depends on  | P7-B2, P7-C1                                |
| Project     | anvilml                                     |
| Planned at  | 2026-06-30T23:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Verify that `crates/anvilml-ipc/src/lib.rs` already exports the complete public surface
per ANVILML_DESIGN.md §8.4's module layout: `IpcError` (from `error`), `WorkerMessage`
and `WorkerEvent` (from `messages`), `RouterTransport` (from `transport`), and
`EventBroadcaster` (from `ws::broadcaster`). Confirm the file stays within the 80-line
hard cap and that `cargo test -p anvilml-ipc` exits 0. This is a verification-only pass —
no implementation logic is added.

## Scope

### In Scope
- Confirm all six `pub use` re-exports are present in `crates/anvilml-ipc/src/lib.rs`:
  `IpcError`, `WorkerMessage`, `WorkerEvent`, `RouterTransport`, `EventBroadcaster`
- Confirm the four `pub mod` declarations match §8.4 module layout: `error`, `messages`,
  `transport`, `ws`
- Confirm `wc -l crates/anvilml-ipc/src/lib.rs` reports ≤ 80
- Run `cargo test -p anvilml-ipc` and confirm exit 0 (full crate suite)

### Out of Scope
None. This task has an empty `defers_to` field and must implement its full scope. The
task context's phrase "confirm ... are all present" is a verification instruction, not
a deferral — the confirmation happens during this session's work.

## Existing Codebase Assessment

The `anvilml-ipc` crate is already fully implemented across Phase 7 Groups A–C:
- `error.rs` defines `IpcError` with six variants and a `From<IpcError> for AnvilError`
  conversion (53 lines).
- `messages.rs` defines `WorkerMessage` (5 variants) and `WorkerEvent` (9 variants) as
  msgpack-serialisable enums with `#[serde(tag = "_type")]` (211 lines).
- `transport.rs` defines `RouterTransport` with the split-lock `send()`/`recv()` pattern
  that prevents the v3 shutdown deadlock (205 lines).
- `ws/broadcaster.rs` defines `EventBroadcaster` wrapping
  `tokio::sync::broadcast::Sender<WsEvent>` with 1024-event capacity (42 lines).

The existing `lib.rs` is 11 lines and already contains all six `pub use` re-exports and
four `pub mod` declarations matching §8.4 exactly. No changes to `lib.rs` are needed —
the task is a verification pass confirming the existing state is correct.

Established patterns: all public items carry `///` doc comments; `#[tracing::instrument]`
is applied to async I/O functions; error variants use `thiserror` derives. The crate's
tests live in `crates/anvilml-ipc/tests/` (integration test crate style):
`roundtrip_tests.rs` and `error_tests.rs`.

No gap exists between the design doc and current source — `lib.rs` already matches
§8.4's module layout perfectly.

## Resolved Dependencies

No new dependencies are introduced or modified by this task. The existing `Cargo.toml`
dependency on `zeromq` is verified for reference:

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | zeromq  | 0.6.0           | rust-docs MCP  | tokio-runtime, all-transport |

Confirmed types in zeromq 0.6.0: `RouterSocket`, `RouterSendHalf`, `RouterRecvHalf`,
`ZmqMessage`, `Endpoint` — all match the API used in `transport.rs`.

## Approach

This task requires no code changes. The approach is a mechanical verification pass:

1. **Read existing `lib.rs`** (already done at step 3 of inspection). Confirm it contains:
   - `pub mod error;`
   - `pub mod messages;`
   - `pub mod transport;`
   - `pub mod ws;`
   - `pub use error::IpcError;`
   - `pub use messages::{WorkerEvent, WorkerMessage};`
   - `pub use transport::RouterTransport;`
   - `pub use ws::broadcaster::EventBroadcaster;`

2. **Verify each re-export resolves to the correct module-level public type** by
   confirming the target modules exist and export the expected public items:
   - `error.rs` is a `pub mod` that defines `pub enum IpcError` (confirmed by reading)
   - `messages.rs` is a `pub mod` that defines `pub enum WorkerMessage` and
     `pub enum WorkerEvent` (confirmed by reading)
   - `transport.rs` is a `pub mod` that defines `pub struct RouterTransport` (confirmed)
   - `ws/mod.rs` is a `pub mod` that re-exports `broadcaster` as `pub mod broadcaster`
     (confirmed by reading); `broadcaster.rs` defines `pub struct EventBroadcaster`
     (confirmed by reading)

3. **Run `wc -l crates/anvilml-ipc/src/lib.rs`** and confirm the output is ≤ 80.
   Current line count is 11 — well within the cap.

4. **Run `cargo test -p anvilml-ipc`** (full crate suite including both
   `roundtrip_tests.rs` and `error_tests.rs`) and confirm exit 0.

5. **Phase-closing audit** (P7-D1 is the last task in Phase 7):

   **§9a procedure** (defers_to verification for tasks with non-empty defers_to):
   No task in Phase 7 has a non-empty `defers_to` field. All eight tasks (P7-A1
   through P7-D1) declare `"defers_to": []`. Zero entries to verify.

   **§9a.1 Unmarked-stub sweep:**
   ```bash
   grep -rn "NotImplementedError\|unimplemented!\|todo!\|# TODO\|// TODO" \
     crates/anvilml-ipc/src/
   ```
   Result: `0 findings` (grep exit code 1 = no matches). No unmarked stubs.

   **§9a.2 Dual-mode parity-marker sweep:**
   The REAL_PATH_VERIFIED/MOCK_PATH_VERIFIED convention (ANVILML_DESIGN.md §10.6)
   applies only to Python worker node functions in `worker/nodes/` — specifically
   `execute()`, `load()`, `sample()`, `decode()`, and `compute_latent_shape()` methods.
   The `anvilml-ipc` crate contains no Python code and no functions in scope of this
   convention. Zero findings.

## Public API Surface

No new public items are introduced. The existing public API surface (already declared
by prior tasks) is:

| Item | Crate/Module Path | Declared By |
|------|-------------------|-------------|
| `pub enum IpcError` | `anvilml_ipc::error` | P7-A1 |
| `pub enum WorkerMessage` | `anvilml_ipc::messages` | P7-A2 |
| `pub enum WorkerEvent` | `anvilml_ipc::messages` | P7-A3, P7-A4 |
| `pub struct RouterTransport` | `anvilml_ipc::transport` | P7-B1, P7-B2 |
| `pub struct EventBroadcaster` | `anvilml_ipc::ws::broadcaster` | P7-C1 |

All five are re-exported at the crate root via `pub use` in `lib.rs`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Read | crates/anvilml-ipc/src/lib.rs | Verify re-exports and line count (no changes) |

No files are created or modified. This task is a verification pass only.

## Tests

This task does not introduce new tests — it verifies the existing test suite. The
acceptance criterion is that the full crate test suite exits 0:

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| crates/anvilml-ipc/tests/error_tests.rs | All 6+ error display + From conversion tests | IpcError variants' Display output and AnvilError conversion | `cargo test -p anvilml-ipc --test error_tests` exits 0 |
| crates/anvilml-ipc/tests/roundtrip_tests.rs | All roundtrip + EventBroadcaster tests | msgpack roundtrip for all message variants; publish/subscribe behaviour | `cargo test -p anvilml-ipc --test roundtrip_tests` exits 0 |

Acceptance command: `cargo test -p anvilml-ipc` exits 0 (runs both test files).

## CI Impact

No CI changes required. No new files, no new gates, no new test modules are added.
The existing CI job `rust-linux` already runs `cargo test --workspace --features
mock-hardware` which includes `anvilml-ipc`.

## Platform Considerations

None identified. The `lib.rs` file contains only re-exports and module declarations —
no platform-specific code, no `#[cfg(unix)]` or `#[cfg(windows)]` guards, no path
handling. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| A prior task's `lib.rs` re-export was later removed or renamed by a subsequent refactor, leaving the current lib.rs incomplete | Low | Medium | The verification step reads the actual file and confirms each `pub use` resolves to a real public type in the corresponding module. If any re-export is missing, the task flags it. |
| `cargo test -p anvilml-ipc` fails due to a pre-existing defect in a prior task's code (not introduced by this task) | Low | Medium | Per FORGE_AGENT_RULES §9.4, a pre-existing error in files this task does not modify is a blocker. The task would STOP and write a blocker rather than proceed. |

## Acceptance Criteria

- [ ] `wc -l crates/anvilml-ipc/src/lib.rs` reports a number ≤ 80
- [ ] `grep -c "pub use" crates/anvilml-ipc/src/lib.rs` returns exactly 4 (IpcError, WorkerMessage+WorkerEvent as one line, RouterTransport, EventBroadcaster)
- [ ] `grep -c "pub mod" crates/anvilml-ipc/src/lib.rs` returns exactly 4 (error, messages, transport, ws)
- [ ] `cargo test -p anvilml-ipc` exits 0
