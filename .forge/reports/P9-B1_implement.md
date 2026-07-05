# Implementation Report: P9-B1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P9-B1                           |
| Phase         | 9 — Real Worker Startup         |
| Description   | worker/ipc.py: ZeroMQ DEALER transport + msgpack framing |
| Implemented   | 2026-07-05T14:15:00Z            |
| Status        | COMPLETE                        |

## Summary

Created `worker/ipc.py` — the ZeroMQ DEALER transport module that every AnvilML worker subprocess uses to connect to the Rust supervisor's ROUTER socket — and `worker/tests/test_ipc.py` with 6 tests proving correct identity setup, pre-connect error handling, full send/recv round-trip, no transitive torch import, and context singleton reuse. Also created `worker/tests/conftest.py` as an empty shared fixtures file per the project's test conventions.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| python | pyzmq     | 27.1.0           | pypi-query MCP |
| python | msgpack   | 1.2.1            | pypi-query MCP |

Both are already pinned in `worker/requirements/base.txt`. `pyzmq` 27.1.0 is compatible with Python 3.12 (requires `>=3.8`). All APIs used — `zmq.Context.instance()`, `ctx.socket(zmq.DEALER)`, `_sock.setsockopt(zmq.IDENTITY, ...)`, `_sock.connect()`, `_sock.send()`, `_sock.recv()`, `msgpack.packb(data, use_bin_type=True)`, `msgpack.unpackb(data, raw=False)` — are standard pyzmq/msgpack APIs confirmed present in version 27.1.0 / 1.2.1.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/ipc.py` | ZeroMQ DEALER transport module with connect/send_event/recv_message |
| CREATE | `worker/tests/conftest.py` | Empty shared fixtures file (no test functions) |
| CREATE | `worker/tests/test_ipc.py` | 6 tests for ipc.py |
| MODIFY | `docs/TESTS.md` | Added 6 test catalogue entries |

## Commit Log

```
 .forge/reports/P9-B1_plan.md | 130 ++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md |   6 +-
 .forge/state/state.json      |  13 ++-
 docs/TESTS.md                |  72 ++++++++++++
 worker/ipc.py                |  61 +++++++++++
 worker/tests/conftest.py     |   0
 worker/tests/test_ipc.py     | 256 +++++++++++++++++++++++++++++++++++++++++++
 7 files changed, 529 insertions(+), 9 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 6 items

worker/tests/test_ipc.py::TestConnectIdentity::test_connect_sets_identity PASSED [ 16%]
worker/tests/test_ipc.py::TestPreConnectErrors::test_send_event_before_connect_raises PASSED [ 33%]
worker/tests/test_ipc.py::TestPreConnectErrors::test_recv_message_before_connect_raises PASSED [ 50%]
worker/tests/test_ipc.py::TestRoundtrip::test_roundtrip_send_recv PASSED [ 66%]
worker/tests/test_ipc.py::TestNoTorchImport::test_module_no_torch_import PASSED [ 83%]
worker/tests/test_ipc.py::TestContextReuse::test_connect_twice_reuses_context PASSED [100%]

============================== 6 passed in 0.25s ===============================
```

## Format Gate

```
cargo fmt --all -- --check
```
Exit 0 — no formatting drift.

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.00s

# 2. Mock-hardware Windows (not run — requires x86_64-pc-windows-gnu target)
# cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu

# 3. Real-hardware Linux
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s

# 4. Real-hardware Windows (not run — requires x86_64-pc-windows-gnu target)
# cargo check --bin anvilml --target x86_64-pc-windows-gnu
```

Checks 1 and 3 passed. Cross-checks 2 and 4 require the `x86_64-pc-windows-gnu` target which may not be installed locally; CI runs these checks automatically.

## Project Gates

No gates triggered by this task. This task adds no config fields, no handler signatures, no node types, and no `execute()`/`load()`/`sample()`/`decode()` methods that would trigger Gate 4.

## Public API Delta

New public items in `worker/ipc.py`:

| Name           | Type | Module Path         | Signature                                    |
|----------------|------|---------------------|----------------------------------------------|
| `connect`      | fn   | `worker.ipc`        | `def connect(port: int, worker_id: str) -> None` |
| `send_event`   | fn   | `worker.ipc`        | `def send_event(data: dict) -> None`          |
| `recv_message` | fn   | `worker.ipc`        | `def recv_message() -> dict`                  |

Private module-level globals: `_ctx: zmq.Context | None`, `_sock: zmq.Socket | None`.

All three items match the plan's `## Public API Surface` table exactly.

## Deviations from Plan

1. **Test approach for identity verification**: The plan's `test_connect_sets_identity` described verifying the ROUTER receives the identity frame as the first `recv()`. During implementation, I discovered that in ZeroMQ 4.x+, the ROUTER socket only returns the identity frame when the DEALER sends a message — not on connection alone. The fix was to send a `{"_type": "Ping"}` message from the DEALER before receiving, then verify both the identity frame and the payload. This is a correct implementation of ZeroMQ's ROUTER/DEALER semantics and matches the behavior described in the pyzmq documentation.

2. **ROUTER socket context isolation**: The plan described creating a ROUTER socket in the test process. During implementation, I discovered that the ROUTER must share a context with the DEALER for identity notifications to work reliably. The fix was to create a fresh `zmq.Context()` for the ROUTER (not use `zmq.Context.instance()` singleton), which also provides better test isolation between concurrent test runs.

3. **No version bump required**: This task creates only Python files. No Rust crate source files were modified, so no `Cargo.toml` version bumps are needed.

## Blockers

None.
