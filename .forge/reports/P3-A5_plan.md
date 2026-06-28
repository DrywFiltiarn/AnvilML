# Plan Report: P3-A5

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P3-A5                                                       |
| Phase       | 003 — Core Domain Types: Data Model                         |
| Description | anvilml-core: InferenceCaps, EnumerationSource, CapabilitySource |
| Depends on  | P3-A4                                                       |
| Project     | anvilml                                                     |
| Planned at  | 2026-06-28T17:15:00Z                                        |
| Attempt     | 1                                                           |

## Objective

Complete the hardware type module in `crates/anvilml-core/src/types/hardware.rs` by adding `InferenceCaps`, `EnumerationSource`, and `CapabilitySource` types with their derives, doc comments, and serde annotations — plus sufficient test coverage (>=9 total tests in `hardware_tests.rs`) so that `cargo test -p anvilml-core --test hardware_tests` exits 0.

## Scope

### In Scope
- `InferenceCaps` struct with 6 `bool` fields (`fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention`), deriving `Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema`; `Default` produces all-`false` values.
- `EnumerationSource` enum with 7 variants (`Vulkan, Dxgi, Sysfs, Nvml, Cpu, Mock, Override`), deriving `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema`; `#[serde(rename_all = "snake_case")]`.
- `CapabilitySource` enum with 3 variants (`PyTorch, DeviceTable, Fallback`), deriving `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema`; `#[serde(rename_all = "snake_case")]`.
- Doc comments per the design spec: `InferenceCaps` pre-spawn values are hints only; `CapabilitySource::PyTorch` is the only source an arch loader may use for runtime dtype decisions; `EnumerationSource::Cpu` marks a synthesized device.
- >=5 tests covering `InferenceCaps::default()` and each enum variant's serde roundtrip in `hardware_tests.rs`.
- `cargo test -p anvilml-core --test hardware_tests` exits 0 with >=9 total tests.

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope. No stubs, no deferred functionality.

## Existing Codebase Assessment

The types `InferenceCaps`, `EnumerationSource`, and `CapabilitySource` already exist in `crates/anvilml-core/src/types/hardware.rs` with the correct fields, derives, doc comments, and serde annotations as specified in ANVILML_DESIGN.md §5.5. This is because the task's prerequisite (P3-A4) and this task (P3-A5) were merged in the same session — they both modify `hardware.rs` and no `cargo build` is required to succeed between them.

Established patterns to follow:
- Tests use `anvilml_core::types::*` re-exports from `crates/anvilml-core/tests/` integration test files.
- Serde roundtrip tests construct a value, serialise to JSON via `serde_json::to_string`, deserialise back, and assert equality.
- Enum variant tests iterate over a `[(Variant, &str)]` array, checking both JSON string output and roundtrip equality.
- All test files follow the `test_<type>_<property>` naming convention.

Gap between design doc and current source: None. The source already matches the design spec exactly. The only gap is test count: 7 tests exist in `hardware_tests.rs`, but the acceptance criteria requires >=9 total.

## Resolved Dependencies

| Type   | Name   | Version verified | MCP source   | Feature flags confirmed |
|--------|--------|-----------------|--------------|------------------------|
| crate  | utoipa | 5.5.0           | rust-docs MCP | uuid, chrono (in Cargo.toml) |

The `ToSchema` derive macro is provided by `utoipa` itself. Version 5.5.0's `utoipa::ToSchema` trait exists and supports derive macros on both structs and enums. No new dependency is introduced — `utoipa` is already declared in `anvilml-core`'s `Cargo.toml`.

## Approach

1. **Verify existing types match the spec.** Confirm that `hardware.rs` already contains `InferenceCaps` (6 bool fields, Default all-false), `EnumerationSource` (7 variants with Cpu as 5th, Copy+PartialEq+Eq), and `CapabilitySource` (3 variants, Copy+PartialEq+Eq) with correct derives, doc comments, and `#[serde(rename_all = "snake_case")]`. No code changes needed — this step is a verification pass.

