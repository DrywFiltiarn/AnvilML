# Plan Report: P2-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P2-A4                                         |
| Phase       | 002 — Core Types & IPC                        |
| Description | anvilml-core: hardware and worker types + WebSocket event types |
| Depends on  | P2-A1, P2-A2, P2-A3                           |
| Project     | anvilml                                       |
| Planned at  | 2026-05-29T23:33:03Z                          |
| Attempt     | 1                                             |

## Objective

Define the remaining domain types that close out `anvilml-core` for the MVP feature set: hardware detection output types (hardware.rs), worker state types (worker.rs), and the full WebSocket event enum with all nine event structs (events.rs). These types are consumed by every downstream crate — hardware detection, worker management, scheduler, and the HTTP/WebSocket server — and must be serializable, cloneable, debuggable, and annotated with `utoipa::ToSchema`. The WsEvent enum uses serde internally-tagged serialization to produce `{ "event": "<type>", "timestamp": "...", ...fields }` wire format. At least 10 tests must pass across all modules.

## Scope

### In Scope
- `types/hardware.rs`: `HardwareInfo`, `GpuDevice`, `DeviceType`, `HostInfo`, `InferenceCaps` — exact fields and types from ANVILML_DESIGN.md §4.3
- `types/worker.rs`: `WorkerInfo`, `WorkerStatus` — exact fields and variants from ANVILML_DESIGN.md §4.4
- `types/events.rs`: `WsEvent` enum + 9 event structs (`SystemStatsEvent`, `JobQueuedEvent`, `JobStartedEvent`, `JobProgressEvent`, `JobImageReadyEvent`, `JobCompletedEvent`, `JobFailedEvent`, `JobCancelledEvent`, `WorkerStatusChangedEvent`) + `GpuStatSnapshot` — from ANVILML_DESIGN.md §4.5
- `types/mod.rs`: add module declarations and public re-exports for the three new modules
- Tests: serialization round-trips, enum variant checks, WsEvent event-discriminator assertion

### Out of Scope
- Hardware detection implementation (anvilml-hardware crate — phase 003)
- Worker pool spawn/supervise logic (anvilml-worker crate — phase 005)
- WebSocket server handler and broadcaster (anvilml-server crate — phase 007)
- IPC message enums (anvilml-ipc crate — P2-B1)
- Any I/O, async runtime, or external process spawning

## Approach

1. **Create `types/hardware.rs`** with the five types from §4.3:
   - `DeviceType` enum: three variants (`Cuda`, `Rocm`, `Cpu`) with `Serialize`, `Deserialize`, `Clone`, `Debug`, `PartialEq`, `Eq`, `ToSchema`.
   - `GpuDevice` struct: fields `index: u32`, `name: String`, `device_type: DeviceType`, `vram_total_mib: u32`, `vram_free_mib: u32`, `driver_version: String`. Derives `Serialize`, `Deserialize`, `Clone`, `Debug`, `PartialEq`, `ToSchema`.
   - `HardwareInfo` struct: fields `host: HostInfo`, `gpus: Vec<GpuDevice>`, `inference_caps: InferenceCaps`. Derives `Serialize`, `Deserialize`, `Clone`, `Debug`, `PartialEq`, `ToSchema`.
   - `HostInfo` struct: fields `os: String`, `cpu_model: String`, `ram_total_mib: u64`, `ram_free_mib: u64`. Derives `Serialize`, `Deserialize`, `Clone`, `Debug`, `PartialEq`, `ToSchema`.
   - `InferenceCaps` struct: fields `fp16: bool`, `bf16: bool`, `flash_attention: bool`. Derives `Serialize`, `Deserialize`, `Clone`, `Debug`, `PartialEq`, `ToSchema`.
   - Add a test verifying all three `DeviceType` variants serialize to lowercase snake_case JSON strings.
   - Add a test round-tripping `HardwareInfo` with a sample GPU and host.

