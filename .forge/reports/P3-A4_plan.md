# Plan Report: P3-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-A4                                         |
| Phase       | 003 — Core Domain Types                     |
| Description | anvilml-core: Hardware and Worker domain types|
| Depends on  | P3-A3                                         |
| Project     | anvilml                                       |
| Planned at  | 2026-06-01T12:30:00Z                          |
| Attempt     | 1                                             |

## Objective

Create the hardware and worker domain type modules in `anvilml-core`, defining all structs and enums specified in ANVILML_DESIGN §4.3 (HardwareInfo, GpuDevice, DeviceType, HostInfo, InferenceCaps) and §4.4 (WorkerInfo, WorkerStatus), plus the EnvReport struct used by the preflight system (§6.1). These are pure, serializable data types with no I/O or async logic, following the same derive conventions (Serialize, Deserialize, Clone, Debug, ToSchema) established in P3-A2 and P3-A3.

## Scope

### In Scope
- Create `crates/anvilml-core/src/types/hardware.rs` with:
  - `HardwareInfo` struct (§4.3)
  - `GpuDevice` struct (§4.3)
  - `DeviceType` enum: `Cuda`, `Rocm`, `Cpu` (§4.3) — re-exported from `config.rs` to avoid duplication, since it already exists there with identical variants
  - `HostInfo` struct (§4.3)
  - `InferenceCaps` struct (§4.3)
- Create `crates/anvilml-core/src/types/worker.rs` with:
  - `WorkerInfo` struct (§4.4)
  - `WorkerStatus` enum: `Initializing`, `Idle`, `Busy`, `Dead`, `Respawning` (§4.4)
  - `EnvReport` struct: `{ python_path, python_version, torch_version, preflight_ok, reason }` (§6.1)
- Update `crates/anvilml-core/src/types/mod.rs` to register the two new modules
- Update `crates/anvilml-core/src/lib.rs` to re-export new types for downstream crates
- Add unit tests in each new module (JSON round-trip, default impl, variant completeness)
- Gate: `cargo test -p anvilml-core -- hardware` exits 0

### Out of Scope
- Hardware detection implementation (`anvilml-hardware` crate) — covered by later phases
- Worker pool management (`anvilml-worker` crate) — covered by later phases
- The `/v1/system/env` HTTP handler — covered by P3-A6
- WebSocket event types (`WsEvent`) — covered by P3-A5
- Any I/O, async, or runtime logic

## Approach

1. **Create `hardware.rs`** in `crates/anvilml-core/src/types/`
   - Re-export `DeviceType` from `crate::config::DeviceType` (already defined in config.rs with identical variants `Cuda`, `Rocm`, `Cpu`)
   - Define `HostInfo` with fields: `os: String`, `cpu_model: String`, `ram_total_mib: u64`, `ram_free_mib: u64`
   - Define `InferenceCaps` with fields: `fp16: bool`, `bf16: bool`, `flash_attention: bool`
   - Define `GpuDevice` with fields: `index: u32`, `name: String`, `device_type: DeviceType`, `vram_total_mib: u32`, `vram_free_mib: u32`, `driver_version: String`
   - Define `HardwareInfo` with fields: `host: HostInfo`, `gpus: Vec<GpuDevice>`, `inference_caps: InferenceCaps`
   - All types derive `Serialize, Deserialize, Clone, Debug, ToSchema` (and `PartialEq, Eq` where semantically appropriate)
   - Add `impl Default` for structs with sensible defaults
   - Add unit tests: JSON round-trip, variant count for DeviceType, default values

2. **Create `worker.rs`** in `crates/anvilml-core/src/types/`
   - Define `WorkerStatus` enum with variants: `Initializing`, `Idle`, `Busy`, `Dead`, `Respawning`
   - Define `WorkerInfo` with fields: `worker_id: String`, `device_index: u32`, `device_name: String`, `status: WorkerStatus`, `current_job_id: Option<Uuid>`, `vram_used_mib: u32`
   - Define `EnvReport` with fields: `python_path: String`, `python_version: String`, `torch_version: String`, `preflight_ok: bool`, `reason: Option<String>`
   - All types derive `Serialize, Deserialize, Clone, Debug, ToSchema` (and `PartialEq, Eq` where appropriate)
   - Add `impl Default` for structs
   - Add unit tests: JSON round-trip, variant count, default values, EnvReport stub fields

