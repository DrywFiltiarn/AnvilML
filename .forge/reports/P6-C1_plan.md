# Plan Report: P6-C1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P6-C1                                             |
| Phase       | 006 — Model Registry                              |
| Description | anvilml-core: add serde snake_case to FrontendMode and DeviceType config enums |
| Depends on  | P6-B2                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-06-04T10:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add `#[serde(rename_all = "snake_case")]` to the `FrontendMode` and `DeviceType` enums in `crates/anvilml-core/src/config.rs` so that serde serialises and deserialises variant names as lowercase strings (`"cuda"`, `"rocm"`, `"cpu"`, `"headless"`, `"local"`, `"remote"`). This aligns them with the already-correct `ModelKind` and `DType` enums and fixes the startup panic caused by `anvilml.toml` using lowercase values that the current deserialiser rejects.

## Scope

### In Scope
- Add `#[serde(rename_all = "snake_case")]` attribute to the `DeviceType` enum in `crates/anvilml-core/src/config.rs`.
- Add `#[serde(rename_all = "snake_case")]` attribute to the `FrontendMode` enum in `crates/anvilml-core/src/config.rs`.
- Update test `device_type_json_strings` in `crates/anvilml-core/src/types/hardware.rs`: change assertions from `"Cuda"`, `"Rocm"`, `"Cpu"` to `"cuda"`, `"rocm"`, `"cpu"`.
- Update test `gpu_device_backward_compat` in `crates/anvilml-core/src/types/hardware.rs`: change hardcoded JSON `"device_type": "Cuda"` to `"device_type": "cuda"`.

### Out of Scope
- No changes to `anvilml.toml` — it already uses lowercase values throughout.
- No changes to `EnumerationSource` or `CapabilitySource` — the task explicitly excludes these internal runtime types which must retain their current PascalCase serialisation.
- No new tests created; only existing tests are updated.
- No changes to CI workflow files, OpenAPI docs, or other crates.

## Approach

1. **Edit `crates/anvilml-core/src/config.rs`** — Add the attribute line `#[serde(rename_all = "snake_case")]` directly above the `DeviceType` enum definition (line 30) and above the `FrontendMode` enum definition (line 81). These are the only two lines changed in this file.

2. **Edit `crates/anvilml-core/src/types/hardware.rs`** — In the `device_type_json_strings` test (lines 389–398), update the three assertions:
   - Line 391: `"\"Cuda\"" → "\"cuda\""`
   - Line 394: `"\"Rocm\"" → "\"rocm\""`
   - Line 397: `"\"Cpu\"" → "\"cpu\""`

3. **Edit `crates/anvilml-core/src/types/hardware.rs`** — In the `gpu_device_backward_compat` test (lines 240–274), update the JSON literal on line 245:
   - `"device_type": "Cuda"` → `"device_type": "cuda"`

4. **Verify** — Run `cargo test --workspace --features mock-hardware` to confirm all tests pass. Run `cargo run --bin anvilml -- --print-hardware` to confirm no config-load panic.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/config.rs` | Add `#[serde(rename_all = "snake_case")]` to `DeviceType` and `FrontendMode` enums |
| Modify | `crates/anvilml-core/src/types/hardware.rs` | Update `device_type_json_strings` and `gpu_device_backward_compat` test assertions to lowercase |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-core/src/types/hardware.rs` | `device_type_json_strings` | DeviceType serialises to `"cuda"`, `"rocm"`, `"cpu"` (snake_case) |
| `crates/anvilml-core/src/types/hardware.rs` | `gpu_device_backward_compat` | GpuDevice deserialises from JSON with `"device_type": "cuda"` and maps to `DeviceType::Cuda` |
| `crates/anvilml-core/src/config.rs` | `test_toml_roundtrip` | ServerConfig round-trips through TOML (includes DeviceType/FrontendMode) |
| `crates/anvilml-core/src/types/hardware.rs` | `gpu_device_roundtrip` | GpuDevice round-trips through JSON (device_type serialised as snake_case) |

## CI Impact

No CI workflow file changes. The task only modifies Rust source and test files in `anvilml-core`. The existing CI matrix (`rust`, `rust-windows`, `python-worker`, `openapi-diff`) covers this crate via the standard `cargo test --workspace --features mock-hardware` command. No new CI steps are needed.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Breaking change to `GET /v1/hardware` JSON response shape: `device_type` field changes from `"Cuda"` to `"cuda"`. | Acceptable pre-release — no external clients exist yet. Document in implementation report under Deviations from Plan. |
| Other code depends on the old PascalCase serialisation strings (e.g. string comparisons, pattern matching on JSON). | All other usage goes through Rust enum variants and round-trip serde; grep for `"Cuda"` / `"Rocm"` / `"Cpu"` in non-test source to confirm no hard-coded string dependencies exist. The task scope explicitly excludes `EnumerationSource`/`CapabilitySource`. |
| TOML config file (`anvilml.toml`) might need updating if it used PascalCase values. | Verified: `anvilml.toml` already uses lowercase (`mode = "headless"`, `device_type = "cpu"`). No changes needed. |

## Acceptance Criteria

- [ ] `#[serde(rename_all = "snake_case")]` added to both `DeviceType` and `FrontendMode` in `crates/anvilml-core/src/config.rs`
- [ ] `device_type_json_strings` test assertions updated to `"cuda"`, `"rocm"`, `"cpu"`
- [ ] `gpu_device_backward_compat` JSON literal changed to `"device_type": "cuda"`
- [ ] `cargo test --workspace --features mock-hardware` exits 0
- [ ] `cargo run --bin anvilml -- --print-hardware` completes without config-load panic
