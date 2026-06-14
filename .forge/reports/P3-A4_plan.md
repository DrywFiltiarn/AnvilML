# Plan Report: P3-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-A4                                       |
| Phase       | 003 — Core Domain Types                     |
| Description | anvilml-core: node and worker types          |
| Depends on  | P3-A1, P3-A2, P3-A3                         |
| Project     | anvilml                                     |
| Planned at  | 2026-06-14T19:10:00Z                        |
| Attempt     | 1                                           |

## Objective

Create two new source files in `crates/anvilml-core/src/types/` — `node.rs` and `worker.rs` — containing all domain types specified in `ANVILML_DESIGN.md §5.6` (NodeTypeDescriptor, SlotDescriptor, SlotType) and §5.7 (WorkerInfo, WorkerStatus, EnvReport, ProvisioningState). Update `types/mod.rs` to declare and re-export these modules, and update `lib.rs` to re-export the new public types. Add integration test files `tests/node_tests.rs` and `tests/worker_tests.rs` with ≥ 3 tests each. The observable outcome is that `cargo test -p anvilml-core -- types::node` and `cargo test -p anvilml-core -- types::worker` both exit 0 with all tests passing, and the new types are importable as `anvilml_core::NodeTypeDescriptor`, `anvilml_core::SlotType`, `anvilml_core::WorkerInfo`, etc.

## Scope

### In Scope
- Create `crates/anvilml-core/src/types/node.rs` with:
  - `NodeTypeDescriptor` struct (type_name, display_name, category, description, inputs, outputs)
  - `SlotDescriptor` struct (name, slot_type, optional)
  - `SlotType` enum (Model, Clip, Vae, Conditioning, Latent, Image, String, Int, Float, Bool, Any)
- Create `crates/anvilml-core/src/types/worker.rs` with:
  - `WorkerInfo` struct (id, device_index, device_name, status, current_job_id, vram_used_mib)
  - `WorkerStatus` enum (Initializing, Idle, Busy, Dead, Respawning)
  - `EnvReport` struct (python_path, python_version, torch_version, provisioning, preflight_ok, reason, node_types)
  - `ProvisioningState` enum (Ready, Provisioning, Failed, NotStarted)
- Update `crates/anvilml-core/src/types/mod.rs` to declare `pub mod node` and `pub mod worker`, and re-export all new pub types
- Update `crates/anvilml-core/src/lib.rs` to re-export all new pub types
- Create `crates/anvilml-core/tests/node_tests.rs` with ≥ 3 tests
- Create `crates/anvilml-core/tests/worker_tests.rs` with ≥ 3 tests
- Bump `anvilml-core` patch version from 0.1.6 to 0.1.7

### Out of Scope
- Any usage of these types in other crates (anvilml-scheduler, anvilml-server, etc.) — those are separate tasks
- The `/v1/system/env` stub endpoint (P3-C1)
- The `WsEvent` enum and WebSocket event types (P3-A5)
- Any changes to `anvilml-ipc` message types
- Any changes to Python worker node registry or Ready event serialization

## Existing Codebase Assessment

The `anvilml-core` crate already contains four type modules (`job.rs`, `model.rs`, `artifact.rs`, `hardware.rs`) plus the config module. Each follows a consistent pattern:
- Module-level `//!` doc comment describing what the file owns.
- Every struct/enum derives `Debug, Clone, Serialize, Deserialize, ToSchema` (utoipa).
- Enums use `#[serde(rename_all = "snake_case")]` for JSON representation.
- Doc comments on every `pub` item follow the pattern: one-sentence summary, then a paragraph explaining the purpose and any non-obvious details.
- Tests live in `crates/anvilml-core/tests/` as separate test crate files, importing from `anvilml_core::` (the crate name, not `anvilml_core`).

The `types/mod.rs` currently declares `pub mod artifact`, `pub mod hardware`, `pub mod job`, `pub mod model` and re-exports their pub types. It does not yet declare `node` or `worker` modules — those are what this task creates.

