# Plan Report: P8-B1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P8-B1                                             |
| Phase       | 8 — IPC Stress Gate & Worker Pool                 |
| Description | anvilml-worker: WorkerEnv environment variable map builder |
| Depends on  | P8-A1                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-07-01T00:45:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `crates/anvilml-worker/src/env.rs` implementing `WorkerEnv::build()` — a pure function that constructs a `HashMap<String, String>` of environment variables to inject into every Python worker subprocess. This establishes the exact variable contract defined in `ANVILML_DESIGN.md §9.7` before any subprocess is spawned, and exports `WorkerEnv` as a public type via `lib.rs`.

## Scope

### In Scope
- Create `crates/anvilml-worker/src/env.rs` with `WorkerEnv::build(ipc_port: u16, worker_id: &str, device_index: u32, device_type: DeviceType, mock: bool, log_level: &str, max_ipc_payload_mib: u32) -> HashMap<String, String>`.
- Map all 7 input parameters to the correct `ANVILML_*` env var keys per §9.7's table.
- `ANVILML_WORKER_MOCK` is absent from the map when `mock == false`; present as `"1"` when `mock == true`.
- `ANVILML_FORCE_WORKER_MOCK` is NOT set by this builder (caller handles it separately).
- Declare `mod env;` and `pub use env::WorkerEnv;` in `lib.rs`.
- Create `crates/anvilml-worker/tests/env_tests.rs` with ≥5 tests covering the acceptance criteria.

### Out of Scope
None. This task's `defers_to` field is empty (`defers_to (from JSON): []`); no functionality is deferred.

## Existing Codebase Assessment

`anvilml-worker` is an empty stub crate at Phase 8's start. Its `lib.rs` contains only a one-line `//!` crate doc comment. No source modules exist yet (no `env.rs`, `spawn.rs`, etc.). No `tests/` directory exists. The crate depends on `anvilml-ipc`, `anvilml-hardware`, and `anvilml-core` (all path dependencies within the workspace). `DeviceType` is already defined and exported from `anvilml-core::types::hardware` (with `#[serde(rename_all = "snake_case")]` mapping `Cuda`→`"cuda"`, `Rocm`→`"rocm"`, `Cpu`→`"cpu"`). The established crate-level pattern is a minimal `lib.rs` with only `pub mod` / `pub use` declarations and a `//!` crate doc comment, staying well under the 80-line hard cap.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source     | Feature flags confirmed |
|--------|-------------|-----------------|----------------|------------------------|
| crate  | anvilml-core| 0.1.21 (workspace) | Cargo.toml (workspace lock) | none (path dep, no features needed) |
| std    | HashMap     | —                | Rust std lib   | n/a |

No new external crates are introduced. `HashMap` is from `std::collections`. `DeviceType` is re-exported transitively via `anvilml_core` (already a declared dependency).

## Approach

1. **Create `crates/anvilml-worker/src/env.rs`** with the `WorkerEnv` struct and its `build` method:
   - Import `std::collections::HashMap` and `anvilml_core::DeviceType`.
   - Define `pub struct WorkerEnv;` (zero-sized unit struct — the builder pattern uses a static method, not an instance).
   - Implement `pub fn WorkerEnv::build(ipc_port: u16, worker_id: &str, device_index: u32, device_type: DeviceType, mock: bool, log_level: &str, max_ipc_payload_mib: u32) -> HashMap<String, String>`:
     - Allocate a new `HashMap`.
     - Insert `("ANVILML_IPC_PORT", ipc_port.to_string())`.
     - Insert `("ANVILML_WORKER_ID", worker_id.to_string())`.
     - Insert `("ANVILML_DEVICE_INDEX", device_index.to_string())`.
     - Insert `("ANVILML_DEVICE_TYPE", device_type.as_str())` — use a helper or match to convert `DeviceType` to `"cuda"`, `"rocm"`, or `"cpu"` string. (Rationale: `DeviceType` has `#[serde(rename_all = "snake_case")]` but no `AsRef<str>` or `Display` impl; a match is the most explicit and zero-dependency approach.)
     - If `mock` is `true`, insert `("ANVILML_WORKER_MOCK", "1")`. If `mock` is `false`, do NOT insert this key (absent from map).
     - Insert `("ANVILML_LOG_LEVEL", log_level.to_string())`.
     - Insert `("ANVILML_MAX_IPC_PAYLOAD_MIB", max_ipc_payload_mib.to_string())`.
     - Return the populated map.
   - Add a `///` doc comment on `WorkerEnv` describing its purpose.
   - Add a `///` doc comment on `build()` with parameter descriptions and return type.

