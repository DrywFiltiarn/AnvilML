# Plan Report: P2-A5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P2-A5                                       |
| Phase       | 002 — Core Domain Types: Config & Errors    |
| Description | anvilml-core: config_load env var + CLI flag layers |
| Depends on  | P2-A4                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-26T21:50:00Z                        |
| Attempt     | 1                                           |

## Objective

Extend `config_load::load()` in `crates/anvilml-core/src/config_load.rs` to implement layers 3–4 of the four-layer config precedence chain: scan `ANVILML_*` environment variables (with `__` nested-field convention) and apply them after the TOML merge, then accept an optional `CliOverrides` struct applied last as the highest-precedence layer. This completes the config loading contract that `backend/main.rs` will call in P2-A6.

## Scope

### In Scope
- Define `pub struct CliOverrides { host: Option<String>, port: Option<u16> }` in `config_load.rs`.
- Extend `load()` signature from `load(toml_path: Option<&Path>)` to `load(toml_path: Option<&Path>, cli_overrides: Option<CliOverrides>)`.
- Implement env var scan: read each `ANVILML_*` variable, parse `__`-separated nested keys, override matching `ServerConfig` fields.
- Apply CLI overrides (`host`, `port`) last, after env vars.
- Update `lib.rs` to re-export `CliOverrides`.
- Add ≥5 new tests in `config_load_tests.rs` (≥9 total in file).
- Bump `anvilml-core` patch version 0.1.4 → 0.1.5.

### Out of Scope
None. This task's `defers_to` is `[]` — no scope is deferred. All described functionality is implemented in full.

## Existing Codebase Assessment

The existing `config_load.rs` (P2-A4) implements layers 1–2: compiled-in defaults merged with an optional TOML file. The `load()` function starts from `ServerConfig::default()`, reads the TOML into a `toml::Value`, and applies field-by-field overrides using per-field `if let` guards. Nested structs are handled by five helper functions (`apply_model_dirs`, `apply_gpu_selection`, `apply_limits`, `apply_rocm`, `apply_hardware_override`).

The existing `config.rs` defines `ServerConfig` with all scalar fields, `GpuSelectionConfig`, `LimitsConfig`, `RocmConfig`, `HardwareOverrideConfig`, and `ModelDirConfig` — all derive `Serialize`/`Deserialize`. The `Default` impl is complete.

The existing test file `config_load_tests.rs` has 6 tests covering missing file fallback, partial TOML override, malformed TOML error, full round-trip, default path resolution, and nested struct partial override. Tests follow the project's pattern of using temp files, comparing against `ServerConfig::default()`, and cleaning up after themselves.

No new external dependencies are needed — the env var scan uses `std::env::var` and `std::env::var_as_integer` (Rust 1.96). The `toml` crate at 1.1.2 (confirmed in `Cargo.lock`) is already a dependency.

## Resolved Dependencies

None. This task uses only `std::env` (built-in) and existing workspace dependencies (`toml`, `serde`, `serde_json`). No new crates are introduced.

## Approach

### Step 1: Define `CliOverrides` struct

In `config_load.rs`, add a public struct:

```rust
/// CLI flag overrides for config loading.
///
/// Applied as the final (highest-precedence) layer after environment variables.
/// Only `host` and `port` are exposed here — other fields are overridden via env vars.
#[derive(Debug, Clone)]
pub struct CliOverrides {
    /// HTTP bind address override. `None` means no override.
    pub host: Option<String>,
    /// HTTP port override. `None` means no override.
    pub port: Option<u16>,
}
```

### Step 2: Extend `load()` signature

Change the function signature from:

```rust
pub fn load(toml_path: Option<&Path>) -> Result<ServerConfig, AnvilError>
```

to:

```rust
pub fn load(toml_path: Option<&Path>, cli_overrides: Option<CliOverrides>) -> Result<ServerConfig, AnvilError>
```

The function body after the TOML merge (after line 93 in the current file) will:
1. Call a new `apply_env_vars(&mut config)` helper.
2. Apply `cli_overrides` if `Some`.

### Step 3: Implement `apply_env_vars()` helper

Add a new `fn apply_env_vars(config: &mut ServerConfig)` that reads each `ANVILML_*` environment variable and overrides the matching field. The env var name is derived from the config field path by:
- Converting field names to `UPPER_SNAKE_CASE`.
- Using `ANVILML_` prefix.
- Using `__` (double underscore) to separate nested struct paths.

The mapping (from `ENVIRONMENT.md §3`):

