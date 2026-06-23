# Plan Report: P904-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P904-A4                                     |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects) |
| Description | worker/nodes/sampler.py: EmptyLatent real path references unbound name ctx instead of self.ctx |
| Depends on  | P18-D17, P904-A3                            |
| Project     | anvilml                                     |
| Planned at  | 2026-06-23T19:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Fix a one-line `NameError` in `EmptyLatent.execute()`'s real-mode branch: the code references a bare `ctx.device` where the `NodeContext` is available as `self.ctx` (inherited from `BaseNode.__init__`). This is the only change required — no surrounding logic, dispatch, or shape-computation code is touched.

## Scope

### In Scope
- Change `device=ctx.device` to `device=self.ctx.device` on line 183 of `worker/nodes/sampler.py` inside `EmptyLatent.execute()`'s real-mode branch.
- Verify the fix with `grep -n "device=self.ctx.device" worker/nodes/sampler.py` (at least one match inside EmptyLatent.execute).
- Verify the bug is gone with `grep -n "device=ctx.device" worker/nodes/sampler.py` (zero matches).

### Out of Scope
None. `defers_to (from JSON): absent`. No scope is deferred. The task's `context` says "confirm at ACT time" — that means verify during implementation and then confirm the fix is correct; it is not permission to stub.

## Existing Codebase Assessment

`worker/nodes/sampler.py` defines two `@register`ed node classes: `EmptyLatent` (line 67) and `Sampler` (line 187). Both inherit from `BaseNode` (`worker/nodes/base.py`), which sets `self.ctx = ctx` in its `__init__` (line 199). The `NodeContext` provides `.device`, `.job_id`, `.cancel_flag`, `.emit()`, and `.pipeline_cache`.

The established pattern in this file is clear: `Sampler.execute()` correctly uses `self.ctx.device` on line 331 when passing the device string to the architecture module. `EmptyLatent.execute()` follows the same structural pattern (read inputs → check mock mode → dispatch to arch → compute shape → create tensor) but incorrectly writes `ctx.device` (bare name) instead of `self.ctx.device`. This is a copy-paste or typo defect, not a design issue.

No gap exists between the design doc and current source beyond this one bug. The arch dispatch logic (`arch.get_module(model)`) and shape computation (`mod.compute_latent_shape(...)`) are correct and should not be touched.

## Resolved Dependencies

None. This task introduces no new dependencies and references no external crate or package APIs. It only changes a Python attribute access from an unbound name to a bound instance attribute.

## Approach

1. Open `worker/nodes/sampler.py`. Navigate to line 182–184, inside `EmptyLatent.execute()`'s real-mode branch (the `torch.randn(...)` call).
2. Change line 183 from `device=ctx.device` to `device=self.ctx.device`. The surrounding lines (182–184) change from:
   ```python
   return {"latent": torch.randn(
       shape, dtype=torch.float32, device=ctx.device
   )}
   ```
   to:
   ```python
   return {"latent": torch.randn(
       shape, dtype=torch.float32, device=self.ctx.device
   )}
   ```
3. Verify no other bare `ctx` references remain in the file: run `grep -n "device=ctx\." worker/nodes/sampler.py` — must return zero matches.
4. Verify the fix is present: run `grep -n "device=self\.ctx\.device" worker/nodes/sampler.py` — must return at least one match (line 183, and line 331 from Sampler).

Rationale: This is a direct substitution of an unbound name for the correct bound instance attribute. No surrounding code changes are needed because `self` is always available in instance methods, and `self.ctx` was already correctly used elsewhere in the same file (Sampler at line 331).

## Public API Surface

None. This task modifies only a private implementation detail (an attribute access inside `execute()`). No public signatures, class attributes, or module-level exports change.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/sampler.py` | Change `device=ctx.device` to `device=self.ctx.device` in EmptyLatent.execute() (line 183) |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/nodes/sampler.py` | (grep verification) | The bare `ctx.device` reference is eliminated and replaced with `self.ctx.device` | File exists and is readable | N/A | grep returns zero matches for `device=ctx.device` | `grep -n "device=ctx\.device" worker/nodes/sampler.py; echo $?` exits 1 (no match) |
| `worker/nodes/sampler.py` | (grep verification) | The corrected `self.ctx.device` reference is present in EmptyLatent.execute() | File exists and is readable | N/A | grep returns at least one match | `grep -n "device=self\.ctx\.device" worker/nodes/sampler.py` exits 0 with at least one match |

Note: The existing mock-mode test suite (`test_nodes_sampler.py`) cannot exercise this real-mode path because `conftest.py` forces `ANVILML_WORKER_MOCK=1` for every test. The real-mode path in `EmptyLatent.execute()` is unreachable under mock mode, so no existing test will break or pass based on this change. The grep-based acceptance criteria are the appropriate verification mechanism for this defect-class, consistent with how other P904 Group A tasks validate their fixes.

## CI Impact

No CI changes required. This task modifies only one source file with a single-character-level fix (`ctx` → `self.ctx`). No new test files, no new configuration, no new CI gates. The existing `worker` CI job's `pytest worker/tests/ -v` will continue to exercise the same mock-mode tests, which are unaffected by this change.

## Platform Considerations

None identified. This is a Python attribute-access fix with no platform-specific behavior. The `self.ctx.device` string value (e.g. `"cpu"`, `"cuda:0"`) is already proven cross-platform-safe by the Sampler class's own use of `self.ctx.device` on line 331.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The `self.ctx` attribute could theoretically be `None` if `BaseNode.__init__` were never called, causing an `AttributeError` on `self.ctx.device` | Very Low | Low | `BaseNode.__init__` is always called because `EmptyLatent` is instantiated via the `@register` decorator's factory path, which passes a valid `NodeContext`. The existing codebase inspection confirms `mock_context` in tests always provides a complete `NodeContext`. No mitigation needed. |
| The change could accidentally affect the Sampler class if the grep or edit was imprecise and touched line 331 as well | Very Low | Low | The edit is scoped to line 183 only (inside `EmptyLatent.execute()`). Line 331 already uses `self.ctx.device` correctly. Post-fix grep confirms no residual `device=ctx.device` anywhere in the file. |

## Acceptance Criteria

- [ ] `grep -n "device=self\.ctx\.device" worker/nodes/sampler.py` exits 0 with at least one match inside EmptyLatent.execute()
- [ ] `grep -n "device=ctx\.device" worker/nodes/sampler.py` exits 1 (zero matches)
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py -v` exits 0 (no regression in mock-mode tests)
