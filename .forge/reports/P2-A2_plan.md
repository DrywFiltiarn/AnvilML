# Plan Report: P2-A2

| Field       | Value                                              |
|-------------|----------------------------------------------------|
| Task ID     | P2-A2                                              |
| Phase       | 002 — Config & Graceful Shutdown                   |
| Description | anvilml-core: layered config loading (toml + ANVILML_* env override) |
| Depends on  | P2-A1 (ServerConfig struct)                        |
| Project     | anvilml                                            |
| Planned at  | 2026-06-14T12:30:00Z                               |
| Attempt     | 1                                                  |

## Objective

Implement the four-level config precedence chain in `anvilml-core`: compiled-in defaults → `anvilml.toml` → `ANVILML_*` env vars → `ConfigOverrides` (CLI). This produces a single `pub fn load()` that downstream code (`backend/src/main.rs` in P2-B1) calls to obtain a fully-resolved `ServerConfig`. When complete, `cargo test -p anvilml-core -- config_load` exits 0 with ≥ 4 tests verifying: missing file uses defaults, env var overrides toml, CLI override beats env.

## Scope

### In Scope

- **Create `crates/anvilml-core/src/config_load.rs`** with:
  - `ConfigOverrides` struct (`host: Option<String>`, `port: Option<u16>`) — carries only CLI-overridable fields per TASKS_PHASE002.md §106.
  - `pub fn load(path: &Path, overrides: &ConfigOverrides) -> Result<ServerConfig, AnvilError>` implementing the four-level precedence chain.
  - Env var resolution helper: reads `ANVILML_*` vars with double-underscore nesting (e.g. `ANVILML_GPU_SELECTION__DEFAULT_DEVICE` → `gpu_selection.default_device`).
  - TOML file reading via `toml::from_str::<ServerConfig>()` when the file exists; skips silently when absent.
- **Create `crates/anvilml-core/src/error.rs`** with minimal `AnvilError` enum:
  - `Io(std::io::Error)` — for file read errors.
  - `Toml(serde_json::de::Error)` — for TOML deserialisation errors (reusing `serde_json::de::Error` since `toml::de::Error` wraps it; we store the underlying error for portability).
  - `EnvVar(String, String)` — for malformed env var values (var_name, raw_value).
  - Derives `Debug, Clone, thiserror::Error`.
- **Modify `crates/anvilml-core/Cargo.toml`**: add `toml` workspace dep and `thiserror` workspace dep.
- **Modify `crates/anvilml-core/src/lib.rs`**: add `pub mod error;` and `pub use error::{AnvilError, ConfigLoadError};` (or re-export `AnvilError` directly).
- **Create `crates/anvilml-core/tests/config_load_tests.rs`** with ≥ 4 tests.

### Out of Scope

- CLI parsing (`clap` Args struct) — handled in P2-B1.
- `backend/src/main.rs` wiring — handled in P2-B1.
- Expansion of `AnvilError` to all variants listed in ANVILML_DESIGN.md §5.2 — handled in P3-B1.
- TOML serialisation (writing config back) — not needed for loading.
- Config drift guard (`config_reference` test) — handled in Phase 003.

## Existing Codebase Assessment

The `anvilml-core` crate currently contains two source files: `lib.rs` (re-exports from `config` module) and `config.rs` (215 lines, `ServerConfig` + all nested structs with `Default` impls). The `ServerConfig::default()` is fully implemented and matches documented defaults from `ENVIRONMENT.md §4`. The `config_tests.rs` integration test file (165 lines) uses the crate's public API via `use anvilml_core::config::*;` and follows the project's test style: doc comments on every test, assertions on individual fields, no inline `#[cfg(test)]` blocks.

Established patterns:
- **Error handling**: `AnvilError` is planned in Phase 003 (P3-B1) but does not exist yet. No error type is currently defined in `anvilml-core`.
- **Naming**: `snake_case` for functions/variables, `PascalCase` for types. Module files match their module name exactly (`config.rs`, `lib.rs`).
- **Test style**: integration tests live in `crates/{name}/tests/` as separate test crates, importing via the crate's public API. Tests use `assert_eq!` and `assert!` with descriptive doc comments.
- **Dependencies**: `serde` (with derive), `serde_json`, `uuid` are workspace deps. `toml` and `thiserror` are NOT yet added to anvilml-core.
- **PathBuf handling**: Uses `path_as_string` serde module for JSON roundtrips; TOML will need the same treatment since `PathBuf` does not implement `Serialize`/`Deserialize` by default — but `toml` crate v1.1.2 supports `PathBuf` natively via `#[serde(with = "path_as_string")]`.

