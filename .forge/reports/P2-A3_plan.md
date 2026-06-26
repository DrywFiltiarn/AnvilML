# Plan Report: P2-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P2-A3                                       |
| Phase       | 2 — Core Domain Types: Config & Errors      |
| Description | anvilml-core: ServerConfig nested table structs |
| Depends on  | P2-A2                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-26T18:59:00Z                        |
| Attempt     | 1                                           |

## Objective

Extend `ServerConfig` in `crates/anvilml-core/src/config.rs` with five nested-table fields (`model_dirs`, `gpu_selection`, `limits`, `rocm`, `hardware_override`) and their corresponding struct definitions, completing the struct to the shape expected by the TOML config file format. This task receives the scope explicitly deferred by P2-A2, finishing the `ServerConfig` type so that subsequent tasks (config loading, main.rs wiring, config_reference drift test) can operate on a complete schema.

## Scope

### In Scope
- Define five nested structs in `crates/anvilml-core/src/config.rs`:
  - `ModelDirConfig { path: PathBuf, recursive: bool, max_depth: Option<u32> }`
  - `GpuSelectionConfig { default_device: String }`
  - `LimitsConfig { max_queued_jobs: u32 }`
  - `RocmConfig { hsa_override_gfx_version: Option<String> }`
  - `HardwareOverrideConfig { device_type: String, vram_total_mib: u32 }`
