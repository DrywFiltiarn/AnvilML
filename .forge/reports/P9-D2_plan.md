# Plan Report: P9-D2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P9-D2                                       |
| Phase       | 009 — Real Worker Startup                   |
| Description | worker_main.py: real-mode node-import stub + Ready event + loop |
| Depends on  | P9-D1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-07-05T16:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Complete the real-mode startup sequence in `worker_main.py` by adding three steps that P9-D1 deferred: `_import_nodes()` (a stub returning an empty list, since the node system is Phase 10's scope), `ipc.send_event(Ready{...})` with `capabilities_source="pytorch"` and `node_types=[]`, and a message dispatch loop placeholder that receives messages via `ipc.recv_message()`, logs them at DEBUG level, and continues. This produces the full real-mode startup flow defined in `ANVILML_DESIGN.md §14.2`. The observable outcome is a real-mode worker subprocess that connects over IPC, probes capabilities, sends a `Ready` event with the correct metadata, and enters a dispatch loop without exiting nonzero.

## Scope

### In Scope
- Add `_import_nodes()` function to `worker/worker_main.py` that returns `[]` (empty list) — node import is Phase 10's scope.
- Add `_dispatch_loop()` function to `worker/worker_main.py` that loops calling `ipc.recv_message()`, logs each message at DEBUG level, and continues (real dispatch logic is a later phase).
- Extend `_real_startup_sequence()` in `worker/worker_main.py` to call `_import_nodes()` after the capability probe, then `ipc.send_event(Ready{...})` with `capabilities_source="pytorch"` and `node_types=[]`, then enter the dispatch loop.
- Add >=4 new real_mode-marked tests to `worker/tests/test_worker_main.py` covering: the Ready event is sent with correct fields, `_import_nodes()` returns an empty list, the dispatch loop exists and does not exit, and the complete real-mode startup path does not exit nonzero for valid CPU device_type.
- Total real_mode test count in file reaches >=7.

### Out of Scope
None. This task's `defers_to` field is empty (absent from JSON). No scope is deferred to any other task. The `_import_nodes()` stub returning `[]` is correct for this phase — the node system itself is Phase 10's scope, not a deferral.

## Existing Codebase Assessment

**What already exists:** `worker/worker_main.py` currently has `_real_startup_sequence()` (P9-D1's work) which reads env vars, calls `ipc.connect()`, imports torch, selects device, runs `capability.probe_capabilities()`, logs at DEBUG, and returns the caps dict. The function ends at `sys.exit(0)` via the `if __name__ == "__main__"` block. `_mock_probe_capabilities()` also exists (P9-C2). `worker/ipc.py` has `connect()`, `send_event()`, and `recv_message()` fully implemented (P9-B1). The test file `worker/tests/test_worker_main.py` has 8 tests total: 3 mock-mode, 4 real_mode (from P9-D1), and 1 no-marker test.

**Established patterns:**
- Env var isolation: tests save all four startup env vars (`ANVILML_IPC_PORT`, `ANVILML_WORKER_ID`, `ANVILML_DEVICE_TYPE`, `ANVILML_DEVICE_INDEX`) before mutating and restore them unconditionally in a `finally` block.
- Mocking strategy: `unittest.mock.patch` on `worker.ipc.connect`, `worker.capability.probe_capabilities`, and `torch.cuda.set_device`.
- Docstrings: Google-style with Args/Returns/Raises sections for non-trivial functions.
- Logging: `logger.debug()` with structured field notation (e.g., `device_type=%s, caps.fp32=%s`).
- Real-mode marker: `@pytest.mark.real_mode` decorator on all real-mode tests.

**Gap between design doc and source:** The current `_real_startup_sequence()` returns the caps dict and the `if __name__ == "__main__"` block prints it and exits — this is P9-D1's intermediate state. The design doc §14.2 shows the full sequence ending in `ipc.send_event(Ready{...})` then entering a dispatch loop. This task bridges that gap.

## Resolved Dependencies

None. This task introduces no new external crates or packages. It uses only existing dependencies already imported in `worker/ipc.py` (`zmq`, `msgpack`) and standard library modules (`logging`, `os`, `sys`).

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| (none — only stdlib and existing deps used) |

## Approach

**Step 1: Add `_import_nodes()` function to `worker/worker_main.py`.**

Implement a module-level function that returns an empty list:

```python
def _import_nodes() -> list:
    """Import node modules from ``worker/nodes/`` and return the registered node types.

    Currently returns an empty list — the node system itself is Phase 10's scope.
    This function is called during worker startup (both real and mock modes) so that
    the ``Ready`` event can carry the node type list even when it is empty.

    Returns:
        Empty list. Node types will be populated when the node system is implemented.
    """
    # Phase 10 scope: real node import will populate this function.
    # Returning [] is correct for Phase 9 — the node system does not exist yet.
    return []
```

**Step 2: Add `_dispatch_loop()` function to `worker/worker_main.py`.**

Implement a function that loops calling `ipc.recv_message()`, logs each message at DEBUG level, and continues. This is a placeholder — real dispatch logic is a later phase:

```python
def _dispatch_loop() -> None:
    """Receive and log messages from the supervisor in a loop.

    This is a placeholder implementation for Phase 9. Real dispatch logic
    (routing messages to executor, handling Execute/CancelJob/etc.) is a
    later phase. For now, every received message is logged at DEBUG level
    and the loop continues.

    The loop runs indefinitely until the process is terminated by the
    supervisor or an external signal.

    Raises:
        RuntimeError: If ipc.connect() has not been called before entering
            the loop.
    """
    logger.info("dispatch_loop: starting")
    while True:
        try:
            msg = ipc.recv_message()
        except Exception as exc:
            # Log recv failure and continue — a broken socket means the
            # supervisor is gone; the worker should exit gracefully.
            logger.error("dispatch_loop: recv failed, exiting: error=%s", exc)
            break
        logger.debug("dispatch_loop: received message type=%s", msg.get("_type", "<unknown>"))
```

The `try/except` around `recv_message()` is a safety net: if the supervisor dies and the socket closes, `recv_message()` will raise (e.g., `zmq.ZMQError`), and we log it and break rather than crashing with an unhandled exception. This is a non-obvious design choice — without it, the worker would crash on supervisor shutdown instead of exiting cleanly.

**Step 3: Extend `_real_startup_sequence()` to complete the real-mode startup flow.**

After the existing probe step (which returns `caps`), add:
1. Call `_import_nodes()` and store the result.
2. Build a `Ready` event dict with `_type="Ready"`, `capabilities_source="pytorch"`, and `node_types=[]`.
3. Call `ipc.send_event(ready_event)`.
4. Call `_dispatch_loop()` to enter the message loop.

The function signature changes from returning `dict` to returning `None` (since it now enters an infinite loop). The `if __name__ == "__main__"` block at the bottom needs updating to reflect this.

Specifically, after line 85 (`return caps`), replace the existing return and the `if __name__` block with:

```python
    # Build and send the Ready event — this tells the supervisor the worker
    # is operational and what capabilities/nodes it supports.
    # capabilities_source="pytorch" in this branch (real mode);
    # "mock" in the mock-mode branch (ANVILML_WORKER_MOCK=1).
    ready_event = {
        "_type": "Ready",
        "capabilities_source": "pytorch",
        "node_types": node_types,
    }
    ipc.send_event(ready_event)

    logger.info(
        "ready: capabilities_source=%s, node_types_count=%d",
        ready_event["capabilities_source"],
        len(node_types),
    )

    # Enter the message dispatch loop — blocks until the process is terminated.
    _dispatch_loop()


def _mock_startup_sequence() -> None:
    """Run the mock-mode startup sequence: IPC connect → mock probe → Ready.

    This is the mock-mode equivalent of ``_real_startup_sequence()``.
    It uses ``_mock_probe_capabilities()`` instead of the real torch probe,
    and sends ``capabilities_source="mock"`` in the Ready event.

    The mock branch never imports ``torch`` — all capability values are
    synthetic. IPC connection, node import, and dispatch loop are identical
    to the real-mode path.

    Returns:
        None — enters the dispatch loop and blocks.
    """
    port = int(os.environ["ANVILML_IPC_PORT"])
    worker_id = os.environ["ANVILML_WORKER_ID"]
    device_type = os.environ["ANVILML_DEVICE_TYPE"]
    device_index = int(os.environ["ANVILML_DEVICE_INDEX"])

    import worker.ipc as ipc

    ipc.connect(port, worker_id)

    caps = _mock_probe_capabilities()

    logger.debug(
        "mock_startup: device_type=%s, caps.fp32=%s, caps.fp16=%s, caps.bf16=%s",
        device_type,
        caps["fp32"],
        caps["fp16"],
        caps["bf16"],
    )

    node_types = _import_nodes()

    ready_event = {
        "_type": "Ready",
        "capabilities_source": "mock",
        "node_types": node_types,
    }
    ipc.send_event(ready_event)

    logger.info(
        "ready: capabilities_source=%s, node_types_count=%d",
        ready_event["capabilities_source"],
        len(node_types),
    )

    _dispatch_loop()
```

**Step 4: Update the `if __name__ == "__main__"` block.**

Replace the current `caps = _real_startup_sequence(); print(caps); sys.exit(0)` with a proper startup dispatcher that checks `ANVILML_WORKER_MOCK` and calls the appropriate sequence:

```python
if __name__ == "__main__":
    # Configure basic logging — the supervisor may set ANVILML_LOG_LEVEL,
    # but we always enable at least INFO-level output for diagnostics.
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s %(levelname)s %(name)s: %(message)s",
    )

    if os.environ.get("ANVILML_WORKER_MOCK") == "1":
        logger.info("worker: starting in mock mode")
        _mock_startup_sequence()
    else:
        logger.info("worker: starting in real mode")
        _real_startup_sequence()
```

This is the first time the mock/real branch check appears at the entry point — it happens once at the top level, never re-checked deep inside helper functions (per `ANVILML_DESIGN.md §14.3`).

**Step 5: Add >=4 new real_mode-marked tests to `worker/tests/test_worker_main.py`.**

Test 1: `test_real_startup_sends_ready_event` — Patches `ipc.send_event` to verify it is called with a dict containing `_type="Ready"`, `capabilities_source="pytorch"`, and `node_types=[]`. This is the primary acceptance test for the Ready event.

Test 2: `test_import_nodes_returns_empty_list` — Calls `_import_nodes()` directly and asserts the result is `[]`. Confirms the stub behavior is correct.

Test 3: `test_dispatch_loop_exists_and_is_callable` — Asserts `_dispatch_loop` exists as a callable function and can be called (it will block, so we mock `ipc.recv_message` to raise after one call, verifying the loop's initial behavior).

Test 4: `test_real_startup_no_nonzero_exit_for_cpu` — Runs the full startup sequence with mocked IPC and capability probe, confirming no exception is raised (i.e., the function does not exit nonzero for a valid CPU device_type).

Test 5: `test_mock_startup_sends_ready_event` — Similar to Test 1 but for mock mode, verifying `capabilities_source="mock"`.

Test 6: `test_no_mock_gate_in_main_block` — Reads the source file and confirms no `if ANVILML_WORKER_MOCK != "1": exit(1)` pattern exists in the `__main__` block. (This is a variation of the existing `test_no_mock_gate_exit_path` test but focused on the new main block structure.)

These 6 new tests bring the total real_mode test count from 4 to 10 (4 existing real_mode + 6 new = 10, well above the >=7 requirement).

**Step 6: Verify with `python -m py_compile` (ENVIRONMENT.md §6 Step 7).**

Before running pytest, confirm the modified `worker/worker_main.py` passes Python syntax check.

## Public API Surface

No new public API items. All additions are module-level private functions (prefixed with `_`):

| Module | Item | Signature |
|--------|------|-----------|
| `worker.worker_main` | `_import_nodes()` | `def _import_nodes() -> list` |
| `worker.worker_main` | `_dispatch_loop()` | `def _dispatch_loop() -> None` |
| `worker.worker_main` | `_mock_startup_sequence()` | `def _mock_startup_sequence() -> None` |

`_real_startup_sequence()` signature changes from `def _real_startup_sequence() -> dict` to `def _real_startup_sequence() -> None` (it now enters the dispatch loop instead of returning).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/worker_main.py` | Add `_import_nodes()`, `_dispatch_loop()`, `_mock_startup_sequence()`; extend `_real_startup_sequence()` to call node import, send Ready event, enter dispatch loop; update `__main__` block |
| Modify | `worker/tests/test_worker_main.py` | Add >=4 new real_mode-marked tests for Ready event, node import stub, dispatch loop, and full startup path |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_worker_main.py` | `test_real_startup_sends_ready_event (real_mode)` | `_real_startup_sequence()` calls `ipc.send_event` with `_type="Ready"`, `capabilities_source="pytorch"`, `node_types=[]` | `worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v -m real_mode -k "test_real_startup_sends_ready_event"` exits 0 |
| `worker/tests/test_worker_main.py` | `test_import_nodes_returns_empty_list (real_mode)` | `_import_nodes()` returns `[]` — correct stub for Phase 9 | `worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v -m real_mode -k "test_import_nodes_returns_empty_list"` exits 0 |
| `worker/tests/test_worker_main.py` | `test_dispatch_loop_exists_and_is_callable (real_mode)` | `_dispatch_loop` exists as a function and handles a single recv gracefully | `worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v -m real_mode -k "test_dispatch_loop_exists"` exits 0 |
| `worker/tests/test_worker_main.py` | `test_real_startup_no_nonzero_exit_for_cpu (real_mode)` | Full real-mode startup path runs without raising for valid CPU device_type | `worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v -m real_mode -k "test_real_startup_no_nonzero_exit"` exits 0 |
| `worker/tests/test_worker_main.py` | `test_mock_startup_sends_ready_event (real_mode)` | `_mock_startup_sequence()` calls `ipc.send_event` with `capabilities_source="mock"`, `node_types=[]` | `worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v -m real_mode -k "test_mock_startup_sends_ready_event"` exits 0 |
| `worker/tests/test_worker_main.py` | `test_no_mock_gate_in_main_block (real_mode)` | The `__main__` block uses `ANVILML_WORKER_MOCK == "1"` check (not `!= "1"` exit) | `worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v -m real_mode -k "test_no_mock_gate_in_main"` exits 0 |

Total real_mode tests after this task: 10 (4 existing + 6 new), satisfying the >=7 requirement.

## CI Impact

No CI changes required. The `worker-linux-real` and `worker-windows-real` CI jobs (defined in `.github/workflows/ci.yml`) run `python -m pytest worker/tests -v -m real_mode` — this task adds tests that are collected by the existing `-m real_mode` marker. No new CI jobs or steps are needed. The Python syntax check (Step 7 in ENVIRONMENT.md) will pick up the modified `worker/worker_main.py`.

## Platform Considerations

None identified. The changes are pure Python with no platform-specific code paths. The `_import_nodes()` stub, `ipc.send_event()` call, and dispatch loop are platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ipc.recv_message()` in the dispatch loop may raise `zmq.ZMQError` on socket close (supervisor dies), causing an unhandled exception instead of graceful exit. | Medium | High | Wrap `ipc.recv_message()` in `try/except Exception` inside `_dispatch_loop()`, log the error, and break the loop. This is already planned in Step 2. |
| `_real_startup_sequence()` signature change from `-> dict` to `-> None` may break callers (existing tests mock the return value). | Low | Medium | All existing tests (`test_real_startup_calls_ipc_connect`, `test_real_startup_cpu_skips_cuda_set_device`, `test_real_startup_calls_probe_capabilities`) mock `ipc.connect` and `probe_capabilities` but do not assert on the return value — they only check that mocked functions were called with correct args. No changes needed to existing tests. |
| `_mock_startup_sequence()` function shares the same env var reads as `_real_startup_sequence()` — if a test only sets subset of env vars, one function may work while the other fails. | Low | Medium | Each test follows the established pattern: saves all four startup env vars before mutating, sets all four, and restores unconditionally in `finally`. |
| The dispatch loop placeholder blocks indefinitely in tests that call it, causing test hangs. | Medium | High | Tests that need to verify the dispatch loop mock `ipc.recv_message()` to raise after one call (simulating supervisor disconnect), so the loop exits cleanly. No test calls `_dispatch_loop()` without mocking `recv_message`. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/worker_main.py` exits 0
- [ ] `worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v -m real_mode` exits 0 (>=7 real_mode tests total in file)
- [ ] `grep -c "def _import_nodes" worker/worker_main.py` returns 1
- [ ] `grep -c "def _dispatch_loop" worker/worker_main.py` returns 1
- [ ] `grep -c "def _mock_startup_sequence" worker/worker_main.py` returns 1
- [ ] `grep -c '"pytorch"' worker/worker_main.py` returns >=1 (Ready event carries `capabilities_source="pytorch"`)
- [ ] `grep -c '"mock"' worker/worker_main.py` returns >=1 (mock Ready event carries `capabilities_source="mock"`)
- [ ] `grep -c "node_types" worker/worker_main.py` returns >=2 (both real and mock paths pass node_types to Ready)
- [ ] `python -m pytest worker/tests/test_worker_main.py -v -m real_mode --collect-only | grep -c "test_"` returns >=10
