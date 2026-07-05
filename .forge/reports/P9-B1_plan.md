# Plan Report: P9-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P9-B1                                       |
| Phase       | 9 — Real Worker Startup                     |
| Description | worker/ipc.py: ZeroMQ DEALER transport + msgpack framing |
| Depends on  | P9-A3                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-07-05T12:55:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `worker/ipc.py` — the Python-side ZeroMQ DEALER transport module that every AnvilML worker subprocess uses to connect to the Rust supervisor's ROUTER socket — and `worker/tests/test_ipc.py` with ≥5 tests proving correct identity setup, pre-connect error handling, and a full send/recv round-trip against a live ROUTER socket.

## Scope

### In Scope
- Create `worker/ipc.py` with module-level `_ctx`/`_sock` globals and three functions: `connect(port, worker_id)`, `send_event(data)`, `recv_message()` — signatures and docstrings copied verbatim from `ANVILML_DESIGN.md §14.4`.
- Create `worker/tests/test_ipc.py` with ≥5 tests covering: identity verification, pre-connect RuntimeError for `send_event`, pre-connect RuntimeError for `recv_message`, a real send/recv round-trip against a test ROUTER socket in the same process, and a test verifying the module does not import `torch` at top level (subprocess-isolated).
- Create `worker/tests/conftest.py` (empty, shared fixtures only, as required by §11.2 convention).

### Out of Scope
None. `defers_to (from JSON): []` — this task implements its full scope. No deferred functionality.

## Existing Codebase Assessment

No prior source exists for `worker/ipc.py` or `worker/tests/`. The `worker/` directory currently contains only `pyproject.toml` (with the `real_mode` pytest marker registered by P9-A3) and `requirements/base.txt` (which already pins `pyzmq==27.1.0` and `msgpack==1.2.1`). The `worker/tests/` directory does not yet exist. This task establishes the baseline patterns for Python IPC testing in this project.

The established patterns from the broader codebase to follow:
- Google-style docstrings with `Args:`/`Returns:`/`Raises:` sections for all non-trivial functions.
- Test files live in `worker/tests/`, one per source module.
- `conftest.py` contains only shared fixtures, never test functions.
- Tests must use the venv interpreter directly (`worker/.venv/bin/python`), never bare `python`.
- Subprocess isolation uses `subprocess.run()` with `timeout=` and stderr capture (per §11.3 of ENVIRONMENT.md).

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| python | pyzmq   | 27.1.0          | pypi-query MCP | n/a                    |
| python | msgpack | 1.2.1           | pypi-query MCP | n/a                    |

Both are already pinned in `worker/requirements/base.txt`. `pyzmq` 27.1.0 is compatible with Python 3.12 (requires `>=3.8`). All APIs used — `zmq.Context.instance()`, `ctx.socket(zmq.DEALER)`, `_sock.setsockopt(zmq.IDENTITY, ...)`, `_sock.connect()`, `_sock.send()`, `_sock.recv()`, `msgpack.packb(data, use_bin_type=True)`, `msgpack.unpackb(data, raw=False)` — are standard pyzmq/msgpack APIs confirmed present in version 27.1.0 / 1.2.1.

## Approach

1. **Create `worker/tests/conftest.py`** — empty file (shared fixtures only, no test functions). This satisfies the ENVIRONMENT.md §11.2 convention that `conftest.py` exists for the test directory.