2. **Create `types/worker.rs`** with two types from §4.4:
   - `WorkerStatus` enum: five variants (`Initializing`, `Idle`, `Busy`, `Dead`, `Respawning`) with standard derives including `PartialEq`/`Eq`.
   - `WorkerInfo` struct: fields `worker_id: String`, `device_index: u32`, `device_name: String`, `status: WorkerStatus`, `current_job_id: Option<Uuid>`, `vram_used_mib: u32`. Derives `Serialize`, `Deserialize`, `Clone`, `Debug`, `PartialEq`, `ToSchema`.
   - Add a test verifying all five `WorkerStatus` variants serialize round-trip.
   - Add a test constructing `WorkerInfo` and round-tripping via JSON.

3. **Create `types/events.rs`** with the WsEvent enum and nine event structs:
   - Define `GpuStatSnapshot` struct first: fields `index: u32`, `vram_used_mib: u32`, `vram_total_mib: u32`. Derives `Serialize`, `Deserialize`, `Clone`, `Debug`, `PartialEq`, `ToSchema`.
   - Define all nine event structs as named structs (required for serde internally-tagged enum). Each has `event: &'static str` and `timestamp: DateTime<Utc>` plus variant-specific fields:
     - `SystemStatsEvent`: `gpus: Vec<GpuStatSnapshot>`, `ram_used_mib: u64`, `ram_total_mib: u64`
     - `JobQueuedEvent`: `job_id: Uuid`, `model_id: Uuid`
     - `JobStartedEvent`: `job_id: Uuid`, `worker_id: String`
     - `JobProgressEvent`: `job_id: Uuid`, `node_index: u32`, `node_total: u32`, `node_type: String`, `step: Option<u32>`, `step_total: Option<u32>` (step/step_total reserved, always None in MVP)
     - `JobImageReadyEvent`: `job_id: Uuid`, `artifact_hash: String`, `width: u32`, `height: u32`, `seed: i64`
     - `JobCompletedEvent`: `job_id: Uuid`
     - `JobFailedEvent`: `job_id: Uuid`, `error: String`, `traceback: Option<String>`
     - `JobCancelledEvent`: `job_id: Uuid`
     - `WorkerStatusChangedEvent`: `worker_id: String`, `status: WorkerStatus`
   - Define `WsEvent` enum with serde internally-tagged serialization: `#[serde(tag = "event", rename_all = "snake_case")]`. Derives `Serialize`, `Deserialize`, `Clone`, `Debug`, `ToSchema`.
   - Add a serialization test for `WsEvent::SystemStats` that asserts the JSON output contains `"event": "system_stats"` as a top-level key (serde internally-tagged uses the variant name with snake_case renaming).
   - Add round-trip tests for each event struct individually.

4. **Update `types/mod.rs`** to:
   - Add `pub mod hardware;`, `pub mod worker;`, `pub mod events;`
   - Add public re-exports: `pub use hardware::{HardwareInfo, GpuDevice, DeviceType, HostInfo, InferenceCaps};`, `pub use worker::{WorkerInfo, WorkerStatus};`, `pub use events::{WsEvent, SystemStatsEvent, JobQueuedEvent, JobStartedEvent, JobProgressEvent, JobImageReadyEvent, JobCompletedEvent, JobFailedEvent, JobCancelledEvent, WorkerStatusChangedEvent, GpuStatSnapshot};`

5. **Verify tests pass**: Run `cargo test -p anvilml-core` to confirm ≥10 tests total across all modules exit 0.

## Files Affected

| Action   | Path                                          | Description                                    |
|----------|-----------------------------------------------|------------------------------------------------|
| CREATE   | crates/anvilml-core/src/types/hardware.rs     | HardwareInfo, GpuDevice, DeviceType, HostInfo, InferenceCaps types + tests |
| CREATE   | crates/anvilml-core/src/types/worker.rs       | WorkerInfo, WorkerStatus types + tests         |
| CREATE   | crates/anvilml-core/src/types/events.rs       | WsEvent enum, 9 event structs, GpuStatSnapshot + tests |
| MODIFY   | crates/anvilml-core/src/types/mod.rs          | Add three module declarations and public re-exports for all new types |

## Tests

