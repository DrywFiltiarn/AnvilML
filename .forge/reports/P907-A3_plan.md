# Plan Report: P907-A3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P907-A3                                           |
| Phase       | 907 — ZeroMQ IPC Transport                        |
| Description | anvilml-worker: env.rs replace ANVILML_IPC_SOCKET with ANVILML_IPC_PORT |
| Depends on  | P907-A1 (ZeroMQ transport wired in managed.rs)    |
| Project     | anvilml                                           |
| Planned at  | 2026-06-13T12:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Replace the `ANVILML_IPC_SOCKET` environment variable with `ANVILML_IPC_PORT` in
`anvilml-worker::env::build_worker_env`. The new variable carries a `u16` port number
(as a decimal string) instead of a filesystem socket path, reflecting the ZeroMQ TCP
transport introduced in P907-A1.

## Scope

### In Scope
- `crates/anvilml-worker/src/env.rs`: change `build_worker_env` signature and body
- `crates/anvilml-worker/src/managed.rs`: update the single call site to pass port instead of address string
- `crates/anvilml-worker/src/env.rs` tests: update all 6 tests for the new parameter and assertions
- `crates/anvilml-worker/Cargo.toml`: bump patch version `0.1.23 → 0.1.24`

### Out of Scope
- `worker/ipc.py` — Python worker side (handled in P907-A5)
- `managed.rs` ZeroMQ rewrite (handled in P907-A4)
- `docs/ENVIRONMENT.md §3.7` update (human-owned per TASKS_PHASE907.md "Prereq update required")
- Any other crate

## Approach

1. **Read `managed.rs` lines 166–180** to confirm the current `ipc_addr` construction:
   `TcpListener::bind("127.0.0.1:0")` → `local_addr()` → `to_string()` → `"127.0.0.1:XXXX"`.
   The call site on line 190 passes `&ipc_addr` (a `String` like `"127.0.0.1:XXXX"`).

2. **Modify `managed.rs` call site** (line 190): extract the port from `local_addr()`
   before calling `build_worker_env`. Change:
   ```rust
   let ipc_addr = local_addr.to_string();
   *self.ipc_socket_path.lock().unwrap() = ipc_addr.clone();
   // ...
   .envs(build_worker_env(device, cfg, &ipc_addr))
   ```
   to:
   ```rust
   let ipc_port = local_addr.port();
   *self.ipc_socket_path.lock().unwrap() = format!("127.0.0.1:{}", ipc_port);
   // ...
   .envs(build_worker_env(device, cfg, ipc_port))
   ```
   This preserves the `ipc_socket_path` field for logging while passing a clean `u16`
   port to `build_worker_env`.

3. **Modify `env.rs` function signature** (line 21):
   - Change `ipc_socket_path: &str` → `ipc_port: u16`
   - Update doc comment: replace "IPC socket path" with "IPC port"

4. **Modify `env.rs` env var insertion** (lines 77–81):
   - Change key from `"ANVILML_IPC_SOCKET"` → `"ANVILML_IPC_PORT"`
   - Change value from `ipc_socket_path.to_string()` → `ipc_port.to_string()`

5. **Update all 6 tests** in `env.rs`:
   - `test_build_env_cuda` (line ~158): change `build_worker_env(&device, &cfg, "")` →
     `build_worker_env(&device, &cfg, 55555)` (or any valid u16)
   - `test_build_env_rocm_linux_hsa` (line ~208): same change
   - `test_build_env_rocm_windows_no_hsa` (line ~255): same change
   - `test_build_env_cpu` (line ~296): same change
   - `test_build_env_mock_propagation` (line ~342): same change
   - `test_build_env_ipc_socket_path` (line ~355–365): **rename** to
     `test_build_env_ipc_port`, assert `ANVILML_IPC_PORT` is present with the correct
     value, assert `ANVILML_IPC_SOCKET` is absent

6. **Bump crate version** in `crates/anvilml-worker/Cargo.toml`:
   `0.1.23` → `0.1.24`

7. **Verify**:
   - `cargo test -p anvilml-worker --features mock-hardware -- env` exits 0
   - `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/env.rs` | Change param type, env var key, update doc comment, update 6 tests |
| Modify | `crates/anvilml-worker/src/managed.rs` | Update single call site: pass `u16` port instead of `&str` address |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version `0.1.23 → 0.1.24` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `env.rs` | `test_build_env_cuda` | CUDA env vars + ANVILML_IPC_PORT present |
| `env.rs` | `test_build_env_rocm_linux_hsa` | ROCm env vars + ANVILML_IPC_PORT present |
| `env.rs` | `test_build_env_rocm_windows_no_hsa` | ROCm no-HSA env vars + ANVILML_IPC_PORT present |
| `env.rs` | `test_build_env_cpu` | CPU env vars + ANVILML_IPC_PORT present |
| `env.rs` | `test_build_env_mock_propagation` | Mock propagation + ANVILML_IPC_PORT present |
| `env.rs` | `test_build_env_ipc_port` (renamed) | ANVILML_IPC_PORT value correct, ANVILML_IPC_SOCKET absent |

## CI Impact

No CI workflow changes required. The task only modifies source files in one crate.
The existing CI gates (`cargo test --workspace --features mock-hardware`,
`cargo check --target x86_64-pc-windows-gnu`) will cover this change.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `managed.rs` call site change breaks if `ipc_socket_path` field is used elsewhere for socket construction | Low | Medium | Scope is limited to the `build_worker_env` call; `ipc_socket_path` field is only used for logging/cleanup, not socket construction (that is handled in P907-A4) |
| Cross-compilation to x86_64-pc-windows-gnu fails due to type mismatch | Low | Medium | `u16` is a primitive — no platform-specific issues. Verify with the cross-check command. |
| Test assertions fail because old `ANVILML_IPC_SOCKET` key still expected | Low | Low | All 6 tests are updated in the same change to use the new key and parameter type |

## Acceptance Criteria

- [ ] `crates/anvilml-worker/src/env.rs` `build_worker_env` signature: `ipc_port: u16` (was `ipc_socket_path: &str`)
- [ ] `ANVILML_IPC_PORT` present in returned map; `ANVILML_IPC_SOCKET` absent
- [ ] All 6 tests in `env.rs` pass: `cargo test -p anvilml-worker --features mock-hardware -- env` exits 0
- [ ] Windows cross-check passes: `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0
- [ ] `managed.rs` call site passes port as `u16` (no string formatting of full address)
- [ ] Crate version bumped to `0.1.24`
- [ ] No public API surface change (signature change is internal-only: param type, not visibility)
