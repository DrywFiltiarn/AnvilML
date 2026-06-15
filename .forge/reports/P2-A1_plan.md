# Plan Report: P2-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P2-A1                                             |
| Phase       | 002 — Config & Graceful Shutdown                  |
| Description | ServerConfig struct with all fields and Default impl |
| Depends on  | P1 (Phase 001 complete — binary builds, `/health` served) |
| Project     | anvilml                                           |
| Planned at  | 2026-06-14T12:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Define the `ServerConfig` struct and all nested configuration structs in `crates/anvilml-core/src/config.rs`, implementing `Default`, `Serialize`, and `Deserialize` derives with documented default values. This establishes the type authority for the entire configuration system that Phase 002's config loading (P2-A2) and CLI wiring (P2-B1) depend on. The observable outcome is: `cargo test -p anvilml-core -- config` exits 0 with ≥ 3 passing tests verifying default values, serialisation roundtrip, and env-override-compatible values.

## Scope

### In Scope
- **CREATE** `crates/anvilml-core/src/config.rs` — `ServerConfig` struct with all 13 fields and all 5 nested structs (`ModelDirConfig`, `GpuSelectionConfig`, `LimitsConfig`, `RocmConfig`, `HardwareOverrideConfig`) as `pub` items, all in the same file.
- **MODIFY** `crates/anvilml-core/src/lib.rs` — add `pub mod config;` and `pub use config::*;` (or specific `pub use` exports).
- Implement `Default` for `ServerConfig` and all nested structs using documented defaults from `ENVIRONMENT.md §4` and `ANVILML_DESIGN.md §14.1`.
- Derive `Serialize + Deserialize` on all structs.
- **CREATE** `crates/anvilml-core/tests/config_tests.rs` — ≥ 3 tests: default values, serialisation roundtrip, env-override-compatible values.

### Out of Scope
- Config loading from TOML or env vars (P2-A2).
- CLI argument parsing (P2-B1).
- `anvilml.toml` file creation or modification (created in a later phase when config loading is wired).
- The `error.rs` file with `AnvilError` enum (created in a later phase).
- The `types/` subdirectory with domain types (created in a later phase).
- `config_reference` integration test (Phase 003).

## Existing Codebase Assessment

`anvilml-core` is the foundational crate in the dependency graph — it has zero runtime dependencies beyond `serde`, `serde_json`, and `uuid`. Currently, its `src/` directory contains only `lib.rs` with a stub `pub fn stub()`. No other source files exist: there is no `config.rs`, no `error.rs`, no `types/` subdirectory. The crate's `Cargo.toml` already declares `serde` (with `derive` feature from workspace), `serde_json`, and `uuid` as dependencies — no new crate dependencies are needed for this task.

The `lib.rs` follows the established pattern: it begins with a `//!` crate-level doc comment describing the crate's purpose and hard constraints ("Zero I/O. Zero async. Zero network."). The crate uses workspace-level dependency declarations (`workspace = true` for version/edition).

Phase 001 already established the workspace structure, `rust-toolchain.toml`, and a minimal `backend/src/main.rs` that serves `/health`. The `anvilml-core` stub is the placeholder awaiting this phase's type definitions.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source     | Feature flags confirmed |
|--------|-------------|-----------------|----------------|------------------------|
| crate  | serde       | 1.0.228         | Workspace Cargo.toml | derive              |
| crate  | serde_json  | 1.0.150         | Workspace Cargo.toml | (none)             |

No new external dependencies are introduced. `serde` and `serde_json` are already declared in `crates/anvilml-core/Cargo.toml`. The `toml` crate needed for config loading (P2-A2) is not introduced here — that task adds it separately.

## Approach

