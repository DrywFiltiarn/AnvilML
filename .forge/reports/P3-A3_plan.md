# Plan Report: P3-A3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P3-A3                                              |
| Phase       | 003 — Core Domain Types                           |
| Description | anvilml-core: hardware types (HardwareInfo, GpuDevice, InferenceCaps) |
| Depends on  | P3-A1, P3-A2                                      |
| Project     | anvilml                                            |
| Planned at  | 2026-06-14T18:10:00Z                              |
| Attempt     | 1                                                  |

## Objective

Create `crates/anvilml-core/src/types/hardware.rs` containing the hardware domain types
specified in `ANVILML_DESIGN.md §5.5`: `HardwareInfo`, `GpuDevice`, `DeviceType`,
`HostInfo`, `InferenceCaps`, `EnumerationSource`, and `CapabilitySource`. Update
`types/mod.rs` to declare the module and re-export all public types. Add `utoipa`'s
`ToSchema` derive to every public type for OpenAPI schema generation.

When complete, `cargo test -p anvilml-core -- types::hardware` exits 0 with ≥ 4 tests.
The types are pure data — zero I/O, zero async — serialisable via serde and
OpenAPI-schema-ready via utoipa.

## Scope

### In Scope
- Create `crates/anvilml-core/src/types/hardware.rs` with seven public types:
  - `HardwareInfo` struct (host, gpus, inference_caps fields)
  - `GpuDevice` struct (index, name, device_type, vram_total_mib, vram_free_mib,
    driver_version, pci_vendor_id, pci_device_id, arch, caps, enumeration_source,
    capabilities_source fields)
  - `DeviceType` enum (Cuda, Rocm, Cpu variants)
  - `HostInfo` struct (os, cpu, ram_total_mib fields — inferred from context;
    design doc §5.5 references `HostInfo` as a field type in `HardwareInfo` but
    does not provide an explicit struct definition)
  - `InferenceCaps` struct (fp32, fp16, bf16, fp8, fp4, flash_attention fields,
    derives `Default`)
  - `EnumerationSource` enum (Vulkan, Dxgi, Sysfs, Nvml, Mock, Override variants)
  - `CapabilitySource` enum (PyTorch, DeviceTable, Fallback variants)
- Update `crates/anvilml-core/src/types/mod.rs`: add `pub mod hardware;` and
  re-export all seven types via `pub use hardware::...`.
- Update `crates/anvilml-core/src/lib.rs`: add re-exports for the new public types
  (following the established pattern of re-exporting at the crate root).
- Create `crates/anvilml-core/tests/hardware_tests.rs` with ≥ 4 tests.
- No new external dependencies — all types use existing workspace deps (serde,
  chrono, uuid, utoipa).

### Out of Scope
- Hardware detection logic (owned by `anvilml-hardware` crate, future tasks).
- VRAM refresh or dynamic capability updates (runtime concerns).
- Serialization format beyond serde JSON (msgpack handling is in `anvilml-ipc`).
- `#[tracing::instrument]` annotations — this crate has zero I/O and these types
  are pure data; logging belongs in the detection crate.
- Doc comments on `#[serde(...)]` attributes — only `///` doc comments on `pub`
  items (per §12.1).

## Existing Codebase Assessment

Phase 003 is mid-flight. `anvilml-core` already has `types/job.rs`, `types/model.rs`,
and `types/artifact.rs` with established patterns:

(a) **What exists:** `Job`, `JobStatus`, `JobSettings`, `ModelMeta`, `ModelKind`,
`ModelDtype`, `ModelFormat`, and `ArtifactMeta` — all using `#[derive(Debug, Clone,
Serialize, Deserialize, ToSchema)]` with `///` doc comments on every `pub` item.
Enums use `#[serde(rename_all = "snake_case")]`. Structs with optional fields use
`Option<T>`. The `types/mod.rs` re-exports via `pub use`. Tests live in
`crates/anvilml-core/tests/` as separate test-crate files.

(b) **Established patterns:**
- Doc comments: `///` on every `pub` item, one-sentence summary followed by a
  paragraph of context. Enum variants have `///` comments too.
