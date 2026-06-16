# Plan Report: P9-A1

| Field       | Value                                    |
|-------------|------------------------------------------|
| Task ID     | P9-A1                                    |
| Phase       | 009 — Worker Spawn & Handshake           |
| Description | anvilml-worker: env.rs WorkerEnv env builder |
| Depends on  | none                                     |
| Project     | anvilml                                  |
| Planned at  | 2026-06-16T16:30:00Z                     |
| Attempt     | 1                                        |

## Objective

Create `crates/anvilml-worker/src/env.rs` with a public function `build_worker_env` that constructs an environment variable map for a Python worker subprocess, injecting all seven `ANVILML_*` variables defined in `ENVIRONMENT.md §3.4` and `ANVILML_DESIGN.md §9.6`. Add `log_level: String` to `ServerConfig` so the function can forward it. Tests in `tests/env_tests.rs` verify each key is present with the correct value. The observable outcome is that `cargo test -p anvilml-worker --features mock-hardware -- env` exits 0 with at least 5 passing tests.

## Scope

### In Scope
- Add `log_level: String` field (default `"info"`) to `ServerConfig` in `crates/anvilml-core/src/config.rs`.
- Create `crates/anvilml-worker/src/env.rs` with `pub fn build_worker_env(device: &GpuDevice, cfg: &ServerConfig, port: u16) -> HashMap<String, String>`.
- Inject the following env vars: `ANVILML_IPC_PORT` (port decimal), `ANVILML_WORKER_ID` (device.index as string), `ANVILML_DEVICE_INDEX` (device.index as string), `ANVILML_DEVICE_TYPE` (lowercase variant name), `ANVILML_LOG_LEVEL` (from cfg), `ANVILML_MAX_IPC_PAYLOAD_MIB` (from cfg).
- When compiled with `#[cfg(feature = "mock-hardware")]`, also inject `ANVILML_WORKER_MOCK=1`.
- Declare `pub mod env;` and `pub use env::build_worker_env;` in `crates/anvilml-worker/src/lib.rs`.
- Create `crates/anvilml-worker/tests/env_tests.rs` with ≥ 5 tests verifying each env var.
- Bump `anvilml-worker` patch version from `0.1.0` to `0.1.1`.

### Out of Scope
- `spawn.rs` (subprocess Command construction) — handled by P9-A2.
- `bridge.rs`, `keepalive.rs`, `managed.rs`, `pool.rs`, `respawn.rs` — handled by P9-A3 through P9-A6.
- Updating `anvilml.toml` with `log_level` — not required by this task; the field uses a compiled-in default.
- Any changes to `anvilml-server`, `backend`, or `anvilml-scheduler`.

## Existing Codebase Assessment

The `anvilml-worker` crate is currently a stub: `lib.rs` declares only a `pub fn stub()` placeholder. No source modules (env, spawn, bridge, etc.) exist yet, and no `tests/` directory is present. The crate's `Cargo.toml` already declares the correct dependencies (`anvilml-core`, `anvilml-hardware`, `anvilml-ipc`, `tokio`, `tracing`) and the `mock-hardware` feature forwarding rule.

`ServerConfig` in `anvilml-core/src/config.rs` does not currently have a `log_level` field. This field is referenced by the task context (`cfg.log_level`) and by `ENVIRONMENT.md §3.4`, so it must be added as part of this task. The field is a `String` with a default of `"info"`.

`GpuDevice` in `anvilml-core/src/types/hardware.rs` has a well-defined public API: `index: u32`, `device_type: DeviceType`, and other fields. `DeviceType` is a `#[serde(rename_all = "snake_case")]` enum with variants `Cuda`, `Rocm`, `Cpu`. The `to_string()` method on `DeviceType` will produce `"cuda"`, `"rocm"`, or `"cpu"` because of the serde rename attribute — however, `to_string()` on a non-derivable enum requires using `format!("{:?}", ...)` or a custom method. I will use `matches!()` or a helper function to produce the lowercase string to avoid any serde-attribute confusion.

The established pattern in this codebase is: `lib.rs` contains only `pub mod` and `pub use` declarations with a `//!` crate-level doc comment; all implementation lives in sibling `.rs` files; tests live in `crates/{name}/tests/` as separate test crate files.

## Resolved Dependencies

None. This task uses only `std::collections::HashMap` (standard library) and types already available through `anvilml-core`. No new external crates are introduced.

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| (stdlib) | HashMap | N/A | N/A | n/a |

## Approach

1. **Add `log_level` field to `ServerConfig`.** In `crates/anvilml-core/src/config.rs`, add `pub log_level: String` to the `ServerConfig` struct with doc comment. Add `#[serde(default = "default_log_level")]` attribute. Implement `default_log_level()` returning `"info"`. Update the `Default` impl for `ServerConfig` to include `log_level: "info".to_string()`. This field is needed because the task context references `cfg.log_level` and `ENVIRONMENT.md §3.4` specifies `ANVILML_LOG_LEVEL` is forwarded from server config.