Gap: `AnvilError` does not exist. The task context names it in the function signature. A minimal version must be created in this task to unblock the function, then expanded in P3-B1.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | toml    | 1.1.2           | cargo search   | n/a                    |
| crate  | thiserror | 2.0.18        | cargo search   | n/a (already in workspace) |

**Notes:**
- `toml` 1.1.2 is the current stable. It supports TOML 1.1.0 spec. The `toml::from_str()` function accepts any type implementing `serde::Deserialize`, including `ServerConfig`.
- `thiserror` 2.0.18 is already declared in the workspace deps (`Cargo.toml` line 29). No new version needed.
- `serde` and `serde_json` are already workspace deps in anvilml-core. No new dependencies required.

## Approach

1. **Add `toml` and `thiserror` to anvilml-core's Cargo.toml.**
   - Add `toml = { workspace = true }` and `thiserror = { workspace = true }` to the `[dependencies]` section.
   - Add `toml = "1.1.2"` to `[workspace.dependencies]` in root `Cargo.toml`.
   - **Rationale:** `toml` is needed for deserialising the config file. `thiserror` is needed for the `AnvilError` enum. Both are workspace deps following the project's convention.

2. **Create `crates/anvilml-core/src/error.rs` with minimal `AnvilError` enum.**
   - Define:
     ```rust
     #[derive(Debug, Clone, thiserror::Error)]
     pub enum AnvilError {
         #[error("I/O error: {0}")]
         Io(#[from] std::io::Error),
         #[error("TOML deserialisation error: {0}")]
         Toml(#[from] serde_json::de::Error),
         #[error("Invalid env var {name}: {value}")]
         EnvVar { name: String, value: String },
     }
     ```
   - Derive `Debug, Clone`. Implement `thiserror::Error`. Use `#[from]` for `Io` and `Toml` to enable implicit conversion.
   - **Rationale:** This is a minimal version. P3-B1 will expand it with all variants from ANVILML_DESIGN.md §5.2 (`Db`, `Serde`, `Ipc`, `PayloadTooLarge`, etc.). The `#[from]` derives ensure `?` works naturally in the load function.
   - Add `///` doc comment on the enum and each variant per FORGE_AGENT_RULES §12.1.

3. **Create `ConfigOverrides` struct in `config_load.rs`.**
   - Define:
     ```rust
     #[derive(Debug, Clone, Default)]
     pub struct ConfigOverrides {
         pub host: Option<String>,
         pub port: Option<u16>,
     }
     ```
   - **Rationale:** Per TASKS_PHASE002.md §106, `ConfigOverrides` carries only CLI-overridable fields (host, port). Other fields are not overridden by CLI. Using `Option` allows the struct to be `Default`-initialised (no overrides).

4. **Implement env var resolution helper in `config_load.rs`.**
   - Write a private function `apply_env_overrides(cfg: ServerConfig) -> ServerConfig` that:
     - Reads `ANVILML_HOST` → if set, replaces `cfg.host`.
     - Reads `ANVILML_PORT` → parse as u16; if set, replaces `cfg.port`.
     - Reads `ANVILML_DB_PATH` → replaces `cfg.db_path`.
     - Reads `ANVILML_ARTIFACT_DIR` → replaces `cfg.artifact_dir`.
     - Reads `ANVILML_VENV_PATH` → replaces `cfg.venv_path`.
     - Reads `ANVILML_SEEDS_PATH` → replaces `cfg.seeds_path`.
     - Reads `ANVILML_MAX_IPC_PAYLOAD_MIB` → parse as u32; replaces `cfg.max_ipc_payload_mib`.
     - Reads `ANVILML_NUM_THREADS` → parse as usize; replaces `cfg.num_threads` (wraps in `Some`).
     - Reads `ANVILML_GPU_SELECTION__DEFAULT_DEVICE` → replaces `cfg.gpu_selection.default_device`.
     - **Rationale:** Double-underscore nesting per ENVIRONMENT.md §3. Each nested field is read as a separate env var with the parent field name, double underscore, and child field name in uppercase.
   - For each env var that fails to parse, return `AnvilError::EnvVar { name, value }`.
   - **Rationale:** Explicit env var handling is safer than a generic serde-based approach — it gives precise error messages and avoids silently ignoring unknown vars.

