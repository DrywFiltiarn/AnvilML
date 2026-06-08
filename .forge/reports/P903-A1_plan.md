# Plan Report: P903-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P903-A1                                           |
| Phase       | 903 — IPC Transport Rework                        |
| Description | anvilml-worker: add ANVILML_IPC_SOCKET to build_worker_env |
| Depends on  | none                                              |
| Project     | anvilml                                           |
| Planned at  | 2026-06-08T19:42:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add the `ANVILML_IPC_SOCKET` environment variable injection into the worker environment
builder (`build_worker_env`) so that every spawned Python worker receives the IPC socket
path at startup. This is the preparatory step for the full stdin/stdout → Unix socket /
named pipe transport rework in Phase 903.

## Scope

### In Scope
- Add `ipc_socket_path: &str` as the third parameter to `build_worker_env` in
  `crates/anvilml-worker/src/env.rs`
- Insert `ANVILML_IPC_SOCKET` into the returned `HashMap<String, String>` with the
  provided value (always, even if empty)
- Update the single call site in `crates/anvilml-worker/src/managed.rs` to pass `""`
  as the placeholder third argument
- Update all five existing tests in `env.rs` that call `build_worker_env` to pass `""`
- Add one new test asserting that a non-empty `ipc_socket_path` value appears in the
  returned map under the key `ANVILML_IPC_SOCKET`
- Bump `anvilml-worker` crate patch version from `0.1.14` to `0.1.15`

### Out of Scope
- Actual socket creation / binding logic (P903-A2)
- Python-side socket connection code (P903-A3)
- Any changes to `pool.rs`, `framing.rs`, message types, or other crates
- IPC probe binary changes (P903-A4)

## Approach

1. **Modify `crates/anvilml-worker/src/env.rs`:**
   - Change the function signature from
     `pub fn build_worker_env(device: &GpuDevice, cfg: &ServerConfig)` to
     `pub fn build_worker_env(device: &GpuDevice, cfg: &ServerConfig, ipc_socket_path: &str)`
   - After all existing env var inserts (after the mock-mode propagation block, before
     `env`), insert one line:
     ```rust
     env.insert(
         "ANVILML_IPC_SOCKET".to_string(),
         ipc_socket_path.to_string(),
     );
     ```
   - Update the doc comment to mention `ANVILML_IPC_SOCKET` in the returned map.

2. **Update all five existing tests in `env.rs`:**
   - `test_build_env_cuda` (line ~147): change `build_worker_env(&device, &cfg)` to
     `build_worker_env(&device, &cfg, "")`
   - `test_build_env_rocm_linux_hsa` (line ~197): same change
   - `test_build_env_rocm_windows_no_hsa` (line ~244): same change
   - `test_build_env_cpu` (line ~285): same change
   - `test_build_env_mock_propagation` (line ~331): same change

3. **Add one new test in `env.rs`:**
   - Name: `test_build_env_ipc_socket_path`
   - Creates a CUDA device and default config, calls
     `build_worker_env(&device, &cfg, "/tmp/anvilml-12345/worker-0.sock")`
   - Asserts `env.get("ANVILML_IPC_SOCKET")` equals
     `Some("/tmp/anvilml-12345/worker-0.sock")`

4. **Update call site in `crates/anvilml-worker/src/managed.rs`:**
   - Line 157: change `envs(build_worker_env(device, cfg))` to
     `envs(build_worker_env(device, cfg, ""))`

5. **Bump crate version:**
   - `crates/anvilml-worker/Cargo.toml`: change `version = "0.1.14"` to
     `version = "0.1.15"` (patch bump per FORGE_AGENT_RULES §12)

6. **Verify:** run `cargo test -p anvilml-worker --features mock-hardware` and confirm
   exit 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/env.rs` | Add `ipc_socket_path` param, insert env var, update 5 tests, add 1 new test |
| Modify | `crates/anvilml-worker/src/managed.rs` | Update call site to pass `""` placeholder |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version `0.1.14 → 0.1.15` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `env.rs` | `test_build_env_cuda` | Existing test updated — still validates CUDA env vars with new arg |
| `env.rs` | `test_build_env_rocm_linux_hsa` | Existing test updated — still validates ROCm env vars with new arg |
| `env.rs` | `test_build_env_rocm_windows_no_hsa` | Existing test updated — still validates ROCm/no-HSA env vars with new arg |
| `env.rs` | `test_build_env_cpu` | Existing test updated — still validates CPU env vars with new arg |
| `env.rs` | `test_build_env_mock_propagation` | Existing test updated — still validates mock propagation with new arg |
| `env.rs` | `test_build_env_ipc_socket_path` (new) | New `ANVILML_IPC_SOCKET` key is present with the exact value passed as third arg |

## CI Impact

No CI changes required. The change is confined to the `anvilml-worker` crate. The
existing CI test command `cargo test --workspace --features mock-hardware` will
automatically include the updated crate tests. No new CI gates are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Adding a new required parameter breaks compilation at the call site | Low | Medium | The plan explicitly lists the single call site in `managed.rs` and updates it in the same session |
| New test fails due to assertion mismatch | Low | Low | The test simply asserts the exact value passed in — deterministic, no external dependencies |
| `ANVILML_IPC_SOCKET` inserted even when empty may confuse downstream code | Low | Low | Empty string is the documented placeholder; the real path is wired in P903-A2. The Python worker in P903-A3 will read and use the value |

## Acceptance Criteria

- [ ] `build_worker_env` accepts `ipc_socket_path: &str` as the third parameter
- [ ] `ANVILML_IPC_SOCKET` is present in the returned `HashMap` with the passed value
- [ ] All five existing tests compile and pass with `""` as the third argument
- [ ] New test `test_build_env_ipc_socket_path` passes and verifies non-empty path injection
- [ ] Call site in `managed.rs` passes `""` placeholder
- [ ] `anvilml-worker` crate version bumped to `0.1.15`
- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0
