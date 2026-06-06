# Plan Report: P9-A3

| Field       | Value                                           |
|-------------|-------------------------------------------------|
| Task ID     | P9-A3                                           |
| Phase       | 009 — Worker Spawn & Handshake                  |
| Description | anvilml-worker: env.rs build_worker_env         |
| Depends on  | P9-A2                                           |
| Project     | anvilml                                         |
| Planned at  | 2026-06-06T09:17:10Z                            |
| Attempt     | 1                                               |

## Objective

Create `crates/anvilml-worker/src/env.rs` with the `build_worker_env(device, cfg)` function that produces a `HashMap<String, String>` of environment variables to inject into each Python worker child process. The function must handle all three device types (CUDA, ROCm, CPU), set platform-appropriate device isolation vars (`CUDA_VISIBLE_DEVICES` for CUDA, `HIP_VISIBLE_DEVICES` for ROCm on both Linux and Windows), ROCm-specific performance flags (`ROCBLAS_USE_HIPBLASLT`, `HSA_OVERRIDE_GFX_VERSION` with Unix-only cfg-gating), universal threading variables, worker identity variables, and mock mode propagation.

## Scope

### In Scope
- Create `crates/anvilml-worker/src/env.rs` with the `build_worker_env` function
- Add `anvilml-core` dependency to `anvilml-worker/Cargo.toml` (already present)
- Re-export the function from `crates/anvilml-worker/src/lib.rs`
- Write unit tests covering four scenarios: CUDA, ROCm-Linux-with-HSA, ROCm-Windows-no-HSA, CPU

### Out of Scope
- Modifying `managed.rs`, `pool.rs`, or any other worker crate module (P9-A4, P9-A5)
- Modifying the server, scheduler, or backend crates
- Adding integration tests that spawn real child processes
- Modifying CI workflow files
- Adding logging instrumentation (logging is out of scope for this task — managed.rs in P9-A4 will handle spawn logging)

## Approach

1. **Add `anvilml-core` to `anvilml-worker/Cargo.toml`** — The file already lists `anvilml-core`, `anvilml-hardware`, and `anvilml-ipc` as dependencies with `mock-hardware` feature forwarding. No change needed.

2. **Create `crates/anvilml-worker/src/env.rs`** with the following structure:
   - Import `std::collections::HashMap`, `std::env`, and types from `anvilml_core` (`GpuDevice`, `ServerConfig`, `DeviceType`)
   - Implement `pub fn build_worker_env(device: &GpuDevice, cfg: &ServerConfig) -> HashMap<String, String>`:
     a. Start with an empty `HashMap::new()`.
     b. **Device-specific isolation vars:**
        - `DeviceType::Cuda`: insert `CUDA_VISIBLE_DEVICES={device.index}`
        - `DeviceType::Rocm`: insert `HIP_VISIBLE_DEVICES={device.index}` (both Linux and Windows)
        - `DeviceType::Cpu`: no device isolation var
     c. **ROCm-specific flags** (only when `device.device_type == Rocm`):
        - `ROCBLAS_USE_HIPBLASLT`: `"1"` if `cfg.rocm.use_hipblaslt`, else `"0"`
        - `HSA_OVERRIDE_GFX_VERSION`: only on Unix (`#[cfg(unix)]`). When the feature gate is active, insert the value from `cfg.rocm.hsa_override_gfx_version` if `Some`. On Windows or when `None`, do not insert. The cfg-gate ensures this code never compiles for Windows targets.
     d. **Threading variables** (all device types):
        - `OMP_NUM_THREADS = cfg.num_threads.to_string()`
        - `MKL_NUM_THREADS = cfg.num_threads.to_string()`
        - `OPENBLAS_NUM_THREADS = cfg.num_threads.to_string()`
        - `VECLIB_MAXIMUM_THREADS = cfg.num_threads.to_string()` (macOS vecLib)
     e. **AnvilML-specific threading variables** (all device types):
        - `ANVILML_NUM_THREADS = cfg.num_threads.to_string()`
        - `ANVILML_NUM_INTEROP_THREADS = cfg.num_interop_threads.to_string()`
     f. **Worker identity variables** (all device types):
        - `ANVILML_WORKER_ID = format!("worker-{}", device.index)`
        - `ANVILML_DEVICE_INDEX = device.index.to_string()`
     g. **Mock mode propagation**: Check `std::env::var("ANVILML_WORKER_MOCK")` — if it is set (to any value including "1"), propagate the same value into the child env map. This ensures mock-mode workers stay in mock mode.

3. **Re-export from `lib.rs`** — Update `crates/anvilml-worker/src/lib.rs` to replace the stub with:
   - `pub mod env;`
   - `pub use env::build_worker_env;`

