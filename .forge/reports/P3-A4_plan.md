# Plan Report: P3-A4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P3-A4                                               |
| Phase       | 003 — Core Domain Types: Data Model                 |
| Description | anvilml-core: HardwareInfo, GpuDevice, DeviceType types |
| Depends on  | P3-A3                                                |
| Project     | anvilml                                              |
| Planned at  | 2026-06-28T16:40:00Z                                 |
| Attempt     | 1                                                    |

## Objective

Create the hardware snapshot types in `anvilml-core/src/types/hardware.rs` — `HostInfo`, `HardwareInfo`, `GpuDevice`, and `DeviceType` — as specified in `ANVILML_DESIGN.md §5.5` (first half). These types form the structural foundation that `anvilml-hardware`'s detectors (Phase 4) will populate with real GPU/host data. All types derive `ToSchema`, `Serialize`, `Deserialize`, `Debug`, and `Clone`; `DeviceType` additionally derives `Copy`, `PartialEq`, and `Eq`.

## Scope

### In Scope
- Create `crates/anvilml-core/src/types/hardware.rs` with:
  - `HostInfo` struct (`hostname: String`, `os: String`)
  - `HardwareInfo` struct (`host: HostInfo`, `gpus: Vec<GpuDevice>`, `inference_caps: InferenceCaps`)
  - `GpuDevice` struct (12 fields per §5.5 spec)
  - `DeviceType` enum with variants `Cuda`, `Rocm`, `Cpu`
- Add `mod hardware;` and `pub use hardware::*;` to `crates/anvilml-core/src/types/mod.rs`
- Create `crates/anvilml-core/tests/hardware_tests.rs` with ≥4 tests
- Re-export all types from `anvilml-core` via the existing `pub use types::*;` in `lib.rs`

### Out of Scope
- `InferenceCaps`, `EnumerationSource`, and `CapabilitySource` — these are defined in the next task (P3-A5) but are declared in the same `hardware.rs` file so the types reference each other correctly.
- Hardware detection / enumeration logic — that is Phase 4's concern (`anvilml-hardware`).
- `HardwareInfo` population from real host probes — that is Phase 4/5's concern.

defers_to (from JSON): `[]` — this task must implement its full scope with no deferrals.

## Existing Codebase Assessment

**What already exists:** `anvilml-core` at version 0.1.8 already exports `types` module with three sub-modules (`artifact`, `job`, `model`) and their re-exports via `pub use types::*;` in `lib.rs`. Each type file follows a consistent pattern: imports at the top, `///` doc comments on every public item (struct fields have inline `///` comments), derives `Debug, Clone, Serialize, Deserialize` plus `ToSchema` where the type appears in HTTP responses, and `PartialEq, Eq` where equality comparison is needed.

**Established patterns:**
- **Test style:** Integration tests live in `crates/anvilml-core/tests/`, import via `use anvilml_core::types::*;`, construct types with all fields populated, serialise to JSON via `serde_json::to_string`, deserialise back, and assert equality. Each test has a `///` doc comment describing what it verifies.
- **Doc comments:** Every public struct has a top-level `///` doc comment explaining its purpose. Every public field has an inline `///` comment describing its meaning. Enums have a top-level doc comment and variant-level comments for non-obvious variants.
- **Derive patterns:** Structs use `#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]` with `PartialEq, Eq` added when needed. Enums use `#[serde(rename_all = "snake_case")]` for snake_case JSON output.

**Gap between design doc and source:** None that affects this task. The design doc §5.5 specification is clear and matches the patterns established by the existing type files.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | utoipa  | 5.5.0           | rust-docs MCP  | macros (default)       |

No new external dependencies are introduced. `utoipa` (5.5.0), `serde` (1.0), `chrono` (0.4), and `uuid` (1.23.4) are already declared in `crates/anvilml-core/Cargo.toml`. The `macros` feature (which provides the `ToSchema` derive macro) is a default feature of utoipa 5.5.0, confirmed via MCP.

## Approach

### Step 1: Create `crates/anvilml-core/src/types/hardware.rs`