2. **Add `test_inference_caps_non_default_roundtrip` to `hardware_tests.rs`.** Construct an `InferenceCaps` with mixed true/false values (e.g., `fp32: true, fp16: true, bf16: true, fp8: false, fp4: false, flash_attention: true`), serialise to JSON, deserialise back, assert equality. Also verify JSON field names (`fp32`, `fp16`, etc.) via `serde_json::Value` parsing. This is the 8th test.

3. **Add `test_enumeration_source_copy_trait` to `hardware_tests.rs`.** Verify that `EnumerationSource` implements `Copy` by assigning a variant to a new variable and using both. Also verify `CapabilitySource` implements `Copy` the same way. This is the 9th test.

4. **Run the acceptance command.** `cargo test -p anvilml-core --test hardware_tests` must exit 0 with >=9 tests. If it does not, diagnose and fix.

5. **Update `docs/TESTS.md`** per `FORGE_AGENT_RULES.md §5.10` and `ENVIRONMENT.md §11.4` — add entries for the two new tests with Mode, context, inputs, expected outputs.

## Public API Surface

No new public items are added. The task only adds tests for existing types:

| Item | Crate/Module Path | Status |
|------|-------------------|--------|
| `InferenceCaps` | `anvilml_core::types::InferenceCaps` | Already exists |
| `EnumerationSource` | `anvilml_core::types::EnumerationSource` | Already exists |
| `CapabilitySource` | `anvilml_core::types::CapabilitySource` | Already exists |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-core/tests/hardware_tests.rs` | Add 2 new tests: `test_inference_caps_non_default_roundtrip` and `test_enumeration_source_copy_trait` |
| MODIFY | `docs/TESTS.md` | Add entries for the 2 new tests (per §5.10 test catalogue sync) |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-core/tests/hardware_tests.rs` | `test_inference_caps_non_default_roundtrip` | `InferenceCaps` with mixed true/false fields serialises to correct JSON, roundtrips to equal value, and JSON field names are correct | None | `InferenceCaps { fp32: true, fp16: true, bf16: true, fp8: false, fp4: false, flash_attention: true }` | Roundtrip equality + JSON field assertions | `cargo test -p anvilml-core --test hardware_tests test_inference_caps_non_default_roundtrip` exits 0 |
| `crates/anvilml-core/tests/hardware_tests.rs` | `test_enumeration_source_copy_trait` | Both `EnumerationSource` and `CapabilitySource` implement `Copy` — assigning a variant to a new variable does not move it | None | `EnumerationSource::Cpu`, `CapabilitySource::PyTorch` | Both variables usable after assignment (Copy semantics) | `cargo test -p anvilml-core --test hardware_tests test_enumeration_source_copy_trait` exits 0 |

## CI Impact

No CI changes required. The tests added are integration tests in `crates/anvilml-core/tests/`, which are automatically picked up by `cargo test --workspace --features mock-hardware` (the CI Rust test command). No new CI jobs, gates, or file patterns are introduced.

## Platform Considerations

None identified. The types are pure data with no platform-specific behaviour. `#[cfg(unix)]` / `#[cfg(windows)]` guards are not needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The existing `hardware.rs` types may have been modified by a prior task with subtle differences from the design spec (e.g., missing derives, wrong field names) that would cause compilation failure when tests reference them. | Low | High | Step 1 of the approach verifies every derive, field name, variant name, doc comment, and serde attribute against the design spec before writing tests. If any mismatch is found, the plan is updated with the correct values. |
| Adding `test_enumeration_source_copy_trait` could fail if `Copy` is not derived on one of the enums due to a prior refactoring. | Low | Medium | The test itself is the verification — if it fails to compile, that confirms `Copy` is missing and the type definition must be corrected as part of this task. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core --test hardware_tests` exits 0 with >=9 tests total
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (no warnings in modified files)
- [ ] `cargo fmt --all -- --check` exits 0 (formatted code)
