# Plan Report: P8-B1

| Field       | Value                        |
|-------------|------------------------------|
| Task ID     | P8-B1                        |
| Phase       | 008 — ZeroMQ IPC Transport   |
| Description | worker/ipc.py: ZeroMQ DEALER transport with identity |
| Depends on  | P8-A1, P8-A2, P8-A3          |
| Project     | anvilml                      |
| Planned at  | 2026-06-16T13:30:00Z         |
| Attempt     | 1                            |

## Objective

Create the Python worker ZeroMQ DEALER transport module (`worker/ipc.py`) and its dependency file (`worker/requirements/base.txt`). This module provides the network communication layer between the Python worker subprocess and the Rust supervisor's ROUTER socket. When complete, `python3 -c "from worker import ipc"` exits 0, and the test suite in `worker/tests/test_ipc.py` passes with ≥ 6 tests using in-process ZeroMQ socket pairs.

## Scope

### In Scope
- Create `worker/ipc.py` with module-level globals `_ctx` and `_sock`, and three functions:
  - `connect(port: int, worker_id: str) -> None` — creates a DEALER socket, sets identity, connects to ROUTER
  - `send_event(data: dict) -> None` — serialises and sends a msgpack-encoded WorkerEvent dict
  - `recv_message() -> dict` — receives and deserialises a msgpack-encoded WorkerMessage dict
- Create `worker/requirements/base.txt` with five pinned dependency versions
- Create `worker/tests/test_ipc.py` with ≥ 6 tests using in-process ZeroMQ socket pairs
- Create `worker/__init__.py` (empty, so `from worker import ipc` works as a package import)
- Create `worker/requirements/` directory

### Out of Scope
- Worker startup sequence (`worker_main.py`) — handled in P8-B2 and later tasks
- Hardware probing (`_probe_hardware()`) — handled in P8-B2
- Node registry and node execution (`executor.py`, `nodes/`) — handled in later phases
- Rust-side IPC bridge and transport — handled in P8-A2, P8-A3
- Stress test — handled in P8-C1

## Existing Codebase Assessment

No prior Python source code exists in the `worker/` directory. The `worker/` directory contains only a `tests/` subdirectory with a placeholder test file (`test_placeholder.py`) and a `pytest.ini`. There is no `worker/__init__.py`, no `ipc.py`, and no `requirements/` directory.

The Rust side of the IPC stack (Phase 008 Group A) is being built in parallel tasks (P8-A1 through P8-A3). The Python worker must conform to the protocol specified in `ANVILML_DESIGN.md §8.4–8.6` (msgpack flat dicts with `_type` discriminator) and `§13.2` (the `ipc.py` interface).

Established patterns from the Rust side that this task must mirror:
- msgpack flat dict serialisation with `_type` key as enum variant discriminator
- DEALER socket identity set to `worker_id` (UTF-8 encoded bytes)
- TCP transport on `127.0.0.1:{port}` where port is supplied via `ANVILML_IPC_PORT` env var
- `RuntimeError` raised if `connect()` has not been called before send/recv

No discrepancies between the design doc and current source — the design doc section 13.2 provides a complete reference implementation, and the current codebase has no conflicting Python code.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source     | Feature flags confirmed |
|--------|-------------|-----------------|----------------|------------------------|
| python | pyzmq       | 27.1.0          | pypi-query MCP | n/a                    |
| python | msgpack     | 1.2.0           | pypi-query MCP | n/a                    |
| python | pillow      | 12.2.0          | pypi-query MCP | n/a                    |
| python | safetensors | 0.8.0           | pypi-query MCP | n/a                    |
| python | pytest      | 9.1.0           | pypi-query MCP | n/a                    |

