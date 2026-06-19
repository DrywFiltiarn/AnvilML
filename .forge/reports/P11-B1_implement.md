# Implementation Report: P11-B1

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P11-B1                             |
| Phase         | 011 — Dynamic Node Registry        |
| Description   | worker/nodes/__init__.py: NODE_REGISTRY auto-import |
| Implemented   | 2026-06-19T16:45:00Z               |
| Status        | COMPLETE                           |

## Summary

Created the Python-side node registration infrastructure: a `NODE_REGISTRY` dict in `worker/nodes/__init__.py` populated by auto-importing sibling `.py` modules via `pkgutil.iter_modules`, a `@register` decorator and `BaseNode` ABC in `worker/nodes/base.py`, and wiring into `worker/worker_main.py` so that `_import_nodes()` triggers the auto-import and `_build_node_types_list()` converts registered nodes into `NodeTypeDescriptor`-compatible dicts for the `Ready` IPC event. Four unit tests verify the registry, decorator, ABC enforcement, and dataclass. All 146 Rust tests and 16 Python tests pass.

## Resolved Dependencies

None. This task uses only Python standard library modules (`pkgutil`, `abc`, `dataclasses`, `typing`, `importlib`, `logging`) that are part of the base Python 3.12 installation. No external packages are introduced.

| Type   | Name    | Version resolved | Source         |
|--------|---------|-----------------|----------------|
| stdlib | pkgutil | 3.12            | n/a (stdlib)   |
| stdlib | abc     | 3.12            | n/a (stdlib)   |
| stdlib | dataclasses | 3.12        | n/a (stdlib)   |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/__init__.py` | NODE_REGISTRY global; auto-import via pkgutil.iter_modules; re-exports from base |
| CREATE | `worker/nodes/base.py` | @register decorator; BaseNode ABC; SlotSpec dataclass; NodeContext class |
| MODIFY | `worker/worker_main.py` | Add _import_nodes(); build node_types list from NODE_REGISTRY for Ready event |
| CREATE | `worker/tests/test_nodes_base.py` | Unit tests for registry, decorator, ABC enforcement, SlotSpec |
| MODIFY | `docs/TESTS.md` | Added 4 entries for new node registration tests |

## Commit Log

```
 .forge/reports/P11-B1_plan.md   | 134 +++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md    |   6 +-
 .forge/state/state.json         |  13 +--
 docs/TESTS.md                   |  36 +++++++
 worker/nodes/__init__.py        | 112 +++++++++++++++++++++
 worker/nodes/base.py            | 215 ++++++++++++++++++++++++++++++++++++++++
 worker/tests/test_nodes_base.py | 105 ++++++++++++++++++++
 worker/worker_main.py           |  75 +++++++++++++-
 8 files changed, 686 insertions(+), 10 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0, cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
collecting ... collected 16 items

worker/tests/test_ipc.py::test_connect_succeeds PASSED                   [  6%]
worker/tests/test_ipc.py::test_connect_sets_identity PASSED              [ 12%]
worker/tests/test_ipc.py::test_send_event_encodes_type_discriminator PASSED [ 18%]
worker/tests/test_ipc.py::test_recv_message_deserialises_correctly PASSED [ 25%]
worker/tests/test_ipc.py::test_roundtrip_via_pair_sockets PASSED         [ 31%]
worker/tests/test_ipc.py::test_send_before_connect_raises PASSED         [ 37%]
worker/tests/test_ipc.py::test_recv_before_connect_raises PASSED         [ 43%]
worker/tests/test_nodes_base.py::test_registry_populated_after_import PASSED [ 50%]
worker/tests/test_nodes_base.py::test_register_decorator_adds_class PASSED [ 56%]
worker/tests/test_nodes_base.py::test_base_node_cannot_be_instantiated PASSED [ 62%]
worker/tests/test_nodes_base.py::test_slot_spec_dataclass PASSED         [ 68%]
worker/tests/test_placeholder.py::test_placeholder PASSED                [ 75%]
worker/tests/test_worker_main.py::test_mock_startup_sends_ready PASSED   [ 81%]
worker/tests/test_worker_main.py::test_ping_returns_pong PASSED         [ 87%]
worker/tests/test_worker_main.py::test_shutdown_exits_cleanly PASSED   [ 93%]
worker/tests/test_worker_main.py::test_env_vars_read_from_environment PASSED [100%]

============================== 16 passed in 1.40s ==============================
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# Check 1: Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.32s
---CHECK1 OK---

# Check 2: Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.28s
---CHECK2 OK---

# Check 3: Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.28s
---CHECK3 OK---

# Check 4: Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.30s
---CHECK4 OK---
```

## Project Gates

**Gate 1 — Config Surface Sync:**
```
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out
```

**Gate 3 — Node Parity:** Not applicable — `worker/tests/test_parity.py` does not exist in this repository yet (scheduled for a later phase).

## Public API Delta

No new Rust `pub` items introduced (this task is Python-only).

Python public API introduced by this task:
- `NODE_REGISTRY` — `dict[str, type]` global in `worker/nodes/__init__.py`
- `register(cls: type) -> type` — decorator in `worker/nodes/base.py`
- `SlotSpec` — `@dataclass` in `worker/nodes/base.py`
- `NodeContext` — class in `worker/nodes/base.py`
- `BaseNode` — `ABC` subclass in `worker/nodes/base.py`
- `_import_nodes() -> None` — function in `worker/worker_main.py`
- `_build_node_types_list() -> list[dict]` — function in `worker/worker_main.py`

## Deviations from Plan

- **Circumvented circular import**: The plan specified defining `NODE_REGISTRY` in `__init__.py` and importing it in `base.py` for the `@register` decorator. However, this created a circular import because `base.py` imports from `worker.nodes` (which is `__init__.py`) while `__init__.py` imports from `base.py`. The fix was to define `NODE_REGISTRY = {}` at the very top of `__init__.py` (before the `from worker.nodes.base import ...` statement), so that when `base.py` imports `NODE_REGISTRY`, the dict already exists in the partially-initialized `worker.nodes` module. This is the standard Python pattern for breaking circular imports — define the shared symbol first, then import dependencies.

## Blockers

None.