| Test ID / Name                          | File                      | Validates                                      |
|-----------------------------------------|---------------------------|------------------------------------------------|
| `device_type_serialization_round_trip`  | hardware.rs               | All 3 DeviceType variants serialize/deserialize correctly |
| `hardware_info_round_trip`              | hardware.rs               | HardwareInfo with sample GpuDevice and HostInfo serializes and deserializes identically |
| `worker_status_serialization_round_trip`| worker.rs                 | All 5 WorkerStatus variants round-trip via JSON |
| `worker_info_construct_and_round_trip`  | worker.rs                 | WorkerInfo construction and JSON round-trip    |
| `gpu_stat_snapshot_round_trip`          | events.rs                 | GpuStatSnapshot serializes/deserializes        |
| `system_stats_event_serialization`      | events.rs                 | WsEvent::SystemStats produces `{"event":"...","timestamp":"...",...}` JSON with correct discriminator key |
| `job_queued_event_round_trip`           | events.rs                 | JobQueuedEvent round-trip                       |
| `job_started_event_round_trip`          | events.rs                 | JobStartedEvent round-trip                      |
| `job_progress_event_round_trip`         | events.rs                 | JobProgressEvent with None step/step_total      |
| `job_image_ready_event_round_trip`      | events.rs                 | JobImageReadyEvent round-trip                   |
| `job_completed_event_round_trip`        | events.rs                 | JobCompletedEvent round-trip                    |
| `job_failed_event_round_trip`           | events.rs                 | JobFailedEvent with error and optional traceback|
| `job_cancelled_event_round_trip`        | events.rs                 | JobCancelledEvent round-trip                    |
| `worker_status_changed_event_round_trip`| events.rs                 | WorkerStatusChangedEvent with WorkerStatus enum |
| `ws_event_all_variants_serialize`       | events.rs                 | Each WsEvent variant serializes to valid JSON with correct event discriminator |

## CI Impact

No CI changes required. The test suite already runs `cargo test -p anvilml-core` as part of the existing CI matrix (phase 001 P1-A2 established this). Adding new types and tests to anvilml-core does not change any CI workflow configuration.

## Risks and Mitigations

| Risk                                    | Likelihood | Impact | Mitigation                                       |
|-----------------------------------------|-----------|--------|---------------------------------------------------|
| Serde internally-tagged enum produces unexpected JSON key format (`system_stats` vs `system.stats`) | Medium     | High   | Verify serde tag serialization behavior; use a manual serialization test to assert the exact JSON shape. If snake_case renaming on the tag conflicts with the wire spec (`system.stats`), fall back to a custom serializer or use `#[serde(rename = "system.stats")]` on variants. |
| Missing `utoipa::ToSchema` derive causes OpenAPI generation failure in later phases | Low      | Medium | Include `utoipa::ToSchema` on all new types from the start, matching the pattern used in P2-A3. |
| Circular dependency between events.rs and worker.rs (WorkerStatusChangedEvent uses WorkerStatus) | Low       | Low    | WorkerStatus is defined in worker.rs which is a peer module; Rust allows cross-module references within the same parent module. No circular dependency. |

## Acceptance Criteria

- [ ] `crates/anvilml-core/src/types/hardware.rs` exists and defines HardwareInfo, GpuDevice, DeviceType, HostInfo, InferenceCaps with exact fields and types from ANVILML_DESIGN.md §4.3
- [ ] `crates/anvilml-core/src/types/worker.rs` exists and defines WorkerInfo, WorkerStatus with exact fields and variants from ANVILML_DESIGN.md §4.4
- [ ] `crates/anvilml-core/src/types/events.rs` exists and defines WsEvent enum + all 9 event structs + GpuStatSnapshot from ANVILML_DESIGN.md §4.5
- [ ] WsEvent serializes with an "event" discriminator field producing `{ "event": "...", "timestamp": "...", ... }` wire format
- [ ] `crates/anvilml-core/src/types/mod.rs` re-exports all new types publicly
- [ ] `cargo test -p anvilml-core` exits 0 with ≥10 tests total across all modules
