# Plan Report: P9-B1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P9-B1                                             |
| Phase       | 009 — Worker Spawn & Handshake                    |
| Description | worker/worker_main.py mock mode startup, Ready event, Ping/Pong loop |
| Depends on  | none (Phase 008: RouterTransport verified)        |
| Project     | anvilml                                           |
| Planned at  | 2026-06-16T22:55:00Z                              |
| Attempt     | 1                                                 |

## Objective

Implement `worker/worker_main.py` — the Python worker entry point spawned by the Rust supervisor — in mock mode. When `ANVILML_WORKER_MOCK=1` is set, the worker reads its identity from environment variables (`ANVILML_IPC_PORT`, `ANVILML_WORKER_ID`, `ANVILML_DEVICE_INDEX`, `ANVILML_DEVICE_TYPE`), connects to the Rust ROUTER socket via `ipc.connect()`, sends a `Ready` event with synthetic hardware capability values (no torch import), and enters a message dispatch loop that responds to `Ping` with `Pong` and exits cleanly on `Shutdown`. This is the foundational Python-side lifecycle that the Rust `ManagedWorker` state machine waits for before transitioning a worker to `Idle`.

## Scope

### In Scope
- `worker/worker_main.py`: mock-mode startup, Ready event emission, Ping/Pong heartbeat loop, Shutdown exit
- `worker/__init__.py`: ensure empty package init file exists (already exists)
- `worker/tests/test_worker_main.py`: ≥ 4 tests covering mock startup, Ready event, Ping→Pong, Shutdown exit

### Out of Scope
- Real hardware probing (`_probe_hardware()` with torch) — deferred to non-mock mode
- Node importing (`_import_nodes()`) — deferred to non-mock mode
- Job execution (`Execute` message handling) — deferred to non-mock mode
- Error recovery beyond clean exit on Shutdown
- Rust-side supervisor integration — covered by P9-A5 (managed.rs)

## Existing Codebase Assessment

The worker Python module already has two key files that establish the patterns this task follows:

1. **`worker/ipc.py`** — The ZeroMQ DEALER transport module with `connect(port, worker_id)`, `send_event(data)`, and `recv_message()` functions. These use module-level globals (`_ctx`, `_sock`) initialised once and guarded by `None` checks. The module docstring follows Google style and the code uses `from __future__ import annotations`.

2. **`worker/ipc_echo.py`** — A minimal echo worker that demonstrates the exact startup pattern needed: `connect()` → `send_event(Ready{...})` → `while True: recv_message()` → dispatch on `_type` discriminator. The Ready payload in `ipc_echo.py` (lines 46-61) is the exact template for the mock Ready event, including all required fields (`worker_id`, `device_index`, `device_name`, `device_type`, `vram_total_mib`, `vram_free_mib`, `torch_version`, `fp16`, `bf16`, `fp8`, `flash_attention`, `node_types`).

3. **`worker/tests/conftest.py`** — Contains an `autouse=True` `mock_mode` fixture that sets `ANVILML_WORKER_MOCK=1` for every test and restores the original value in a `finally` block. This is the established pattern for env var isolation (ENVIRONMENT.md §11.3).

4. **`worker/tests/test_ipc.py`** — Demonstrates test style: uses `_reset_ipc_state()` helper to clean up module globals between tests, creates ROUTER sockets for integration testing, and follows Google-style docstrings with `Preconditions:` and `Expects:` sections.

The design doc (§13.1) specifies the startup sequence and §13.3 defines mock mode behaviour. The task context specifies the exact Ready event fields. No gap exists between the design doc and current source for this task's scope.

## Resolved Dependencies

This task introduces no new external dependencies. All imports are from the existing `worker/ipc.py` module (pyzmq, msgpack) and the Python standard library. The existing `base.txt` already declares these packages.

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| python | pyzmq   | 27.1.0          | pypi-query MCP | n/a                    |
| python | msgpack | 1.2.0           | pypi-query MCP | n/a                    |

Note: `base.txt` declares `pyzmq>=26.0` and `msgpack>=1.0`. The MCP-verified current versions (27.1.0 and 1.2.0) are within range and API-compatible. No version override needed.

## Approach