2. **Update `crates/anvilml-worker/src/lib.rs`**:
   - Add `mod env;` after the existing `//!` doc comment.
   - Add `pub use env::WorkerEnv;` after the `mod` declaration.
   - Keep the file ≤ 80 lines (it will be ~4 lines).

3. **Create `crates/anvilml-worker/tests/env_tests.rs`** with ≥5 integration tests:
   - Test that all env vars are present with correct values (one comprehensive test).
   - Test `ANVILML_WORKER_MOCK` absent when `mock = false`.
   - Test `ANVILML_WORKER_MOCK = "1"` when `mock = true`.
   - Test `ANVILML_DEVICE_TYPE` maps correctly for all 3 `DeviceType` variants (`Cuda`→`"cuda"`, `Rocm`→`"rocm"`, `Cpu`→`"cpu"`).
   - Test a 4th variant combining specific values to verify `ANVILML_FORCE_WORKER_MOCK` is NOT present.

4. **Run `cargo test -p anvilml-worker --test env_tests`** to verify all tests pass.

No logging calls are needed — `WorkerEnv::build()` is a pure data transformation with no side effects, no I/O, and no decision points that would require observability. No `#[tracing::instrument]` is needed.

## Public API Surface

```rust
// crates/anvilml-worker/src/env.rs
pub struct WorkerEnv;

impl WorkerEnv {
    /// Build the environment variable map for a Python worker subprocess.
    ///
    /// Returns a `HashMap` containing all `ANVILML_*` variables that should be
    /// injected into the worker's subprocess `Command`. See `ANVILML_DESIGN.md §9.7`.
    ///
    /// # Arguments
    /// * `ipc_port` — TCP port of the ROUTER socket.
    /// * `worker_id` — Bare device index as a string (e.g. "0").
    /// * `device_index` — GPU device index.
    /// * `device_type` — Compute backend (cuda, rocm, or cpu).
    /// * `mock` — Whether the mock-hardware cargo feature is active.
    /// * `log_level` — Forwarded from server config.
    /// * `max_ipc_payload_mib` — Maximum IPC message size in MiB.
    pub fn build(
        ipc_port: u16,
        worker_id: &str,
        device_index: u32,
        device_type: DeviceType,
        mock: bool,
        log_level: &str,
        max_ipc_payload_mib: u32,
    ) -> HashMap<String, String>;
}
```

