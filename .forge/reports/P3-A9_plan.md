# Plan Report: P3-A9

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-A9                                       |
| Phase       | 003 — Core Domain Types: Data Model         |
| Description | anvilml-core: WsEvent worker/system/provisioning variants |
| Depends on  | P3-A8                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-28T20:18:00Z                        |
| Attempt     | 1                                           |

## Objective

Extend the `WsEvent` enum in `crates/anvilml-core/src/types/events.rs` with three system-level event variants (`WorkerStatusChanged`, `SystemStats`, `ProvisioningProgress`) that P3-A8 deferred. These variants complete the full ten-variant event vocabulary the `/v1/events` WebSocket broadcaster will emit. The task also adds three serde roundtrip tests to `tests/events_tests.rs`, bringing the total to at least ten.

## Scope

### In Scope
- Add `WorkerStatusChanged{worker_id: String, status: WorkerStatus, device_index: u32}` variant to `WsEvent` enum.
- Add `SystemStats{cpu_pct: f32, ram_used_mib: u64, workers: Vec<WorkerInfo>}` variant to `WsEvent` enum.
- Add `ProvisioningProgress{message: String, pct: u8}` variant to `WsEvent` enum.
- Import `WorkerStatus` and `WorkerInfo` from `super::worker` (not redefine them locally).
- Add three serde roundtrip tests in `tests/events_tests.rs` for the new variants.
- Update the module-level doc comment on `WsEvent` to reflect ten variants instead of seven.

### Out of Scope
None. `defers_to (from JSON): []` — this task implements its full scope without deferring any functionality.

## Existing Codebase Assessment

**What already exists:** `crates/anvilml-core/src/types/events.rs` contains the `WsEvent` enum with seven job-lifecycle variants (`JobQueued`, `JobStarted`, `JobProgress`, `JobImageReady`, `JobCompleted`, `JobFailed`, `JobCancelled`), all using `#[serde(tag = "type", rename_all = "snake_case")]`. The enum derives `Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema`. The `crates/anvilml-core/src/types/worker.rs` file already defines `WorkerStatus` (enum with five variants) and `WorkerInfo` (struct with six fields), both `pub` and re-exported via `types/mod.rs`.

**Established patterns:** Each `WsEvent` variant uses named struct fields with `///` doc comments. The test file `tests/events_tests.rs` has one roundtrip test per variant, each constructing the variant, serialising to JSON, asserting the `"type"` key equals the snake_case variant name, deserialising back, and verifying equality. All tests use a fixed Uuid (`550e8400-e29b-41d4-a716-446655440000`). The crate currently has version `0.1.14`.

**Gap between design doc and current source:** None. P3-A8 has already created the seven-job-variant enum with the correct serde attributes. P3-A6 has already defined `WorkerStatus` and `WorkerInfo`. The design doc's event table specifies the exact field shapes for the three deferred variants, and they match what is needed here.

## Resolved Dependencies

None. This task only adds enum variants that reference existing types (`WorkerStatus`, `WorkerInfo`) from the same crate. No new external dependencies are introduced.

| Type   | Name | Version verified | MCP source | Feature flags confirmed |
|--------|------|-----------------|------------|------------------------|
| (none) | —    | —               | —          | —                      |

## Approach

1. **Add imports to `events.rs`.** At the top of `crates/anvilml-core/src/types/events.rs`, after the existing `use` block, add:
   ```rust
   use super::worker::{WorkerInfo, WorkerStatus};
   ```
   This imports the two types defined in P3-A6's `worker.rs` without redefining them. The `super::worker` path resolves because `events.rs` is inside `types/` alongside `worker.rs`.

2. **Add three variants to the `WsEvent` enum.** Append the following three variants to the enum body (after `JobCancelled`), maintaining the existing doc-comment style:
   ```rust
   /// A worker's lifecycle status has changed.
   ///
   /// Emitted by the scheduler when a worker transitions between states
   /// (e.g. Idle → Busy, Busy → Idle, Idle → Dead).
   WorkerStatusChanged {
       /// Identifier of the worker whose status changed.
       worker_id: String,
       /// The new lifecycle state.
       status: WorkerStatus,
       /// Zero-based device index of the worker.
       device_index: u32,
   },

   /// Periodic system health report broadcast to all WebSocket subscribers.
   ///
   /// Emitted on a fixed interval (every 5s) by the server's stats tick.
   SystemStats {
       /// CPU utilisation percentage (0.0–100.0).
       cpu_pct: f32,
       /// Resident memory used by the server process, in MiB.
       ram_used_mib: u64,
       /// Current state of all tracked workers.
       workers: Vec<WorkerInfo>,
   },

   /// An update on the provisioning progress for a worker.
   ///
   /// Emitted while the provisioning subsystem installs or verifies
   /// Python dependencies for a worker.
   ProvisioningProgress {
       /// Human-readable progress message.
       message: String,
       /// Completion percentage (0–100).
       pct: u8,
   },
   ```
   **Rationale:** `SystemStats.workers` uses `Vec<WorkerInfo>` (not `HashMap`) because the broadcast channel sends events to all subscribers and the simplest representation is an ordered list; the consumer can build a map if needed. The field names match the design doc exactly.

3. **Update the module-level doc comment.** Change "The seven variants cover the job states..." to "The ten variants cover the job states and system-level events..." in the module-level `//!` comment at the top of `events.rs`.

