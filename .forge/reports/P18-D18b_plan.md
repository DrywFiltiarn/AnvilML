# Plan Report: P18-D18b

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P18-D18b                                          |
| Phase       | 018 — ZiT Generic Nodes                           |
| Description | worker/nodes/arch/zit.py: callback_on_step_end adapter for progress and cancellation |
| Depends on  | P18-D18a                                          |
| Project     | anvilml                                           |
| Planned at  | 2026-06-23T12:45:00Z                              |
| Attempt     | 1                                                 |

## Objective

Implement a private `_make_callback` adapter in `worker/nodes/arch/diffusion/zit.py` that bridges `diffusers`' real `callback_on_step_end` signature `(self, i, t, callback_kwargs) -> dict` to the simpler 2-argument `emit_progress(step, total)` interface that `sample()` exposes to the rest of the codebase. The adapter calls `emit_progress` per step, checks a cancellation flag, and raises a module-private `_SamplingCancelled` sentinel if cancelled. This enables P18-D18c to wire the callback into `ZImagePipeline.__call__` for cooperative cancellation and progress reporting.

## Scope

### In Scope
- Add `_SamplingCancelled` as a module-private exception class in `worker/nodes/arch/diffusion/zit.py`.
- Add `_make_callback(emit_progress, cancel_flag, total_steps) -> Callable` private helper in `worker/nodes/arch/diffusion/zit.py`.
- Add two unit tests in `worker/tests/test_arch_zit.py`: one verifying `emit_progress` is called with correct `(step, total)` values, one verifying cancellation raises `_SamplingCancelled`.

### Out of Scope
None. `defers_to (from JSON): absent`. This task implements its full scope; the adapter being unused until P18-D18c wires it in is ordinary sequencing, not deferred scope.

## Existing Codebase Assessment

The `worker/nodes/arch/diffusion/zit.py` module already contains `can_handle()`, `compute_latent_shape()`, `MockLatent`, and `VAE_SCALE_FACTOR`. The `sample()` function assembles a `ZImagePipeline` via `pipeline_cache.get_or_load()` and currently raises `NotImplementedError` with a `# defers_to: P18-D18c` comment at the stub site. The module docstring already describes the callback shape mismatch between diffusers' API and `sample()`'s public interface.

The test file `worker/tests/test_arch_zit.py` follows a consistent pattern: mock-mode tests use the autouse `conftest.py` fixture, real-mode tests temporarily override `ANVILML_WORKER_MOCK` with capture-and-restore in a `finally` block, and all tests use `type("Class", (), {...})()` for lightweight mock objects. The module's `__all__` exports only public symbols (no underscore-prefixed names).

The design doc specifies `NodeContext.cancel_flag` as a `threading.Event` (line 1550 of `ANVILML_DESIGN.md`). The `base.py` source types it as `Any` but the docstring matches. Existing tests pass `cancel_flag=[False]` (a list), which does not have `.is_set()` — this discrepancy must be resolved at ACT time by using `threading.Event` in the new tests to match the design doc specification.

## Resolved Dependencies

None. This task introduces no new external dependencies. It uses only Python standard library (`threading`, `typing.Callable`) and the existing `diffusers` package (already a dependency).

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| None   | —       | —               | —              | —                      |

## Approach

1. **Add `_SamplingCancelled` sentinel exception class** to `worker/nodes/arch/diffusion/zit.py`, placed after the `VAE_SCALE_FACTOR` constant and before `compute_latent_shape()`. This is a module-private exception (underscore-prefixed, not in `__all__`) that the diffusers pipeline will propagate when the callback raises it during step execution. P18-D18c will catch this in a `try/except _SamplingCancelled` block around the pipeline call.

