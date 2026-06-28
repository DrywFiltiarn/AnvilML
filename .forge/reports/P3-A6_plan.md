# Plan Report: P3-A6

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-A6                                       |
| Phase       | 003 — Core Domain Types: Data Model         |
| Description | anvilml-core: WorkerInfo, WorkerStatus, EnvReport, ProvisioningState |
| Depends on  | P3-A5                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-28T17:51:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-core/src/types/worker.rs` defining four types — `WorkerStatus`, `WorkerInfo`, `EnvReport`, and `ProvisioningState` — that the scheduler's dispatch logic and the `/v1/workers` and `/v1/system` HTTP handlers will consume. Register the module in `types/mod.rs`. Ship four integration tests covering `WorkerInfo` construction/serde and each enum's serde roundtrip.

## Scope

### In Scope
- Create `crates/anvilml-core/src/types/worker.rs` with:
  - `WorkerStatus` enum: `Spawning`, `Idle`, `Busy`, `Dying`, `Dead` — derive `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema`; `#[serde(rename_all = "snake_case")]`.
  - `WorkerInfo` struct: fields `worker_id: String`, `status: WorkerStatus`, `device_index: u32`, `device_type: DeviceType`, `pid: Option<u32>`, `current_job_id: Option<Uuid>` — derive `Debug, Clone, Serialize, Deserialize, ToSchema`.
  - `EnvReport` struct: fields `python_version: String`, `torch_version: Option<String>`, `torch_importable: bool` — derive `Debug, Clone, Serialize, Deserialize, ToSchema`.
  - `ProvisioningState` enum: `NotStarted`, `InProgress`, `Complete`, `Failed` — derive `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema`; `#[serde(rename_all = "snake_case")]`.
- Add `mod worker;` to `crates/anvilml-core/src/types/mod.rs`.
- Add `pub use worker::*;` to `types/mod.rs`.
- Create `crates/anvilml-core/tests/worker_tests.rs` with ≥ 4 tests.
- Bump `anvilml-core` patch version `0.1.10 → 0.1.11`.

### Out of Scope
None. This task's `defers_to (from JSON): []` is empty — no scope is deferred.

## Existing Codebase Assessment

**What already exists:** `anvilml-core` is a pure-data crate (zero I/O, zero async) with four type modules already declared in `types/mod.rs` — `artifact`, `hardware`, `job`, `model` — and their corresponding test files in `tests/`. The `DeviceType` enum (used by `WorkerInfo.device_type`) lives in `types/hardware.rs` with `#[serde(rename_all = "snake_case")]` and derives `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema`. The `uuid` crate (v1.23.4, with `serde` feature) and `utoipa` crate (v5.5.0, with `uuid` feature) are already declared as dependencies, providing `Uuid` with `Serialize`/`Deserialize` and the `ToSchema` derive macro respectively.

**Established patterns:** Test files live in `crates/{name}/tests/` as separate integration-test crates using `use anvilml_core::types::*;`. Each test constructs types via the public API, serialises to JSON via `serde_json::to_string()`, deserialises back, and asserts equality. Enums are tested variant-by-variant with expected `snake_case` JSON strings. Structs are tested with all fields populated. Doc comments on types use `///` with a one-sentence description.

**Gap between design doc and task context:** `ANVILML_DESIGN.md §5.7` defines `WorkerInfo` with different field names (`id` vs `worker_id`, `device_name` vs `device_type`, `vram_used_mib` vs `pid`) and a different `WorkerStatus` variant list (`Initializing, Respawning` vs `Spawning, Dying`). The design doc also defines `EnvReport` with six additional fields and `ProvisioningState` with five variants. The task context's definitions are authoritative for this session — the design doc will need to be reconciled in a later refactoring task.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | utoipa  | 5.5.0           | rust-docs MCP  | uuid, chrono (already in Cargo.toml) |
| crate  | uuid    | 1.23.4          | rust-docs MCP  | v4, serde (already in Cargo.toml) |
| crate  | serde   | 1.0             | (existing)     | derive (already in Cargo.toml) |