5. **Implement `pub fn load(path: &Path, overrides: &ConfigOverrides) -> Result<ServerConfig, AnvilError>`.**
   - Step 1: Start with `ServerConfig::default()`.
   - Step 2: Try to read the TOML file at `path`. If it exists, parse with `toml::from_str()` and merge field-by-field with the defaults (TOML values override defaults; missing TOML fields keep defaults). If the file does not exist, skip silently — defaults remain.
   - Step 3: Apply env var overrides to the result from step 2.
   - Step 4: Apply `ConfigOverrides` last (host/port if `Some`).
   - Return the final `ServerConfig`.
   - **Rationale:** The four-level precedence chain (defaults < toml < env < CLI) is implemented as sequential mutations. Each layer starts from the previous result, ensuring correct precedence. TOML fields that are absent in the file simply don't override defaults — this works naturally with serde's `#[serde(default)]` attributes on `ServerConfig` fields, but since `ServerConfig` is already deserialised from defaults, we need to merge carefully: deserialize the TOML file into a `ServerConfig` (which gets defaults for missing fields), then for each field, prefer the TOML value over the default only if the TOML file was successfully read. Actually, the simplest correct approach: deserialize the TOML file directly into `ServerConfig` — if the file is valid TOML, serde will use the file's values where present and `#[serde(default)]` for missing ones. Then we compare field-by-field between the TOML result and the defaults to determine which fields were actually set in the file. For each field that differs from default, use the TOML value; for fields that match default, check if env var overrides them. This is complex.

   **Simpler correct approach:** Since `ServerConfig` derives `Deserialize` with `#[serde(default)]` on optional fields, deserialising a TOML file that omits some fields will fill those with defaults. We need to distinguish "explicitly set to default value" from "omitted, using default." The simplest correct approach:
   - Read the file as a string.
   - Parse into `toml::Value` (untyped).
   - Convert `toml::Value` to `ServerConfig` using `serde_ignored` or manual merging.
   - **Actually, simplest correct approach:** Just deserialize the TOML file directly into `ServerConfig`. Since `ServerConfig` has `#[serde(default)]` on optional fields and `Default` on nested structs, any field missing from the TOML file will get its default value — which is exactly what we want. Fields present in the TOML file override defaults. This is correct because the TOML file represents explicit user configuration, and serde's deserialiser naturally handles missing fields via defaults.

   Wait — there's a subtle issue. If the TOML file explicitly sets `port = 8488` (the default), serde will produce `port: 8488`, which is indistinguishable from "not set, using default." But this is fine: the result is the same. The precedence chain is about which source *wins*, not about detecting which fields were explicitly set. If both TOML and defaults say `port = 8488`, the result is `port = 8488` regardless.

   **Final approach:** Simply `toml::from_str::<ServerConfig>(&content)` — this correctly applies TOML values over defaults for all fields present in the file, and fills missing fields with defaults. No manual merging needed.

6. **Update `lib.rs`** to export the new modules:
   - Add `pub mod error;`
   - Add `pub mod config_load;`
   - Add `pub use error::AnvilError;`
   - Add `pub use config_load::{load, ConfigOverrides};`

7. **Create `crates/anvilml-core/tests/config_load_tests.rs`** with ≥ 4 tests:
   - **test_missing_file_uses_defaults:** Call `load("/nonexistent/path.toml", &ConfigOverrides::default())`. Assert the result equals `ServerConfig::default()`.
   - **test_env_var_beats_toml:** Write a TOML file with `port = 9001`. Set `ANVILML_PORT=8080`. Call `load()`. Assert `cfg.port == 8080` (env beats toml).
   - **test_cli_override_beats_env:** Write a TOML file with `port = 9001`. Set `ANVILML_PORT=8080`. Call `load()` with `ConfigOverrides { port: Some(7070) }`. Assert `cfg.port == 7070` (CLI beats env).
   - **test_nested_env_var:** Write a TOML file without `gpu_selection`. Set `ANVILML_GPU_SELECTION__DEFAULT_DEVICE=cpu`. Call `load()`. Assert `cfg.gpu_selection.default_device == "cpu"`.
   - **Rationale for test count:** 4 tests cover the three precedence levels (toml vs defaults, env vs toml, CLI vs env) plus the nested field path (double-underscore nesting).

   **Test isolation:** Each test that sets env vars captures the prior value and restores it unconditionally (ENVIRONMENT.md §11.3). Tests that create temp files use `tempfile` or clean up in a closure.

8. **Run `cargo test -p anvilml-core -- config_load`** to verify all tests pass before writing the report.

## Public API Surface

