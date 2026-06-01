# Plan Report: P2-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P2-A2                                       |
| Phase       | 002 — Config & Graceful Shutdown             |
| Description | anvilml-core: layered config loader (defaults -> toml -> env -> overrides) |
| Depends on  | P2-A1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-01T07:26:35Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-core/src/config_load.rs` implementing a layered configuration loader that resolves `ServerConfig` from four precedence levels (lowest to highest): built-in defaults, optional TOML file on disk, `ANVILML_*` environment variables (with double-underscore nesting for sub-fields), and explicit CLI overrides. This module is the runtime glue that turns static type definitions (P2-A1) into a fully-resolved configuration instance usable by the server. The function signature is `fn load_config(toml_path: Option<&Path>, overrides: ConfigOverrides) -> Result<ServerConfig>`. Tests verify env-overrides-toml and override-beats-env precedence.

## Scope

### In Scope
- Create `crates/anvilml-core/src/config_load.rs` with the following components:
  - `ConfigOverrides` struct with `host: Option<IpAddr>` and `port: Option<u16>` fields
  - `ConfigError` enum covering file I/O errors, TOML deserialization errors, and invalid environment variable values
  - `load_config(toml_path: Option<&Path>, overrides: ConfigOverrides) -> Result<ServerConfig, ConfigError>` function implementing the four-level precedence chain
  - Internal helpers: `resolve_env_var(name: &str) -> Option<String>`, `apply_env_to_config(config: ServerConfig, env: &HashMap<String, String>) -> ServerConfig`
- Update `crates/anvilml-core/src/lib.rs` to register `pub mod config_load;` and re-export `load_config`, `ConfigOverrides`, and `ConfigError`
- Add inline `#[cfg(test)]` module in `config_load.rs` with three tests:
  - Test that env vars override TOML values (e.g., set `ANVILML_PORT=9999` in env, put `port = 8488` in TOML, verify loaded config has port 9999)
  - Test that explicit overrides beat env vars (e.g., env sets `ANVILML_PORT=9999`, overrides set port to `7777`, verify loaded config has port 7777)
  - Test that missing TOML file produces a warning log line and falls back cleanly to defaults + env
- Use only existing dependencies in `anvilml-core` (`serde`, `toml`, `url`) plus standard library (`std::env`, `std::net::IpAddr`, `std::collections::HashMap`)
- No new crate dependencies required

### Out of Scope
- CLI parsing with clap (P2-A3)
- Tracing/logging subscriber initialization (P2-A4)
- Graceful shutdown signal handling (P2-A5)
- Any I/O beyond reading one TOML file and reading environment variables
- Async code or runtime dependencies (anvilml-core has zero async — per ARCHITECTURE.md §4)
- Writing the TOML file to disk (reading only)
- Parsing `--host` / `--port` CLI flags (those are resolved by P2-A3 into a `ConfigOverrides` instance passed to this function)
- Other domain types, hardware detection, job scheduling, or server logic

## Approach

1. **Define `ConfigError` enum** in `config_load.rs`. Variants: `Io(io::Error)` for file read failures, `Toml(toml::de::Error)` for deserialization errors, `EnvParse(String, String)` for invalid env var value parsing (field name + raw value). Implement `std::fmt::Display` and `std::error::Error` for each variant. This keeps error handling self-contained in anvilml-core.

2. **Define `ConfigOverrides` struct** with two fields: `pub host: Option<IpAddr>` and `pub port: Option<u16>`. Derive `Debug, Clone, Default` (both fields default to `None`). This struct is the bridge between P2-A3's clap CLI parsing and this loader.

