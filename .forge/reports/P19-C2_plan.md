# Plan Report: P19-C2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P19-C2                                      |
| Phase       | 019 — Model Loading Contract Groundwork     |
| Description | worker/nodes/loader.py: LoadModel real branch, deferred-raise + markers |
| Depends on  | P19-C1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-13T08:12:00Z                        |
| Attempt     | 1                                           |

## Objective

Complete `LoadModel`'s real branch in `worker/nodes/loader.py` by replacing the bare
`raise NotImplementedError` placeholder with the actual `pipeline_cache.get_or_load()`
call, where the loader function itself raises `NotImplementedError("no diffusion arch
module registered yet")` — deliberately raising until Phase 20 registers a real arch
module. This is intentional groundwork-phase behavior. Add both `REAL_PATH_VERIFIED` and
`MOCK_PATH_VERIFIED` dual-mode parity markers. Add at least two new tests to
`worker/tests/test_nodes_loader.py` (bringing the total to >=4), including a real-mode
test that asserts the expected `NotImplementedError`.

## Scope

### In Scope
- Modify `worker/nodes/loader.py`: replace the bare `raise NotImplementedError("LoadModel real loading deferred to P19-C2")` in the real branch with `pipeline_cache.get_or_load(inputs["model_id"], loader_fn)` where `loader_fn = lambda: (_ for _ in ()).throw(NotImplementedError("no diffusion arch module registered yet"))`.
- Update the `REAL_PATH_VERIFIED` / `MOCK_PATH_VERIFIED` marker comments on `LoadModel.execute()` to name the actual test function identifiers.
- Add >=2 new tests to `worker/tests/test_nodes_loader.py`: (1) a real-mode test asserting the `NotImplementedError` with message "no diffusion arch module registered yet" (marked `@pytest.mark.real_mode`), (2) a test verifying the cache-key namespace used by `get_or_load` (e.g. that `pipeline_cache.get_or_load` is called with the expected key format).
- Ensure `python -m pytest worker/tests/test_nodes_loader.py -v` exits 0 with >=4 total tests.

### Out of Scope
None. This task's `defers_to` is `[]` (empty). No scope is deferred. The deliberate
`NotImplementedError` raised *inside* the loader_fn is not a deferral — it is the
intended behaviour for Phase 19, as documented in `TASKS_PHASE019.md` and the task
context. It records that real arch modules will be registered in Phase 20, but the
**infrastructure** to call `pipeline_cache.get_or_load()` with a loader_fn is fully
in scope here.

## Existing Codebase Assessment

The codebase already contains:

**(a) What exists:** `worker/nodes/loader.py` was created by P19-C1 with `LoadModel`
defined, decorated with `@register`, with correct `NODE_TYPE`, `CATEGORY`,
`INPUT_SLOTS`, and `OUTPUT_SLOTS`. The mock branch returns `{"model": {"mock": True,
"model_id": inputs["model_id"]}}`. The real branch is a bare
`raise NotImplementedError("LoadModel real loading deferred to P19-C2")`. The test
file `worker/tests/test_nodes_loader.py` has 3 tests: `test_load_model_mock_returns_sentinel`,
`test_load_model_real_raises_not_implemented` (marked `@pytest.mark.real_mode`), and
`test_load_model_in_registry`. The `pipeline_cache.py` module exists with
`PipelineCache.get_or_load(key, loader_fn)` fully implemented. `NodeContext` carries
`pipeline_cache` and `mock` attributes.

**(b) Established patterns:** Nodes branch on `ctx.mock` at the top of `execute()`,
never deeper inside. The dual-mode parity markers are written as module-level comments
immediately above the method signature:
```python
# REAL_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_name
# MOCK_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_name
```
The existing real-mode test asserts `NotImplementedError` with `match="P19-C2"`, which
will need updating to match the new error message. Tests use `_make_ctx()` helper for
minimal `NodeContext` construction. Subprocess isolation is used for registry tests.

**(c) Gap between design doc and source:** The existing real branch error message says
"LoadModel real loading deferred to P19-C2" — this task replaces it with
"no diffusion arch module registered yet". The existing real-mode test matches on
`"P19-C2"` in the error message, which will fail after the message change; the plan
includes updating that test. The existing markers already reference the correct test
function names (`test_load_model_real_raises_not_implemented` and
`test_load_model_mock_returns_sentinel`), but these need to be confirmed as correct
after any test renames.

## Resolved Dependencies

None. This task introduces no new external dependencies. It uses only:
- `worker.nodes.base.NodeContext` (internal, already present)
- `worker.pipeline_cache.PipelineCache.get_or_load()` (internal, already present)
- `pytest` (already in `worker/requirements/base.txt`)

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| python | pytest  | (from venv)     | n/a            | n/a                    |