| Env Var | Config Field | Parse Type |
|---------|-------------|------------|
| `ANVILML_HOST` | `host` | `String` via `var()` |
| `ANVILML_PORT` | `port` | `u16` via `var_as_u16()` |
| `ANVILML_DB_PATH` | `db_path` | `PathBuf` from `String` |
| `ANVILML_ARTIFACT_DIR` | `artifact_dir` | `PathBuf` from `String` |
| `ANVILML_VENV_PATH` | `venv_path` | `PathBuf` from `String` |
| `ANVILML_MODEL_SCAN_DEPTH` | `model_scan_depth` | `u32` via `var_as_u32()` |
| `ANVILML_MAX_IPC_PAYLOAD_MIB` | `max_ipc_payload_mib` | `u32` via `var_as_u32()` |
| `ANVILML_NUM_THREADS` | `num_threads` | `Option<u32>` via `var_as_u32()` |
| `ANVILML_GPU_SELECTION__DEFAULT_DEVICE` | `gpu_selection.default_device` | `String` via `var()` |

Each env var is read with `std::env::var(name)`. If `Ok(value)`, the corresponding field is overridden. If `Err(NotPresent)`, the variable is silently skipped — the prior layer's value (defaults or TOML) is retained. This implements the "unset vars leave prior layer's value" behavior.

For the nested field `ANVILML_GPU_SELECTION__DEFAULT_DEVICE`, the `__` separator splits into `gpu_selection` and `default_device`. The implementation checks if the first part is `"GPU_SELECTION"` (uppercase of the nested struct name) and the second part is `"DEFAULT_DEVICE"` (uppercase of the field name), then applies to `config.gpu_selection.default_device`.

### Step 4: Apply CLI overrides after env vars

After `apply_env_vars(config)`, if `cli_overrides` is `Some(overrides)`:
- If `overrides.host.is_some()`, set `config.host = overrides.host.unwrap()`.
- If `overrides.port.is_some()`, set `config.port = overrides.port.unwrap()`.

This ensures CLI flags have the highest precedence, overriding both TOML and env var values.

### Step 5: Update `lib.rs` re-export

Add `pub use config_load::CliOverrides;` to `lib.rs` so downstream crates can construct `CliOverrides`.

### Step 6: Write tests in `config_load_tests.rs`

Add the following new tests (the file currently has 6 tests, bringing total to 11):

1. **`test_env_var_overrides_toml_value`** — Write a TOML with `host = "0.0.0.0"`, set `ANVILML_HOST = "10.0.0.1"`, call `load()`, assert `config.host == "10.0.0.1"`. Verifies env var beats TOML.

2. **`test_env_var_overrides_default_no_toml`** — Call `load(Some(nonexistent_path), None)` with `ANVILML_PORT = "9999"` set, assert `config.port == 9999`. Verifies env var beats defaults when no TOML.

3. **`test_cli_override_beats_env_var`** — Set `ANVILML_HOST = "10.0.0.1"`, call `load()` with `Some(CliOverrides { host: Some("127.0.0.2".into()), port: None })`, assert `config.host == "127.0.0.2"`. Verifies CLI beats env var.

4. **`test_nested_env_var_gpu_selection`** — Set `ANVILML_GPU_SELECTION__DEFAULT_DEVICE = "cuda"`, call `load()`, assert `config.gpu_selection.default_device == "cuda"`. Verifies `__` nested field parsing.

5. **`test_unset_env_vars_leave_prior_layer_value`** — Write a TOML with `host = "0.0.0.0"`, do NOT set `ANVILML_HOST`, call `load()`, assert `config.host == "0.0.0.0"`. Verifies unset vars preserve the prior layer.

6. **`test_env_var_port_override`** — Set `ANVILML_PORT = "7777"`, call `load()`, assert `config.port == 7777`. Verifies scalar numeric env var parsing.

7. **`test_num_threads_env_var`** — Set `ANVILML_NUM_THREADS = "4"`, call `load()`, assert `config.num_threads == Some(4)`. Verifies `Option<u32>` env var parsing.

### Step 7: Bump version

Bump `crates/anvilml-core/Cargo.toml` version from `0.1.4` to `0.1.5`.

## Public API Surface

| Item | Path | Signature |
|------|------|-----------|
| `struct CliOverrides` | `anvilml_core::CliOverrides` | `pub struct CliOverrides { pub host: Option<String>, pub port: Option<u16> }` |
| `fn load` | `anvilml_core::config_load::load` | `pub fn load(toml_path: Option<&Path>, cli_overrides: Option<CliOverrides>) -> Result<ServerConfig, AnvilError>` |