All dependencies are already present in `crates/anvilml-core/Cargo.toml`. No new dependency is introduced. The `ToSchema` derive macro is available via the existing `utoipa` dependency with the `macros` default feature enabled. The `Uuid` type's `Serialize`/`Deserialize` derives are available via the existing `serde` feature on the `uuid` dependency.

## Approach

1. **Create `crates/anvilml-core/src/types/worker.rs`.** Write the file with:
   - A module-level `//!` doc comment describing the worker-process status types and their consumers (scheduler dispatch logic, `/v1/workers` and `/v1/system` HTTP handlers).
   - `WorkerStatus` enum (line ~10): `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]`, `#[serde(rename_all = "snake_case")]`, doc comment on the enum and each variant describing the lifecycle state.
   - `WorkerInfo` struct (line ~25): `#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]`, doc comment on the struct and each field. `device_type` field references `DeviceType` from `super::hardware`. `current_job_id` uses `Uuid` (imported from `uuid` crate).
   - `EnvReport` struct (line ~40): `#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]`, doc comment. Fields: `python_version: String`, `torch_version: Option<String>`, `torch_importable: bool`.
   - `ProvisioningState` enum (line ~50): `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]`, `#[serde(rename_all = "snake_case")]`, doc comment on the enum and each variant.
   - Imports at top: `use serde::{Deserialize, Serialize};`, `use utoipa::ToSchema;`, `use uuid::Uuid;`, `use super::hardware::DeviceType;`.

2. **Update `crates/anvilml-core/src/types/mod.rs`.** Add `pub mod worker;` after the existing `pub mod model;` line, and add `pub use worker::*;` after the existing `pub use model::*;` line. This follows the established pattern where each task adds exactly one `mod` declaration and one `pub use` line.

3. **Create `crates/anvilml-core/tests/worker_tests.rs`.** Write integration tests following the project's established style:
   - `test_worker_info_construction_and_serde_roundtrip()`: construct a `WorkerInfo` with all fields populated (including a valid `Uuid` for `current_job_id`, a `DeviceType::Cuda` for `device_type`, and a `WorkerStatus::Idle`), serialise to JSON, deserialise back, assert equality, and verify JSON field names.
   - `test_worker_status_serde_snake_case()`: iterate over all five `WorkerStatus` variants with expected JSON strings (`"spawning"`, `"idle"`, `"busy"`, `"dying"`, `"dead"`), serialise and roundtrip each.
   - `test_provisioning_state_serde_snake_case()`: iterate over all four `ProvisioningState` variants with expected JSON strings (`"not_started"`, `"in_progress"`, `"complete"`, `"failed"`), serialise and roundtrip each.
   - `test_env_report_serde_roundtrip()`: construct an `EnvReport` with all fields set, serialise to JSON, deserialise back, assert equality, and verify JSON field names.

4. **Bump `anvilml-core` version.** Edit `crates/anvilml-core/Cargo.toml`: change `version = "0.1.10"` to `version = "0.1.11"`.

## Public API Surface

| Path | Item | Kind |
|------|------|------|
| `anvilml_core::types::WorkerStatus` | `enum { Spawning, Idle, Busy, Dying, Dead }` | New public enum |
| `anvilml_core::types::WorkerInfo` | `struct { worker_id: String, status: WorkerStatus, device_index: u32, device_type: DeviceType, pid: Option<u32>, current_job_id: Option<Uuid> }` | New public struct |
| `anvilml_core::types::EnvReport` | `struct { python_version: String, torch_version: Option<String>, torch_importable: bool }` | New public struct |
| `anvilml_core::types::ProvisioningState` | `enum { NotStarted, InProgress, Complete, Failed }` | New public enum |