- Add five fields to the existing `ServerConfig` struct (P2-A2's 8 scalar fields remain untouched).
- Extend `ServerConfig::Default` impl to include defaults for all five new fields.
- Write ≥5 new tests in `crates/anvilml-core/tests/config_tests.rs` — one per nested struct — asserting its default value.
- All new structs derive `Debug, Clone, Serialize, Deserialize`.

### Out of Scope
None. This task's `defers_to` field is empty (`[]` from JSON). No scope is deferred. The `anvilml.toml` update and the `config_reference` drift test are explicitly scoped to P2-A7. The `config_load` implementation is scoped to P2-A4/P2-A5. The `backend/main.rs` wiring is scoped to P2-A6.

## Existing Codebase Assessment

**What already exists:** `ServerConfig` is defined in `crates/anvilml-core/src/config.rs` with eight scalar fields (`host`, `port`, `db_path`, `artifact_dir`, `venv_path`, `model_scan_depth`, `max_ipc_payload_mib`, `num_threads`) and a correct `Default` impl, all established by P2-A2. The crate's `lib.rs` re-exports `ServerConfig`. The `config_tests.rs` integration test file has 8 tests, one per scalar field, each asserting the compiled-in default against the value from `ENVIRONMENT.md §4`.

**Established patterns:**
- Structs derive `#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]` on a single line.
- Doc comments use `///` format with a one-line summary for each field.
- `PathBuf` defaults use `PathBuf::from("...")`.
- Tests live in `crates/{name}/tests/` as separate test crate files (not inline `#[cfg(test)]`).
- Each test has a `///` doc comment explaining what it asserts.
- The existing codebase uses `serde` (with `derive` feature) for serialization — confirmed in `Cargo.toml`.

**Gap between design doc and current source:** None. The design doc (§15) and `ENVIRONMENT.md §4` describe the nested table fields that this task will add. The current `ServerConfig` matches the design for scalar fields. The comment on `ServerConfig`'s doc comment explicitly notes that nested tables are deferred to P2-A3.

## Resolved Dependencies

No new external dependencies are introduced. All derives (`Debug`, `Clone`, `Serialize`, `Deserialize`) use the existing `serde` crate (version 1.0, `derive` feature enabled) and the standard library's `#[derive(Debug)]` / `#[derive(Clone)]`.

| Type   | Name    | Version verified | MCP source | Feature flags confirmed |
|--------|---------|-----------------|------------|------------------------|
| crate  | serde   | 1.0 (existing)  | Cargo.lock | derive                   |

## Approach

1. **Read existing `config.rs` to confirm P2-A2's eight scalar fields are present.** The file has 43 lines with the full scalar struct and Default impl. No changes to these fields — they remain verbatim.

2. **Define five nested structs above `ServerConfig`** in `config.rs`, each with doc comments, all deriving `Debug, Clone, serde::Serialize, serde::Deserialize`:

   ```rust
   /// Configuration for a single model directory entry.
   ///
   /// Used as an element of `ServerConfig::model_dirs`.
   #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
   pub struct ModelDirConfig {
       /// Directory path to scan for models.
       pub path: PathBuf,
       /// Whether to scan subdirectories recursively.
       pub recursive: bool,
       /// Maximum scan depth when `recursive = true`. Caps at
       /// `ServerConfig::model_scan_depth` if both are set.
       pub max_depth: Option<u32>,
   }

   /// GPU selection preferences.
   #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
   pub struct GpuSelectionConfig {
       /// Default device selector: `"auto"`, `"cpu"`, or integer device index as string.
       pub default_device: String,
   }

   /// Resource limits for the scheduler.
   #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
   pub struct LimitsConfig {
       /// Maximum jobs allowed in `Queued` state simultaneously.
       pub max_queued_jobs: u32,
   }

   /// Optional ROCm configuration overrides.
   #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
   pub struct RocmConfig {
       /// Override `HSA_OVERRIDE_GFX_VERSION` for unsupported GFX targets.
       pub hsa_override_gfx_version: Option<String>,
   }

   /// Optional hardware override for CI and isolated testing.
   ///
   /// NEVER include in a release build or production config.
   #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
   pub struct HardwareOverrideConfig {
       /// Device type: `"cuda"`, `"rocm"`, or `"cpu"`.
       pub device_type: String,
       /// VRAM to report in MiB.
       pub vram_total_mib: u32,
   }
   ```

   Each struct follows the established pattern: doc comment (one-line summary, with multi-line for structs that need context like `HardwareOverrideConfig`), `#[derive(...)]` on a single line, and `pub` fields with doc comments.

3. **Add five fields to `ServerConfig`** after `num_threads`, using the same field ordering convention as `ENVIRONMENT.md §4` (array table first, then nested tables, then optional sections):

   ```rust
   /// Model directories to scan.
   pub model_dirs: Vec<ModelDirConfig>,
   /// GPU selection preferences.
   pub gpu_selection: GpuSelectionConfig,
   /// Resource limits.
   pub limits: LimitsConfig,
   /// Optional ROCm configuration.
   pub rocm: Option<RocmConfig>,
   /// Optional hardware override for CI/testing.
   pub hardware_override: Option<HardwareOverrideConfig>,
   ```

4. **Extend `ServerConfig::Default`** to include defaults for the five new fields:

   ```rust
   impl Default for ServerConfig {
       fn default() -> Self {
           Self {
               host: "127.0.0.1".to_string(),
               port: 8488,
               db_path: PathBuf::from("./anvilml.db"),
               artifact_dir: PathBuf::from("./artifacts"),
               venv_path: PathBuf::from("./worker/.venv"),
               model_scan_depth: 2,
               max_ipc_payload_mib: 256,
               num_threads: None,
               // Nested table defaults (P2-A3):
               model_dirs: Vec::new(),
               gpu_selection: GpuSelectionConfig { default_device: "auto".to_string() },
               limits: LimitsConfig { max_queued_jobs: 100 },
               rocm: None,
               hardware_override: None,
           }
       }
   }
   ```

   The `Vec::new()` default for `model_dirs` produces an empty vec (the TOML `[[model_dirs]]` array would be absent). The `None` defaults for optional sections match the design doc. `"auto"` for `default_device` matches `ENVIRONMENT.md §3.2`.

5. **Update the `ServerConfig` doc comment** to remove the "nested tables are added by P2-A3" sentence, since they are now present:

   Change:
   ```
   /// Fields are loaded through a four-layer precedence chain:
   /// defaults → TOML → environment variables → CLI flags.
   /// Only the scalar fields are defined here; nested tables
   /// (`model_dirs`, `gpu_selection`, `limits`, `rocm`, `hardware_override`)
   /// are added by P2-A3.
   ```
   To:
   ```
   /// Fields are loaded through a four-layer precedence chain:
   /// defaults → TOML → environment variables → CLI flags.
   ```

6. **Write five new tests** in `crates/anvilml-core/tests/config_tests.rs`, one per nested struct, following the established pattern (doc comment + test that creates `ServerConfig::default()` and asserts the field):

   ```rust
   /// `ServerConfig::default().model_dirs` is an empty vec.
   #[test]
   fn test_model_dirs_default() {
       let config = ServerConfig::default();
       assert!(config.model_dirs.is_empty());
   }

   /// `ServerConfig::default().gpu_selection.default_device` equals `"auto"`.
   #[test]
   fn test_gpu_selection_default() {
       let config = ServerConfig::default();
       assert_eq!(config.gpu_selection.default_device, "auto");
   }

   /// `ServerConfig::default().limits.max_queued_jobs` equals `100`.
   #[test]
   fn test_limits_default() {
       let config = ServerConfig::default();
       assert_eq!(config.limits.max_queued_jobs, 100);
   }

   /// `ServerConfig::default().rocm` is `None`.
   #[test]
   fn test_rocm_default() {
       let config = ServerConfig::default();
       assert!(config.rocm.is_none());
   }

   /// `ServerConfig::default().hardware_override` is `None`.
   #[test]
   fn test_hardware_override_default() {
       let config = ServerConfig::default();
       assert!(config.hardware_override.is_none());
   }
   ```

   These 5 new tests bring the total to 13, exceeding both the ≥9 minimum and the ≥5 per-nested-struct requirement.

7. **Verify the file compiles** with `cargo check -p anvilml-core --features mock-hardware` before writing the report. (This is a pre-stop verification step in the ACT session; the plan assumes it will pass since no new dependencies are introduced.)

## Public API Surface

| Item | Crate/Module | Signature / Definition |
|------|-------------|----------------------|
| `pub struct ModelDirConfig` | `anvilml_core::config` | `pub struct ModelDirConfig { pub path: PathBuf, pub recursive: bool, pub max_depth: Option<u32> }` — derives Debug, Clone, Serialize, Deserialize |
| `pub struct GpuSelectionConfig` | `anvilml_core::config` | `pub struct GpuSelectionConfig { pub default_device: String }` — derives Debug, Clone, Serialize, Deserialize |
| `pub struct LimitsConfig` | `anvilml_core::config` | `pub struct LimitsConfig { pub max_queued_jobs: u32 }` — derives Debug, Clone, Serialize, Deserialize |
| `pub struct RocmConfig` | `anvilml_core::config` | `pub struct RocmConfig { pub hsa_override_gfx_version: Option<String> }` — derives Debug, Clone, Serialize, Deserialize |
| `pub struct HardwareOverrideConfig` | `anvilml_core::config` | `pub struct HardwareOverrideConfig { pub device_type: String, pub vram_total_mib: u32 }` — derives Debug, Clone, Serialize, Deserialize |
| `ServerConfig::model_dirs` | `anvilml_core::config` | New field: `pub model_dirs: Vec<ModelDirConfig>` |
| `ServerConfig::gpu_selection` | `anvilml_core::config` | New field: `pub gpu_selection: GpuSelectionConfig` |
| `ServerConfig::limits` | `anvilml_core::config` | New field: `pub limits: LimitsConfig` |
| `ServerConfig::rocm` | `anvilml_core::config` | New field: `pub rocm: Option<RocmConfig>` |
| `ServerConfig::hardware_override` | `anvilml_core::config` | New field: `pub hardware_override: Option<HardwareOverrideConfig>` |

Note: These structs are `pub` within the `config` module but are not re-exported at the crate root (`lib.rs`). They are accessible as `anvilml_core::config::ModelDirConfig`, etc. This follows the established pattern — `ServerConfig` is the primary public type; nested configs are module-level pub for use by downstream crates that import from `config` directly when needed.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/config.rs` | Add five nested structs, five ServerConfig fields, extend Default impl, update doc comment |
| Modify | `crates/anvilml-core/tests/config_tests.rs` | Add five new tests for nested struct defaults |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-core/tests/config_tests.rs` | `test_model_dirs_default` | `ServerConfig::default().model_dirs` is an empty vec | None | Default config | `config.model_dirs.is_empty()` is true | `cargo test -p anvilml-core --test config_tests -- test_model_dirs_default` exits 0 |
| `crates/anvilml-core/tests/config_tests.rs` | `test_gpu_selection_default` | `ServerConfig::default().gpu_selection.default_device` equals `"auto"` | None | Default config | `config.gpu_selection.default_device == "auto"` | `cargo test -p anvilml-core --test config_tests -- test_gpu_selection_default` exits 0 |
| `crates/anvilml-core/tests/config_tests.rs` | `test_limits_default` | `ServerConfig::default().limits.max_queued_jobs` equals `100` | None | Default config | `config.limits.max_queued_jobs == 100` | `cargo test -p anvilml-core --test config_tests -- test_limits_default` exits 0 |
| `crates/anvilml-core/tests/config_tests.rs` | `test_rocm_default` | `ServerConfig::default().rocm` is `None` | None | Default config | `config.rocm.is_none()` is true | `cargo test -p anvilml-core --test config_tests -- test_rocm_default` exits 0 |
| `crates/anvilml-core/tests/config_tests.rs` | `test_hardware_override_default` | `ServerConfig::default().hardware_override` is `None` | None | Default config | `config.hardware_override.is_none()` is true | `cargo test -p anvilml-core --test config_tests -- test_hardware_override_default` exits 0 |

## CI Impact

No CI changes required. The task modifies only source and test files within `anvilml-core`. The existing CI jobs (`rust-linux`, `rust-windows`) already run `cargo test --workspace --features mock-hardware` which includes this crate's tests. No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The task introduces no platform-specific code — no `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. All five structs and their defaults are platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `serde::Serialize`/`serde::Deserialize` on `PathBuf` may produce non-portable TOML — `PathBuf` serialises as a string, which is fine for TOML, but the roundtrip may differ between Unix and Windows paths. | Low | Medium | The config drift test (P2-A7) validates roundtrip via `config_load::load()`. This task only adds the types; the roundtrip is tested later. For now, `PathBuf` derives are standard and work correctly with `toml` crate. |
| Adding fields to `ServerConfig` may break downstream crates that construct `ServerConfig` directly (e.g., test fixtures) rather than going through `Default` or `config_load::load()`. | Low | Medium | At this point in Phase 2, no downstream crate has been written yet that constructs `ServerConfig` directly. P2-A4 (config_load) and P2-A6 (main.rs wiring) are the first consumers. The `Default` impl change ensures any code using `ServerConfig::default()` continues to compile. |
| Test count confusion — the acceptance criterion says "≥5 tests for nested structs" AND "≥9 total". With 8 existing scalar tests + 5 new nested tests = 13 total, both thresholds are met. | Low | Low | The plan explicitly writes 5 new tests, bringing total to 13. This exceeds both requirements with margin. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-core --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-core --test config_tests` exits 0 with ≥13 tests collected (8 scalar + 5 nested)
- [ ] `cargo test -p anvilml-core --test config_tests -- test_model_dirs_default` exits 0
- [ ] `cargo test -p anvilml-core --test config_tests -- test_gpu_selection_default` exits 0
- [ ] `cargo test -p anvilml-core --test config_tests -- test_limits_default` exits 0
- [ ] `cargo test -p anvilml-core --test config_tests -- test_rocm_default` exits 0
- [ ] `cargo test -p anvilml-core --test config_tests -- test_hardware_override_default` exits 0
- [ ] `cargo clippy -p anvilml-core --features mock-hardware -- -D warnings` exits 0