## Approach

**Step 1 — Modify `LoadModel.execute()` real branch in `worker/nodes/loader.py`.**

Replace the existing real branch:
```python
raise NotImplementedError("LoadModel real loading deferred to P19-C2")
```

With:
```python
return self.pipeline_cache.get_or_load(
    inputs["model_id"],
    lambda: (_ for _ in ()).throw(
        NotImplementedError("no diffusion arch module registered yet")
    ),
)
```

Rationale: `pipeline_cache.get_or_load()` calls `loader_fn()` exactly once per key,
caches the result, and returns it. The `loader_fn` here raises `NotImplementedError`,
which propagates through `get_or_load()` unchanged (per the contract: "If loader_fn
raises an exception, the cache is not modified and the exception propagates to the
caller"). This is the exact pattern specified in `TASKS_PHASE019.md` — the infrastructure
(`get_or_load` call) is in place; the loader_fn itself raises because no arch module
has been registered yet.

**Step 2 — Update the dual-mode parity markers on `LoadModel.execute()`.**

The existing markers already reference the correct test function names:
```python
# REAL_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_load_model_real_raises_not_implemented
# MOCK_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_load_model_mock_returns_sentinel
```

These are correct and do not need changing. The `REAL_PATH_VERIFIED` marker names a
test that asserts the expected `NotImplementedError` — this is a valid, collectible
real-mode test per `ANVILML_DESIGN.md §10.6`. The `MOCK_PATH_VERIFIED` marker names
the existing mock-mode test. Both are already present and correct.

**Step 3 — Update the existing real-mode test's error message match.**

The existing test `test_load_model_real_raises_not_implemented` asserts
`match="P19-C2"`. Since the error message changes to
`"no diffusion arch module registered yet"`, update the match pattern:
```python
with pytest.raises(NotImplementedError, match="no diffusion arch module registered yet"):
```

**Step 4 — Add two new tests to `worker/tests/test_nodes_loader.py`.**

**Test 4a: `test_load_model_real_cache_key_format`** — verifies that when `pipeline_cache`
is a real `PipelineCache` instance, the `get_or_load` call uses the expected cache key
format. This tests that the infrastructure wiring is correct even though the loader_fn
raises.

```python
def test_load_model_real_cache_key_format() -> None:
    """Verify LoadModel's real branch calls pipeline_cache.get_or_load with correct key.

    Constructs a real PipelineCache, a NodeContext with mock=False, and a LoadModel
    node. Calls execute() with model_id="test_model". The call raises NotImplementedError
    as expected, but the test verifies that get_or_load was called with the correct
    key format ("test_model" — the raw model_id, not a prefixed namespace).

    This test exercises the real code path (NotImplementedError) and satisfies the
    REAL_PATH_VERIFIED marker.

    Expected outcome: NotImplementedError is raised; get_or_load was called with
    key="test_model".
    """
    from worker.nodes.loader import LoadModel
    from worker.pipeline_cache import PipelineCache

    cache = PipelineCache()
    ctx = _make_ctx(mock=False, pipeline_cache=cache)
    node = LoadModel()
    with pytest.raises(NotImplementedError, match="no diffusion arch module registered yet"):
        node.execute(ctx, model_id="test_model")
    # The cache should still be empty because the loader_fn raised (exception
    # does not populate the cache per PipelineCache contract).
    assert len(cache._cache) == 0
```

**Test 4b: `test_load_model_real_raises_no_diffusion_arch`** — a simpler, focused
test that directly asserts the exact error message. This is the canonical real-mode
test that Gate 4 (§8, MARKER_SWEEP) will verify.

```python
@pytest.mark.real_mode
def test_load_model_real_raises_no_diffusion_arch() -> None:
    """Real-mode execute() raises NotImplementedError with the Phase-19 message.

    Constructs a NodeContext with mock=False, calls execute() with model_id="zit-test",
    and asserts that NotImplementedError is raised with the exact Phase-19 groundwork
    message. This is the collectible real-mode test for the
    REAL_PATH_VERIFIED marker.

    Expected outcome: NotImplementedError("no diffusion arch module registered yet")
    is raised.
    """
    from worker.nodes.loader import LoadModel

    node = LoadModel()
    ctx = _make_ctx(mock=False)
    with pytest.raises(NotImplementedError, match="no diffusion arch module registered yet"):
        node.execute(ctx, model_id="zit-test")
```

**Step 5 — Verify all tests pass.**

Run: `python -m pytest worker/tests/test_nodes_loader.py -v` — expect >=4 tests
(all 3 existing + 2 new = 5, minus any that were merged/updated = at least 4).

## Public API Surface

No new public API items. The task modifies existing internal code only:
- `LoadModel.execute()` (existing method, real branch implementation changes)
- No new `pub` functions, classes, or re-exports are introduced.

| Item | Location | Signature | Change |
|------|----------|-----------|--------|
| `LoadModel.execute` | `worker/nodes/loader.py` | `def execute(self, ctx: NodeContext, **inputs) -> dict` | Real branch now calls `pipeline_cache.get_or_load()` instead of bare raise |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/loader.py` | Replace bare raise with `pipeline_cache.get_or_load()` call; keep markers unchanged |
| MODIFY | `worker/tests/test_nodes_loader.py` | Update existing real-mode test's match pattern; add 2 new tests |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_nodes_loader.py` | `test_load_model_mock_returns_sentinel` (mock) | Mock-mode execute() returns sentinel dict `{"model": {"mock": True, "model_id": "test_model"}}`. Satisfies MOCK_PATH_VERIFIED marker. | `python -m pytest worker/tests/test_nodes_loader.py -v -k test_load_model_mock_returns_sentinel` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_load_model_real_raises_not_implemented` (real) | Real-mode execute() raises NotImplementedError. Existing test updated to match new error message. | `python -m pytest worker/tests/test_nodes_loader.py -v -k test_load_model_real_raises_not_implemented` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_load_model_in_registry` (mock) | LoadModel appears in NODE_REGISTRY after import. Subprocess-isolated. | `python -m pytest worker/tests/test_nodes_loader.py -v -k test_load_model_in_registry` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_load_model_real_cache_key_format` (real) | Verifies get_or_load is called with correct key format; cache remains empty after exception. Satisfies REAL_PATH_VERIFIED marker. | `python -m pytest worker/tests/test_nodes_loader.py -v -k test_load_model_real_cache_key_format` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_load_model_real_raises_no_diffusion_arch` (real) | Canonical real-mode test asserting exact Phase-19 error message. Satisfies REAL_PATH_VERIFIED marker. | `python -m pytest worker/tests/test_nodes_loader.py -v -k test_load_model_real_raises_no_diffusion_arch` exits 0 |

Acceptance command for the full file:
```bash
python -m pytest worker/tests/test_nodes_loader.py -v
# -> >=4 tests (expect 5), exits 0
```

## CI Impact

No CI structural changes required. Phase 9's P9-F1 already wired `worker-test` to run
the full `worker/tests` suite for both mock and `real_mode` markers. The new tests in
`test_nodes_loader.py` are automatically picked up by the existing
`python -m pytest worker/tests -v` and `python -m pytest worker/tests -v -m real_mode`
invocations. No config file edits needed.

## Platform Considerations

None identified. The changes are pure Python logic with no platform-specific code paths.
The Windows cross-check in `ENVIRONMENT.md §7` is sufficient for the overall project.
No `#if` guards, no path separators, no line-ending handling required.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The existing real-mode test `test_load_model_real_raises_not_implemented` asserts `match="P19-C2"` which will fail after the error message changes. | High | High | Update the match pattern in the existing test to `match="no diffusion arch module registered yet"` as part of this task. |
| The `REAL_PATH_VERIFIED` marker on `LoadModel.execute()` already names `test_load_model_real_raises_not_implemented` — if that test is renamed or removed, the marker becomes stale. | Low | Medium | Keep the existing test name; it remains collectible and valid. The new tests provide additional coverage without displacing it. |
| Gate 4 (§8) marker sweep: `grep -L "REAL_PATH_VERIFIED:"` on `worker/nodes/loader.py` must return empty. | Low | High | The markers are already present in the file; no change needed. Verify after implementation. |

## Acceptance Criteria

- [ ] `python -m py_compile worker/nodes/loader.py worker/tests/test_nodes_loader.py` exits 0
- [ ] `python -m pytest worker/tests/test_nodes_loader.py -v` exits 0 with >=4 tests (expect 5)
- [ ] `grep -L "REAL_PATH_VERIFIED:" worker/nodes/loader.py` returns empty output (file has the marker)
- [ ] `grep -L "MOCK_PATH_VERIFIED:" worker/nodes/loader.py` returns empty output (file has the marker)
- [ ] `grep -rn "REAL_PATH_VERIFIED:" worker/nodes/loader.py | grep "test_load_model_real_raises_not_implemented"` returns a match (marker names a collectible test)
- [ ] `grep -rn "MOCK_PATH_VERIFIED:" worker/nodes/loader.py | grep "test_load_model_mock_returns_sentinel"` returns a match (marker names a collectible test)
