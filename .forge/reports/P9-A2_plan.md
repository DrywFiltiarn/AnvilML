# Plan Report: P9-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P9-A2                                             |
| Phase       | 009 — Worker Spawn & Handshake                    |
| Description | worker: worker_main.py mock-mode message loop (Ping/Pong/Init/Shutdown) |
| Depends on  | P9-A1                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-06-06T08:15:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `worker/worker_main.py`, the Python worker entry point that implements a blocking stdin message loop supporting the full handshake and keepalive protocol: `InitializeHardware` → `Ready{}` (with mock or real GPU properties), `Ping` → `Pong{seq}`, `MemoryQuery` → `MemoryReport(0,0)`, and `Shutdown` → `Dying{reason:shutdown}` + flush + exit 0. A background thread emits `MemoryReport` every 10 seconds. Mock mode (`ANVILML_WORKER_MOCK=1`) skips torch entirely and reports fixed stub values.

## Scope

### In Scope
- Create `worker/worker_main.py` with:
  - argparse for `--worker-id` (str) and `--device-index` (int).
  - Thread environment variable setup (`OMP_NUM_THREADS`, `MKL_NUM_THREADS`, `OPENBLAS_NUM_THREADS`, `VECLIB_MAXIMUM_THREADS`) from `ANVILML_*` env vars before any torch import.
  - Conditional torch import: skip entirely when `ANVILML_WORKER_MOCK=1`.
  - Hardware probe logic: mock path (fixed stubs) and real path (torch CUDA/ROCm properties + `mem_get_info()`).
  - Send `Ready{}` event on startup with the appropriate property set.
  - Blocking message loop reading framed msgpack via `ipc.py`:
    - `InitializeHardware` → trigger Ready send (if not already sent), then enter loop.
    - `Ping{seq}` → `Pong{seq}`.
    - `MemoryQuery{}` → `MemoryReport{vram_used_mib: 0, ram_used_mib: 0}`.
    - `Shutdown{}` → `Dying{reason: "shutdown"}`, flush stdout, exit 0.
  - Background thread: `MemoryReport` every 10 seconds (real: from torch; mock: zeros).
- Test file: `worker/tests/test_worker_main.py` with pytest tests for the message loop in mock mode.

### Out of Scope
- Real hardware GPU property probing beyond what's needed for the mock-mode test (the real path is implemented but not unit-tested here — it will be validated end-to-end via P9-A5 REST).
- Node execution logic (`executor.py`) — handled by a later phase task.
- Rust-side worker spawn/supervision — handled by P9-A3, P9-A4, P9-A5.
- Pipeline cache, OOM handling, node registry — deferred to later phases.

## Approach

1. **Read thread env vars and set them.** At module top (before any ML imports), read `ANVILML_NUM_THREADS` and `ANVILML_NUM_INTEROP_THREADS`. Set `OMP_NUM_THREADS`, `MKL_NUM_THREADS`, `OPENBLAS_NUM_THREADS`, `VECLIB_MAXIMUM_THREADS` to the thread count. Also set `torch.set_num_threads()` and `torch.set_num_interop_threads()` after import. This follows §14.1 of ANVILML_DESIGN.md and ENVIRONMENT.md §3.7.

2. **Conditional torch import.** Check `os.environ.get("ANVILML_WORKER_MOCK") == "1"`. If true, define stub values for all GPU properties and skip torch entirely. If false, `import torch` and probe the active device via `torch.cuda.is_available()` / `torch.cuda.get_device_properties()` / `torch.cuda.mem_get_info()`.

3. **Hardware property resolution.**
   - Mock: `vram_total_mib=8192`, `vram_free_mib=8192`, `arch="gfx1100"`, `fp16=True`, `bf16=True`, `flash_attention=False`.
   - Real: probe via torch CUDA/ROCm API. `mem_get_info()` for VRAM; `get_device_properties(0)` for arch string, fp16/bf16/flash-attention capabilities.

4. **InitializeHardware handler.** When the first message is `InitializeHardware`, resolve device properties, send `Ready{worker_id, device_index, vram_total_mib, vram_free_mib, arch, fp16, bf16, flash_attention}`, set a flag so subsequent messages flow normally.

