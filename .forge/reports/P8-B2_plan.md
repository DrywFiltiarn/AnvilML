# Plan Report: P8-B2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P8-B2                                       |
| Phase       | 008 — ZeroMQ IPC Transport                  |
| Description | worker/tests/test_ipc.py: ipc.py unit tests  |
| Depends on  | P8-B1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-16T14:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `worker/tests/test_ipc.py` (overwriting existing content) and ensure `worker/tests/__init__.py` exists as a package marker. The test file must verify `worker/ipc.py` functions — `connect()`, `send_event()`, and `recv_message()` — using in-process `zmq.PAIR` sockets for the roundtrip test. All tests must set `ANVILML_WORKER_MOCK=1` and restore env vars unconditionally. The acceptance criterion is that `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_ipc.py -v` exits 0 with ≥ 6 tests.

## Scope

### In Scope
- Overwrite `worker/tests/test_ipc.py` with new tests matching the task specification
- Ensure `worker/tests/__init__.py` exists as a Python package marker (already present, no change needed)
- Create `worker/tests/conftest.py` with an autouse fixture for `ANVILML_WORKER_MOCK=1` env var management

### Out of Scope
- Testing other worker modules (`executor.py`, `pipeline_cache.py`, `nodes/`)
- Integration tests between Rust and Python (covered by P8-C1)
- Rust-side `anvilml-ipc` tests (covered by P8-A2, P8-A3)
- Modifying `worker/ipc.py` source code

## Existing Codebase Assessment

`worker/ipc.py` exists with three public functions: `connect(port, worker_id)`, `send_event(data: dict)`, and `recv_message() -> dict`. The module uses module-level globals `_ctx` and `_sock` (both `zmq.Context | None` / `zmq.Socket | None`), initialised by `connect()` and used by the other two functions. Messages are msgpack-serialised flat dicts with a `_type` discriminator key.

`worker/tests/test_ipc.py` already exists with 7 tests, but they use `zmq.ROUTER` sockets (not `zmq.PAIR` as required by the task spec) and do not set `ANVILML_WORKER_MOCK=1` or perform unconditional env var restoration. `worker/tests/__init__.py` exists but is empty. No `conftest.py` exists in the tests directory.

The established test pattern in this project uses per-test isolation with `finally` blocks for cleanup, docstring-based test documentation, and `_reset_*` helper functions for module-level state. The existing tests follow this style well, so the new tests should mirror it.

## Resolved Dependencies

| Type   | Name      | Version verified | MCP source     | Feature flags confirmed |
|--------|-----------|-----------------|----------------|------------------------|
| python | pyzmq     | 27.1.0          | pypi-query MCP | n/a                    |
| python | msgpack   | 1.2.0           | pypi-query MCP | n/a                    |
| python | pytest    | 8.x             | pypi-query MCP | n/a                    |

pyzmq 27.1.0 is compatible with Python 3.12 (requires_python >= 3.8). msgpack 1.2.0 is compatible with Python 3.12 (requires_python >= 3.10). Both `zmq.PAIR` and `zmq.Context.instance()` are stable APIs in pyzmq 27.x.

## Approach

1. **Create `worker/tests/conftest.py`** with a single autouse fixture `mock_mode` that:
   - Captures the pre-existing value of `ANVILML_WORKER_MOCK` (or `None` if absent)
   - Sets `os.environ["ANVILML_WORKER_MOCK"] = "1"` before each test
   - Restores the original value (or removes the var if it was absent) unconditionally after each test, in a `finally` block
   - Rationale: A shared fixture eliminates duplication across all test functions and ensures env var handling is consistent and unconditional.

2. **Overwrite `worker/tests/test_ipc.py`** with new tests:
   - Each test imports `msgpack`, `zmq`, and `from worker import ipc`
   - Each test uses `_reset_ipc_state()` to clean up module-level globals before running
   - Tests use `zmq.PAIR` sockets for the roundtrip test (as required by the task spec), and `zmq.ROUTER`/`zmq.DEALER` for identity-related tests (since PAIR sockets have no identity frames)
   - Each test has a Google-style docstring describing what it verifies, its preconditions, and expected outcome
   - Tests follow the existing pattern: `_reset_ipc_state()` → setup → test body → `finally` cleanup

   Specific tests (7 total, ≥ 6 required):
   a. **`test_connect_succeeds`** — bind a ROUTER socket, call `ipc.connect(port, "test-worker")`, assert `ipc._sock is not None` and `ipc._ctx is not None`. Cleanup: close ROUTER in `finally`.
   b. **`test_connect_sets_identity`** — bind a ROUTER socket, call `ipc.connect(port, "test-worker")`, send a msgpack message via `ipc.send_event({"_type": "Ping"})`, receive the identity frame from the ROUTER, assert it equals `b"test-worker"`.
   c. **`test_send_event_encodes_type_discriminator`** — connect via `ipc.connect()`, call `ipc.send_event({"_type": "Ready", "node_types": ["LoadModel"]})`, receive from ROUTER, deserialize with `msgpack.unpackb(raw, raw=False)`, assert `received["_type"] == "Ready"` and all payload fields are preserved.
   d. **`test_recv_message_deserialises_correctly`** — connect via `ipc.connect()`, send msgpack from ROUTER side (`router.send_multipart([b"test-worker", msgpack.packb(payload)])`), call `ipc.recv_message()`, assert returned dict matches payload exactly.
   e. **`test_roundtrip_via_pair_sockets`** — create two `zmq.PAIR` sockets connected in-process (`p1` and `p2`), pack data with `msgpack.packb({"_type": "Ping", "seq": 42}, use_bin_type=True)` on `p1`, receive on `p2` with `p2.recv()`, unpack with `msgpack.unpackb(raw, raw=False)`, assert content matches. This verifies the msgpack roundtrip mechanism that `ipc.py` relies on, without involving the ROUTER/DEALER identity routing layer.
   f. **`test_send_before_connect_raises`** — call `_reset_ipc_state()`, assert `ipc.send_event({})` raises `RuntimeError`.
   g. **`test_recv_before_connect_raises`** — call `_reset_ipc_state()`, assert `ipc.recv_message()` raises `RuntimeError`.

