# Implementation Report: P10-A1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P10-A1                          |
| Phase         | 10 — Generic Node Groundwork    |
| Description   | worker/nodes/base.py: SlotSpec dataclass + NODE_REGISTRY dict |
| Implemented   | 2026-07-05T22:15:00Z            |
| Status        | COMPLETE                          |

## Summary

Created the `worker/nodes/` package with two files: an empty `__init__.py` and `base.py`
containing the `NODE_REGISTRY` dict and `SlotSpec` dataclass as defined verbatim in
`ANVILML_DESIGN.md §14.5`. Wrote 3 unit tests in `worker/tests/test_base.py` that verify
`NODE_REGISTRY` starts empty and `SlotSpec` constructs with both the default `optional=False`
and explicit `optional=True`. All 214 Rust tests pass, all 13 Python mock-mode tests pass,
all four platform cross-checks pass, and all project gates pass.

## Resolved Dependencies

None. This task uses only Python 3.12 standard library modules: `dataclasses`, `abc`,
`typing`, `from __future__ import annotations`. No external packages are introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/__init__.py` | Empty package init file |
| CREATE | `worker/nodes/base.py` | NODE_REGISTRY dict + SlotSpec dataclass (normative per §14.5) |
| CREATE | `worker/tests/test_base.py` | 3 unit tests for NODE_REGISTRY and SlotSpec |
| MODIFY | `docs/TESTS.md` | Added 3 test catalogue entries for new tests |

## Commit Log

```
 docs/TESTS.md                   | 42 +++++++++++++++++++++++++
 worker/nodes/__init__.py        |  0
 worker/nodes/base.py            | 17 ++++++++++
 worker/tests/test_base.py       | 40 +++++++++++++++++++++
 4 files changed, 99 insertions(+)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 3 items

worker/tests/test_base.py::test_node_registry_starts_empty PASSED        [ 33%]
worker/tests/test_base.py::test_slotspec_optional_defaults_to_false PASSED [ 66%]
worker/tests/test_base.py::test_slotspec_accepts_explicit_optional_true PASSED [100%]

============================== 3 passed in 0.02s ===============================
```

Full Python mock-mode suite (13 tests, all passing):
```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 32 items / 19 deselected / 13 selected

worker/tests/test_base.py::test_node_registry_starts_empty PASSED        [  7%]
worker/tests/test_base.py::test_slotspec_optional_defaults_to_false PASSED [ 15%]
worker/tests/test_base.py::test_slotspec_accepts_explicit_optional_true PASSED [ 23%]
worker/tests/test_ipc.py::TestConnectIdentity::test_connect_sets_identity PASSED [ 30%]
worker/tests/test_ipc.py::TestPreConnectErrors::test_send_event_before_connect_raises PASSED [ 46%]
worker/tests/test_ipc.py::TestPreConnectErrors::test_recv_message_before_connect_raises PASSED [ 46%]
worker/tests/test_ipc.py::TestRoundtrip::test_roundtrip_send_recv PASSED [ 53%]
worker/tests/test_ipc.py::TestNoTorchImport::test_module_no_torch_import PASSED [ 61%]
worker/tests/test_ipc.py::TestContextReuse::test_connect_twice_reuses_context PASSED [ 69%]
worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_returns_six_required_keys PASSED [ 76%]
worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_all_values_are_bool PASSED [ 84%]
worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_fp4_is_false PASSED [ 92%]
worker/tests/test_worker_main.py::TestNoTorchImport::test_no_torch_import_on_module_load PASSED [100%]

====================== 13 passed, 19 deselected in 2.29s =======================
```

## Format Gate

```
(No output — cargo fmt --all -- --check exited 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.22s

# 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.40s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.68s

# 4. Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 27.19s
```

## Project Gates

```
Gate 1 — Config Surface Sync:
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Public API Delta

No new `pub` items introduced (Python uses module-level names, not `pub`). The public
API surface consists of:
- `worker.nodes.base.NODE_REGISTRY` — module-level `dict[str, type["BaseNode"]]`
- `worker.nodes.base.SlotSpec` — `@dataclass class` with fields `name`, `slot_type`, `optional`

## Deviations from Plan

None. Implementation matches the approved plan exactly.

## Blockers

None.