5. **Message loop.** Use `ipc.read_frame()` in a `while True` loop:
   - Dispatch on the `_type` key (msgpack dict).
   - `Ping`: respond `Pong{seq}`.
   - `MemoryQuery`: respond `MemoryReport{vram_used_mib: 0, ram_used_mib: 0}`.
   - `Shutdown`: respond `Dying{reason: "shutdown"}`, flush stdout via `sys.stdout.buffer.flush()`, call `sys.exit(0)`.

6. **Background MemoryReport thread.** Start a `threading.Thread` daemon that calls `torch.cuda.mem_get_info()` (real) or returns `(0, 0)` (mock), computes used = total - free, and emits `MemoryReport{vram_used_mib, ram_used_mib}` every 10 seconds via `ipc.write_frame()`.

7. **Test coverage.** Create `worker/tests/test_worker_main.py` that:
   - Spawns the worker subprocess with `ANVILML_WORKER_MOCK=1`.
   - Writes framed messages to stdin (`Ping`, `MemoryQuery`, `Shutdown`).
   - Reads and asserts the expected framed responses on stdout.
   - Verifies the worker exits with code 0 after Shutdown.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `worker/worker_main.py` | Worker entry point: argparse, env setup, mock/real hardware probe, message loop (InitializeHardware/Ping-Pong/MemoryQuery-MemoryReport/Shutdown-Dying), background MemoryReport thread. |
| Create | `worker/tests/test_worker_main.py` | pytest: spawn worker in mock mode, send Ping→Pong, MemoryQuery→MemoryReport, Shutdown→Dying, verify exit code 0. |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `worker/tests/test_worker_main.py` | `test_ping_pong` | Worker receives Ping{seq} and responds with Pong{seq}. |
| `worker/tests/test_worker_main.py` | `test_memory_query_report` | Worker receives MemoryQuery{} and responds with MemoryReport{0, 0}. |
| `worker/tests/test_worker_main.py` | `test_shutdown_dying_exit` | Worker receives Shutdown{}, responds Dying{reason: "shutdown"}, flushes stdout, exits 0. |
| `worker/tests/test_worker_main.py` | `test_ready_on_init_hardware` | Worker sends Ready{} with correct mock values after InitializeHardware message. |
| `worker/tests/test_worker_main.py` | `test_mock_values` | Mock Ready payload matches spec: vram_total=8192, vram_free=8192, arch="gfx1100", fp16=true, bf16=true, flash_attention=false. |

## CI Impact

No CI workflow files are modified. The existing Python worker test gate (`ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v`) in `docs/ENVIRONMENT.md` §6 and §9 will automatically pick up the new `test_worker_main.py` file. No changes to format, clippy, or Rust test gates are needed since this task only touches Python files.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Mock values mismatch with what Rust side expects (e.g., arch string format) | Medium | High | Use exactly the values specified in the task description: `8192/8192/gfx1100/true/true/false`. The P9-A5 REST integration will catch any discrepancy. |
| Background MemoryReport thread interfering with stdout during shutdown | Low | Medium | Ensure the background thread checks a stop flag and does not write after Shutdown is received. Use `threading.Event` for clean termination. |
| Test subprocess spawning fails in CI due to missing Python | Low | High | CI already has Python 3.12 (ENVIRONMENT.md §21.3). The test requires only msgpack which is installed by P9-A1's base.txt. If torch is absent, mock mode handles it. |
| Pipe read-fully not handling partial reads on Windows | Low | Medium | Rely on `ipc.py`'s existing `read_frame()` which already implements the read-fully loop (§7.1). The worker_main.py only calls `ipc.read_frame()`. |

## Acceptance Criteria

- [ ] `worker/worker_main.py` exists with argparse accepting `--worker-id` and `--device-index`.
- [ ] Thread env vars (OMP/MKL/OPENBLAS/VECLIB) are set before any ML imports.
- [ ] `ANVILML_WORKER_MOCK=1` causes torch import to be skipped entirely.
- [ ] InitializeHardware triggers Ready{} with correct mock values (8192/8192/gfx1100/true/true/false).
- [ ] Ping{seq} elicits Pong{seq}.
- [ ] MemoryQuery{} elicits MemoryReport{0, 0}.
- [ ] Shutdown{} elicits Dying{reason: "shutdown"}, flushes stdout, and exits with code 0.
- [ ] Background MemoryReport thread runs every 10 seconds in mock mode (verifiable by checking stdout output during extended run).
- [ ] `worker/tests/test_worker_main.py` exists and all tests pass with `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py -v`.
- [ ] Full CI gate passes: `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` exits 0.