No external dependencies are added. All types use only `serde` (derive), `utoipa` (ToSchema), and `uuid::Uuid` (already a workspace dep, used in WorkerInfo's `current_job_id`).

## Resolved Dependencies

| Type   | Name      | Version verified | MCP source     | Feature flags confirmed |
|--------|-----------|-----------------|----------------|------------------------|
| crate  | serde     | 1.0.228         | Cargo.toml     | derive                   |
| crate  | utoipa    | 5.5.0           | Cargo.toml     | macros, chrono, uuid     |
| crate  | uuid      | 1.23.3          | Cargo.toml     | serde, v4                |

No new external dependencies are introduced. All types use only existing workspace dependencies already declared in `Cargo.toml`.

## Approach

1. **Create `crates/anvilml-core/src/types/node.rs`.** Define `SlotType` enum first (since `SlotDescriptor` references it), then `SlotDescriptor`, then `NodeTypeDescriptor`. Each gets `#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]`. `SlotType` uses `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` per the design doc §5.6 — this produces `"MODEL"`, `"CLIP"`, etc. in JSON, matching the Python worker's `SlotType` convention. Add a module-level `//!` doc comment and `///` doc comments on every pub item following the pattern established in `hardware.rs` and `model.rs`.

2. **Create `crates/anvilml-core/src/types/worker.rs`.** Define `WorkerStatus` enum first (since `WorkerInfo` references it), then `ProvisioningState` (standalone), then `WorkerInfo`, then `EnvReport` (which references `NodeTypeDescriptor`, `WorkerStatus`, and `ProvisioningState`). Each gets the standard derives. `WorkerStatus` and `ProvisioningState` use `#[serde(rename_all = "snake_case")]` per the design doc. Add doc comments on every pub item.

3. **Update `crates/anvilml-core/src/types/mod.rs`.** Add `pub mod node;` and `pub mod worker;` declarations. Add re-exports: `pub use node::{NodeTypeDescriptor, SlotDescriptor, SlotType};` and `pub use worker::{EnvReport, ProvisioningState, WorkerInfo, WorkerStatus};`. This keeps the module namespace consistent with existing patterns.

4. **Update `crates/anvilml-core/src/lib.rs`.** Add the new types to the existing `pub use types::{...}` line: `EnvReport, NodeTypeDescriptor, ProvisioningState, SlotDescriptor, SlotType, WorkerInfo, WorkerStatus`. This makes them available as `anvilml_core::NodeTypeDescriptor` etc.

5. **Create `crates/anvilml-core/tests/node_tests.rs`.** Write ≥ 3 tests following the established pattern:
   - `test_node_type_descriptor_json_roundtrip`: Build a `NodeTypeDescriptor` with multiple inputs and outputs (including an optional input), serialize to JSON, deserialize back, verify all fields match.
   - `test_slot_type_variants`: Serialize and deserialize all 11 `SlotType` variants, verify roundtrip equality. This confirms the `SCREAMING_SNAKE_CASE` serde attribute produces correct JSON keys.
   - `test_slot_descriptor_optional_field`: Build a `SlotDescriptor` with `optional: true`, verify it roundtrips correctly and that the `optional` field is preserved through the JSON roundtrip.

6. **Create `crates/anvilml-core/tests/worker_tests.rs`.** Write ≥ 3 tests:
   - `test_worker_info_json_roundtrip`: Build a `WorkerInfo` with all fields populated (including `Some` for `current_job_id` and `vram_used_mib`), roundtrip through JSON.
   - `test_worker_status_variants`: Serialize and deserialize all 5 `WorkerStatus` variants, verify roundtrip equality.
   - `test_env_report_default_preflight`: Build an `EnvReport` with `preflight_ok: false` and `provisioning: ProvisioningState::NotStarted` (the default stub state from P3-C1), verify it roundtrips and that `node_types` is an empty vec.

7. **Bump `anvilml-core` version** from `0.1.6` to `0.1.7` in `crates/anvilml-core/Cargo.toml` per §12 of ENVIRONMENT.md.

## Public API Surface

| Item | Type | Module Path | Derives |
|------|------|-------------|---------|
| `NodeTypeDescriptor` | struct | `types::node` | Debug, Clone, Serialize, Deserialize, ToSchema |
| `SlotDescriptor` | struct | `types::node` | Debug, Clone, Serialize, Deserialize, ToSchema |
| `SlotType` | enum | `types::node` | Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema |
| `WorkerInfo` | struct | `types::worker` | Debug, Clone, Serialize, Deserialize, ToSchema |
| `WorkerStatus` | enum | `types::worker` | Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema |
| `EnvReport` | struct | `types::worker` | Debug, Clone, Serialize, Deserialize, ToSchema |
| `ProvisioningState` | enum | `types::worker` | Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema |

Full struct definitions per ANVILML_DESIGN.md §5.6-5.7:

```rust
pub struct NodeTypeDescriptor {
    pub type_name: String,
    pub display_name: String,
    pub category: String,
    pub description: String,
    pub inputs: Vec<SlotDescriptor>,
    pub outputs: Vec<SlotDescriptor>,
}

pub struct SlotDescriptor {
    pub name: String,
    pub slot_type: SlotType,
    pub optional: bool,
}

#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SlotType {
    Model, Clip, Vae, Conditioning, Latent,
    Image, String, Int, Float, Bool, Any,
}

pub struct WorkerInfo {
    pub id: String,
    pub device_index: u32,
    pub device_name: String,
    pub status: WorkerStatus,
    pub current_job_id: Option<Uuid>,
    pub vram_used_mib: Option<u32>,
}

#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    Initializing, Idle, Busy, Dead, Respawning,
}

pub struct EnvReport {
    pub python_path: Option<String>,
    pub python_version: Option<String>,
    pub torch_version: Option<String>,
    pub provisioning: ProvisioningState,
    pub preflight_ok: bool,
    pub reason: Option<String>,
    pub node_types: Vec<NodeTypeDescriptor>,
}

#[serde(rename_all = "snake_case")]
pub enum ProvisioningState {
    Ready, Provisioning, Failed, NotStarted,
}
```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/types/node.rs` | NodeTypeDescriptor, SlotDescriptor, SlotType types |
| CREATE | `crates/anvilml-core/src/types/worker.rs` | WorkerInfo, WorkerStatus, EnvReport, ProvisioningState types |
| MODIFY | `crates/anvilml-core/src/types/mod.rs` | Add `pub mod node`, `pub mod worker`, and re-exports |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Add re-exports of new types to `pub use types::{...}` |
| CREATE | `crates/anvilml-core/tests/node_tests.rs` | Integration tests for node types (≥ 3 tests) |
| CREATE | `crates/anvilml-core/tests/worker_tests.rs` | Integration tests for worker types (≥ 3 tests) |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Bump version 0.1.6 → 0.1.7 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `tests/node_tests.rs` | `test_node_type_descriptor_json_roundtrip` | Fully-populated NodeTypeDescriptor with mixed optional inputs roundtrips through JSON serialization/deserialization without data loss | None | NodeTypeDescriptor with 2 inputs (one optional), 2 outputs | Restored struct equals original on all fields | `cargo test -p anvilml-core -- node_tests::test_node_type_descriptor_json_roundtrip` exits 0 |
| `tests/node_tests.rs` | `test_slot_type_variants` | All 11 SlotType enum variants roundtrip through JSON with correct SCREAMING_SNAKE_CASE keys | None | All SlotType variants | Each variant deserializes back to itself | `cargo test -p anvilml-core -- node_tests::test_slot_type_variants` exits 0 |
| `tests/node_tests.rs` | `test_slot_descriptor_optional_field` | SlotDescriptor with optional=true preserves the field through JSON roundtrip | None | SlotDescriptor{name="seed", slot_type=Int, optional=true} | Restored optional field equals true | `cargo test -p anvilml-core -- node_tests::test_slot_descriptor_optional_field` exits 0 |
| `tests/worker_tests.rs` | `test_worker_info_json_roundtrip` | Fully-populated WorkerInfo with Some current_job_id and vram_used_mib roundtrips through JSON | None | WorkerInfo with all fields set | Restored struct equals original on all fields | `cargo test -p anvilml-core -- worker_tests::test_worker_info_json_roundtrip` exits 0 |
| `tests/worker_tests.rs` | `test_worker_status_variants` | All 5 WorkerStatus enum variants roundtrip through JSON with correct snake_case keys | None | All WorkerStatus variants | Each variant deserializes back to itself | `cargo test -p anvilml-core -- worker_tests::test_worker_status_variants` exits 0 |
| `tests/worker_tests.rs` | `test_env_report_default_preflight` | EnvReport with preflight_ok=false, provisioning=NotStarted, empty node_types roundtrips correctly | None | EnvReport{preflight_ok: false, provisioning: NotStarted, node_types: []} | Restored struct matches original; node_types is empty vec | `cargo test -p anvilml-core -- worker_tests::test_env_report_default_preflight` exits 0 |

## CI Impact

No CI changes required. The new test files follow the existing convention of `crates/{name}/tests/*.rs`, which `cargo test --workspace` already picks up automatically. No new file types, gates, or CI job configurations are needed.

## Platform Considerations

None identified. All types are pure data with no I/O, no platform-specific paths, and no platform-dependent behavior. The `#[serde(rename_all = ...)]` attributes produce consistent JSON on all platforms. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `SlotType` uses `SCREAMING_SNAKE_CASE` which differs from the `snake_case` convention used by all other enums in the crate. The ACT agent may accidentally use `snake_case` for SlotType, producing `"model"` instead of `"MODEL"` in JSON, breaking Python worker compatibility. | Medium | High | The design doc §5.6 explicitly specifies `SCREAMING_SNAKE_CASE` for SlotType. The plan's Approach step 1 and Public API Surface table both state this explicitly. The ACT agent must verify the serde attribute matches the design doc before writing code. |
| `EnvReport.node_types` is `Vec<NodeTypeDescriptor>`, which is a recursive type reference (EnvReport references NodeTypeDescriptor which references SlotDescriptor). The JSON roundtrip test must handle nested vectors correctly. | Low | Medium | The test `test_env_report_default_preflight` uses an empty vec for node_types, avoiding the nesting complexity. The roundtrip test `test_node_type_descriptor_json_roundtrip` exercises the full nesting. Both are covered. |
| The `uuid::Uuid` type used in `WorkerInfo.current_job_id` requires the `serde` feature flag on the uuid dependency. If the feature is not enabled, serialization will fail at compile time. | Low | High | The workspace Cargo.toml already declares `uuid = { version = "1.23.3", features = ["serde", "v4"] }`, so the `serde` feature is enabled. No action needed. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core -- types::node` exits 0
- [ ] `cargo test -p anvilml-core -- types::worker` exits 0
- [ ] `cargo test -p anvilml-core` exits 0 (full crate test suite)
- [ ] `grep "^pub use" crates/anvilml-core/src/lib.rs` contains `NodeTypeDescriptor`, `SlotType`, `WorkerInfo`, `WorkerStatus`, `EnvReport`, `ProvisioningState`
- [ ] `head -1 crates/anvilml-core/Cargo.toml` shows version = "0.1.7"