All items derive `Debug, Clone, Serialize, Deserialize, ToSchema`. The two enums additionally derive `Copy, PartialEq, Eq`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/types/worker.rs` | New module with `WorkerStatus`, `WorkerInfo`, `EnvReport`, `ProvisioningState` |
| MODIFY | `crates/anvilml-core/src/types/mod.rs` | Add `mod worker;` and `pub use worker::*;` |
| CREATE | `crates/anvilml-core/tests/worker_tests.rs` | Integration tests (≥ 4 tests) |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Bump patch version `0.1.10 → 0.1.11` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-core/tests/worker_tests.rs` | `test_worker_info_construction_and_serde_roundtrip` | `WorkerInfo` construction with all fields populated, JSON serialisation produces correct field names, roundtrip equality holds | None | `WorkerInfo` with `worker_id="w-0"`, `status=Idle`, `device_index=0`, `device_type=Cuda`, `pid=Some(1234)`, `current_job_id=Some(Uuid::new_v4())` | `serde_json::from_str` roundtrip equals original; JSON contains `worker_id`, `status`, `device_index`, `device_type`, `pid`, `current_job_id` keys | `cargo test -p anvilml-core --test worker_tests test_worker_info_construction_and_serde_roundtrip` exits 0 |
| `crates/anvilml-core/tests/worker_tests.rs` | `test_worker_status_serde_snake_case` | All five `WorkerStatus` variants serialise to correct `snake_case` JSON strings and roundtrip to equal values | None | Each variant individually | Each variant serialises to `"spawning"`, `"idle"`, `"busy"`, `"dying"`, `"dead"` respectively; roundtrip equality holds | `cargo test -p anvilml-core --test worker_tests test_worker_status_serde_snake_case` exits 0 |
| `crates/anvilml-core/tests/worker_tests.rs` | `test_provisioning_state_serde_snake_case` | All four `ProvisioningState` variants serialise to correct `snake_case` JSON strings and roundtrip to equal values | None | Each variant individually | Each variant serialises to `"not_started"`, `"in_progress"`, `"complete"`, `"failed"` respectively; roundtrip equality holds | `cargo test -p anvilml-core --test worker_tests test_provisioning_state_serde_snake_case` exits 0 |
| `crates/anvilml-core/tests/worker_tests.rs` | `test_env_report_serde_roundtrip` | `EnvReport` construction with all fields set, JSON serialisation produces correct field names, roundtrip equality holds | None | `EnvReport` with `python_version="3.12.3"`, `torch_version=Some("2.5.1")`, `torch_importable=true` | `serde_json::from_str` roundtrip equals original; JSON contains `python_version`, `torch_version`, `torch_importable` keys | `cargo test -p anvilml-core --test worker_tests test_env_report_serde_roundtrip` exits 0 |

## CI Impact

No CI changes required. The new test file is picked up by the existing `cargo test --workspace --features mock-hardware` command (ENVIRONMENT.md §6 Step 6), which runs all test crates in the workspace. No new file types, gates, or CI jobs are introduced.

## Platform Considerations

None identified. The types are pure data with no platform-specific logic. `#[cfg(unix)]` / `#[cfg(windows)]` guards are not required. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `DeviceType` is defined in `types::hardware` (a sibling module), not in `types` root — the import `use super::hardware::DeviceType;` must be correct. If the module path is wrong, compilation fails. | Low | High | Verify the import path by reading `types/mod.rs` (confirms `hardware` is a sibling module under `types/`) before writing. The existing `hardware_tests.rs` already imports from `anvilml_core::types::*` which re-exports `DeviceType`, confirming the path is correct. |
| `Uuid` requires the `serde` feature for `Serialize`/`Deserialize`. The task context assumes `Uuid` derives `Serialize` and `Deserialize`. If the feature is missing, compilation fails. | Low | High | Confirmed via MCP: `uuid = { version = "1.23.4", features = ["v4", "serde"] }` is already in Cargo.toml. The `serde` feature is present. |
| `ToSchema` derive requires `utoipa`'s `uuid` feature for `Uuid` schema generation. Without it, the derive macro fails on `WorkerInfo.current_job_id`. | Low | High | Confirmed via MCP: `utoipa = { version = "5.5.0", features = ["uuid", "chrono"] }` is already in Cargo.toml. The `uuid` feature is present. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core --test worker_tests` exits 0
- [ ] `cargo check -p anvilml-core --features mock-hardware` exits 0
- [ ] `grep -c "pub mod worker;" crates/anvilml-core/src/types/mod.rs` returns 1
- [ ] `grep -c "pub use worker::\*;" crates/anvilml-core/src/types/mod.rs` returns 1
- [ ] `grep '^version' crates/anvilml-core/Cargo.toml` contains `0.1.11`