- Derives: `Debug, Clone, Serialize, Deserialize, ToSchema` on all public types.
  `Copy + PartialEq + Eq` on enums with simple discriminants. `Default` only when
  semantically meaningful (e.g., `InferenceCaps`, `ArtifactMeta`).
- Serde attributes: `#[serde(rename_all = "snake_case")]` on enums. No special
  field renaming needed for structs — Rust field names already match the OpenAPI
  contract (snake_case).
- ToSchema: all types derive `utoipa::ToSchema` for OpenAPI generation.
- Tests: integration test crate files in `tests/`, using doc comments on test
  functions describing what they verify and their preconditions.

(c) **Gap/Discrepancy:** The design doc §5.5 references `HostInfo` as the type of
`HardwareInfo.host` but does not provide an explicit struct definition for `HostInfo`
in the code block. This is an omission in the design doc. The plan proposes a minimal
`HostInfo { os: String, cpu: String, ram_total_mib: u32 }` based on the logical
context of a "host snapshot" — this is a low-risk assumption that can be adjusted if
the task author's intent differs.

## Resolved Dependencies

No new external dependencies are introduced. All types use existing workspace
dependencies already declared in `crates/anvilml-core/Cargo.toml`:

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|-----------|-----------------|----------------|------------------------|
| crate  | serde     | 1.0.228         | Cargo.lock     | derive                  |
| crate  | serde_json| 1.0.150         | Cargo.lock     | (none)                  |
| crate  | utoipa    | 5.5.0           | Cargo.lock     | macros, chrono, uuid   |
| crate  | chrono    | 0.4.45          | Cargo.lock     | serde                   |
| crate  | uuid      | 1.23.3          | Cargo.lock     | serde, v4               |

The `utoipa` crate's `ToSchema` derive macro is available via the `macros` feature
flag, which is already enabled in the workspace dependency declaration. No API shape
verification was needed beyond confirming the derive is available — the types are
plain data structs and enums, not external API consumers.

## Approach

1. **Create `crates/anvilml-core/src/types/hardware.rs`.** Write all seven types:

   a. `DeviceType` enum (Cuda, Rocm, Cpu). Derive `Debug, Clone, Copy, PartialEq,
   Eq, Serialize, Deserialize, ToSchema`. Apply `#[serde(rename_all = "snake_case")]`.
   Add `///` doc comment on the enum and each variant explaining the backend it maps to.
   Rationale: `Copy` is appropriate because `DeviceType` is a simple discriminant with
   no heap data — matching the pattern used by `ModelKind`, `ModelDtype`, and `JobStatus`.

   b. `EnumerationSource` enum (Vulkan, Dxgi, Sysfs, Nvml, Mock, Override). Derive
   `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema`. Apply
   `#[serde(rename_all = "snake_case")]`. Add `///` doc comments. Rationale: same
   pattern as `DeviceType` — simple discriminant.

   c. `CapabilitySource` enum (PyTorch, DeviceTable, Fallback). Derive identically
   to `EnumerationSource`. Add `///` doc comments.

   d. `InferenceCaps` struct with fields: `fp32: bool`, `fp16: bool`, `bf16: bool`,
   `fp8: bool`, `fp4: bool`, `flash_attention: bool`. Derive `Debug, Clone, Default,
   Serialize, Deserialize, ToSchema`. Add `///` doc comment on the struct and each field.
   Rationale: `Default` is required — the design doc explicitly states "pre-spawn values
   are hints" so a zero-value (all false) is a valid initial state before the Python
   worker reports actual capabilities. The `Default` derive produces `false` for all
   bool fields, which is the correct "unknown/unspecified" initial state.

   e. `HostInfo` struct with fields: `os: String`, `cpu: String`, `ram_total_mib: u32`.
   Derive `Debug, Clone, Serialize, Deserialize, ToSchema`. Add `///` doc comments.
   Rationale: The design doc references `HostInfo` as `HardwareInfo.host` but provides
   no explicit struct definition. These three fields capture the minimal host-level
   information needed for a hardware snapshot: OS identity, CPU model, and total system
   RAM. This is a reasonable inference from the context of hardware detection.

   f. `GpuDevice` struct with fields: `index: u32`, `name: String`, `device_type:
   DeviceType`, `vram_total_mib: u32`, `vram_free_mib: u32`, `driver_version: String`,
   `pci_vendor_id: u16`, `pci_device_id: u16`, `arch: Option<String>`, `caps:
   InferenceCaps`, `enumeration_source: EnumerationSource`, `capabilities_source:
   CapabilitySource`. Derive `Debug, Clone, Serialize, Deserialize, ToSchema`. Add
   `///` doc comments on the struct and each field. Rationale: `arch` is `Option<String>`
   because not all detection backends (e.g., NVML) report the GPU architecture string —
   `None` is a valid value when the backend does not provide it.

   g. `HardwareInfo` struct with fields: `host: HostInfo`, `gpus: Vec<GpuDevice>`,
   `inference_caps: InferenceCaps`. Derive `Debug, Clone, Serialize, Deserialize,
   ToSchema`. Add `///` doc comment on the struct and each field. Rationale:
   `inference_caps` is the union of all per-device caps, as stated in the design doc.

   All `///` doc comments follow the established pattern: one-sentence summary, then
   a paragraph explaining context, non-obvious preconditions, or usage notes. Each field
   gets a `///` comment.