The `load` signature change is a breaking API change (new parameter), but no downstream code calls `load()` yet — P2-A6 is the first consumer.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/config_load.rs` | Add `CliOverrides` struct, extend `load()` signature, implement `apply_env_vars()`, add CLI override logic |
| Modify | `crates/anvilml-core/src/lib.rs` | Add `pub use config_load::CliOverrides;` re-export |
| Modify | `crates/anvilml-core/tests/config_load_tests.rs` | Add 7 new tests for env var and CLI override layers |
| Modify | `crates/anvilml-core/Cargo.toml` | Bump patch version 0.1.4 → 0.1.5 |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `config_load_tests.rs` | `test_env_var_overrides_toml_value` | Env var `ANVILML_HOST` overrides a TOML-set `host` value | `cargo test -p anvilml-core --test config_load_tests -- test_env_var_overrides_toml_value` exits 0 |
| `config_load_tests.rs` | `test_env_var_overrides_default_no_toml` | Env var `ANVILML_PORT` overrides compiled default when no TOML file | `cargo test -p anvilml-core --test config_load_tests -- test_env_var_overrides_default_no_toml` exits 0 |
| `config_load_tests.rs` | `test_cli_override_beats_env_var` | `CliOverrides { host }` overrides an env var-set `host` | `cargo test -p anvilml-core --test config_load_tests -- test_cli_override_beats_env_var` exits 0 |
| `config_load_tests.rs` | `test_nested_env_var_gpu_selection` | `ANVILML_GPU_SELECTION__DEFAULT_DEVICE` correctly parses nested field via `__` separator | `cargo test -p anvilml-core --test config_load_tests -- test_nested_env_var_gpu_selection` exits 0 |
| `config_load_tests.rs` | `test_unset_env_vars_leave_prior_layer_value` | Unset `ANVILML_HOST` preserves TOML-set value (no override) | `cargo test -p anvilml-core --test config_load_tests -- test_unset_env_vars_leave_prior_layer_value` exits 0 |
| `config_load_tests.rs` | `test_env_var_port_override` | `ANVILML_PORT` parses as `u16` correctly | `cargo test -p anvilml-core --test config_load_tests -- test_env_var_port_override` exits 0 |
| `config_load_tests.rs` | `test_num_threads_env_var` | `ANVILML_NUM_THREADS` parses as `Option<u32>` correctly | `cargo test -p anvilml-core --test config_load_tests -- test_num_threads_env_var` exits 0 |

Total tests in file: 6 (existing) + 7 (new) = 13. Requirement is ≥9.

All tests that call `std::env::set_var` will be annotated `#[serial]` and will capture/restore the prior env value unconditionally, per `ENVIRONMENT.md §11.3` and `FORGE_AGENT_RULES.md §5.10`.

## CI Impact

No CI changes required. The `cargo test --workspace --features mock-hardware` CI job already picks up all tests in `crates/anvilml-core/tests/`. No new file types, no new gates.

## Platform Considerations

None identified. Environment variable names (`ANVILML_*`) and the `__` nested-field convention are platform-neutral. `std::env::var` and `std::env::var_as_u16` work identically on Linux and Windows. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `std::env::var_as_u16()` / `var_as_u32()` API may not exist in Rust 1.96 (they were stabilized in 1.67, but need to confirm the exact method name). | Low | Medium | Verify the method exists in `rustc --version` docs; if the method is `var_as_integer()` instead, adjust accordingly. The existing codebase uses `toml`'s `as_integer()` — the std equivalent is `var_as_u16()` stabilized in 1.67. |
| Env var parsing errors (e.g., `ANVILML_PORT = "abc"`) could panic or produce incorrect values. | Medium | High | Use `var_as_u16()` which returns `Err` on parse failure; skip the field on error (same as "not present") rather than panicking. Log a WARN at DEBUG level via `tracing` if we had tracing available, but since `anvilml-core` is zero-I/O, silently skip with a comment explaining why. |
| The `load()` signature change breaks P2-A6's call site (which hasn't been written yet). | Low | High | P2-A6 is the first consumer of `load()`. The plan for P2-A6 (read at session start) shows it passes `Some(CliOverrides { ... })` from CLI parsing — the signature change is expected and coordinated. |
| Env var names with `__` convention could collide with field names containing underscores. | Low | Low | The env var list in `ENVIRONMENT.md §3` is exhaustive and fixed — only `ANVILML_GPU_SELECTION__DEFAULT_DEVICE` uses `__`. No other field path contains underscores. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core --test config_load_tests` exits 0 with ≥9 tests
- [ ] `cargo clippy -p anvilml-core --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `cargo check -p anvilml-core --features mock-hardware` exits 0
- [ ] `grep -c "fn test_" crates/anvilml-core/tests/config_load_tests.rs` returns ≥13
- [ ] `grep 'version = "0.1.5"' crates/anvilml-core/Cargo.toml` matches
- [ ] `grep 'pub use config_load::CliOverrides;' crates/anvilml-core/src/lib.rs` matches
- [ ] `grep 'cli_overrides: Option<CliOverrides>' crates/anvilml-core/src/config_load.rs` matches
