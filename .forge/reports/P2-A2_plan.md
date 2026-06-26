# Plan Report: P2-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P2-A2                                       |
| Phase       | 002 — Core Domain Types: Config & Errors    |
| Description | anvilml-core: ServerConfig top-level scalar fields |
| Depends on  | P2-A1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-26T17:55:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-core/src/config.rs` defining the `ServerConfig` struct with its eight top-level scalar fields and a `Default` implementation, then export it from `lib.rs`. This establishes the config type that every later subsystem reads from, before nested table structs are layered on in P2-A3. Acceptance: ≥4 tests in `crates/anvilml-core/tests/config_tests.rs` asserting each scalar field's default value; `cargo test -p anvilml-core --test config_tests` exits 0.

## Scope

### In Scope
- Create `crates/anvilml-core/src/config.rs` with the `ServerConfig` struct and its `Default` impl.
- Add `mod config;` and `pub use config::ServerConfig;` to `crates/anvilml-core/src/lib.rs`.
- Create `crates/anvilml-core/tests/config_tests.rs` with ≥4 tests asserting each scalar field's default value.

### Out of Scope
- Nested table structs (`model_dirs`, `gpu_selection`, `limits`, `rocm`, `hardware_override`) — deferred to P2-A3.
- Config loading logic (`config_load::load()`) — P2-A4.
- Environment variable and CLI override layers — P2-A5.
- Wiring `config_load::load()` into `backend/main.rs` — P2-A6.
- Config drift test (`config_reference`) — P2-A7.
- Updating `anvilml.toml` — P2-A7.

## Existing Codebase Assessment

`anvilml-core` exists as a scaffold crate (Phase 1) with `lib.rs` re-exporting `AnvilError` from `error.rs`. The `serde` crate with the `derive` feature is already declared in `Cargo.toml`, providing `Serialize` and `Deserialize` derives without needing a new dependency. No `config.rs` exists yet.

The established test style (from `tests/error_tests.rs`) uses `#[tokio::test]` async tests, each with a `///` doc comment describing what is verified, imports from the crate's public API, and uses direct assertion macros (`assert_eq!`, `assert!`). The `anvilml.toml` reference config currently only contains `host` and `port` (Phase 1 state), to be expanded in P2-A7.

No gap between design doc and current source affects this task — the design doc specifies the struct fields and defaults directly, and the existing `serde` dependency covers all derive needs.

## Resolved Dependencies

No new external dependencies are introduced. The `serde` crate with `features = ["derive"]` is already present in `crates/anvilml-core/Cargo.toml` (version `1.0`), providing the `Serialize` and `Deserialize` derives. `PathBuf` is from `std::path`.

| Type   | Name  | Version verified | MCP source | Feature flags confirmed |
|--------|-------|-----------------|------------|------------------------|
| crate  | serde | 1.0 (existing)  | Cargo.lock | derive                   |

## Approach

1. **Create `crates/anvilml-core/src/config.rs`.** Define the `ServerConfig` struct with exactly eight scalar fields, each with the type and default from ENVIRONMENT.md §4:
   ```rust
   use std::path::PathBuf;

   /// Top-level server configuration with compiled-in defaults.
   ///
   /// Fields are loaded through a four-layer precedence chain:
   /// defaults → TOML → environment variables → CLI flags.
   /// Only the scalar fields are defined here; nested tables
   /// (model_dirs, gpu_selection, limits, rocm, hardware_override)
   /// are added by P2-A3.
   #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
   pub struct ServerConfig {
       /// Bind address for the HTTP server.
       pub host: String,
       /// HTTP server port.
       pub port: u16,
       /// SQLite database file path.
       pub db_path: PathBuf,
       /// Directory for generated image artifacts.
       pub artifact_dir: PathBuf,
       /// Python virtualenv root for worker processes.
       pub venv_path: PathBuf,
       /// Non-recursive model scanner depth.
       pub model_scan_depth: u32,
       /// Maximum IPC message payload in MiB.
       pub max_ipc_payload_mib: u32,
       /// Tokio worker thread count. None = auto (num_cpus).
       pub num_threads: Option<u32>,
   }
   ```
   Then implement `Default` for `ServerConfig`:
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
           }
       }
   }
   ```
   Rationale: `PathBuf::from()` is used instead of string literals to satisfy `PathBuf` construction without a separate constant. Each field uses its exact default from ENVIRONMENT.md §4.

2. **Update `crates/anvilml-core/src/lib.rs`.** Add two lines after the existing `mod error;`:
   ```rust
   mod config;

   pub use config::ServerConfig;
   ```
   The file stays well under the 80-line hard cap (currently 6 lines, will be ~9).

3. **Create `crates/anvilml-core/tests/config_tests.rs`.** Write tests that each construct `ServerConfig::default()` and assert one scalar field's value. Use `#[test]` (sync, no `tokio` needed for simple struct field access). Each test has a `///` doc comment per the project's test documentation obligation (ENVIRONMENT.md §11.4). Write at least 8 tests (one per field), exceeding the ≥4 minimum:
   - `test_host_default` — `config.host == "127.0.0.1"`
   - `test_port_default` — `config.port == 8488`
   - `test_db_path_default` — `config.db_path == PathBuf::from("./anvilml.db")`
   - `test_artifact_dir_default` — `config.artifact_dir == PathBuf::from("./artifacts")`
   - `test_venv_path_default` — `config.venv_path == PathBuf::from("./worker/.venv")`
   - `test_model_scan_depth_default` — `config.model_scan_depth == 2`
   - `test_max_ipc_payload_mib_default` — `config.max_ipc_payload_mib == 256`
   - `test_num_threads_default` — `config.num_threads.is_none()`