1. **Create `worker/worker_main.py`** with the following structure:

   a. **Module docstring** — Google-style, describing the worker entry point, mock mode behaviour, and that it is spawned by the Rust supervisor.

   b. **`main()` function** — The entry point that:
      - Reads env vars `ANVILML_IPC_PORT`, `ANVILML_WORKER_ID`, `ANVILML_DEVICE_INDEX`, `ANVILML_DEVICE_TYPE` via `os.environ.get()` with defaults matching ENVIRONMENT.md §3.4
      - Checks `ANVILML_WORKER_MOCK`; if unset or `"0"`, logs a warning and exits 1 (non-mock mode not implemented yet)
      - In mock mode: calls `ipc.connect(port, worker_id)`
      - Builds and sends the `Ready` event dict with all fields from the task spec:
        ```python
        {
            "_type": "Ready",
            "worker_id": worker_id,
            "device_index": device_index,
            "device_name": "Mock",
            "device_type": device_type,
            "vram_total_mib": 8192,
            "vram_free_mib": 8192,
            "torch_version": "mock",
            "fp16": True,
            "bf16": True,
            "fp8": True,
            "flash_attention": True,
            "node_types": [],
        }
        ```
      - Enters the dispatch loop: `while True:` → `msg = recv_message()` → dispatch on `_type`:
        - `"Ping"`: send `Pong` with same `seq` field
        - `"Shutdown"`: `sys.exit(0)`
        - Any other `_type`: log a warning and continue (future-proofing)

   c. **`if __name__ == "__main__":`** guard calling `main()`.

   **Rationale for non-obvious choices:**
   - The mock Ready event uses `device_name: "Mock"` (not `"CPU"` or similar) because the task spec explicitly names it. This matches the mock mode convention where synthetic values are clearly distinguishable from real hardware.
   - The dispatch loop uses `sys.exit(0)` for Shutdown rather than `break` to ensure the process exits with code 0, matching the task spec's `"Shutdown → exit 0"` contract.
   - Mock mode rejects non-mock invocation (exits 1) because importing torch in test CI would fail. This is a safety gate.

2. **Verify `worker/__init__.py`** exists and is empty — it already does (confirmed by reading the file). No changes needed.

3. **Create `worker/tests/test_worker_main.py`** with ≥ 4 tests:

   a. **`test_mock_startup_sends_ready`** — Spawns `worker_main.py` as a subprocess with `ANVILML_WORKER_MOCK=1`, reads the Ready event from the ROUTER socket, asserts all expected fields are present and have correct values (worker_id, device_index, device_name="Mock", device_type, vram values, torch_version="mock", fp16/bf16/fp8/flash_attention all True, node_types empty).

   b. **`test_ping_returns_pong`** — Starts the worker in mock mode, sends a `Ping{seq: 42}` message, receives the `Pong` response, asserts `_type == "Pong"` and `seq == 42`.

   c. **`test_shutdown_exits_cleanly`** — Starts the worker, sends a `Shutdown` message, asserts the subprocess exits with code 0 within a timeout.

   d. **`test_env_vars_read_from_environment`** — Sets `ANVILML_WORKER_ID`, `ANVILML_DEVICE_INDEX`, `ANVILML_DEVICE_TYPE` to custom values before launching the worker, then verifies the Ready event contains those values in the corresponding fields.

   **Test isolation pattern:** Each test creates its own ROUTER socket on a random port (like `test_ipc.py`), spawns the worker subprocess pointing to that port, and cleans up both the socket and the subprocess in `finally` blocks. The `mock_mode` fixture from conftest.py ensures `ANVILML_WORKER_MOCK=1` is set for the parent process, but each test must also set it for the child subprocess explicitly since `os.environ` is not inherited through `subprocess.run` unless `env` is passed.

## Public API Surface

This task creates a new Python module entry point. No Rust `pub` items are involved.