3. **Verify** by running `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_ipc.py -v` and confirming ≥ 6 tests pass.

## Public API Surface

None. This task only writes test code; no new public API items are introduced.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/tests/conftest.py` | Autouse fixture for `ANVILML_WORKER_MOCK=1` env var management |
| MODIFY | `worker/tests/test_ipc.py` | Overwrite with new tests matching task spec (PAIR sockets, env var handling) |
| (no change) | `worker/tests/__init__.py` | Already exists as empty package marker |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_ipc.py` | `test_connect_succeeds` | `connect()` creates valid socket and sets `_sock` | ROUTER bound on ephemeral port | `port`, `"test-worker"` | `ipc._sock is not None`, `ipc._ctx is not None` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_ipc.py -v -k test_connect_succeeds` exits 0 |
| `worker/tests/test_ipc.py` | `test_connect_sets_identity` | `connect()` sets socket identity visible on ROUTER | ROUTER bound, DEALER connected | `port`, `"test-worker"` | ROUTER identity frame equals `b"test-worker"` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_ipc.py -v -k test_connect_sets_identity` exits 0 |
| `worker/tests/test_ipc.py` | `test_send_event_encodes_type_discriminator` | `send_event()` sends msgpack with correct `_type` key | ROUTER bound, DEALER connected | `{"_type": "Ready", "node_types": [...]}` | `msgpack.unpackb` returns dict with `_type == "Ready"` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_ipc.py -v -k test_send_event_encodes_type_discriminator` exits 0 |
| `worker/tests/test_ipc.py` | `test_recv_message_deserialises_correctly` | `recv_message()` deserialises msgpack correctly | ROUTER sends msgpack, DEALER connected | ROUTER sends `{"_type": "Ping", "seq": 1}` | `ipc.recv_message()` returns matching dict | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_ipc.py -v -k test_recv_message_deserialises_correctly` exits 0 |
| `worker/tests/test_ipc.py` | `test_roundtrip_via_pair_sockets` | msgpack roundtrip via in-process PAIR sockets | Two PAIR sockets connected in-process | `{"_type": "Ping", "seq": 42}` | Unpacked dict matches original exactly | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_ipc.py -v -k test_roundtrip_via_pair_sockets` exits 0 |
| `worker/tests/test_ipc.py` | `test_send_before_connect_raises` | `send_event()` raises `RuntimeError` before connect | `_sock` is `None` | `{}` | `RuntimeError` raised | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_ipc.py -v -k test_send_before_connect_raises` exits 0 |
| `worker/tests/test_ipc.py` | `test_recv_before_connect_raises` | `recv_message()` raises `RuntimeError` before connect | `_sock` is `None` | (none) | `RuntimeError` raised | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_ipc.py -v -k test_recv_before_connect_raises` exits 0 |

## CI Impact

No CI changes required. The `worker-linux` and `worker-windows` CI jobs already run `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v`, which picks up any new test files in the `worker/tests/` directory. Adding `conftest.py` (no test functions) does not change CI behavior.

## Platform Considerations

None identified. The `zmq.PAIR` socket, `msgpack.packb/unpackb`, and `os.environ` manipulation are all platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `zmq.PAIR` sockets in pyzmq 27.x may differ from libzmq 4.x PAIR semantics — PAIR is a strict one-to-one socket that requires both ends to be connected before sending. If the test sends on one end before the other connects, it hangs. | Low | Medium | Add a brief `time.sleep(0.05)` after creating the PAIR pair before sending, or use `p1.bind()` and `p2.connect()` (bind first pattern ensures the server side is ready). |
| The existing `test_ipc.py` has 7 tests that partially overlap with the new tests. Overwriting it loses the ROUTER-based tests which may have been useful for other purposes. | Low | Low | The existing tests use ROUTER sockets which don't match the task spec (PAIR required). The new tests cover the same functional areas (connect, send, recv, roundtrip, error cases) but with the correct socket type and env var handling. |
| `conftest.py` autouse fixture runs for ALL tests in the directory, including `test_placeholder.py`. If `test_placeholder.py` doesn't need `ANVILML_WORKER_MOCK=1`, it still gets it set. | Low | Low | Setting an extra env var on a test that doesn't use it has no side effects. The fixture restores the original value after, so no leakage. |
| `zmq.Context.instance()` is a process-global singleton. Multiple tests using it concurrently could interfere if pytest runs with parallelism. | Medium | Medium | The autouse fixture sets `ANVILML_WORKER_MOCK=1` which is process-global. Run pytest without xdist (`-p no:xdist` or just default single-threaded pytest). Each test creates its own socket pair and cleans up in `finally`, so isolation is maintained. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_ipc.py -v` exits 0 with ≥ 6 tests
- [ ] `grep -c "^def test_" worker/tests/test_ipc.py` outputs a number ≥ 6
- [ ] `grep -c "zmq.PAIR" worker/tests/test_ipc.py` outputs a number ≥ 1 (PAIR sockets used in roundtrip test)
- [ ] `grep -c "ANVILML_WORKER_MOCK" worker/tests/conftest.py` outputs ≥ 1 (conftest.py references the env var)
