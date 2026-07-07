# Implementation Report: P14-B1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P14-B1                          |
| Phase         | 14 — Dispatch & Execute         |
| Description   | worker/nodes/passthrough.py: trivial real node (no-op) |
| Implemented   | 2026-07-07T22:05:00Z            |
| Status        | COMPLETE                          |

## Summary

Created `worker/nodes/passthrough.py` containing the `PassThrough` node — the first concrete node class in the project. The node reads one `ANY`-typed input slot named `"value"` and returns it unchanged as the sole output. Both mock and real branches of `execute()` return the input identically, with the `ctx.mock` branch existing solely to satisfy the dual-mode parity marker convention (§10.6). Created `worker/tests/test_passthrough.py` with 6 tests covering class attributes, mock execute, real execute, registry inclusion, marker collectibility, and dict identity. Fixed 7 pre-existing tests that assumed an empty NODE_REGISTRY to account for the new auto-registered PassThrough node.

## Resolved Dependencies

None. This task introduces no new Python packages or crates.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/passthrough.py` | PassThrough node class — first concrete node file |
| CREATE | `worker/tests/test_passthrough.py` | 6 tests for PassThrough node |
| MODIFY | `worker/tests/test_base.py` | Updated `test_node_registry_starts_empty` to account for PassThrough registration |
| MODIFY | `worker/tests/test_nodes_init.py` | Updated 3 tests (`test_import_does_not_raise`, `test_node_registry_empty_after_import`, `test_reimport_is_idempotent`) to account for PassThrough registration |
| MODIFY | `worker/tests/test_worker_main.py` | Updated 3 tests (`test_real_startup_sends_ready_event`, `test_import_nodes_returns_empty_list`, `test_mock_startup_sends_ready_event`) to expect PassThrough in node_types |
| MODIFY | `crates/anvilml-worker/tests/real_startup_tests.rs` | Updated `test_real_subprocess_sends_ready` to expect PassThrough in node_types |
| MODIFY | `docs/TESTS.md` | Added 6 test catalogue entries for new passthrough tests |

## Commit Log

```
 .forge/reports/P14-B1_plan.md                     | 195 ++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                      |   6 +-
 .forge/state/state.json                           |  13 +-
 crates/anvilml-worker/tests/real_startup_tests.rs |  23 ++-
 docs/TESTS.md                                     |  72 ++++++++
 worker/nodes/passthrough.py                       |  59 +++++++
 worker/tests/test_base.py                         |  14 +-
 worker/tests/test_nodes_init.py                   |  27 +--
 worker/tests/test_passthrough.py                  | 184 ++++++++++++++++++++
 worker/tests/test_worker_main.py                  |  41 +++--
 10 files changed, 587 insertions(+), 47 deletions(-)
```

## Test Results

### Python mock-mode tests (46 passed, 19 deselected)

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 65 items / 19 deselected / 46 selected

worker/tests/test_arch_dispatch.py::test_get_module_returns_none_when_empty PASSED
worker/tests/test_arch_dispatch.py::test_get_module_does_not_raise_for_various_key_types PASSED
worker/tests/test_arch_dispatch.py::test_get_module_skips_module_with_can_handle_false PASSED
worker/tests/test_arch_dispatch.py::test_clip_get_module_returns_none_when_empty PASSED
worker/tests/test_arch_dispatch.py::test_clip_get_module_does_not_raise_for_various_key_types PASSED
worker/tests/test_arch_dispatch.py::test_clip_get_module_skips_module_with_can_handle_false PASSED
worker/tests/test_arch_dispatch.py::test_vae_get_module_returns_none_when_empty PASSED
worker/tests/test_arch_dispatch.py::test_vae_get_module_does_not_raise_for_various_key_types PASSED
worker/tests/test_arch_dispatch.py::test_vae_get_module_skips_module_with_can_handle_false PASSED
worker/tests/test_base.py::test_node_registry_starts_empty PASSED
worker/tests/test_base.py::test_slotspec_optional_defaults_to_false PASSED
worker/tests/test_base.py::test_slotspec_accepts_explicit_optional_true PASSED
worker/tests/test_base.py::test_register_success PASSED
worker/tests/test_base.py::test_register_missing_NODE_TYPE PASSED
worker/tests/test_base.py::test_register_missing_CATEGORY PASSED
worker/tests/test_base.py::test_register_missing_DISPLAY_NAME PASSED
worker/tests/test_base.py::test_register_missing_DESCRIPTION PASSED
worker/tests/test_base.py::test_register_missing_INPUT_SLOTS PASSED
worker/tests/test_base.py::test_register_missing_OUTPUT_SLOTS PASSED
worker/tests/test_base.py::test_register_returns_class_identity PASSED
worker/tests/test_base.py::test_node_context_assigns_all_attrs PASSED
worker/tests/test_base.py::test_node_context_mock_true PASSED
worker/tests/test_base.py::test_node_context_mock_false PASSED
worker/tests/test_base.py::test_node_context_caps_accepts_arbitrary_dict PASSED
worker/tests/test_base.py::test_base_node_cannot_be_instantiated PASSED
worker/tests/test_base.py::test_concrete_subclass_instantiates PASSED
worker/tests/test_base.py::test_execute_calls_subclass_impl PASSED
worker/tests/test_ipc.py::TestConnectIdentity::test_connect_sets_identity PASSED
worker/tests/test_ipc.py::TestPreConnectErrors::test_send_event_before_connect_raises PASSED
worker/tests/test_ipc.py::TestPreConnectErrors::test_recv_message_before_connect_raises PASSED
worker/tests/test_ipc.py::TestRoundtrip::test_roundtrip_send_recv PASSED
worker/tests/test_ipc.py::TestNoTorchImport::test_module_no_torch_import PASSED
worker/tests/test_ipc.py::TestContextReuse::test_connect_twice_reuses_context PASSED
worker/tests/test_nodes_init.py::test_import_does_not_raise PASSED
worker/tests/test_nodes_init.py::test_node_registry_empty_after_import PASSED
worker/tests/test_nodes_init.py::test_reimport_is_idempotent PASSED
worker/tests/test_passthrough.py::test_class_attributes PASSED
worker/tests/test_passthrough.py::test_execute_mock_returns_input PASSED
worker/tests/test_passthrough.py::test_execute_real_returns_input PASSED
worker/tests/test_passthrough.py::test_node_in_registry_after_import PASSED
worker/tests/test_passthrough.py::test_markers_name_collectible_tests PASSED
worker/tests/test_passthrough.py::test_execute_returns_new_dict PASSED
worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_returns_six_required_keys PASSED
worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_all_values_are_bool PASSED
worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_fp4_is_false PASSED
worker/tests/test_worker_main.py::TestNoTorchImport::test_no_torch_import_on_module_load PASSED

============================== 46 passed in 3.06s ===============================
```