2. **Update `crates/anvilml-core/src/types/mod.rs`.** Add:
   ```rust
   pub mod hardware;
   pub use hardware::{CapabilitySource, DeviceType, EnumerationSource, GpuDevice,
     HardwareInfo, HostInfo, InferenceCaps};
   ```
   Update the module-level doc comment to mention hardware types.

3. **Update `crates/anvilml-core/src/lib.rs`.** Add re-exports for the seven new types
   to the existing `pub use types::...` block:
   ```rust
   CapabilitySource, DeviceType, EnumerationSource, GpuDevice, HardwareInfo, HostInfo,
   InferenceCaps,
   ```

4. **Create `crates/anvilml-core/tests/hardware_tests.rs`.** Write ≥ 4 tests:
   - `test_hardware_info_json_roundtrip`: Create a fully-populated `HardwareInfo`,
     serialise to JSON, deserialise back, assert all fields equal. Tests the nested
     `GpuDevice` and `InferenceCaps` roundtrip.
   - `test_device_type_variants`: Serialise each `DeviceType` variant through JSON,
     assert equality. Verifies `#[serde(rename_all = "snake_case")]`.
   - `test_inference_caps_default`: Assert `InferenceCaps::default()` has all bool
     fields set to `false`. Verifies the `Default` derive produces the expected
     "unknown" initial state.
   - `test_enum_variants_roundtrip`: Serialise each variant of `EnumerationSource` and
     `CapabilitySource` through JSON, assert equality. Tests both enums in one test.

5. **Run `cargo test -p anvilml-core -- types::hardware`** to verify all tests pass.

## Public API Surface

All items are `pub` in `crates/anvilml-core/src/types/hardware.rs`, re-exported from
`anvilml_core` crate root:

| Item | Type | Module Path | Derives |
|------|------|-------------|---------|
| `HardwareInfo` | struct | `types::hardware` | Debug, Clone, Serialize, Deserialize, ToSchema |
| `GpuDevice` | struct | `types::hardware` | Debug, Clone, Serialize, Deserialize, ToSchema |
| `DeviceType` | enum | `types::hardware` | Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema |
| `HostInfo` | struct | `types::hardware` | Debug, Clone, Serialize, Deserialize, ToSchema |
| `InferenceCaps` | struct | `types::hardware` | Debug, Clone, Default, Serialize, Deserialize, ToSchema |
| `EnumerationSource` | enum | `types::hardware` | Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema |
| `CapabilitySource` | enum | `types::hardware` | Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema |

Re-exported from crate root (`anvilml_core::...`): all seven types above.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/types/hardware.rs` | Seven hardware domain types per ANVILML_DESIGN.md §5.5 |
| MODIFY | `crates/anvilml-core/src/types/mod.rs` | Add `pub mod hardware;` and re-exports |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Add re-exports for seven new types |
| CREATE | `crates/anvilml-core/tests/hardware_tests.rs` | ≥ 4 integration tests |
| Modify | `crates/anvilml-core/Cargo.toml` | Bump patch version 0.1.4 → 0.1.5 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-core/tests/hardware_tests.rs` | `test_hardware_info_json_roundtrip` | A fully-populated `HardwareInfo` serialises to JSON and deserialises back to an identical value, including nested `GpuDevice`, `HostInfo`, and `InferenceCaps`. | None. Tests use in-memory data only. | Constructed `HardwareInfo` with two `GpuDevice` entries, mixed `Option<String>` values. | `assert_eq!(restored, original)` passes for all fields. | `cargo test -p anvilml-core -- types::hardware test_hardware_info_json_roundtrip` exits 0 |
| `crates/anvilml-core/tests/hardware_tests.rs` | `test_device_type_variants` | All three `DeviceType` variants (Cuda, Rocm, Cpu) roundtrip through JSON serialisation. | None. | Each variant individually serialised via `serde_json::to_string`. | Each variant deserialises back to itself; no data loss. | `cargo test -p anvilml-core -- types::hardware test_device_type_variants` exits 0 |
| `crates/anvilml-core/tests/hardware_tests.rs` | `test_inference_caps_default` | `InferenceCaps::default()` produces all-false bool fields, representing the "unknown" initial state before worker reporting. | None. | `InferenceCaps::default()` | All six bool fields are `false`. | `cargo test -p anvilml-core -- types::hardware test_inference_caps_default` exits 0 |
| `crates/anvilml-core/tests/hardware_tests.rs` | `test_enum_variants_roundtrip` | All variants of `EnumerationSource` and `CapabilitySource` survive JSON roundtrip. | None. | Each of 6 + 3 = 9 enum variants serialised then deserialised. | All 9 variants equal their original values. | `cargo test -p anvilml-core -- types::hardware test_enum_variants_roundtrip` exits 0 |

## CI Impact

No CI changes required. The new test file `crates/anvilml-core/tests/hardware_tests.rs`
is picked up automatically by `cargo test --workspace --features mock-hardware` which
compiles all test crates under `crates/*/tests/`. No new CI jobs, gates, or configuration
files are modified.

## Platform Considerations

None identified. The types are pure data with no platform-specific branches, no
`#[cfg(unix)]` / `#[cfg(windows)]` guards, and no path handling. The `String` fields
(`name`, `driver_version`, `arch`, `os`, `cpu`) carry UTF-8 text which serde handles
portably. The `u16` PCI IDs are platform-neutral integer types. The Windows cross-check
in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `HostInfo` fields are unspecified in the design doc §5.5 — the ACT agent may need to revise the struct definition if the task author has different fields in mind. | Medium | Low (fields can be added/removed without breaking serde compatibility if new fields have defaults or are Option). | Define `HostInfo` with three reasonable fields (`os`, `cpu`, `ram_total_mib`). If the ACT agent discovers the intended fields differ, the serde `#[serde(default)]` pattern on new optional fields ensures backward compatibility. Document any change under `## Deviations from Plan`. |
| The `utoipa::ToSchema` derive may not be available for types with `Option<String>` fields if the `chrono` feature is not properly forwarded. | Low | Medium | The workspace `utoipa` dependency already includes `["macros", "chrono", "uuid"]` features. Verify at build time — if the derive fails, add `chrono` feature to the crate-level `utoipa` dep. |
| Adding re-exports to `lib.rs` could cause a circular dependency if any hardware type references a type from another crate that transitively depends on `anvilml-core`. | Low | High | All hardware types are self-contained within `anvilml-core` and reference only types defined in this crate or standard library. No cross-crate type references exist. The dependency graph confirms `anvilml-core` has no dependencies on other workspace crates. |
| Test file naming conflict: if a future task adds a test file with the same base name. | Low | Low | The naming convention `hardware_tests.rs` is unique to this module and follows the pattern established by `job_tests.rs`, `model_tests.rs`, and `artifact_tests.rs`. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core -- types::hardware` exits 0 with ≥ 4 tests passing
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (no warnings in new or modified files)
- [ ] `cargo fmt --all -- --check` exits 0 (no formatting drift)
- [ ] `cargo check --workspace --features mock-hardware` exits 0 (full workspace compiles)
- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0 (Windows cross-check)
