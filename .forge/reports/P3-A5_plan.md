# Plan Report: P3-A5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-A5                                         |
| Phase       | 003 — Core Domain Types                     |
| Description | anvilml-core: WebSocket event types          |
| Depends on  | P3-A4                                         |
| Project     | anvilml                                       |
| Planned at  | 2026-06-01T12:42:22Z                         |
| Attempt     | 1                                             |

## Objective

Create the WebSocket event type system in `anvilml-core/src/types/events.rs`. This introduces the `WsEvent` enum and nine associated variant structs (`SystemStats`, `JobQueued`, `JobStarted`, `JobProgress`, `JobImageReady`, `JobCompleted`, `JobFailed`, `JobCancelled`, `WorkerStatusChanged`) plus the helper struct `GpuStatSnapshot`, all serializing to `{ "event": "<type>", "timestamp": "<iso8601>", ...fields }` as specified in ANVILML_DESIGN §4.5. These types form the contract for the WebSocket broadcaster in `anvilml-server` and are consumed by the scheduler's event-dispatch logic.

## Scope

### In Scope
- Create `crates/anvilml-core/src/types/events.rs` with all event structs and the `WsEvent` enum
- Define `GpuStatSnapshot{index, vram_used_mib, vram_total_mib}` helper struct
- Define all 9 variant structs with fields per ANVILML_DESIGN §4.5 + §7.3 IPC mapping
- Derive `Serialize`, `Deserialize`, `Clone`, `Debug`, `ToSchema` on every struct/enum
- Use `DateTime<Utc>` for `timestamp` and `serde(default)` where the spec implies defaults
- Add `pub mod events;` to `crates/anvilml-core/src/types/mod.rs`
- Re-export `WsEvent` and all variant structs from `crates/anvilml-core/src/lib.rs`
- Add a `#[cfg(test)]` module with round-trip JSON tests covering every event type
- Verify `cargo test -p anvilml-core -- events` exits 0, including the SystemStats event-name assertion

### Out of Scope
- Any server-side WS handler or broadcaster code (belongs to P3-A6 / later phases)
- IPC message types in `anvilml-ipc` (separate task)
- Changes to `anvilml-server`, `anvilml-scheduler`, or any crate beyond `anvilml-core`
- Adding `PartialEq`, `Eq`, or `Default` derives unless the spec explicitly requires them
- OpenAPI schema generation verification (covered by the general `cargo test` gate)

## Approach

1. **Create `crates/anvilml-core/src/types/events.rs`.**
   - Add module doc comment referencing ANVILML_DESIGN §4.5.
   - Import `serde::{Deserialize, Serialize}`, `chrono::DateTime`, `chrono::Utc`, `utoipa::ToSchema`, `uuid::Uuid`.
   - Define `GpuStatSnapshot` struct with fields `index: u32`, `vram_used_mib: u32`, `vram_total_mib: u32`. Derive Debug, Clone, Serialize, Deserialize, ToSchema. Add field-level doc comments.
   - Define the 9 event structs:
     - `SystemStatsEvent`: `event: &'static str` ("system.stats"), `timestamp: DateTime<Utc>`, `gpus: Vec<GpuStatSnapshot>`, `ram_used_mib: u64`, `ram_total_mib: u64`.
     - `JobQueuedEvent`: `event: &'static str` ("job.queued"), `timestamp: DateTime<Utc>`, `job_id: Uuid`.
     - `JobStartedEvent`: `event: &'static str` ("job.started"), `timestamp: DateTime<Utc>`, `job_id: Uuid`.
     - `JobProgressEvent`: `event: &'static str` ("job.progress"), `timestamp: DateTime<Utc>`, `job_id: Uuid`, `node_index: u32`, `node_total: u32`, `node_type: String`, `step: Option<u32>`, `step_total: Option<u32>`.
     - `JobImageReadyEvent`: `event: &'static str` ("job.image_ready"), `timestamp: DateTime<Utc>`, `job_id: Uuid`, `artifact_hash: String`, `width: u32`, `height: u32`, `seed: i64`.
     - `JobCompletedEvent`: `event: &'static str` ("job.completed"), `timestamp: DateTime<Utc>`, `job_id: Uuid`.
     - `JobFailedEvent`: `event: &'static str` ("job.failed"), `timestamp: DateTime<Utc>`, `job_id: Uuid`, `error: String`, `traceback: Option<String>`.
     - `JobCancelledEvent`: `event: &'static str` ("job.cancelled"), `timestamp: DateTime<Utc>`, `job_id: Uuid`.
     - `WorkerStatusChangedEvent`: `event: &'static str` ("worker.status"), `timestamp: DateTime<Utc>`, `worker_id: String`, `status: crate::types::worker::WorkerStatus`.
   - Define `WsEvent` enum with all 9 variants. Derive Debug, Clone, Serialize, Deserialize, ToSchema.
   - Add a `#[cfg(test)]` module with tests:
     - **`system_stats_event_json`**: Serialize `SystemStatsEvent`, assert JSON string contains `"event":"system.stats"` and has a valid timestamp.
     - **`system_stats_roundtrip`**: Round-trip `SystemStatsEvent` through JSON.
     - **`job_progress_optional_fields`**: Verify `step`/`step_total` serialize as `null` when `None`.
     - **Round-trip tests for each remaining event type** (`JobQueued`, `JobStarted`, `JobImageReady`, `JobCompleted`, `JobFailed`, `JobCancelled`, `WorkerStatusChanged`).