### Python real-mode tests (19 passed, 46 deselected)

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 65 items / 46 deselected / 19 selected

worker/tests/test_capability.py::TestProbeDtypes::test_fp32_cpu_returns_true PASSED
worker/tests/test_capability.py::TestProbeDtypes::test_fp16_cpu_returns_true PASSED
worker/tests/test_capability.py::TestProbeDtypes::test_bf16_cpu_returns_true PASSED
worker/tests/test_capability.py::TestProbeDtypes::test_fp8_cpu_returns_false PASSED
worker/tests/test_capability.py::TestProbeDtypes::test_fp4_cpu_returns_false PASSED
worker/tests/test_capability.py::TestProbeFlashAttention::test_flash_attention_cpu_returns_true PASSED
worker/tests/test_capability.py::TestProbeStructure::test_returns_dict_with_exactly_six_bool_keys PASSED
worker/tests/test_capability.py::TestProbeStructure::test_never_raises_for_cpu PASSED
worker/tests/test_capability.py::TestProbeStructure::test_device_selection_cpu PASSED
worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_calls_ipc_connect PASSED
worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_cpu_skips_cuda_set_device PASSED
worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_calls_probe_capabilities PASSED
worker/tests/test_worker_main.py::TestNoMockGate::test_no_mock_gate_exit_path PASSED
worker/tests/test_worker_main.py::TestNoMockGate::test_real_startup_sends_ready_event PASSED
worker/tests/test_worker_main.py::TestNoMockGate::test_import_nodes_returns_registered_nodes PASSED
worker/tests/test_worker_main.py::TestNoMockGate::test_dispatch_loop_exists_and_is_callable PASSED
worker/tests/test_worker_main.py::TestNoMockGate::test_real_startup_no_nonzero_exit_for_cpu PASSED
worker/tests/test_worker_main.py::TestNoMockGate::test_mock_startup_sends_ready_event PASSED
worker/tests/test_worker_main.py::TestNoMockGate::test_no_mock_gate_in_main_block PASSED

====================== 19 passed, 46 deselected in 1.93s =======================
```

### Rust full test suite

All 340+ tests passed across all crates (anvilml, anvilml-core, anvilml-hardware, anvilml-ipc, anvilml-registry, anvilml-artifacts, anvilml-worker, anvilml-server, anvilml-scheduler, anvilml-openapi).

## Format Gate

```
Not applicable — task wrote no Rust source files.
cargo fmt --all -- --check exited 0 (no drift).
```

## Platform Cross-Check

```
=== Check 1: Mock-hardware Linux ===
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.55s

=== Check 2: Mock-hardware Windows ===
Finished `dev` profile [unoptimized + debuginfo] target(s) in 27.68s

=== Check 3: Real-hardware Linux ===
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.91s

=== Check 4: Real-hardware Windows ===
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.01s

All 4 checks passed.
```

## Project Gates

### Gate 3 — Node Parity
`worker/tests/test_parity.py` does not exist yet — gate is defined but test file not yet created.

### Gate 4 — Mock/Real Parity Markers

```
--- Check 1: marker test collectibility (Python files only) ---
tests/test_passthrough.py::test_execute_real_returns_input
1 test collected in 0.01s
tests/test_passthrough.py::test_execute_mock_returns_input
1 test collected in 0.01s

--- Check 2: files lacking REAL_PATH_VERIFIED ---
(empty — all files have the marker)

--- Check 3: files lacking MOCK_PATH_VERIFIED ---
(empty — all files have the marker)

Gate 4: PASS
```

## Public API Delta

```
--- New class and method definitions ---
7:class PassThrough(BaseNode):
35:    def execute(self, ctx: NodeContext, **inputs) -> dict:
--- Decorator ---
6:@register
```

New items:
- Class `PassThrough` at `worker.nodes.passthrough.PassThrough` — concrete node, inherits `BaseNode.execute()`
- Method `PassThrough.execute(self, ctx: NodeContext, **inputs) -> dict` — returns `{"value": inputs["value"]}`, branches on `ctx.mock`

## Deviations from Plan

- **Pre-existing test fixes:** 7 pre-existing tests across 4 files (`test_base.py`, `test_nodes_init.py`, `test_worker_main.py`, `real_startup_tests.rs`) assumed `NODE_REGISTRY` was empty and `node_types == []`. These were updated to expect the PassThrough node in the registry. This is a necessary consequence of adding a node that auto-registers at package load time. The plan's scope was limited to creating the passthrough node and its tests; updating these existing tests is a minimal fix required by the new registration side-effect.

## Blockers

None.