2. **Create `crates/anvilml-worker/src/env.rs`.** Implement `pub fn build_worker_env(device: &GpuDevice, cfg: &ServerConfig, port: u16) -> HashMap<String, String>`. The function:
   - Creates a new `HashMap::new()`.
   - Inserts `("ANVILML_IPC_PORT".into(), port.to_string())`.
   - Inserts `("ANVILML_WORKER_ID".into(), device.index.to_string())`.
   - Inserts `("ANVILML_DEVICE_INDEX".into(), device.index.to_string())`.
   - Inserts `("ANVILML_DEVICE_TYPE".into(), device_type_label(&device.device_type))` — uses a helper function to produce lowercase `"cuda"`, `"rocm"`, or `"cpu"`.
   - Inserts `("ANVILML_LOG_LEVEL".into(), cfg.log_level.clone())`.
   - Inserts `("ANVILML_MAX_IPC_PAYLOAD_MIB".into(), cfg.max_ipc_payload_mib.to_string())`.
   - Under `#[cfg(feature = "mock-hardware")]`, also inserts `("ANVILML_WORKER_MOCK".into(), "1".into())`.
   - Returns the populated `HashMap`.
   - The function has a `///` doc comment describing its purpose and all parameters.
   - The `device_type_label` helper is a private `fn` that maps `DeviceType` to its lowercase string using a `match` expression with inline comments explaining the mapping (not relying on serde attributes since this is not serialization).

3. **Update `crates/anvilml-worker/src/lib.rs`.** Replace the stub `pub fn stub() {}` with `pub mod env;` and `pub use env::build_worker_env;`. Preserve the existing `//!` crate-level doc comment. The file remains under 80 lines.

4. **Create `crates/anvilml-worker/tests/env_tests.rs`.** Write integration tests that call `build_worker_env` with known inputs and assert each env var is present with the correct value. Tests:
   - **test_ipc_port**: verifies `ANVILML_IPC_PORT` equals the port argument as a decimal string.
   - **test_worker_id**: verifies `ANVILML_WORKER_ID` equals the device index as a string.
   - **test_device_index**: verifies `ANVILML_DEVICE_INDEX` equals the device index as a string.
   - **test_device_type_cuda**: verifies `ANVILML_DEVICE_TYPE` is `"cuda"` for `DeviceType::Cuda`.
   - **test_device_type_rocm**: verifies `ANVILML_DEVICE_TYPE` is `"rocm"` for `DeviceType::Rocm`.
   - **test_device_type_cpu**: verifies `ANVILML_DEVICE_TYPE` is `"cpu"` for `DeviceType::Cpu`.
   - **test_log_level**: verifies `ANVILML_LOG_LEVEL` matches `cfg.log_level`.
   - **test_max_ipc_payload_mib**: verifies `ANVILML_MAX_IPC_PAYLOAD_MIB` matches `cfg.max_ipc_payload_mib`.
   - **test_mock_hardware_flag**: under `#[cfg(feature = "mock-hardware")]`, verifies `ANVILML_WORKER_MOCK` is `"1"`.
   - **test_total_count**: verifies the HashMap contains exactly 6 entries (7 with mock-hardware feature).

5. **Bump `anvilml-worker` patch version.** Update `crates/anvilml-worker/Cargo.toml` from `version.workspace = true` (workspace is `0.1.0`) to `version = "0.1.1"`.

## Public API Surface

| Item | Type | Crate/Module | Signature |
|------|------|-------------|-----------|
| `build_worker_env` | `pub fn` | `anvilml-worker::env` | `pub fn build_worker_env(device: &GpuDevice, cfg: &ServerConfig, port: u16) -> HashMap<String, String>` |