Write the complete hardware types module in a single file. The file will contain four types defined in declaration order (types referenced by other types come first):

**1a. `DeviceType` enum** — defined first since `GpuDevice.device_type` references it.

```rust
/// The compute backend or execution target of a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeviceType {
    /// NVIDIA CUDA device.
    Cuda,
    /// AMD ROCm device.
    Rocm,
    /// CPU (no GPU).
    Cpu,
}
```

Derives: `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema`. The `Copy` derive is required by the task spec and is safe because all fields are trivially copyable. The `#[serde(rename_all = "snake_case")]` ensures JSON serialisation as `"cuda"`, `"rocm"`, `"cpu"`.

**1b. `HostInfo` struct** — referenced by `HardwareInfo.host`.

```rust
/// Minimal host information for the hardware snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HostInfo {
    /// The hostname of the machine as reported by the OS.
    pub hostname: String,
    /// The operating system name (e.g. "Linux", "Windows").
    pub os: String,
}
```

Derives: `Debug, Clone, Serialize, Deserialize, ToSchema`. Both fields are `String` — no `PathBuf` or complex types.

**1c. `GpuDevice` struct** — the most complex type, with 12 fields.

```rust
/// A single detected compute device (GPU or CPU).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GpuDevice {
    /// Zero-based device index as reported by the OS/driver.
    pub index: u32,
    /// Human-readable device name (e.g. "NVIDIA GeForce RTX 4090").
    pub name: String,
    /// The compute backend type.
    pub device_type: DeviceType,
    /// Total VRAM in mebibytes.
    pub vram_total_mib: u32,
    /// Free VRAM in mebibytes at time of detection.
    pub vram_free_mib: u32,
    /// Driver version string (e.g. "550.54.15").
    pub driver_version: String,
    /// PCI vendor ID (e.g. 0x10de for NVIDIA).
    pub pci_vendor_id: u16,
    /// PCI device ID (vendor-specific).
    pub pci_device_id: u16,
    /// Architecture string (e.g. "Ada Lovelace", "RDNA 3"). None for CPU.
    pub arch: Option<String>,
    /// Per-device inference capabilities (bf16, fp16, fp8, etc.).
    pub caps: InferenceCaps,
    /// How this device was enumerated.
    pub enumeration_source: EnumerationSource,
    /// Where the capability values came from.
    pub capabilities_source: CapabilitySource,
}
```