All five packages are compatible with Python 3.12 (the project's required version per ENVIRONMENT.md §1). pyzmq 27.1.0 supports Python >=3.8, msgpack 1.2.0 supports >=3.10, pillow 12.2.0 supports >=3.10, safetensors 0.8.0 supports >=3.10, and pytest 9.1.0 supports >=3.10.

API shape verified via MCP:
- `zmq.Context.instance()` — exists in pyzmq 27.1.0
- `zmq.DEALER` — exists as socket type constant
- `zmq.IDENTITY` — exists as socket option constant
- `socket.setsockopt(zmq.IDENTITY, bytes)` — standard API
- `socket.connect(str)` — standard API
- `socket.send(bytes)` — standard API
- `socket.recv()` — standard API
- `msgpack.packb(dict, use_bin_type=True)` — exists in msgpack 1.2.0
- `msgpack.unpackb(bytes, raw=False)` — exists in msgpack 1.2.0

## Approach

1. **Create `worker/requirements/base.txt`** with the five dependency lines:
   ```
   pyzmq>=26.0
   msgpack>=1.0
   pillow>=10.0
   safetensors>=0.4
   pytest>=8.0
   ```
   Rationale: These are the minimum versions needed for the worker. pyzmq>=26.0 ensures DEALER/ROUTER support; msgpack>=1.0 provides `use_bin_type` and `raw` parameters; pillow>=10.0 for image handling; safetensors>=0.4 for model loading; pytest>=8.0 for test execution.

2. **Create `worker/__init__.py`** as an empty file. This makes `worker` a Python package so that `from worker import ipc` works as the acceptance criterion specifies.

3. **Create `worker/ipc.py`** with the following structure:
   - Module docstring describing the DEALER transport role (per ENVIRONMENT.md §10 Python docstring convention)
   - Import `zmq` and `msgpack` at module level
   - Module globals `_ctx: zmq.Context | None = None` and `_sock: zmq.Socket | None = None`
   - Implement `connect(port: int, worker_id: str) -> None`:
     - Set `global _ctx, _sock`
     - Create context via `zmq.Context.instance()`
     - Create DEALER socket: `_ctx.socket(zmq.DEALER)`
     - Set identity: `_sock.setsockopt(zmq.IDENTITY, worker_id.encode())` — identity MUST be set before connect; this is a ZeroMQ constraint
     - Connect: `_sock.connect(f"tcp://127.0.0.1:{port}")`
     - Google-style docstring with Args section
   - Implement `send_event(data: dict) -> None`:
     - Guard: check `_sock is None`, raise `RuntimeError`
     - Send: `_sock.send(msgpack.packb(data, use_bin_type=True))`
     - Google-style docstring with Args and Raises sections
   - Implement `recv_message() -> dict`:
     - Guard: check `_sock is None`, raise `RuntimeError`
     - Receive: `data = _sock.recv()`
     - Return: `msgpack.unpackb(data, raw=False)`
     - Google-style docstring with Returns and Raises sections
   - Inline comment on the identity-before-connect constraint (ZeroMQ requires identity to be set on the socket before binding/connecting)
   - Inline comment on the guard checks (prevents silent failures if worker lifecycle is broken)

4. **Create `worker/tests/test_ipc.py`** with ≥ 6 tests using in-process ZeroMQ socket pairs:
   - Test 1: `test_connect_succeeds` — creates a ROUTER socket bound on a random port, calls `ipc.connect(port, "test-worker")`, asserts `_sock` is not None
   - Test 2: `test_send_event_roundtrip` — binds a ROUTER socket in the test, connects via `ipc.connect()`, sends a msgpack dict from the test's ROUTER side, calls `ipc.recv_message()`, asserts the dict matches
   - Test 3: `test_send_event_type_discriminator` — sends a dict with `_type: "Ready"`, verifies `recv_message()` returns `_type == "Ready"`
   - Test 4: `test_recv_message_before_connect_raises` — calls `recv_message()` without `connect()`, asserts `RuntimeError` raised
   - Test 5: `test_send_event_before_connect_raises` — calls `send_event({})` without `connect()`, asserts `RuntimeError` raised
   - Test 6: `test_identity_attached` — binds a ROUTER socket, connects via `ipc.connect("test-identity")`, sends a message, reads the multipart frame to verify the identity frame matches `"test-identity"` encoded bytes
   - Each test uses `zmq.Context()` and `zmq.socket()` for its own socket pair (test isolation — no shared state between tests)
   - Google-style docstrings on each test function per ENVIRONMENT.md §11.4

5. **Verify**: Run `python3 -c "from worker import ipc"` and confirm exit 0. Run `python3 -c "import zmq; print(zmq.__version__)"` to confirm pyzmq is importable.

## Public API Surface

| Item | Module Path | Signature |
|------|-------------|-----------|
| Module | `worker.ipc` | `"""ZeroMQ DEALER transport for AnvilML worker IPC."""` |
| Function | `worker.ipc.connect` | `def connect(port: int, worker_id: str) -> None` |
| Function | `worker.ipc.send_event` | `def send_event(data: dict) -> None` |
| Function | `worker.ipc.recv_message` | `def recv_message() -> dict` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/__init__.py` | Empty file to make `worker` a Python package |
| CREATE | `worker/ipc.py` | ZeroMQ DEALER transport module with connect, send_event, recv_message |
| CREATE | `worker/requirements/base.txt` | Python dependency file for worker runtime |
| CREATE | `worker/tests/test_ipc.py` | ≥ 6 tests for ipc.py using in-process ZeroMQ socket pairs |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_ipc.py` | `test_connect_succeeds` | `connect()` creates a valid DEALER socket and sets it on `_sock` | ROUTER socket bound on random port | port, worker_id="test-worker" | `_sock` is not None, socket state is connected | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_ipc.py::test_connect_succeeds -v` exits 0 |
| `worker/tests/test_ipc.py` | `test_send_event_roundtrip` | `send_event()` + `recv_message()` roundtrip preserves msgpack dict content | ROUTER socket bound, DEALER connected | msgpack dict `{"_type": "Ready", "node_types": []}` | `recv_message()` returns identical dict | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_ipc.py::test_send_event_roundtrip -v` exits 0 |
| `worker/tests/test_ipc.py` | `test_send_event_type_discriminator` | `_type` key survives msgpack roundtrip correctly | ROUTER socket bound, DEALER connected | dict with `_type: "Ready"` | `recv_message()["_type"] == "Ready"` | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_ipc.py::test_send_event_type_discriminator -v` exits 0 |
| `worker/tests/test_ipc.py` | `test_recv_message_before_connect_raises` | `recv_message()` raises RuntimeError when not connected | No `connect()` called | (none) | `RuntimeError` raised | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_ipc.py::test_recv_message_before_connect_raises -v` exits 0 |
| `worker/tests/test_ipc.py` | `test_send_event_before_connect_raises` | `send_event()` raises RuntimeError when not connected | No `connect()` called | (none) | `RuntimeError` raised | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_ipc.py::test_send_event_before_connect_raises -v` exits 0 |
| `worker/tests/test_ipc.py` | `test_identity_attached` | DEALER socket identity frame is set correctly and visible on ROUTER side | ROUTER socket bound, DEALER connected with id "test-identity" | worker_id="test-identity" | ROUTER receives multipart frame where identity frame equals `b"test-identity"` | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_ipc.py::test_identity_attached -v` exits 0 |

## CI Impact

No CI changes required. The `worker-linux` and `worker-windows` CI jobs already run `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` which will pick up the new `test_ipc.py` file automatically. No new CI gates or job configurations are needed.

## Platform Considerations

None identified. The ZeroMQ DEALER socket with TCP transport (`tcp://127.0.0.1:{port}`) and msgpack serialisation are platform-neutral. The `zmq.IDENTITY` socket option and `setsockopt` API work identically on Linux, macOS, and Windows. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| pyzmq 27.1.0 API shape differs from the classic `zmq.Context.instance()` / `zmq.DEALER` pattern used in the design doc | Low | High | MCP lookup confirmed all API names exist in pyzmq 27.1.0. The ACT agent should verify at session start by running `python3 -c "import zmq; zmq.Context.instance().socket(zmq.DEALER)"` before writing code. |
| In-process ZeroMQ socket pairs (ROUTER + DEALER) in tests may hang if the ROUTER socket does not receive any message before the DEALER sends — ZeroMQ's lazy pirate pattern can cause the first send to be lost | Medium | Medium | Use `zmq.Poller` with a short timeout on the ROUTER side in tests, or use `socket.send_multipart()` / `socket.recv_multipart()` with explicit multipart framing. The test must handle the ROUTER's identity frame correctly (ROUTER prepends the identity frame on recv). |
| `worker/__init__.py` is empty but required for `from worker import ipc` to work as a package import — if a future task creates `worker/` as a namespace package or uses `__path__`, this could conflict | Low | Low | Keep `__init__.py` empty. If namespace packages are needed later, remove this file and add `__init__.py` to all subdirectories. This is unlikely given the project structure. |
| msgpack `use_bin_type=True` and `raw=False` parameters may not exist in older msgpack versions pinned by `>=1.0` constraint | Low | Low | MCP confirmed msgpack 1.2.0 has both parameters. The `>=1.0` constraint is safe because `use_bin_type` was added in msgpack 0.5.0 and `raw` has existed since early versions. |

## Acceptance Criteria

- [ ] `python3 -c "from worker import ipc"` exits 0
- [ ] `python3 -c "from worker.ipc import connect, send_event, recv_message"` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_ipc.py -v` exits 0 with ≥ 6 tests
- [ ] `cat worker/requirements/base.txt` contains all five dependency lines (pyzmq, msgpack, pillow, safetensors, pytest)
- [ ] `wc -l worker/ipc.py` returns a line count between 40 and 120 (module should be concise)
