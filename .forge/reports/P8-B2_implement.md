# Implementation Report: P8-B2

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P8-B2                              |
| Phase         | 008 — ZeroMQ IPC Transport         |
| Description   | worker/tests/test_ipc.py: ipc.py unit tests |
| Implemented   | 2026-06-16T15:10:00Z               |
| Status        | COMPLETE                           |

## Summary

Created `worker/tests/conftest.py` with an autouse `mock_mode` fixture that unconditionally sets `ANVILML_WORKER_MOCK=1` for every test and restores the original value. Overwrote `worker/tests/test_ipc.py` with 7 new tests covering: connect success, identity attachment, type discriminator encoding, recv deserialisation, PAIR socket roundtrip, and pre-connect error handling for both `send_event()` and `recv_message()`. Updated `docs/TESTS.md` with entries for all 7 new tests, replacing the 6 old entries. All 7 tests pass, all Rust tests pass, all gates pass.

## Resolved Dependencies

None. This task writes only test code; no new dependencies are added or modified. The existing dependencies (`pyzmq`, `msgpack`, `pytest`) are already present in the project and verified compatible with Python 3.12.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/tests/conftest.py` | Autouse pytest fixture for `ANVILML_WORKER_MOCK=1` env var management |
| MODIFY | `worker/tests/test_ipc.py` | Overwritten with 7 new tests using ROUTER/DEALER and PAIR sockets |
| MODIFY | `docs/TESTS.md` | Replaced 6 old IPC test entries with 7 new entries matching the new tests |

## Commit Log

```
 .forge/reports/P8-B2_plan.md | 120 +++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md |   6 +-
 .forge/state/state.json      |  13 ++--
 docs/TESTS.md                |  67 ++++++++++--------
 worker/tests/conftest.py     |  31 ++++++++
 worker/tests/test_ipc.py     | 163 +++++++++++++++++++++++--------------------
 6 files changed, 287 insertions(+), 113 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.0.3, pluggy-1.6.0
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
plugins: anyio-4.12.1
collecting ... collected 7 items

worker/tests/test_ipc.py::test_connect_succeeds PASSED                   [ 14%]
worker/tests/test_ipc.py::test_connect_sets_identity PASSED              [ 28%]
worker/tests/test_ipc.py::test_send_event_encodes_type_discriminator PASSED [ 42%]
worker/tests/test_ipc.py::test_recv_message_deserialises_correctly PASSED [ 57%]
worker/tests/test_ipc.py::test_roundtrip_via_pair_sockets PASSED         [ 71%]
worker/tests/test_ipc.py::test_send_before_connect_raises PASSED         [ 85%]
worker/tests/test_ipc.py::test_recv_before_connect_raises PASSED         [100%]

============================== 7 passed in 0.16s ===============================
```

Full worker test suite (8 tests including placeholder):

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.0.3, pluggy-1.6.0
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
plugins: anyio-4.12.1
collecting ... collected 8 items

worker/tests/test_ipc.py::test_connect_succeeds PASSED                   [ 12%]
worker/tests/test_ipc.py::test_connect_sets_identity PASSED              [ 25%]
worker/tests/test_ipc.py::test_send_event_encodes_type_discriminator PASSED [ 37%]
worker/tests/test_ipc.py::test_recv_message_deserialises_correctly PASSED [ 50%]
worker/tests/test_ipc.py::test_roundtrip_via_pair_sockets PASSED         [ 62%]
worker/tests/test_ipc.py::test_send_before_connect_raises PASSED         [ 75%]
worker/tests/test_ipc.py::test_recv_before_connect_raises PASSED         [ 87%]
worker/tests/test_placeholder.py::test_placeholder PASSED                [100%]

============================== 8 passed in 0.15s ===============================
```

## Format Gate

```
cargo fmt --all -- --check
```
Exit 0 — no formatting drift detected.

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.55s

# 3. Real-hardware Linux
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.27s
```

All four cross-checks exit 0.

## Project Gates

Gate 1 — Config Surface Sync:
```
cargo test -p anvilml --features mock-hardware -- config_reference
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored
```

Gate 2 — OpenAPI Drift: Not applicable — this task does not modify handler function signatures, `#[utoipa::path]` annotations, or `ToSchema` derives.

Gate 3 — Node Parity: Not applicable — this task does not add, remove, or rename node types in `worker/nodes/` or modify `crates/anvilml-scheduler/src/node_registry.rs`.

## Public API Delta

```
git diff HEAD -- worker/tests/conftest.py worker/tests/test_ipc.py | grep "^+.*pub " | head -40
```
(no output)

No new `pub` items introduced. This task only writes test code and a conftest fixture; no public API is exposed.

## Deviations from Plan

None. All 7 tests were implemented exactly as specified in the approved plan. The `conftest.py` fixture matches the plan's specification precisely. The `test_roundtrip_via_pair_sockets` test uses the bind-then-connect pattern (`p1.bind("tcp://127.0.0.1:*")` then `p2.connect(addr)`) to avoid the PAIR socket timing issue identified in the plan's risk table.

## Blockers

None.