| Item | Type | Module Path | Signature / Description |
|------|------|-------------|------------------------|
| `AnvilError` | enum | `anvilml_core::error` | `pub enum AnvilError { Io(std::io::Error), Toml(serde_json::de::Error), EnvVar { name: String, value: String } }` — minimal version; expanded in P3-B1 |
| `load` | fn | `anvilml_core::config_load` | `pub fn load(path: &Path, overrides: &ConfigOverrides) -> Result<ServerConfig, AnvilError>` |
| `ConfigOverrides` | struct | `anvilml_core::config_load` | `pub struct ConfigOverrides { pub host: Option<String>, pub port: Option<u16> }` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/error.rs` | Minimal `AnvilError` enum (Io, Toml, EnvVar) with thiserror derives |
| CREATE | `crates/anvilml-core/src/config_load.rs` | `load()` function, `ConfigOverrides` struct, env var resolution |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Add `pub mod error`, `pub mod config_load`, re-exports |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Add `toml` and `thiserror` dependencies |
| MODIFY | `Cargo.toml` (workspace root) | Add `toml = "1.1.2"` to `[workspace.dependencies]` |
| CREATE | `crates/anvilml-core/tests/config_load_tests.rs` | ≥ 4 integration tests for config loading |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-core/tests/config_load_tests.rs` | `test_missing_file_uses_defaults` | When the TOML file does not exist, `load()` returns `ServerConfig::default()` | No file at given path | `path = "/nonexistent.toml"`, `overrides = ConfigOverrides::default()` | `Result::Ok(ServerConfig::default())` | `cargo test -p anvilml-core -- config_load test_missing_file_uses_defaults` exits 0 |
| `crates/anvilml-core/tests/config_load_tests.rs` | `test_env_var_beats_toml` | Environment variable `ANVILML_PORT` overrides the same field from TOML | TOML file exists with `port = 9001` | TOML with `port = 9001`, `ANVILML_PORT=8080` | `cfg.port == 8080` | `cargo test -p anvilml-core -- config_load test_env_var_beats_toml` exits 0 |
| `crates/anvilml-core/tests/config_load_tests.rs` | `test_cli_override_beats_env` | `ConfigOverrides.port` takes precedence over `ANVILML_PORT` env var | TOML exists with `port = 9001`, env set | TOML `port = 9001`, `ANVILML_PORT=8080`, `overrides.port = Some(7070)` | `cfg.port == 7070` | `cargo test -p anvilml-core -- config_load test_cli_override_beats_env` exits 0 |
| `crates/anvilml-core/tests/config_load_tests.rs` | `test_nested_env_var` | Double-underscore nesting for nested fields works (`ANVILML_GPU_SELECTION__DEFAULT_DEVICE`) | TOML file exists without `gpu_selection` section | TOML without gpu_selection, `ANVILML_GPU_SELECTION__DEFAULT_DEVICE=cpu` | `cfg.gpu_selection.default_device == "cpu"` | `cargo test -p anvilml-core -- config_load test_nested_env_var` exits 0 |

## CI Impact

No CI changes required. The new test file lives in `crates/anvilml-core/tests/` which is already picked up by `cargo test --workspace --features mock-hardware`. The `toml` crate is a new dependency but does not change any CI job's behaviour. The `rust-linux` and `rust-windows` CI jobs will automatically run the new tests.

## Platform Considerations

None identified. The config loading logic is platform-neutral:
- `std::fs::read_to_string` works identically on all platforms.
- `toml::from_str` is platform-independent.
- Environment variables (`std::env::var`) are platform-independent.
- PathBuf handling is already abstracted via `path_as_string` for JSON; TOML crate handles `PathBuf` natively.
- No `#[cfg(unix)]` or `#[cfg(windows)]` guards needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `AnvilError` created here is a minimal subset; P3-B1 will need to expand it. If P3-B1's expansion conflicts with this minimal version (e.g. different variant names or fields), the ACT agent for P3-B1 will need to refactor. | Medium | Medium | Document the minimal `AnvilError` structure in `## Deviations from Plan`. P3-B1 will be the task that expands it — the plan should note that P3-B1 may need to add variants. |
| TOML deserialisation of `ServerConfig` may fail if the TOML file contains fields not in `ServerConfig` (unknown keys). By default, serde ignores unknown keys, so this is safe — but if `toml` v1.1.2 changes this behaviour, it could silently drop config. | Low | Low | Serde's default behaviour is to ignore unknown keys. No action needed unless `toml` v1.1.2 changes this. |
| Environment variable parsing failures (e.g. `ANVILML_PORT=abc`) produce `AnvilError::EnvVar`. If the env var is not set at all, `std::env::var` returns `Err(NotPresent)` — this must not be treated as an error. | Medium | Medium | Use `std::env::var(name).ok().and_then(|v| ...)` pattern: only parse if the var is present and non-empty. Unset vars are simply skipped. |
| The `toml` crate v1.1.2 may have a different API than v0.x (which was commonly used). Method names like `toml::from_str` must be verified. | Medium | High | The MCP search confirmed `toml = "1.1.2"` exists. The `toml::from_str()` function is the standard serde-compatible API that has been stable across versions. If the API differs, the ACT agent will verify via rust-docs MCP. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-core --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-core -- config_load` exits 0 (≥ 4 tests)
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `cargo clippy -p anvilml-core --features mock-hardware -- -D warnings` exits 0