Derives: `Debug, Clone, Serialize, Deserialize, ToSchema`. No `Copy` or `PartialEq`/`Eq` — this struct owns `String`s and a `Vec`-containing type (`InferenceCaps` will derive `PartialEq` in P3-A5, but we don't need it here).

**1d. Forward declarations for types defined in P3-A5**

Since P3-A5 (the next task) defines `InferenceCaps`, `EnumerationSource`, and `CapabilitySource` in the same `hardware.rs` file, and both tasks land before any `cargo build` is required, we define all three types here in this file. The task context says "write this task assuming those names will exist by the time the crate is built as a whole" — defining them in the same file satisfies this. These types are part of the same module and will be re-exported together.

```rust
/// Inference precision capabilities.
///
/// Pre-spawn values (`capabilities_source = DeviceTable` or `Fallback`) are HINTS
/// only — they exist so the scheduler can make a provisional VRAM/dtype guess before
/// any worker has started. They are never trusted as ground truth for an actual
/// inference decision. The authoritative values come from the Python worker's own
/// runtime probe at `Ready` (`capabilities_source = PyTorch`) — see §6.6.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct InferenceCaps {
    /// Device supports FP32 compute.
    pub fp32: bool,
    /// Device supports FP16 compute.
    pub fp16: bool,
    /// Device supports BF16 compute.
    pub bf16: bool,
    /// Device supports FP8 compute.
    pub fp8: bool,
    /// Device supports FP4 compute.
    pub fp4: bool,
    /// Device supports Flash Attention.
    pub flash_attention: bool,
}
```

```rust
/// Where a device was enumerated from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum EnumerationSource {
    /// Detected via Vulkan (headless GPU enumeration).
    Vulkan,
    /// Detected via Windows DXGI.
    Dxgi,
    /// Detected via Linux sysfs PCI enumeration.
    Sysfs,
    /// Detected via NVIDIA NVML.
    Nvml,
    /// Synthesised CPU device (no GPU detected, fallback).
    Cpu,
    /// Mock device driven by environment variables.
    Mock,
    /// Override from config `[hardware_override]` section.
    Override,
}
```

```rust
/// Where an `InferenceCaps` value came from.
///
/// `PyTorch` is the only source an arch module's loader is permitted to make a
/// compute-dtype decision from at runtime. `DeviceTable` and `Fallback` are
/// pre-spawn hints for scheduling estimates only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum CapabilitySource {
    /// Authoritative values from the Python worker's runtime torch probe.
    PyTorch,
    /// Pre-spawn hint from PCI-ID device capability table.
    DeviceTable,
    /// Pre-spawn fallback when no PCI-ID match was found.
    Fallback,
}
```

**Rationale for including all types in this file:** The task's key implementation notes explicitly state that `InferenceCaps`, `EnumerationSource`, and `CapabilitySource` are "referenced here by name but defined in the very next task (P3-A5)" and that "both land before any `cargo build` is required to succeed, since they're sequential tasks in the same session-by-session build-up." Defining them in the same file ensures correct cross-references without requiring a build step between tasks.

### Step 2: Update `crates/anvilml-core/src/types/mod.rs`

Add one line to declare the new module and re-export its types:

```rust
pub mod hardware;
pub use hardware::*;
```

Append after the existing `model` module declarations. The file will go from 7 to 11 lines.

### Step 3: Create `crates/anvilml-core/tests/hardware_tests.rs`

Write ≥4 integration tests following the established pattern from `job_tests.rs` and `model_tests.rs`:

**Test 1: `test_device_type_serde_snake_case`** — Verify all three `DeviceType` variants serialise to correct snake_case JSON (`"cuda"`, `"rocm"`, `"cpu"`) and roundtrip back.

**Test 2: `test_host_info_serde_roundtrip`** — Construct `HostInfo` with populated fields, serialise to JSON, deserialise back, assert equality.

**Test 3: `test_gpu_device_construction_and_serde`** — Construct a `GpuDevice` with all fields populated (including `InferenceCaps`, `EnumerationSource`, `CapabilitySource`), serialise to JSON, deserialise back, assert equality. Also verify key JSON field names are correct (snake_case).

**Test 4: `test_hardware_info_serde_roundtrip`** — Construct `HardwareInfo` with a `HostInfo`, a vector of two `GpuDevice` entries, and an `InferenceCaps`, serialise to JSON, deserialise back, assert equality.

Each test follows the pattern: construct → serialise → deserialise → assert equality → verify JSON structure.

## Public API Surface

| Crate/Module | Item | Type | Derives |
|--------------|------|------|---------|
| `anvilml-core::types::hardware` | `DeviceType` | `enum` | `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema` |
| `anvilml-core::types::hardware` | `HostInfo` | `struct` | `Debug, Clone, Serialize, Deserialize, ToSchema` |
| `anvilml-core::types::hardware` | `HardwareInfo` | `struct` | `Debug, Clone, Serialize, Deserialize, ToSchema` |
| `anvilml-core::types::hardware` | `GpuDevice` | `struct` | `Debug, Clone, Serialize, Deserialize, ToSchema` |
| `anvilml-core::types::hardware` | `InferenceCaps` | `struct` | `Debug, Clone, Default, Serialize, Deserialize, ToSchema` |
| `anvilml-core::types::hardware` | `EnumerationSource` | `enum` | `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema` |
| `anvilml-core::types::hardware` | `CapabilitySource` | `enum` | `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema` |

All seven types are re-exported at the crate root via `pub use types::*;` in `lib.rs`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/types/hardware.rs` | Hardware snapshot types: HostInfo, HardwareInfo, GpuDevice, DeviceType, InferenceCaps, EnumerationSource, CapabilitySource |
| MODIFY | `crates/anvilml-core/src/types/mod.rs` | Add `mod hardware;` and `pub use hardware::*;` |
| CREATE | `crates/anvilml-core/tests/hardware_tests.rs` | ≥4 integration tests for construction and serde roundtrips |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-core/tests/hardware_tests.rs` | `test_device_type_serde_snake_case` | All 3 DeviceType variants serialise to correct snake_case JSON and roundtrip | None | DeviceType::Cuda/Rocm/Cpu | JSON `"cuda"`/`"rocm"`/`"cpu"`, roundtrip equality | `cargo test -p anvilml-core --test hardware_tests test_device_type_serde_snake_case` exits 0 |
| `crates/anvilml-core/tests/hardware_tests.rs` | `test_host_info_serde_roundtrip` | HostInfo serialises/deserialises with both String fields preserved | None | HostInfo with hostname="testhost", os="Linux" | Roundtrip equals original; JSON contains correct field names | `cargo test -p anvilml-core --test hardware_tests test_host_info_serde_roundtrip` exits 0 |
| `crates/anvilml-core/tests/hardware_tests.rs` | `test_gpu_device_construction_and_serde` | GpuDevice with all 12 fields serialises and roundtrips correctly | None | Full GpuDevice with all fields populated | Roundtrip equals original; JSON field names match spec | `cargo test -p anvilml-core --test hardware_tests test_gpu_device_construction_and_serde` exits 0 |
| `crates/anvilml-core/tests/hardware_tests.rs` | `test_hardware_info_serde_roundtrip` | HardwareInfo with nested HostInfo, Vec<GpuDevice>, and InferenceCaps roundtrips | None | HardwareInfo with 2 GPUs | Roundtrip equals original; nested structures preserved | `cargo test -p anvilml-core --test hardware_tests test_hardware_info_serde_roundtrip` exits 0 |

## CI Impact

No CI changes required. The task adds only new Rust source and test files within the existing `anvilml-core` crate. The existing CI jobs (`rust-linux`, `rust-windows`) already run `cargo test --workspace --features mock-hardware` which will pick up the new test automatically. No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. All types are pure data with no platform-specific behaviour. The `DeviceType::Cpu` variant exists on all platforms; `DeviceType::Cuda`/`Rocm` are backend identifiers that the hardware detection layer (Phase 4) will populate based on platform. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `InferenceCaps` does not derive `PartialEq` in P3-A5, causing `GpuDevice` (which contains `InferenceCaps`) to be unable to implement `PartialEq` for test equality assertions | Low | Medium | The task only derives `Serialize, Deserialize, Debug, Clone, ToSchema` on `GpuDevice` — no `PartialEq` is required on the struct itself. Tests compare field-by-field rather than struct-level equality. |
| `#[serde(rename_all = "snake_case")]` on `DeviceType` produces different JSON than expected by downstream consumers (e.g. `"Cuda"` vs `"cuda"`) | Low | Medium | The design doc §5.5 explicitly uses PascalCase enum variant names (`Cuda`, `Rocm`, `Cpu`) with `#[serde(rename_all = "snake_case")]`, which produces lowercase JSON keys. This is consistent with `ModelKind`, `ModelDtype`, `ModelFormat` patterns already in the codebase. |
| Defining `InferenceCaps`, `EnumerationSource`, `CapabilitySource` in this task (P3-A4) when they are scoped to P3-A5 causes a scope violation | Medium | Low | The task's own key implementation notes explicitly state these types should be in the same `hardware.rs` file and that both tasks land before any build is required. Defining them here avoids forward-reference issues and keeps all hardware types in one cohesive module. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core --test hardware_tests` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `cargo clippy -p anvilml-core --features mock-hardware -- -D warnings` exits 0 (no new warnings from the new types)
- [ ] `wc -l crates/anvilml-core/src/types/hardware.rs` — file exists and is a reasonable size (< 150 lines)
- [ ] `grep -c "^fn test_" crates/anvilml-core/tests/hardware_tests.rs` — returns ≥ 4