2. **Create `worker/ipc.py`** — implement the module exactly as specified in `ANVILML_DESIGN.md §14.4`:
   - Module docstring: copy verbatim from §14.4.
   - Imports: `import zmq`, `import msgpack` (in that order, matching §14.4).
   - Globals: `_ctx: zmq.Context | None = None`, `_sock: zmq.Socket | None = None`.
   - `connect(port: int, worker_id: str) -> None`:
     - Uses `global _ctx, _sock` statement.
     - Creates context via `zmq.Context.instance()` (process-wide singleton, not `zmq.Context()` which creates a new context each time).
     - Creates DEALER socket: `_sock = _ctx.socket(zmq.DEALER)`.
     - Sets identity: `_sock.setsockopt(zmq.IDENTITY, worker_id.encode())` — identity is set **before** connect, as required by the ZeroMQ ROUTER socket topology.
     - Connects: `_sock.connect(f"tcp://127.0.0.1:{port}")`.
   - `send_event(data: dict) -> None`:
     - Checks `if _sock is None: raise RuntimeError("ipc: not connected — call connect() first")`.
     - Serialises with `msgpack.packb(data, use_bin_type=True)` and sends via `_sock.send()`.
   - `recv_message() -> dict`:
     - Same pre-connect check as `send_event`.
     - Receives raw bytes via `_sock.recv()` (blocking call).
     - Deserialises with `msgpack.unpackb(data, raw=False)` — `raw=False` returns Python dicts rather than byte strings, matching the Rust side's msgpack framing.

3. **Create `worker/tests/test_ipc.py`** — implement ≥5 tests:
   - **test_connect_sets_identity**: Start a ROUTER socket on a random port, call `connect(port, "test-worker")` on a DEALER socket (same approach the real worker uses), then verify the ROUTER receives a message from the correct identity. The ROUTER socket's first `recv()` returns the identity frame.
   - **test_send_event_before_connect_raises**: Call `send_event({"_type": "Ping"})` without calling `connect()` first, assert `RuntimeError` is raised.
   - **test_recv_message_before_connect_raises**: Call `recv_message()` without calling `connect()` first, assert `RuntimeError` is raised.
   - **test_roundtrip_send_recv**: Set up a ROUTER socket in the test process, connect a DEALER via `ipc.connect()`, send a dict via `ipc.send_event()`, receive it from the ROUTER side via `router.recv()` (identity frame) + `router.recv()` (payload), unpack with `msgpack.unpackb(raw=False)`, and assert the dict matches the sent payload. This is the real integration test that proves the full send/recv path works end-to-end within a single process.
   - **test_module_no_torch_import**: Use `subprocess.run()` to spawn a fresh Python process that imports `worker.ipc` and asserts `"torch" not in sys.modules`. This confirms the module has no transitive torch dependency at import time (required by the mock-mode CI jobs that install only `base.txt` without torch).
   - **test_connect_twice_reuses_context**: Call `connect()` twice with different worker IDs; verify the second call reuses the existing context but creates a new socket with the new identity. This tests the singleton pattern works correctly.

4. **Verify correctness**: Run `python -m pytest worker/tests/test_ipc.py -v` and confirm all tests pass and exit 0.

## Public API Surface

| Module | Item | Signature |
|--------|------|-----------|
| `worker/ipc.py` | `connect` | `def connect(port: int, worker_id: str) -> None` |
| `worker/ipc.py` | `send_event` | `def send_event(data: dict) -> None` |
| `worker/ipc.py` | `recv_message` | `def recv_message() -> dict` |