4. **Write unit tests in `env.rs`** under `#[cfg(test)]`:
   - `test_build_env_cuda`: Mock CUDA device (index 0), verify `CUDA_VISIBLE_DEVICES=0`, verify no `HIP_VISIBLE_DEVICES`, verify threading vars, verify worker ID/index.
   - `test_build_env_rocm_linux_hsa`: Mock ROCm device with `use_hipblaslt=true` and `hsa_override_gfx_version=Some("10.3.0")`. Verify `HIP_VISIBLE_DEVICES=0`, `ROCBLAS_USE_HIPBLASLT=1`, `HSA_OVERRIDE_GFX_VERSION=10.3.0` (on unix cfg), no `CUDA_VISIBLE_DEVICES`.
   - `test_build_env_rocm_windows_no_hsa`: Mock ROCm device with `use_hipblaslt=false` and `hsa_override_gfx_version=None`. Verify `HIP_VISIBLE_DEVICES=0`, `ROCBLAS_USE_HIPBLASLT=0`, no `HSA_OVERRIDE_GFX_VERSION`, no `CUDA_VISIBLE_DEVICES`.
   - `test_build_env_cpu`: CPU device type, verify no `CUDA_VISIBLE_DEVICES` or `HIP_VISIBLE_DEVICES`, verify threading vars and worker identity.
   - `test_build_env_mock_propagation`: Set `ANVILML_WORKER_MOCK=1` before calling, verify the returned map contains `ANVILML_WORKER_MOCK=1`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-worker/src/env.rs` | New module: `build_worker_env` function + tests |
| Modify | `crates/anvilml-worker/src/lib.rs` | Replace stub with `pub mod env; pub use env::build_worker_env;` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-worker/src/env.rs` | `test_build_env_cuda` | CUDA: `CUDA_VISIBLE_DEVICES=0`, no HIP vars, threading vars present, worker identity correct |
| `crates/anvilml-worker/src/env.rs` | `test_build_env_rocm_linux_hsa` | ROCm Linux: `HIP_VISIBLE_DEVICES=0`, `ROCBLAS_USE_HIPBLASLT=1`, `HSA_OVERRIDE_GFX_VERSION=10.3.0` (unix cfg) |
| `crates/anvilml-worker/src/env.rs` | `test_build_env_rocm_windows_no_hsa` | ROCm Windows: `HIP_VISIBLE_DEVICES=0`, `ROCBLAS_USE_HIPBLASLT=0`, no `HSA_OVERRIDE_GFX_VERSION` |
| `crates/anvilml-worker/src/env.rs` | `test_build_env_cpu` | CPU: no device isolation var, threading vars present, worker identity correct |
| `crates/anvilml-worker/src/env.rs` | `test_build_env_mock_propagation` | Mock mode env var is propagated from parent to child env map |

## CI Impact

No CI changes required. The task only adds a new module and unit tests within the existing `anvilml-worker` crate. The existing CI gates (`cargo test --workspace --features mock-hardware`, `cargo clippy --workspace --features mock-hardware`) will automatically cover the new code. The `#[cfg(unix)]` gate on HSA_OVERRIDE_GFX_VERSION is a compile-time conditional — it compiles out on Windows targets and includes the code on Unix targets, both of which are exercised by the existing cross-compilation checks.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `#[cfg(unix)]` gate on HSA may not compile for Windows cross-target if the test agent lacks mingw-w64 toolchain | Low | Build failure on cross-check | The cross-check (`cargo check --target x86_64-pc-windows-gnu`) is already a documented CI gate in ENVIRONMENT.md §7. If the toolchain is missing, note it as a blocker. |
| Mock device index may not match expected test values (mock uses index 0) | Low | Test assertion mismatch | All mock devices use `index: 0` per mock.rs; tests assert on index 0 consistently. |
| `ANVILML_WORKER_MOCK` env var set during test execution leaks to other tests | Medium | Flaky cross-test interference | Each test that sets/clears env vars uses `#[serial]` attribute and restores the previous value at the end of the test function. |

## Acceptance Criteria

- [ ] `crates/anvilml-worker/src/env.rs` exists with `pub fn build_worker_env(device: &GpuDevice, cfg: &ServerConfig) -> HashMap<String, String>`
- [ ] CUDA device produces `CUDA_VISIBLE_DEVICES={index}` and no `HIP_VISIBLE_DEVICES`
- [ ] ROCm device produces `HIP_VISIBLE_DEVICES={index}` on both platforms; `ROCBLAS_USE_HIPBLASLT` set correctly from config
- [ ] `HSA_OVERRIDE_GFX_VERSION` is only present when `#[cfg(unix)]` is active AND `hsa_override_gfx_version` is `Some` in config
- [ ] All device types produce `OMP_NUM_THREADS`, `MKL_NUM_THREADS`, `OPENBLAS_NUM_THREADS`, `VECLIB_MAXIMUM_THREADS`, `ANVILML_NUM_THREADS`, `ANVILML_NUM_INTEROP_THREADS`, `ANVILML_WORKER_ID`, `ANVILML_DEVICE_INDEX`
- [ ] `ANVILML_WORKER_MOCK` is propagated when set in parent environment
- [ ] `cargo test -p anvilml-worker --features mock-hardware -- env` exits 0 with all five tests passing
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