## Public API Surface

| Item | Path | Description |
|------|------|-------------|
| `pub struct ServerConfig` | `anvilml_core::ServerConfig` | Top-level config with 8 scalar fields; derives Debug, Clone, Serialize, Deserialize |
| `impl Default for ServerConfig` | `anvilml_core::ServerConfig` | Provides compiled-in defaults for all fields |
| `ServerConfig::default()` | `anvilml_core::ServerConfig::default()` | Returns `Self` with all eight scalar defaults |

No new `pub fn`, `pub enum`, `pub trait`, `pub const`, or `pub type` items.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/config.rs` | `ServerConfig` struct, `Default` impl, doc comments |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Add `mod config;` and `pub use config::ServerConfig;` |
| CREATE | `crates/anvilml-core/tests/config_tests.rs` | ≥4 tests asserting each scalar field's default value |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-core/tests/config_tests.rs` | `test_host_default` | `ServerConfig::default().host == "127.0.0.1"` | `cargo test -p anvilml-core --test config_tests test_host_default` exits 0 |
| `crates/anvilml-core/tests/config_tests.rs` | `test_port_default` | `ServerConfig::default().port == 8488` | `cargo test -p anvilml-core --test config_tests test_port_default` exits 0 |
| `crates/anvilml-core/tests/config_tests.rs` | `test_db_path_default` | `ServerConfig::default().db_path == PathBuf::from("./anvilml.db")` | `cargo test -p anvilml-core --test config_tests test_db_path_default` exits 0 |
| `crates/anvilml-core/tests/config_tests.rs` | `test_artifact_dir_default` | `ServerConfig::default().artifact_dir == PathBuf::from("./artifacts")` | `cargo test -p anvilml-core --test config_tests test_artifact_dir_default` exits 0 |
| `crates/anvilml-core/tests/config_tests.rs` | `test_venv_path_default` | `ServerConfig::default().venv_path == PathBuf::from("./worker/.venv")` | `cargo test -p anvilml-core --test config_tests test_venv_path_default` exits 0 |
| `crates/anvilml-core/tests/config_tests.rs` | `test_model_scan_depth_default` | `ServerConfig::default().model_scan_depth == 2` | `cargo test -p anvilml-core --test config_tests test_model_scan_depth_default` exits 0 |
| `crates/anvilml-core/tests/config_tests.rs` | `test_max_ipc_payload_mib_default` | `ServerConfig::default().max_ipc_payload_mib == 256` | `cargo test -p anvilml-core --test config_tests test_max_ipc_payload_mib_default` exits 0 |
| `crates/anvilml-core/tests/config_tests.rs` | `test_num_threads_default` | `ServerConfig::default().num_threads.is_none()` | `cargo test -p anvilml-core --test config_tests test_num_threads_default` exits 0 |

## CI Impact

No CI changes required. The new test file `config_tests.rs` is picked up automatically by `cargo test --workspace --features mock-hardware` (it is a standard test crate under `crates/anvilml-core/tests/`). No new file types, gates, or CI configuration is needed.

## Platform Considerations

None identified. The `ServerConfig` struct uses only `String`, `u16`, `u32`, `Option<u32>`, and `PathBuf` — all platform-neutral types. `PathBuf` correctly handles platform-specific path separators at runtime. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `serde::Serialize`/`Deserialize` on `PathBuf` may produce platform-specific string representations that differ across OS, causing the config-drift test (P2-A7) to fail on Windows if `anvilml.toml` was written with Unix-style paths. | Medium | High | Use `PathBuf::from()` consistently in both `Default` impl and `anvilml.toml` (P2-A7). The `toml` crate serialises `PathBuf` as a string; P2-A7's config_reference test compares serialised key sets, not path string values, so this is only a concern if P2-A7 ever asserts path equality — which it does not per the task context (it asserts every field equals `ServerConfig::default()` via round-trip). |
| Adding `mod config;` and `pub use config::ServerConfig;` to `lib.rs` could conflict with a pre-existing `config` module if one was added by another concurrent task. | Low | Medium | Confirm `lib.rs` currently has no `config` module (verified: it does not). If a conflict arises at ACT time, resolve by reading the current `lib.rs` and adjusting the insertion point. |
| The test file path `crates/anvilml-core/tests/config_tests.rs` may collide with a test file already created by a concurrent task. | Low | Low | Verified no `config_tests.rs` exists in `crates/anvilml-core/tests/` (only `error_tests.rs` exists). |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-core` exits 0
- [ ] `cargo test -p anvilml-core --test config_tests` exits 0
- [ ] `grep -c "^fn test_" crates/anvilml-core/tests/config_tests.rs` outputs a number ≥ 4
- [ ] `grep "^## " .forge/reports/P2-A2_plan.md` shows all 12 required section headings
