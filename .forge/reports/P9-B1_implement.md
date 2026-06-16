# Implementation Report: P9-B1

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P9-B1                              |
| Phase         | 009 — Worker Spawn & Handshake     |
| Description   | worker/worker_main.py mock mode startup, Ready event, Ping/Pong loop |
| Implemented   | 2026-06-17T00:00:00Z               |
| Status        | COMPLETE                           |

## Summary

Implemented `worker/worker_main.py` — the Python worker entry point spawned by the Rust supervisor — in mock mode. The worker reads identity from environment variables (`ANVILML_IPC_PORT`, `ANVILML_WORKER_ID`, `ANVILML_DEVICE_INDEX`, `ANVILML_DEVICE_TYPE`), connects to the Rust ROUTER socket via `worker.ipc.connect()`, emits a `Ready` event with synthetic hardware capability values (no torch import), and enters a message dispatch loop that responds to `Ping` with `Pong` and exits cleanly on `Shutdown`. Created 4 tests in `worker/tests/test_worker_main.py` covering mock startup, Ready event verification, Ping→Pong heartbeat, Shutdown exit, and custom env var passthrough. All 12 Python tests and 143 Rust tests pass.

## Resolved Dependencies

None. This task introduces no new external dependencies. All imports are from the existing `worker/ipc.py` module (pyzmq, msgpack) and the Python standard library.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/worker_main.py` | Mock mode worker entry point with startup, Ready event, Ping/Pong loop, Shutdown exit |
| CREATE | `worker/tests/test_worker_main.py` | 4 tests: mock startup, Ping→Pong, Shutdown, custom env vars |
| MODIFY | `docs/TESTS.md` | Added 4 test entries for new worker_main tests |

## Commit Log

```
 .forge/reports/P9-B1_plan.md     | 158 ++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md     |   6 +-
 .forge/state/state.json          |  13 +-
 docs/TESTS.md                    |  36 ++++++
 worker/tests/test_worker_main.py | 252 +++++++++++++++++++++++++++++++++++++++
 worker/worker_main.py            | 133 +++++++++++++++++++++
 6 files changed, 589 insertions(+), 9 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
collecting ... collected 12 items

worker/tests/test_ipc.py::test_connect_succeeds PASSED                   [  8%]
worker/tests/test_ipc.py::test_connect_sets_identity PASSED              [ 16%]
worker/tests/test_ipc.py::test_send_event_encodes_type_discriminator PASSED [ 25%]
worker/tests/test_ipc.py::test_recv_message_deserialises_correctly PASSED [ 33%]
worker/tests/test_ipc.py::test_roundtrip_via_pair_sockets PASSED         [ 41%]
worker/tests/test_ipc.py::test_send_before_connect_raises PASSED         [ 50%]
worker/tests/test_ipc.py::test_recv_before_connect_raises PASSED         [ 58%]
worker/tests/test_placeholder.py::test_placeholder PASSED                [ 66%]
worker/tests/test_worker_main.py::test_mock_startup_sends_ready PASSED   [ 75%]
worker/tests/test_worker_main.py::test_ping_returns_pong PASSED          [ 83%]
worker/tests/test_worker_main.py::test_shutdown_exits_cleanly PASSED     [ 91%]
worker/tests/test_worker_main.py::test_env_vars_read_from_environment PASSED [100%]

============================== 12 passed in 1.40s ==============================
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.31s

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.57s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.24s

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.27s
```

## Project Gates

```
# Gate 1: config_reference
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

# Gate 2: openapi drift — not triggered (task writes no handler signatures or ToSchema derives)
# Gate 3: node parity — not triggered (task writes no node types)
```

## Public API Delta

```
(no output — grep returned nothing)
No new pub items introduced.
```

The `main()` function in `worker/worker_main.py` is module-level (not `pub`) and is only called via the `if __name__ == "__main__":` guard. This matches the pattern used in `worker/ipc_echo.py`.

## Deviations from Plan

- The `main()` function was intentionally made module-level (not `pub`) to match the established pattern in `worker/ipc_echo.py`, which also uses `def main()` without `pub`. This is consistent with the plan's public API surface table which lists `def main() -> None` as a module-level entry point.

- The `test_ping_returns_pong` test required an additional step: consuming the `Ready` event from the ROUTER before sending the `Ping`. This was necessary because the worker sends the `Ready` event on startup (before the test sends its `Ping`), and the ROUTER delivers messages in order. The fix adds `router.recv()` + `msgpack.unpackb()` to drain the Ready event before sending the Ping. This is a test implementation detail, not a deviation from the plan's logic.

- No version bumps were required since this task only writes Python files (no Rust crate source files were modified).

## Blockers

None.