4. **Add three serde roundtrip tests to `tests/events_tests.rs`.** Follow the exact pattern of existing tests:
   - `test_ws_event_worker_status_changed_serde_roundtrip()` — constructs `WorkerStatusChanged { worker_id: "gpu:0".to_string(), status: WorkerStatus::Busy, device_index: 0 }`, serialises, asserts `type == "worker_status_changed"`, roundtrips.
   - `test_ws_event_system_stats_serde_roundtrip()` — constructs `SystemStats { cpu_pct: 45.5, ram_used_mib: 512, workers: vec![WorkerInfo { worker_id: "0".to_string(), status: WorkerStatus::Idle, device_index: 0, device_type: DeviceType::Cpu, pid: None, current_job_id: None }] }`, serialises, asserts `type == "system_stats"`, roundtrips.
   - `test_ws_event_provisioning_progress_serde_roundtrip()` — constructs `ProvisioningProgress { message: "Installing torch".to_string(), pct: 50 }`, serialises, asserts `type == "provisioning_progress"`, roundtrips.

   **Rationale:** The `SystemStats` test includes a minimal `WorkerInfo` in the `workers` vec to verify that nested serialisation of the imported type works correctly — this catches any `ToSchema` or serde derive mismatch between `WsEvent` and its embedded `WorkerInfo` type.

## Public API Surface

| Item | Crate/Module Path | Description |
|------|-------------------|-------------|
| `WsEvent::WorkerStatusChanged` | `anvilml_core::types::events::WsEvent` | New enum variant with fields `worker_id: String`, `status: WorkerStatus`, `device_index: u32` |
| `WsEvent::SystemStats` | `anvilml_core::types::events::WsEvent` | New enum variant with fields `cpu_pct: f32`, `ram_used_mib: u64`, `workers: Vec<WorkerInfo>` |
| `WsEvent::ProvisioningProgress` | `anvilml_core::types::events::WsEvent` | New enum variant with fields `message: String`, `pct: u8` |

No new `pub fn`, `pub struct`, or `pub trait` items. The three new enum variants are `pub` by virtue of `WsEvent` being `pub`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/events.rs` | Add three `WsEvent` variants and update module doc comment |
| Modify | `crates/anvilml-core/tests/events_tests.rs` | Add three serde roundtrip tests |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-core/tests/events_tests.rs` | `test_ws_event_worker_status_changed_serde_roundtrip` | `WorkerStatusChanged` serialises with `"type": "worker_status_changed"`, all fields roundtrip correctly | `cargo test -p anvilml-core --test events_tests -- test_ws_event_worker_status_changed_serde_roundtrip` exits 0 |
| `crates/anvilml-core/tests/events_tests.rs` | `test_ws_event_system_stats_serde_roundtrip` | `SystemStats` serialises with `"type": "system_stats"`, nested `WorkerInfo` in `workers` vec roundtrips correctly | `cargo test -p anvilml-core --test events_tests -- test_ws_event_system_stats_serde_roundtrip` exits 0 |
| `crates/anvilml-core/tests/events_tests.rs` | `test_ws_event_provisioning_progress_serde_roundtrip` | `ProvisioningProgress` serialises with `"type": "provisioning_progress"`, all fields roundtrip correctly | `cargo test -p anvilml-core --test events_tests -- test_ws_event_provisioning_progress_serde_roundtrip` exits 0 |

## CI Impact

No CI changes required. The task only adds enum variants and tests within the existing `anvilml-core` crate. The existing `rust-linux` and `rust-windows` CI jobs already run `cargo test --workspace --features mock-hardware`, which includes `anvilml-core`'s test suite. No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The task adds pure data types with no platform-specific code, no `#[cfg(unix)]`/`#[cfg(windows)]` guards, and no path-separator or line-ending handling. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `WorkerStatus` and `WorkerInfo` are already `pub` in `types::worker` and re-exported via `types/mod.rs` — but if the `use super::worker::{WorkerInfo, WorkerStatus}` import path is wrong, compilation fails. | Low | High | The path `super::worker` is correct because `events.rs` and `worker.rs` are sibling modules under `types/`. Verified by reading `types/mod.rs` which declares `pub mod worker;` alongside `pub mod events;`. |
| Adding variants to `WsEvent` changes the `ToSchema` output for OpenAPI, potentially causing the `openapi-drift` CI gate to fail. | Low | Medium | The `openapi-drift` gate is only triggered when handler function signatures or `ToSchema` derives change (ENVIRONMENT.md §8, Gate 2). Adding enum variants to an existing `ToSchema` type is not a handler-level change, but if the gate triggers, the ACT agent must regenerate `api/openapi.json` and stage it. |
| The `SystemStats` test's nested `WorkerInfo` construction may fail to compile if `DeviceType` is not imported in the test file. | Low | Medium | The test file already imports `anvilml_core::types::WsEvent` via `pub use`. `WorkerInfo` and `DeviceType` are also in the same `types::` module, so the test can reference them as `anvilml_core::types::WorkerInfo` and `anvilml_core::types::DeviceType`. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core --test events_tests` exits 0 (≥10 tests total in file)
- [ ] `cargo test -p anvilml-core --test events_tests -- test_ws_event_worker_status_changed_serde_roundtrip` exits 0
- [ ] `cargo test -p anvilml-core --test events_tests -- test_ws_event_system_stats_serde_roundtrip` exits 0
- [ ] `cargo test -p anvilml-core --test events_tests -- test_ws_event_provisioning_progress_serde_roundtrip` exits 0
- [ ] `grep -c "^fn test_" crates/anvilml-core/tests/events_tests.rs` outputs ≥ 10