Private module-level globals: `_ctx: zmq.Context | None`, `_sock: zmq.Socket | None`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/ipc.py` | ZeroMQ DEALER transport module with connect/send_event/recv_message |
| CREATE | `worker/tests/conftest.py` | Empty shared fixtures file (no test functions) |
| CREATE | `worker/tests/test_ipc.py` | ≥5 tests for ipc.py |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_ipc.py` | `test_connect_sets_identity` | DEALER socket connects with correct ZEROMQ identity | ROUTER socket bound on random port | port, worker_id="test-worker" | ROUTER receives identity frame equal to b"test-worker" | `worker/.venv/bin/python -m pytest worker/tests/test_ipc.py::test_connect_sets_identity -v` exits 0 |
| `worker/tests/test_ipc.py` | `test_send_event_before_connect_raises` | send_event raises RuntimeError when not connected | None (globals are None) | `{"_type": "Ping"}` | RuntimeError with message "ipc: not connected — call connect() first" | `worker/.venv/bin/python -m pytest worker/tests/test_ipc.py::test_send_event_before_connect_raises -v` exits 0 |
| `worker/tests/test_ipc.py` | `test_recv_message_before_connect_raises` | recv_message raises RuntimeError when not connected | None (globals are None) | (none) | RuntimeError with message "ipc: not connected — call connect() first" | `worker/.venv/bin/python -m pytest worker/tests/test_ipc.py::test_recv_message_before_connect_raises -v` exits 0 |
| `worker/tests/test_ipc.py` | `test_roundtrip_send_recv` | Full msgpack round-trip via ROUTER/DEALER pair | ROUTER socket bound, DEALER connected | dict `{"_type": "Ping"}` | ROUTER receives identical dict after stripping identity frame | `worker/.venv/bin/python -m pytest worker/tests/test_ipc.py::test_roundtrip_send_recv -v` exits 0 |
| `worker/tests/test_ipc.py` | `test_module_no_torch_import` | Module does not transitively import torch | Fresh subprocess | imports `worker.ipc` | `"torch" not in sys.modules` succeeds | `worker/.venv/bin/python -m pytest worker/tests/test_ipc.py::test_module_no_torch_import -v` exits 0 |
| `worker/tests/test_ipc.py` | `test_connect_twice_reuses_context` | Second connect() call reuses zmq.Context singleton | First connect() succeeded | Different worker_id on second call | New DEALER socket with updated identity | `worker/.venv/bin/python -m pytest worker/tests/test_ipc.py::test_connect_twice_reuses_context -v` exits 0 |

## CI Impact

No CI changes required. This task only adds Python source and test files under `worker/`, which are already picked up by the existing CI worker jobs (`worker-linux-mock`, `worker-linux-real`, `worker-windows-mock`, `worker-windows-real`) as defined in `.github/workflows/ci.yml`. The `python -m py_compile` step (ENVIRONMENT.md §6 Step 7) will validate syntax, and pytest will collect the new tests.

## Platform Considerations

None identified. ZeroMQ TCP loopback (`tcp://127.0.0.1`) and the pyzmq API are cross-platform — identical on Linux and Windows. The `worker_id.encode()` call produces UTF-8 bytes on all platforms, which is the correct ZeroMQ identity format. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `zmq.Context.instance()` returns a process-wide singleton — if a prior test already created a context, subsequent tests may see stale sockets. | Medium | High | Each test that uses `connect()` must also clean up: close `_sock`, set `_sock = None`, and if the context was created by this test, call `_term_ctx()` to destroy it. Tests must be isolated — never share state across test functions. |
| The ROUTER socket in `test_roundtrip_send_recv` may hang if the DEALER sends before the ROUTER is ready to receive. | Low | Medium | Bind the ROUTER first, then call `ipc.connect()` on the DEALER — the ROUTER's `recv()` call after `connect()` ensures the DEALER is connected before the ROUTER attempts to receive. ZeroMQ's connection handshake guarantees ordering. |
| `msgpack.unpackb(data, raw=False)` may behave differently on older msgpack versions. | Low | Low | The `raw=False` parameter is confirmed present in msgpack 1.2.1 (the version in `base.txt`). The MCP lookup confirmed this API exists. No version downgrade needed. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/ipc.py` exits 0
- [ ] `worker/.venv/bin/python -m pytest worker/tests/test_ipc.py -v --tb=short` exits 0 with ≥6 tests collected
- [ ] `worker/.venv/bin/python -c "import subprocess, sys; r = subprocess.run([sys.executable, '-c', 'import worker.ipc; import sys; assert \"torch\" not in sys.modules'], timeout=10); r.check_returncode()"` exits 0
- [ ] `grep -c "^def " worker/ipc.py` outputs exactly 3 (connect, send_event, recv_message)
- [ ] `head -1 worker/ipc.py` outputs `"""ZeroMQ DEALER transport for AnvilML worker IPC.` (module docstring match)