Re-exported from `lib.rs`:
```rust
pub use env::WorkerEnv;
```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-worker/src/env.rs` | `WorkerEnv` struct and `build()` method |
| MODIFY | `crates/anvilml-worker/src/lib.rs` | Add `mod env;` and `pub use env::WorkerEnv;` |
| CREATE | `crates/anvilml-worker/tests/env_tests.rs` | ≥5 integration tests for `WorkerEnv::build()` |
| Bump   | `crates/anvilml-worker/Cargo.toml` | Bump patch version (per §14 of FORGE_AGENT_RULES) |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-worker/tests/env_tests.rs` | `test_build_all_vars_present` | All 6 builder-set env vars are present with correct string values | None | `ipc_port=5555, worker_id="0", device_index=1, device_type=Cuda, mock=false, log_level=debug, max_ipc_payload_mib=512` | Map contains exactly 6 entries with correct keys and values | `cargo test -p anvilml-worker --test env_tests -- test_build_all_vars_present` exits 0 |
| `crates/anvilml-worker/tests/env_tests.rs` | `test_worker_mock_absent_when_false` | `ANVILML_WORKER_MOCK` key is absent from map when `mock=false` | None | `mock=false` | `"ANVILML_WORKER_MOCK"` not in map keys | `cargo test -p anvilml-worker --test env_tests -- test_worker_mock_absent_when_false` exits 0 |
| `crates/anvilml-worker/tests/env_tests.rs` | `test_worker_mock_present_when_true` | `ANVILML_WORKER_MOCK="1"` when `mock=true` | None | `mock=true` | `"ANVILML_WORKER_MOCK"` maps to `"1"` | `cargo test -p anvilml-worker --test env_tests -- test_worker_mock_present_when_true` exits 0 |
| `crates/anvilml-worker/tests/env_tests.rs` | `test_device_type_cuda` | `DeviceType::Cuda` → `"cuda"` | None | `device_type=DeviceType::Cuda` | `"ANVILML_DEVICE_TYPE"` maps to `"cuda"` | `cargo test -p anvilml-worker --test env_tests -- test_device_type_cuda` exits 0 |
| `crates/anvilml-worker/tests/env_tests.rs` | `test_device_type_rocm` | `DeviceType::Rocm` → `"rocm"` | None | `device_type=DeviceType::Rocm` | `"ANVILML_DEVICE_TYPE"` maps to `"rocm"` | `cargo test -p anvilml-worker --test env_tests -- test_device_type_rocm` exits 0 |
| `crates/anvilml-worker/tests/env_tests.rs` | `test_device_type_cpu` | `DeviceType::Cpu` → `"cpu"` | None | `device_type=DeviceType::Cpu` | `"ANVILML_DEVICE_TYPE"` maps to `"cpu"` | `cargo test -p anvilml-worker --test env_tests -- test_device_type_cpu` exits 0 |
| `crates/anvilml-worker/tests/env_tests.rs` | `test_force_worker_mock_absent` | `ANVILML_FORCE_WORKER_MOCK` is never set by the builder | None | Any inputs | `"ANVILML_FORCE_WORKER_MOCK"` not in map keys | `cargo test -p anvilml-worker --test env_tests -- test_force_worker_mock_absent` exits 0 |

## CI Impact

No CI changes required. The new test file `tests/env_tests.rs` is picked up automatically by `cargo test --workspace --features mock-hardware` (ENVIRONMENT.md §6 Step 6), which runs all integration tests in the workspace. No new CI jobs or gates are needed.

## Platform Considerations

None identified. The `WorkerEnv::build()` function is a pure data transformation with no platform-specific behaviour — no `#[cfg(unix)]` / `#[cfg(windows)]` guards needed. The device type string mapping (`"cuda"`, `"rocm"`, `"cpu"`) is the same on all platforms. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `DeviceType` lacks an `AsRef<str>` or `Display` impl, requiring a match expression for the string conversion | Medium | Low | Read the actual `DeviceType` definition in `hardware.rs` before writing; use a simple `match` with the three known variants. This is the most explicit and zero-dependency approach. |
| `anvilml-core` re-exports `DeviceType` through `pub use types::*` in `lib.rs` — the import path must be `anvilml_core::DeviceType` (not a direct path to `hardware.rs`) | Low | Medium | The `anvilml-core` crate already re-exports `DeviceType` via `pub use types::*;` in its `lib.rs`. Use `use anvilml_core::DeviceType;` which is the established pattern. |
| Test file placement convention requires `tests/` directory to exist before creating test files | Low | Low | Create the `tests/` directory first (it will be empty until the test file is written). Rust test crates automatically discover `*.rs` files in `tests/`. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --test env_tests` exits 0
- [ ] `wc -l crates/anvilml-worker/src/lib.rs` returns ≤ 80
- [ ] `grep -c "mod env;" crates/anvilml-worker/src/lib.rs` returns 1
- [ ] `grep -c "pub use env::WorkerEnv" crates/anvilml-worker/src/lib.rs` returns 1
- [ ] `grep -c "ANVILML_FORCE_WORKER_MOCK" crates/anvilml-worker/src/env.rs` returns 0 (builder never sets this key)