2. **Add `_make_callback()` private helper** to `worker/nodes/arch/diffusion/zit.py`, placed after `_SamplingCancelled` and before `MockLatent`. The function signature is:
   ```python
   def _make_callback(
       emit_progress: Callable[[int, int], None],
       cancel_flag: Any,
       total_steps: int,
   ) -> Callable[[Any, int, int, dict[str, Any]], dict[str, Any]]:
   ```
   
   Implementation details:
   - Returns a closure with the signature `(self, i, t, callback_kwargs) -> dict` matching `diffusers`' `callback_on_step_end` API.
   - The closure body: (a) calls `emit_progress(i, total_steps)` to report progress; (b) checks `cancel_flag.is_set()` — using the `threading.Event` API as specified in the design doc (`ANVILML_DESIGN.md §1550`); (c) if cancelled, raises `_SamplingCancelled`; (d) returns `callback_kwargs` unchanged on the non-cancelled path.
   - The `self` parameter is accepted for API compatibility with diffusers but unused (diffusers passes the pipeline instance as `self`; the adapter doesn't need it).
   - The `t` (timestamp) parameter is accepted but unused — diffusers passes the current time, but the adapter only needs the step index for progress reporting.
   - Inline comments explain: (a) why the closure accepts `self` but ignores it; (b) why `cancel_flag.is_set()` is used (design doc specifies `threading.Event`); (c) why `callback_kwargs` is returned unchanged (diffusers expects the callback to return the kwargs dict; returning it unmodified means diffusers proceeds with its internal state unchanged).

3. **Add test `test_make_callback_emits_progress`** to `worker/tests/test_arch_zit.py`. This test:
   - Uses mock `emit_progress` (a `list` accumulator) and a `threading.Event` as `cancel_flag` (not unset, so no cancellation).
   - Calls `_make_callback(emit_progress, cancel_flag, total_steps=4)` to obtain the closure.
   - Invokes the closure with `(None, 0, None, {})` — `self=None`, `i=0`, `t=None`, `callback_kwargs={}`.
   - Asserts `emit_progress` was called exactly once with `(0, 4)`.
   - Asserts the return value equals `{}` (unchanged callback_kwargs).
   - Follows the existing test style with docstring documenting preconditions, tests, and expected output.

4. **Add test `test_make_callback_raises_on_cancellation`** to `worker/tests/test_arch_zit.py`. This test:
   - Creates a `threading.Event()` and sets it before calling the callback (simulating a cancellation request).
   - Calls `_make_callback(emit_progress, cancel_flag, total_steps=4)` to obtain the closure.
   - Invokes the closure with `(None, 2, None, {})`.
   - Asserts `emit_progress` was called with `(2, 4)` (progress is emitted before cancellation check).
   - Asserts `_SamplingCancelled` is raised.
   - Follows the same test style as above.

5. **No changes to `__all__`** — `_make_callback` and `_SamplingCancelled` are private (underscore-prefixed) and must not be exported.

## Public API Surface

None. Both `_make_callback` and `_SamplingCancelled` are module-private (underscore-prefixed) and are not added to `__all__`. They are internal implementation details consumed only by P18-D18c within the same module.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/zit.py` | Add `_SamplingCancelled` sentinel class and `_make_callback()` private helper |
| MODIFY | `worker/tests/test_arch_zit.py` | Add two unit tests for the adapter |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_arch_zit.py` | `test_make_callback_emits_progress` | `_make_callback()` returns a closure that calls `emit_progress(step, total)` and returns `callback_kwargs` unchanged when not cancelled. | `ANVILML_WORKER_MOCK=1` (conftest.py autouse fixture). | `emit_progress` = list accumulator; `cancel_flag` = unset `threading.Event`; `total_steps=4`; closure called with `i=0, t=None, callback_kwargs={}`. | `emit_progress` called once with `(0, 4)`; return value is `{}`. | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_make_callback_raises_on_cancellation` | `_make_callback()` returns a closure that calls `emit_progress` then raises `_SamplingCancelled` when `cancel_flag.is_set()` is True. | `ANVILML_WORKER_MOCK=1` (conftest.py autouse fixture). | `emit_progress` = list accumulator; `cancel_flag` = `threading.Event` with `.set()` called before callback invocation; `total_steps=4`; closure called with `i=2, t=None, callback_kwargs={}`. | `emit_progress` called once with `(2, 4)`; `_SamplingCancelled` raised. | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v` exits 0 |

## CI Impact

No CI changes required. The tests run under the existing `worker-linux` and `worker-windows` CI jobs which already execute `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v`. No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The `threading.Event` API and `Callable` type are cross-platform. The callback adapter has no file I/O, no path handling, and no platform-specific behavior. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `cancel_flag` type mismatch: design doc specifies `threading.Event` but existing tests pass `[False]` (a list) which lacks `.is_set()`. The new tests must use `threading.Event` to match the spec. | Low | Medium | Use `threading.Event` in the new tests per the design doc specification. Document the discrepancy in the approach; the ACT agent confirms at session start. |
| `diffusers` callback_kwargs mutability: if `ZImagePipeline.__call__` modifies `callback_kwargs` in-place before passing it to the callback, returning it unchanged is correct; if it expects the callback to return a modified copy, the adapter needs to copy. | Low | Low | Return `callback_kwargs` unchanged (the design doc says "Returns callback_kwargs unchanged"). At ACT time, confirm against the installed `diffusers` source; if modification is needed, return `dict(callback_kwargs)`. |
| `_make_callback` closure captures `cancel_flag` by reference: if the caller replaces the event object between calls, only the original object is checked. | Low | Low | This is the expected behavior — `cancel_flag` is a single `threading.Event` shared across all steps of a single sampling run. Document this in the inline comment. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m py_compile worker/nodes/arch/diffusion/zit.py worker/tests/test_arch_zit.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v` exits 0
