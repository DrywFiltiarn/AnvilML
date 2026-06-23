# Plan Report: P904-A5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P904-A5                                     |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects) |
| Description | worker/nodes/arch/diffusion/zit.py + worker/worker_main.py: reconcile cancel_flag type contract (threading.Event vs list[bool]) |
| Depends on  | P18-D18b, P904-A4                           |
| Project     | anvilml                                     |
| Planned at  | 2026-06-23T19:50:00Z                        |
| Attempt     | 1                                           |

## Objective

Fix the `cancel_flag` type mismatch in `worker/worker_main.py`: it constructs a `list[bool]` (`_cancel_flag: list[bool] = [False]`) but `zit.py`'s `_make_callback()` calls `.is_set()` on it, which requires a `threading.Event`. This causes an `AttributeError` on the first real denoising step. The fix replaces the `list[bool]` with `threading.Event()` in `worker_main.py` only — `threading.Event` is the documented contract per `ANVILML_DESIGN.md §1550` and `zit.py`'s own docstring.

## Scope

### In Scope
- `worker/worker_main.py`: Replace `_cancel_flag: list[bool] = [False]` with `_cancel_flag = threading.Event()` (line 48).
- `worker/worker_main.py`: Replace `_cancel_flag[0] = False` with `_cancel_flag.clear()` (line 238, inside the Execute handler).
- `worker/worker_main.py`: Replace `_cancel_flag[0] = True` with `_cancel_flag.set()` (line 280, inside the CancelJob handler).
- `worker/worker_main.py`: Add `import threading` at the top of the file if not already present.
- Update the module-level docstring comment on line 44–47 to reflect the new type.
- `worker/tests/test_worker_main.py`: Add one test (`test_cancel_flag_is_threading_event`) that verifies `_cancel_flag` is a `threading.Event` by inspecting the worker subprocess's source.

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope. `zit.py` is not modified; it already uses the correct `threading.Event` contract. No documentation changes are needed.

## Existing Codebase Assessment

`worker_main.py` (299 lines) is the Python worker entry point spawned by the Rust supervisor. At module level it defines `_cancel_flag` (line 48) as a `list[bool] = [False]` — a mutable container chosen so that the `CancelJob` handler and the `Execute` handler can share state without `global` declarations. The `Execute` handler resets it with `_cancel_flag[0] = False` before each job, and the `CancelJob` handler sets it with `_cancel_flag[0] = True`.

`zit.py`'s `_make_callback()` (lines 59–120) builds a `callback_on_step_end` adapter for `diffusers`' `ZImagePipeline`. On each denoising step, it calls `cancel_flag.is_set()` (line 112), which is a `threading.Event` method. The docstring at lines 73–75 explicitly states: *"The `cancel_flag` is expected to be a `threading.Event` (as specified in `ANVILML_DESIGN.md §1550`); `.is_set()` is used to check whether cancellation was requested."*

`ANVILML_DESIGN.md` line 1566 confirms: `cancel_flag: threading.Event; set when the job is cancelled.` — this is the authoritative contract. The `list[bool]` in `worker_main.py` is the side that diverges from the documented design. The defect is latent because mock-mode tests never reach the real-mode `sample()` path in `zit.py`, and mock mode returns early without calling `_make_callback`.

The existing test suite in `worker/tests/test_worker_main.py` (469 lines) exercises the worker subprocess via IPC (Ready, Ping/Pong, Shutdown, env vars, pipeline cache reuse) but does not exercise the cancel flag path — no test sends a `CancelJob` message and no test inspects the cancel flag type.

Established patterns in `worker_main.py`: module-level docstrings use Google style; comments at decision points explain the *why*; imports follow `from __future__ import annotations` at the top; the file uses `os`, `sys`, `time` for standard library and imports from `worker.*` for internal modules.

## Resolved Dependencies

None. `threading` is a Python standard library module (part of the language runtime, not an external package). No version resolution or MCP lookup is required.

| Type   | Name    | Version verified | MCP source | Feature flags confirmed |
|--------|---------|-----------------|------------|------------------------|
| stdlib | threading | Python 3.12 (built-in) | n/a | n/a |

## Approach

1. **Read `worker/worker_main.py`** and confirm the three locations that reference `_cancel_flag`: line 48 (declaration), line 238 (`_cancel_flag[0] = False`), line 280 (`_cancel_flag[0] = True`). Confirm no other references exist by grepping for `_cancel_flag` in the file.

2. **Add `import threading`** at the top of `worker/worker_main.py`, after the existing `import time` line (line 35) and before the `from worker.ipc import ...` imports (line 37). This follows the established convention of standard library imports grouped together before third-party/project imports.

3. **Replace the module-level declaration** (line 48): change `_cancel_flag: list[bool] = [False]` to `_cancel_flag = threading.Event()`. Remove the type annotation since `threading.Event` is a concrete class and the assignment is explicit.

4. **Update the module-level comment** (lines 44–47): change the comment block from *"A list is used (mutable container) so it can be modified from the CancelJob handler without needing global state"* to *"A `threading.Event` is used so the CancelJob handler can signal cancellation to any thread checking the flag. The `.set()` / `.clear()` API is used instead of list indexing."*