3. **Update `types/mod.rs`**
   - Add `pub mod hardware;` and `pub mod worker;`

4. **Update `lib.rs`**
   - Add re-exports for new public types: `HardwareInfo`, `GpuDevice`, `HostInfo`, `InferenceCaps`, `WorkerInfo`, `WorkerStatus`, `EnvReport`

5. **Verify**: Run `cargo test -p anvilml-core -- hardware` and confirm exit 0.

## Files Affected

| Action   | Path                                        | Description                                      |
|----------|---------------------------------------------|--------------------------------------------------|
| CREATE   | crates/anvilml-core/src/types/hardware.rs   | HardwareInfo, GpuDevice, DeviceType (re-export), HostInfo, InferenceCaps |
| CREATE   | crates/anvilml-core/src/types/worker.rs     | WorkerInfo, WorkerStatus, EnvReport              |
| MODIFY   | crates/anvilml-core/src/types/mod.rs        | Register hardware and worker modules             |
| MODIFY   | crates/anvilml-core/src/lib.rs              | Re-export new public types                       |

## Tests

| Test ID / Name            | File                     | Validates                          |
|---------------------------|--------------------------|------------------------------------|
| `device_type_variants`    | hardware.rs (mod tests)  | DeviceType has exactly 3 variants, all distinct |
| `host_info_roundtrip`     | hardware.rs (mod tests)  | HostInfo serializes/deserializes correctly |
| `inference_caps_defaults` | hardware.rs (mod tests)  | InferenceCaps defaults to all-false |
| `gpu_device_roundtrip`    | hardware.rs (mod tests)  | GpuDevice JSON round-trip preserves all fields |
| `hardware_info_roundtrip` | hardware.rs (mod tests)  | HardwareInfo with nested Vec<GpuDevice> round-trips |
| `worker_status_variants`  | worker.rs (mod tests)    | WorkerStatus has exactly 5 variants, all distinct |
| `worker_info_roundtrip`   | worker.rs (mod tests)    | WorkerInfo JSON round-trip preserves all fields |
| `env_report_stub`         | worker.rs (mod tests)    | EnvReport serializes with stub values matching P3-A6 expectations |

## CI Impact

No CI changes required. The task only adds types within an existing crate; no new dependencies, features, or CI jobs are introduced.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| DeviceType already defined in config.rs creates duplicate symbol | Medium | High | Re-export from config.rs using `pub use crate::config::DeviceType;` — same pattern as ModelKind re-export in model.rs |
| ToSchema derive conflicts with existing types | Low | Low | Follow the exact derive set used in P3-A2/P3-A3 modules; utoipa 5.x is already a dependency |
| Test compilation fails due to missing imports | Low | Low | Use same import pattern as existing modules (chrono, serde, serde_json, utoipa, uuid) |

## Acceptance Criteria

- [ ] `crates/anvilml-core/src/types/hardware.rs` exists with HardwareInfo, GpuDevice, DeviceType (re-exported), HostInfo, InferenceCaps — all derive Serialize, Deserialize, Clone, Debug, ToSchema
- [ ] `crates/anvilml-core/src/types/worker.rs` exists with WorkerInfo, WorkerStatus enum (5 variants), EnvReport struct — all derive Serialize, Deserialize, Clone, Debug, ToSchema
- [ ] `types/mod.rs` registers both new modules
- [ ] `lib.rs` re-exports the new public types
- [ ] `cargo test -p anvilml-core -- hardware` exits 0 with all tests passing
- [ ] `cargo clippy -p anvilml-core --no-default-features --features mock-hardware -D warnings` passes with zero warnings