1. **Write `crates/anvilml-core/src/config.rs`** containing all 5 nested structs and `ServerConfig`:
   - `ModelDirConfig`: fields `path: PathBuf`, `recursive: bool` (default `false`), `max_depth: Option<u32>` (default `None`).
   - `GpuSelectionConfig`: field `default_device: String` (default `"auto"`).
   - `LimitsConfig`: fields `max_queued_jobs: u32` (default `100`), `max_concurrent_jobs: u32` (default `1`).
   - `RocmConfig`: field `hsa_override_gfx_version: Option<String>` (default `None`).
   - `HardwareOverrideConfig`: fields `device_type: String` (default `"cpu"`), `vram_total_mib: u32` (default `8192`).
   - `ServerConfig`: all 13 fields in the exact order from `ANVILML_DESIGN.md §14.1`, with `seeds_path` at the end as specified.
   - All structs derive `Debug, Clone, Serialize, Deserialize`. Nested structs also derive `Default`.
   - Rationale: Deriving `Default` on nested structs allows `ServerConfig::default()` to be implemented as a single expression using struct update syntax, keeping the impl clean and readable.

2. **Implement `Default` for `ServerConfig`** using documented defaults from `ENVIRONMENT.md §4`:
   ```rust
   impl Default for ServerConfig {
       fn default() -> Self {
           Self {
               host: "127.0.0.1".to_string(),
               port: 8488,
               db_path: PathBuf::from("./anvilml.db"),
               artifact_dir: PathBuf::from("./artifacts"),
               num_threads: None,
               venv_path: PathBuf::from("./worker/.venv"),
               max_ipc_payload_mib: 256,
               model_dirs: Vec::new(),
               gpu_selection: GpuSelectionConfig::default(),
               limits: LimitsConfig::default(),
               rocm: None,
               hardware_override: None,
               seeds_path: PathBuf::from("./database/seeds"),
           }
       }
   }
   ```
   - Rationale: `num_threads: None` means "use num_cpus" — the loader in P2-A2 will resolve this. `model_dirs: Vec::new()` means "no model directories configured" — the scanner will have no directories to walk until config loading populates them. Both `Option` fields (`rocm`, `hardware_override`) default to `None` because they are optional configuration sections that are only present when explicitly set.
   - `hardware_override` defaults to `"cpu"` device type with `8192` MiB VRAM — these are the same defaults used by the `mock-hardware` feature's `MockDetector`, ensuring consistency between compile-time and runtime mocking.

3. **Update `crates/anvilml-core/src/lib.rs`** — add `pub mod config;` and `pub use config::{ServerConfig, ModelDirConfig, GpuSelectionConfig, LimitsConfig, RocmConfig, HardwareOverrideConfig};` after the existing `//!` doc comment and before the stub function. The stub function is removed since this is the first real module being added.

4. **Write `crates/anvilml-core/tests/config_tests.rs`** — three tests:
   - `test_default_values`: Assert `ServerConfig::default()` fields match documented defaults (host, port, db_path, artifact_dir, venv_path, seeds_path, max_ipc_payload_mib, gpu_selection defaults, limits defaults). This is the acceptance gate for correctness of the `Default` impl.
   - `test_serialisation_roundtrip`: Serialize `ServerConfig::default()` to JSON via `serde_json::to_string`, deserialize back via `serde_json::from_str`, assert equality. Verifies that `Serialize`/`Deserialize` derives produce correct field mappings and that `PathBuf` fields round-trip as strings correctly.
   - `test_env_override_values`: Construct a `ServerConfig` with values that mimic what environment variable overrides would produce (e.g., `host = "0.0.0.0"`, `port = 9001`, `max_ipc_payload_mib = 512`, `rocm = Some(RocmConfig { hsa_override_gfx_version: Some("gfx942".into()) })`), serialize and deserialize, assert all overridden values are preserved. This tests that the struct correctly handles non-default values including `Option` variants.

## Public API Surface