5. **Replace `_cancel_flag[0] = False`** (line 238, inside the Execute handler) with `_cancel_flag.clear()`. This resets the cancel flag for the new job execution, matching the original intent of setting the flag to `False`.

6. **Replace `_cancel_flag[0] = True`** (line 280, inside the CancelJob handler) with `_cancel_flag.set()`. This signals cancellation to any thread checking the flag, matching the original intent of setting the flag to `True`.

7. **Update the NodeContext construction comment** (lines 229–232): change *"The cancel_flag is a list (mutable container) so nodes can check and set it during long-running operations"* to *"The cancel_flag is a `threading.Event` — nodes check `.is_set()` during long-running operations."*

8. **Verify no other `_cancel_flag` references** exist by running `grep -n '_cancel_flag' worker/worker_main.py` and confirming only the four expected lines remain (import, declaration, clear, set).

9. **Run the existing test suite** (`ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v`) to confirm no regressions. The existing tests do not exercise the cancel flag path, so they should pass unchanged.

10. **Add a verification test** in `worker/tests/test_worker_main.py`: `test_cancel_flag_is_threading_event` — spawns the worker subprocess, inspects the `_cancel_flag` attribute of the `worker.worker_main` module to confirm it is a `threading.Event` instance (not a list).

## Public API Surface

No new public API items are introduced. The task modifies only private module-level state (`_cancel_flag`) and its usage within `worker_main.py`. The `NodeContext` class (in `worker/nodes/base.py`) already accepts `cancel_flag: Any` — no signature change is needed on that side.

| Item | Type | Path | Change |
|------|------|------|--------|
| `_cancel_flag` | private module variable | `worker/worker_main.py` | `list[bool]` → `threading.Event` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/worker_main.py` | Replace `list[bool]` cancel flag with `threading.Event()`, update all usages and comments |
| MODIFY | `worker/tests/test_worker_main.py` | Add `test_cancel_flag_is_threading_event` verification test |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_worker_main.py` | `test_cancel_flag_is_threading_event` | `_cancel_flag` in `worker.worker_main` is a `threading.Event` instance (not a `list[bool]`) | Worker subprocess can be imported; `threading` available | Import `worker.worker_main`, inspect `worker.worker_main._cancel_flag` | `isinstance(_cancel_flag, threading.Event)` is `True` | `python3 -c "import threading; from worker.worker_main import _cancel_flag; assert isinstance(_cancel_flag, threading.Event)"` exits 0 |
| `worker/tests/test_worker_main.py` | (existing tests) | No regressions from the cancel_flag type change | `ANVILML_WORKER_MOCK=1`, venv with pyzmq/msgpack | Full test suite | All existing tests pass | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v` exits 0 |

## CI Impact

No CI changes required. The task modifies only `worker/worker_main.py` (production code) and `worker/tests/test_worker_main.py` (existing test file). Both are already collected by the `worker` CI job (`ANVILML_WORKER_MOCK=1 <matrix-python> -m pytest worker/tests -v`). No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. `threading.Event()` is a cross-platform standard library primitive that behaves identically on Linux and Windows. The `.set()` / `.clear()` / `.is_set()` methods have the same semantics on all platforms. No `#[cfg(...)]` guards, path separators, or line-ending handling are involved.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| A test or code path outside this task's files references `_cancel_flag` directly (e.g., a test in `test_arch_zit.py` that inspects the cancel flag type) | Low | Medium | The grep for `_cancel_flag` across all `worker/` files at step 1 will surface any other references. If found, assess whether they expect `list[bool]` and fix them too. |
| The `threading.Event` approach changes cancellation semantics — `threading.Event` is a one-shot flag (`.set()` is sticky, `.clear()` resets only explicitly), while `list[0]=True` was also sticky — the semantics are actually identical, but a reader might assume `threading.Event` behaves differently | Low | Low | The updated comment at line 44–47 explicitly states the semantics. The `.clear()` call in the Execute handler resets the flag for each new job, preserving the original per-job reset behavior. |
| The `threading.Event` type annotation on `NodeContext.cancel_flag` (in `worker/nodes/base.py`) says `Any` — after this fix, it is known to be `threading.Event`, but changing that annotation is out of scope for this task | Low | Low | No action needed. `Any` is intentionally permissive to accept any object with an `.is_set()` method. Changing the annotation would be a separate task. |

## Acceptance Criteria

- [ ] `python3 -c "import threading; from worker.worker_main import _cancel_flag; assert isinstance(_cancel_flag, threading.Event)"` exits 0
- [ ] `grep -n '_cancel_flag\[0\]' worker/worker_main.py` returns no hits
- [ ] `grep -n '_cancel_flag = threading.Event()' worker/worker_main.py` returns exactly one match (line 48)
- [ ] `grep -n '_cancel_flag.clear()' worker/worker_main.py` returns exactly one match (line 238)
- [ ] `grep -n '_cancel_flag.set()' worker/worker_main.py` returns exactly one match (line 280)
- [ ] `grep -n 'import threading' worker/worker_main.py` returns exactly one match (new import line)
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v` exits 0
- [ ] `python3 -m py_compile worker/worker_main.py` exits 0
