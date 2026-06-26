# Implementation Report: P2-A3

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P2-A3                                       |
| Phase         | 2 — Core Domain Types: Config & Errors      |
| Description   | anvilml-core: ServerConfig nested table structs |
| Implemented   | 2026-06-26T20:05:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Extended `ServerConfig` in `crates/anvilml-core/src/config.rs` with five nested-table fields and their corresponding struct definitions (`ModelDirConfig`, `GpuSelectionConfig`, `LimitsConfig`, `RocmConfig`, `HardwareOverrideConfig`), completing the struct to the shape expected by the TOML config file format. All five structs derive `Debug, Clone, Serialize, Deserialize`. The `Default` impl for `ServerConfig` was extended with appropriate defaults for each new field. Five new tests were added to `config_tests.rs`, bringing the total to 13 (8 scalar + 5 nested). The `ServerConfig` doc comment was updated to remove the deferred nested-tables sentence. Version bumped from 0.1.2 to 0.1.3.

## Resolved Dependencies

None. All derives use the existing `serde` crate (version 1.0, `derive` feature enabled) and the standard library's `#[derive(Debug)]` / `#[derive(Clone)]`. No new external dependencies introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/config.rs` | Add 5 nested structs, 5 ServerConfig fields, extend Default impl, update doc comment |
| Modify | `crates/anvilml-core/tests/config_tests.rs` | Add 5 new tests for nested struct defaults |
| Modify | `crates/anvilml-core/Cargo.toml` | Bump patch version 0.1.2 → 0.1.3 |
| Modify | `docs/TESTS.md` | Add 5 new test catalogue entries |
| Modify | `Cargo.lock` | Auto-updated by cargo |

## Commit Log

```
 Cargo.lock                                |  2 +-
 crates/anvilml-core/Cargo.toml            |  2 +-
 crates/anvilml-core/src/config.rs         | 69 +++++++++++++++++++++++++++++--
 crates/anvilml-core/tests/config_tests.rs | 35 ++++++++++++++++
 docs/TESTS.md                             | 60 +++++++++++++++++++++++++++
 5 files changed, 163 insertions(+), 5 deletions(-)
```

## Test Results

```
     Running tests/config_tests.rs (target/debug/deps/config_tests-36814a98ad73a63d)

running 13 tests
test test_artifact_dir_default ... ok
test test_db_path_default ... ok
test test_hardware_override_default ... ok
test test_host_default ... ok
test test_gpu_selection_default ... ok
test test_limits_default ... ok
test test_max_ipc_payload_mib_default ... ok
test test_model_dirs_default ... ok
test test_model_scan_depth_default ... ok
test test_num_threads_default ... ok
test test_port_default ... ok
test test_rocm_default ... ok
test test_venv_path_default ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

All 13 config tests passed (8 scalar + 5 nested). Full workspace test suite: 33 tests, 0 failures.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# Check 1: Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.66s
--- CHECK 1 PASSED ---

# Check 2: Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 21.31s
--- CHECK 2 PASSED ---

# Check 3: Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.53s
--- CHECK 3 PASSED ---

# Check 4: Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.57s
--- CHECK 4 PASSED ---
```

All four platform cross-checks passed.

## Project Gates

**Gate 1 — Config Surface Sync (`config_reference`):** 0 tests matched — the `config_reference` test is not yet present (deferred to P2-A7 per the approved plan). Gate 1 is not triggered until that test exists.

**Gate 2 — OpenAPI Drift:** Not triggered — no handler function signatures, `#[utoipa::path]` annotations, `ToSchema` derives, or `AppState` fields were modified.

**Gate 3 — Node Parity:** Not triggered — no node types in `worker/nodes/` were added, removed, or renamed, and `crates/anvilml-core/src/node_registry.rs` was not modified.

**Gate 4 — Mock/Real Parity Markers:** Not triggered — no node's `execute()` or arch module's `load()`/`sample()`/`decode()`/`compute_latent_shape()` was added or modified.

## Public API Delta

```
+pub struct ModelDirConfig {
+    pub path: PathBuf,
+    pub recursive: bool,
+    pub max_depth: Option<u32>,
+pub struct GpuSelectionConfig {
+    pub default_device: String,
+pub struct LimitsConfig {
+    pub max_queued_jobs: u32,
+pub struct RocmConfig {
+    pub hsa_override_gfx_version: Option<String>,
+pub struct HardwareOverrideConfig {
+    pub device_type: String,
+    pub vram_total_mib: u32,
+    pub model_dirs: Vec<ModelDirConfig>,
+    pub gpu_selection: GpuSelectionConfig,
+    pub limits: LimitsConfig,
+    pub rocm: Option<RocmConfig>,
+    pub hardware_override: Option<HardwareOverrideConfig>,
```

All 5 new pub structs and 5 new pub fields on `ServerConfig` match the plan's Public API Surface table exactly. No unexpected additions or removals.

## Deviations from Plan

None. Implementation matches the approved plan exactly.

## Blockers

None.