3. **Implement the precedence chain in `load_config`**:
   - Step A: Start with `ServerConfig::default()` as the base layer.
   - Step B: If `toml_path` is `Some(path)`, attempt to read and deserialize the TOML file into a `ServerConfig`. On success, merge its fields over the defaults (since every field has `#[serde(default)]`, a partial TOML file will only override specified keys). On failure (file not found), log a warning message via `eprintln!` (no tracing dependency — anvilml-core has zero runtime deps) and continue with defaults. On other errors, return `ConfigError::Toml`.
   - Step C: Read all environment variables starting with `ANVILML_` using `std::env::vars()`. For each variable, parse the key to determine which config field it targets:
     - `ANVILML_HOST` → `config.host = val.parse::<IpAddr>()`
     - `ANVILML_PORT` → `config.port = val.parse::<u16>()`
     - `ANVILML_DB_PATH` → `config.db_path = PathBuf::from(val)`
     - `ANVILML_ARTIFACT_DIR` → `config.artifact_dir = PathBuf::from(val)`
     - `ANVILML_VENV_PATH` → `config.venv_path = PathBuf::from(val)`
     - `ANVILML_WORKER_LOG_DIR` → `config.worker_log_dir = Some(PathBuf::from(val))`
     - `ANVILML_NUM_THREADS` → `config.num_threads = val.parse::<usize>()`
     - `ANVILML_NUM_INTEROP_THREADS` → `config.num_interop_threads = val.parse::<usize>()`
     - `ANVILML_FRONTEND__MODE` → parse the double-underscore key into a path through the config tree: split on `__`, map `FRONTEND` → `config.frontend`, then `MODE` → set `config.frontend.mode` to `FrontendMode::Headless` (or handle `local`/`remote` with additional parsing)
     - `ANVILML_GPU_SELECTION__DEFAULT_DEVICE` → `config.gpu_selection.default_device = val`
     - Any unrecognized prefix is silently ignored (future-proofing).
     - If a parse fails, log via `eprintln!` and skip that variable (don't fail the entire load).
   - Step D: Apply explicit overrides last. For each `Some` field in `overrides`, overwrite the corresponding config field unconditionally:
     ```
     if let Some(host) = overrides.host { config.host = host; }
     if let Some(port) = overrides.port { config.port = port; }
     ```
   - Step E: Return `Ok(config)`.

4. **Implement env var key normalization**: Build a `HashMap<String, String>` from `std::env::vars()`, then filter keys starting with `ANVILML_`. Strip the prefix and convert remaining chars to uppercase for matching (e.g., `ANVILML_FRONTEND__MODE` → strip prefix → `FRONTEND__MODE`). Double underscores (`__`) represent nested struct access.

5. **Update `crates/anvilml-core/src/lib.rs`**: Add `pub mod config_load;` after the existing `pub mod config;` line. Add `pub use config_load::{load_config, ConfigOverrides, ConfigError};` to re-export the public API at the crate root.

6. **Write tests** in an inline `#[cfg(test)] mod tests { ... }` within `config_load.rs`:
   - **Test `env_overrides_toml`**: Write a temporary TOML file with `port = 8488`, set env var `ANVILML_PORT=9999`, call `load_config(Some(&toml_path), ConfigOverrides::default())`. Assert the result has `port == 9999`.
   - **Test `override_beats_env`**: Write a temporary TOML file with `port = 8488`, set env var `ANVILML_PORT=9999`, call `load_config(Some(&toml_path), ConfigOverrides { host: None, port: Some(7777) })`. Assert the result has `port == 7777`.
   - **Test `missing_toml_fallback`**: Call `load_config(None, ConfigOverrides::default())` and `load_config(Some(&nonexistent_path), ConfigOverrides::default())`. Both should return `Ok(ServerConfig::default())` (the latter with a warning printed to stderr).
   - **Test `env_nested_field`**: Set env var `ANVILML_FRONTEND__MODE=headless`, call `load_config(None, ...)`. Assert `config.frontend.mode == FrontendMode::Headless`.

## Files Affected

| Action   | Path                              | Description |
|----------|-----------------------------------|-------------|
| CREATE   | crates/anvilml-core/src/config_load.rs | Layered config loader: ConfigError enum, ConfigOverrides struct, load_config function with precedence chain, and inline tests |
| MODIFY   | crates/anvilml-core/src/lib.rs    | Add `pub mod config_load;` and re-export `load_config`, `ConfigOverrides`, `ConfigError` from crate root |

## Tests

| Test ID / Name            | File                              | Validates               |
|---------------------------|-----------------------------------|-------------------------|
| `config_load::tests::env_overrides_toml` | crates/anvilml-core/src/config_load.rs | Environment variable ANVILML_PORT=9999 overrides port=8488 from TOML file; loaded config has port 9999 |
| `config_load::tests::override_beats_env` | crates/anvilml-core/src/config_load.rs | Explicit ConfigOverrides { port: Some(7777) } beats ANVILML_PORT=9999 env var; loaded config has port 7777 |
| `config_load::tests::missing_toml_fallback` | crates/anvilml-core/src/config_load.rs | Passing None or a nonexistent path for toml_path returns Ok(defaults) with a warning printed to stderr |
| `config_load::tests::env_nested_field` | crates/anvilml-core/src/config_load.rs | ANVILML_FRONTEND__MODE=headless correctly sets config.frontend.mode to FrontendMode::Headless via double-underscore nesting |

## CI Impact

No CI changes required. This task adds no new dependencies, no new crate in the workspace, and no platform-specific code paths. The existing CI matrix (fmt + clippy + test with `--features mock-hardware` on Linux, clippy + test on Windows) will automatically pick up the new module and tests when run via `cargo test -p anvilml-core`. The `eprintln!` fallback for warnings is standard library only and has no platform differences.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| Double-underscore env parsing for nested fields (e.g., `ANVILML_FRONTEND__MODE`) becomes complex as config tree deepens | Medium | Medium | Implement a simple recursive path walker that traverses the ServerConfig struct by field name; handle only the flat and one-level-nested paths needed for Phase 2; defer deeper nesting to later phases |
| `eprintln!` for warnings may not be visible in production logging (no tracing dep) | Low | Low | Acceptable for Phase 2 — P2-A4 introduces tracing; anvilml-core has zero runtime deps, so eprintln is the only option. Document this limitation in a TODO comment |
| Test writes temporary TOML files to the filesystem | Low | Low | Use `tempfile::NamedTempFile` from the standard library's `std::env::temp_dir()` + `File::create` to avoid leaking temp files; clean up via `drop()` at end of each test. No new dependency needed |
| `toml` crate deserialization silently ignores unknown fields | Low | Low | This is actually desirable behavior (forward compatibility); document in code that unknown TOML keys are ignored per serde/toml defaults |
| `FrontendMode::Remote` requires URL parsing from env var — edge case with complex URLs | Low | Low | Only implement basic parsing for Phase 2 (`local`, `remote`, `headless` string → enum); full URL validation deferred to P2-A3 CLI parsing which uses clap's built-in validation |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core -- config_load` exits 0 (all four tests pass)
- [ ] `cargo clippy -p anvilml-core` exits 0 with no warnings
- [ ] `crates/anvilml-core/src/config_load.rs` defines `ConfigError`, `ConfigOverrides`, and `load_config`
- [ ] `ConfigOverrides` has fields `host: Option<IpAddr>` and `port: Option<u16>`
- [ ] Precedence order is verified by tests: defaults < toml < env vars < explicit overrides
- [ ] Missing TOML file produces a warning (via eprintln) and falls back to defaults + env
- [ ] Double-underscore env var nesting works for at least `ANVILML_FRONTEND__MODE`
- [ ] `lib.rs` re-exports `load_config`, `ConfigOverrides`, and `ConfigError` from crate root
- [ ] No new crate dependencies added to anvilml-core (only std library used)
- [ ] No async code, no I/O beyond file read + env vars, zero runtime deps in anvilml-core