| Path | Item | Type | Description |
|------|------|------|-------------|
| `worker/worker_main.py` | `main()` | `def main() -> None` | Entry point: reads env vars, connects IPC, sends Ready, enters dispatch loop |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/worker_main.py` | Mock mode worker entry point with startup, Ready event, Ping/Pong loop |
| (no-op) | `worker/__init__.py` | Already exists and is empty; verified, no change needed |
| CREATE | `worker/tests/test_worker_main.py` | ≥ 4 tests for mock startup, Ready, Ping/Pong, Shutdown |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_worker_main.py` | `test_mock_startup_sends_ready` | Worker spawns in mock mode, connects IPC, and sends a valid Ready event with all required fields | A ROUTER socket is bound on a random port; `ANVILML_WORKER_MOCK=1` | Subprocess with env vars `ANVILML_IPC_PORT`, `ANVILML_WORKER_ID`, `ANVILML_DEVICE_INDEX=0`, `ANVILML_DEVICE_TYPE=cpu` | Ready event received with `_type="Ready"`, all fields match spec | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::test_mock_startup_sends_ready -v` exits 0 |
| `worker/tests/test_worker_main.py` | `test_ping_returns_pong` | Worker responds to Ping messages with Pong containing the same sequence number | Worker is running (mock mode); ROUTER connected to worker's DEALER | `Ping{seq: 42}` sent via ROUTER | `Pong{seq: 42}` received, `_type == "Pong"`, `seq == 42` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::test_ping_returns_pong -v` exits 0 |
| `worker/tests/test_worker_main.py` | `test_shutdown_exits_cleanly` | Worker exits with code 0 when it receives a Shutdown message | Worker is running (mock mode); ROUTER connected | `Shutdown` sent via ROUTER | Subprocess exit code == 0 within timeout | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::test_shutdown_exits_cleanly -v` exits 0 |
| `worker/tests/test_worker_main.py` | `test_env_vars_read_from_environment` | Worker reads `ANVILML_WORKER_ID`, `ANVILML_DEVICE_INDEX`, `ANVILML_DEVICE_TYPE` from environment and includes them in the Ready event | ROUTER bound; custom env vars set | `ANVILML_WORKER_ID=custom-worker`, `ANVILML_DEVICE_INDEX=3`, `ANVILML_DEVICE_TYPE=cuda` | Ready event `worker_id == "custom-worker"`, `device_index == 3`, `device_type == "cuda"` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::test_env_vars_read_from_environment -v` exits 0 |

## CI Impact

The `worker-linux` and `worker-windows` CI jobs run `pytest worker/tests/` which will automatically pick up the new `test_worker_main.py` file. No CI workflow changes are needed — the pytest discovery pattern `worker/tests/` already includes all test files in the directory. The Rust CI jobs (`rust-linux`, `rust-windows`) are unaffected since this task writes only Python files.

## Platform Considerations

None identified. The worker_main.py module uses only Python standard library (`os`, `sys`, `subprocess` in tests) and existing cross-platform dependencies (pyzmq, msgpack). ZeroMQ TCP connections work identically on Linux and Windows. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| ZeroMQ ROUTER/DEALER connection timing — the worker's DEALER socket may connect before the test's ROUTER socket is ready, causing the first send/receive to hang or fail. | Medium | High | Add a brief `time.sleep(0.1)` after spawning the worker subprocess and before sending messages, matching the pattern already used in `test_ipc.py::test_recv_message_deserialises_correctly`. Also use `router.recv()` with a short timeout where appropriate. |
| Subprocess exit detection — on some platforms, `subprocess.Popen.wait()` may block indefinitely if the worker process is stuck (e.g., if `ipc.connect()` fails silently). | Medium | Medium | Use `subprocess.Popen.communicate(timeout=10)` with a 10-second timeout. If timeout fires, kill the process and assert the test failed rather than hanging the CI. This matches the established pattern in `backend/tests/cli_tests.rs` which uses similar subprocess handling. |
| `ANVILML_WORKER_MOCK` not inherited by child process — `os.environ` changes in the parent are not automatically inherited by `subprocess.Popen` unless the `env` parameter is explicitly passed. | Low | High | In each test, copy `os.environ` and set `ANVILML_WORKER_MOCK=1` in the child env dict before passing to `subprocess.Popen`. This is explicit and avoids relying on default inheritance. |
| Test ordering dependency — if tests run in parallel, the `mock_mode` fixture's env var mutation could race. | Low | Medium | The conftest `mock_mode` fixture already uses capture-and-restore in a `finally` block. Additionally, each test manages its own subprocess with explicit env, so there is no shared mutable state between tests. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v` exits 0 with ≥ 4 tests passing
- [ ] `ANVILML_WORKER_MOCK=1 python3 worker/worker_main.py` exits 0 when run with `ANVILML_IPC_PORT`, `ANVILML_WORKER_ID`, `ANVILML_DEVICE_INDEX`, `ANVILML_DEVICE_TYPE` set and a ROUTER socket bound on the specified port
- [ ] `worker/__init__.py` exists (verified, no change)
- [ ] All 4 tests in `test_worker_main.py` have Google-style docstrings with `Preconditions:` and `Expects:` sections
- [ ] No new external dependencies added — only imports from `worker.ipc` and standard library