2. **Register the module in `crates/anvilml-core/src/types/mod.rs`.**
   - Add `pub mod events;` to the module list.
   - Update the module-level doc comment to mention events from §4.5.

3. **Re-export from `crates/anvilml-core/src/lib.rs`.**
   - Add re-exports: `WsEvent`, `GpuStatSnapshot`, and all 9 event structs via `pub use types::events::*;` (or individual re-exports for clarity).

4. **Verify with the test gate.**
   - The `#[cfg(test)]` module in `events.rs` runs under `cargo test -p anvilml-core -- events`.
   - The SystemStats assertion (`event='system.stats'`) is the critical acceptance criterion from the task spec.

## Files Affected

| Action   | Path                                            | Description                                                |
|----------|-------------------------------------------------|------------------------------------------------------------|
| CREATE   | `crates/anvilml-core/src/types/events.rs`       | New file: WsEvent enum, 9 variant structs, GpuStatSnapshot, tests |
| MODIFY   | `crates/anvilml-core/src/types/mod.rs`          | Add `pub mod events;` and update module doc comment        |
| MODIFY   | `crates/anvilml-core/src/lib.rs`                | Re-export WsEvent and all event types                      |

## Tests

| Test ID / Name              | File                                        | Validates                                              |
|-----------------------------|---------------------------------------------|--------------------------------------------------------|
| `system_stats_event_json`   | `events.rs`                                 | SystemStats JSON contains `"event":"system.stats"`     |
| `system_stats_roundtrip`    | `events.rs`                                 | All SystemStats fields round-trip through JSON          |
| `job_queued_roundtrip`      | `events.rs`                                 | JobQueuedEvent serializes/deserializes correctly        |
| `job_started_roundtrip`     | `events.rs`                                 | JobStartedEvent serializes/deserializes correctly       |
| `job_progress_optional_fields` | `events.rs`                             | step/step_total serialize as null when None             |
| `job_progress_roundtrip`    | `events.rs`                                 | All JobProgressEvent fields round-trip through JSON      |
| `job_image_ready_roundtrip` | `events.rs`                                 | JobImageReadyEvent serializes/deserializes correctly     |
| `job_completed_roundtrip`   | `events.rs`                                 | JobCompletedEvent serializes/deserializes correctly     |
| `job_failed_roundtrip`      | `events.rs`                                 | JobFailedEvent (with traceback) round-trips correctly    |
| `job_cancelled_roundtrip`   | `events.rs`                                 | JobCancelledEvent serializes/deserializes correctly     |
| `worker_status_changed_roundtrip` | `events.rs`                          | WorkerStatusChangedEvent round-trips WorkerStatus enum  |
| `ws_event_enum_variants`    | `events.rs`                                 | WsEvent has exactly 9 variants, all distinct            |

## CI Impact

No CI changes required. The task only adds types and tests within `anvilml-core`, which is already compiled in CI via `cargo test -p anvilml-core --features mock-hardware`. No new dependencies, features, or workflow files are needed.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| `serde(default)` on `&'static str` fields may not produce the expected event name in JSON | Low | Medium | Use a custom serde serializer or `#[serde(serialize_with)]` to always emit the literal event string; alternatively, accept that the enum variant name is the contract and test it explicitly via the variant's struct field which is manually set. |  |
| Missing `utoipa::ToSchema` derive on one variant struct breaks OpenAPI generation in downstream crates | Low | Medium | Ensure every struct and the `WsEvent` enum has `#[derive(ToSchema)]`; verify with a compile check after implementation. |
| `GpuStatSnapshot` conflicts with existing `GpuDevice` naming | Low | Low | The types serve different purposes (runtime snapshot vs hardware report); names are disambiguated by context. No conflict expected. |
| `Option<u32>` for step/step_total may cause test confusion about null vs missing | Low | Low | Tests explicitly assert JSON contains `"step":null` to match the MVP contract. |

## Acceptance Criteria

- [ ] `crates/anvilml-core/src/types/events.rs` exists with all 9 event structs and `WsEvent` enum
- [ ] `GpuStatSnapshot{index, vram_used_mib, vram_total_mib}` is defined and derives the standard set
- [ ] Every event struct has `event: &'static str` field set to the correct type string (e.g. "system.stats", "job.progress")
- [ ] `JobProgressEvent.step` and `JobProgressEvent.step_total` are `Option<u32>` (None in MVP)
- [ ] `JobFailedEvent.traceback` is `Option<String>`
- [ ] `WorkerStatusChangedEvent.status` uses `crate::types::worker::WorkerStatus`
- [ ] All types derive `Serialize`, `Deserialize`, `Clone`, `Debug`, `ToSchema`
- [ ] `pub mod events;` added to `crates/anvilml-core/src/types/mod.rs`
- [ ] `WsEvent` and all variant structs re-exported from `crates/anvilml-core/src/lib.rs`
- [ ] `cargo test -p anvilml-core -- events` exits 0
- [ ] SystemStats JSON assertion: serialized JSON contains `"event":"system.stats"`
