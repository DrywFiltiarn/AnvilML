# Implementation Report: P8-B1

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P8-B1                              |
| Phase         | 008 — ZeroMQ IPC Transport         |
| Description   | worker/ipc.py: ZeroMQ DEALER transport with identity |
| Implemented   | 2026-06-16T14:00:00Z               |
| Status        | COMPLETE                           |

## Summary

Created the Python worker ZeroMQ DEALER transport module (`worker/ipc.py`) with three public functions (`connect`, `send_event`, `recv_message`), the dependency file (`worker/requirements/base.txt`), the package init (`worker/__init__.py`), and 7 tests in `worker/tests/test_ipc.py`. All 130 Rust tests pass, all 8 Python tests pass, all 4 platform cross-checks pass, and both project gates (config reference, OpenAPI drift) pass.

## Resolved Dependencies

| Type   | Name        | Version resolved | Source        |
|--------|-------------|------------------|---------------|
| python | pyzmq       | 27.1.0           | pypi-query MCP |
| python | msgpack     | 1.2.0            | pypi-query MCP |
| python | pillow      | 12.2.0           | pypi-query MCP |
| python | safetensors | 0.8.0            | pypi-query MCP |
| python | pytest      | 9.1.0            | pypi-query MCP |

All five packages are compatible with Python 3.12 (the project's required version). The plan's minimum version constraints (>=26.0, >=1.0, >=10.0, >=0.4, >=8.0) are floor constraints — the MCP-resolved versions are all higher, so the plan's constraints are preserved as-is.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/__init__.py` | Empty file to make `worker` a Python package |
| CREATE | `worker/ipc.py` | ZeroMQ DEALER transport module with connect, send_event, recv_message |
| CREATE | `worker/requirements/base.txt` | Python dependency file for worker runtime |
| CREATE | `worker/tests/test_ipc.py` | 7 tests for ipc.py using in-process ZeroMQ socket pairs |
| MODIFY | `docs/TESTS.md` | Added 7 test catalogue entries for worker.ipc tests |

## Commit Log

```
 .forge/reports/P8-B1_plan.md | 174 +++++++++++++++++++++
 .forge/state/CURRENT_TASK.md |   6 +-
 .forge/state/state.json      |  13 +--
 docs/TESTS.md                |  54 ++++++
 worker/__init__.py           |   0
 worker/ipc.py                |  86 +++++++++
 worker/requirements/base.txt |   5 ++
 worker/tests/test_ipc.py     | 196 +++++++++++++++++++
 8 files changed, 525 insertions(+), 9 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.0.3, pluggy-1.6.0
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
plugins: anyio-4.12.1
collecting ... collected 8 items

worker/tests/test_ipc.py::test_connect_succeeds PASSED                   [ 12%]
worker/tests/test_ipc.py::test_send_event_roundtrip PASSED               [ 25%]
worker/tests/test_ipc.py::test_send_event_type_discriminator PASSED      [ 37%]
worker/tests/test_ipc.py::test_recv_message_before_connect_raises PASSED [ 50%]
worker/tests/test_ipc.py::test_send_event_before_connect_raises PASSED   [ 62%]
worker/tests/test_ipc.py::test_identity_attached PASSED                  [ 75%]
worker/tests/test_ipc.py::test_recv_message_from_router PASSED           [ 87%]
worker/tests/test_placeholder.py::test_placeholder PASSED                [100%]

============================== 8 passed in 0.15s ==============================
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
    Checking anvilml-ipc v0.1.3 (/home/dryw/AnvilML/crates/anvilml-ipc)
    Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.14 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Checking anvilml v0.1.10 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.02s

# 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
    Checking anvilml-ipc v0.1.3 (/home/dryw/AnvilML/crates/anvilml-ipc)
    Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.14 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml v0.1.10 (/home/dryw/AnvilML/backend)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.47s

# 3. Real-hardware Linux
    Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.14 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml v0.1.10 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.70s

# 4. Real-hardware Windows (x86_64-pc-windows-gnu)
    Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.14 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml v0.1.10 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.80s
```

## Project Gates

```
# Gate 1 — Config Surface Sync
     Running tests/config_reference.rs
     Running tests/config_reference.rs (target/debug/deps/config_reference-cca2b00433f7b)
     running 1 test
     test config_reference ... ok
     test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

# Gate 2 — OpenAPI Drift
     (cargo run -p anvilml-openapi completed; git diff --exit-code api/openapi.json returned 0 — no drift)

# Gate 3 — Node Parity
     Gate 3 skipped — no test_parity.py exists yet (node types not yet implemented)
```

## Public API Delta

```
# Public functions in worker/ipc.py:
def connect(port: int, worker_id: str) -> None:
def send_event(data: dict) -> None:
def recv_message() -> dict:
```

Three public functions matching the plan's Public API Surface table exactly.

## Deviations from Plan

- The approved plan's `test_send_event_roundtrip` was fundamentally flawed: it sent a message from the DEALER to the ROUTER via `ipc.send_event()`, then called `ipc.recv_message()` on the DEALER side expecting to receive the same message. But the message flow was DEALER→ROUTER, not ROUTER→DEALER, so `recv_message()` would block indefinitely. Fixed by having the test read from the ROUTER side (receiving `[identity, data]` multipart) and verifying the msgpack payload matches.
- Added a 7th test `test_recv_message_from_router` to verify the reverse direction (ROUTER→DEALER) works, which is the direction the Rust supervisor will use to send jobs to the worker.
- Added `time.sleep(0.1)` after `ipc.connect()` in `test_recv_message_from_router` to allow ZeroMQ's asynchronous connection establishment to complete before the ROUTER sends — without this, the ROUTER's `send_multipart` could reach the DEALER before the connection is fully established, causing a timeout.
- The `_reset_ipc_state()` helper closes the old `_sock` before setting it to None, preventing lingering DEALER socket connections from interfering with subsequent tests.

## Blockers

None.