| Item | Type | Module Path | Description |
|------|------|-------------|-------------|
| `ServerConfig` | `pub struct` | `anvilml_core::config::ServerConfig` | Top-level server configuration with 13 fields |
| `ModelDirConfig` | `pub struct` | `anvilml_core::config::ModelDirConfig` | Single model directory entry (path, recursive, max_depth) |
| `GpuSelectionConfig` | `pub struct` | `anvilml_core::config::GpuSelectionConfig` | GPU selection policy (default_device) |
| `LimitsConfig` | `pub struct` | `anvilml_core::config::LimitsConfig` | Job queue and concurrency limits |
| `RocmConfig` | `pub struct` | `anvilml_core::config::RocmConfig` | ROCm-specific settings (hsa_override_gfx_version) |
| `HardwareOverrideConfig` | `pub struct` | `anvilml_core::config::HardwareOverrideConfig` | Hardware override for CI/testing (device_type, vram_total_mib) |
| `ServerConfig::default()` | `impl Default` | `anvilml_core::config::ServerConfig` | Returns ServerConfig with documented defaults |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/config.rs` | ServerConfig + 5 nested structs, Default impl |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Add `pub mod config;` and `pub use` exports; remove stub |
| CREATE | `crates/anvilml-core/tests/config_tests.rs` | 3 tests: default values, roundtrip, env-override values |
| Modify | `crates/anvilml-core/Cargo.toml` | Bump patch version 0.1.0 → 0.1.1 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-core/tests/config_tests.rs` | `test_default_values` | All `ServerConfig` and nested struct fields match documented defaults from ENVIRONMENT.md §4 | `ServerConfig::default()` is callable | None | All assertions pass; no field deviates from documented default | `cargo test -p anvilml-core -- config` exits 0 |
| `crates/anvilml-core/tests/config_tests.rs` | `test_serialisation_roundtrip` | `Serialize`/`Deserialize` roundtrip preserves all field values including `PathBuf` (string) and `Option` fields | `ServerConfig::default()` is serialisable | `ServerConfig::default()` | `from_str(&to_string(&cfg)) == cfg` | `cargo test -p anvilml-core -- config` exits 0 |
| `crates/anvilml-core/tests/config_tests.rs` | `test_env_override_values` | Non-default values including `Option::Some` variants survive serialisation roundtrip | Config with overridden values constructed | `ServerConfig` with host=`0.0.0.0`, port=`9001`, rocm=`Some(...)` | All overridden values preserved after roundtrip | `cargo test -p anvilml-core -- config` exits 0 |

## CI Impact

No CI changes required. The task adds a new test module under `crates/anvilml-core/tests/` which is picked up by `cargo test --workspace --features mock-hardware` (the standard CI test command). No new CI jobs or gates are introduced. The `config` test filter in the acceptance criterion is a subset of the full workspace test suite.

## Platform Considerations

None identified. The `ServerConfig` struct is platform-neutral: `PathBuf` correctly handles platform-specific path separators on both Linux and Windows. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The `String` fields (`host`, `default_device`, `device_type`, `hsa_override_gfx_version`) are platform-independent text. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Field order in `ServerConfig` differs between `ANVILML_DESIGN.md §14.1` and the task description, causing a mismatch with downstream consumers that expect a specific field order. | Low | Medium | Follow `ANVILML_DESIGN.md §14.1` as the authoritative source (design doc takes precedence over task description). Document the field order explicitly in the Approach section. |
| `PathBuf` serialisation via `serde_json` may produce platform-dependent paths (e.g., `./anvilml.db` on Linux vs `.\anvilml.db` on Windows), causing the roundtrip test to fail on Windows. | Low | Medium | The default paths use Unix-style separators (`./anvilml.db`, `./artifacts`, etc.) which are valid as JSON strings on all platforms. The roundtrip test deserialises from the same string that was serialised — it does not compare against a hardcoded path. This is safe on both platforms. |
| The `anvilml.toml` file does not exist yet, so the `config_reference` gate (Phase 003) will fail until that file is created. This is expected and does not block this task. | N/A | Low | Out of scope — acknowledged in Scope section. The gate will be addressed in the phase that creates `anvilml.toml`. |
| `lib.rs` currently contains `pub fn stub()` which will become dead code after adding `pub mod config`. Removing it is necessary to keep `lib.rs` clean. | Low | Low | Remove the stub function entirely — it was a Phase 001 placeholder and has no downstream references. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core -- config` exits 0 with ≥ 3 passing tests
- [ ] `head -1 .forge/reports/P2-A1_plan.md` prints `# Plan Report: P2-A1`
- [ ] `grep "^## " .forge/reports/P2-A1_plan.md` shows exactly 11 section headings
- [ ] `wc -l .forge/reports/P2-A1_plan.md` returns a value > 40