New field on existing type:
| Item | Type | Path | Description |
|------|------|------|-------------|
| `log_level` | `pub log_level: String` | `anvilml_core::config::ServerConfig` | Forwarded to worker subprocess as `ANVILML_LOG_LEVEL`. Default: `"info"`. |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | crates/anvilml-core/src/config.rs | Add `log_level: String` field with default `"info"` to `ServerConfig` |
| CREATE | crates/anvilml-worker/src/env.rs | New module with `build_worker_env` function |
| MODIFY | crates/anvilml-worker/src/lib.rs | Replace stub with `pub mod env;` and `pub use env::build_worker_env;` |
| CREATE | crates/anvilml-worker/tests/env_tests.rs | Integration tests for `build_worker_env` |
| MODIFY | crates/anvilml-worker/Cargo.toml | Bump patch version `0.1.0` → `0.1.1` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `tests/env_tests.rs` | `test_ipc_port` | `ANVILML_IPC_PORT` equals port as decimal string | None | port=9000, any device, any cfg | `map["ANVILML_IPC_PORT"] == "9000"` | `cargo test -p anvilml-worker --features mock-hardware -- env::test_ipc_port` exits 0 |
| `tests/env_tests.rs` | `test_worker_id` | `ANVILML_WORKER_ID` equals device index as string | None | device.index=2 | `map["ANVILML_WORKER_ID"] == "2"` | `cargo test -p anvilml-worker --features mock-hardware -- env::test_worker_id` exits 0 |
| `tests/env_tests.rs` | `test_device_index` | `ANVILML_DEVICE_INDEX` equals device index as string | None | device.index=0 | `map["ANVILML_DEVICE_INDEX"] == "0"` | `cargo test -p anvilml-worker --features mock-hardware -- env::test_device_index` exits 0 |
| `tests/env_tests.rs` | `test_device_type_cuda` | `ANVILML_DEVICE_TYPE` is `"cuda"` for Cuda device | None | device_type=DeviceType::Cuda | `map["ANVILML_DEVICE_TYPE"] == "cuda"` | `cargo test -p anvilml-worker --features mock-hardware -- env::test_device_type_cuda` exits 0 |
| `tests/env_tests.rs` | `test_device_type_rocm` | `ANVILML_DEVICE_TYPE` is `"rocm"` for Rocm device | None | device_type=DeviceType::Rocm | `map["ANVILML_DEVICE_TYPE"] == "rocm"` | `cargo test -p anvilml-worker --features mock-hardware -- env::test_device_type_rocm` exits 0 |
| `tests/env_tests.rs` | `test_device_type_cpu` | `ANVILML_DEVICE_TYPE` is `"cpu"` for Cpu device | None | device_type=DeviceType::Cpu | `map["ANVILML_DEVICE_TYPE"] == "cpu"` | `cargo test -p anvilml-worker --features mock-hardware -- env::test_device_type_cpu` exits 0 |
| `tests/env_tests.rs` | `test_log_level` | `ANVILML_LOG_LEVEL` matches cfg.log_level | None | cfg.log_level="debug" | `map["ANVILML_LOG_LEVEL"] == "debug"` | `cargo test -p anvilml-worker --features mock-hardware -- env::test_log_level` exits 0 |
| `tests/env_tests.rs` | `test_max_ipc_payload_mib` | `ANVILML_MAX_IPC_PAYLOAD_MIB` matches cfg value | None | cfg.max_ipc_payload_mib=512 | `map["ANVILML_MAX_IPC_PAYLOAD_MIB"] == "512"` | `cargo test -p anvilml-worker --features mock-hardware -- env::test_max_ipc_payload_mib` exits 0 |
| `tests/env_tests.rs` | `test_mock_hardware_flag` | `ANVILML_WORKER_MOCK` is `"1"` when mock-hardware feature active | Feature `mock-hardware` enabled | Any inputs | `map.contains_key("ANVILML_WORKER_MOCK") && map["ANVILML_WORKER_MOCK"] == "1"` | `cargo test -p anvilml-worker --features mock-hardware -- env::test_mock_hardware_flag` exits 0 |
| `tests/env_tests.rs` | `test_total_count` | HashMap contains exactly 7 entries with mock-hardware (6 without) | None | Any inputs | `map.len() == 7` (with mock) | `cargo test -p anvilml-worker --features mock-hardware -- env::test_total_count` exits 0 |

## CI Impact

No CI changes required. The task adds a new test module under `crates/anvilml-worker/tests/`, which is picked up automatically by the existing `cargo test --workspace --features mock-hardware` CI job (defined in `docs/ENVIRONMENT.md §6`). No new CI jobs, gates, or file types are introduced.

## Platform Considerations

None identified. The `build_worker_env` function is a pure data transformation with no I/O, no path handling, no platform-specific branching (the `#[cfg(feature = "mock-hardware")]` guard is feature-gated, not platform-gated). The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ServerConfig::default()` roundtrip test (Gate 1 `config_reference`) fails because the new `log_level` field is not present in `anvilml.toml` | Medium | High | The `config_reference` test asserts that the key set of `ServerConfig::default()` (serialised to TOML) exactly matches `anvilml.toml`. Since `log_level` is a new field with a default, the serialised output will include it. However, the task does not list updating `anvilml.toml` — the ACT agent should run `cargo test -p anvilml --features mock-hardware -- config_reference` and if it fails, add `log_level = "info"` to `anvilml.toml` as a minimal fix. Document under `## Deviations from Plan`. |
| `DeviceType` enum does not have a built-in `ToString` method — using `format!("{:?}", ...)` produces `"Cuda"` (PascalCase) instead of `"cuda"` (snake_case) | Low | Medium | Use an explicit `match` expression in `device_type_label()` to produce lowercase strings. This avoids any confusion with serde rename attributes which only apply during serialisation. |
| `HashMap` ordering makes test assertions order-dependent | Low | Low | Tests use key lookup (`map["KEY"]`) rather than iterating the map, so order is irrelevant. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --features mock-hardware -- env` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0
