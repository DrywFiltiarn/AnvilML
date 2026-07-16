# Plan Report: P22-D1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P22-D1                                       |
| Phase       | 022 — Qwen3 CLIP Arch Module                |
| Description | worker/nodes/loader.py: LoadClip real branch calls qwen3.py via dispatch |
| Depends on  | P22-C2                                       |
| Project     | anvilml                                      |
| Planned at  | 2026-07-16T09:45:00Z                         |
| Attempt     | 1                                            |

## Objective

Replace `LoadClip`'s `NotImplementedError` placeholder (introduced in Phase 19's P19-C3) with a real branch that dispatches to the Qwen3 CLIP arch module via `arch.clip.get_module()` and caches the loaded encoder through `pipeline_cache.get_or_load()`. Remove the stale `NotImplementedError`-asserting test for `LoadClip` and replace it with a passing real-mode test that exercises the full loading chain against the P22-B1 fixture checkpoint, updating the `REAL_PATH_VERIFIED` marker to point at the new test.

## Scope

### In Scope
- **`worker/nodes/loader.py`** — Replace `LoadClip.execute()`'s real branch (lines 231–244) with dispatch logic that mirrors `LoadModel`'s real branch pattern: import `get_module` from `worker.nodes.arch.clip`, call `get_module(inputs.get("clip_type", "qwen3"))`, and pass the result to `pipeline_cache.get_or_load()`. Update the `REAL_PATH_VERIFIED` marker to point at the new passing test.
- **`worker/tests/test_nodes_loader.py`** — Remove `test_load_clip_real_raises_not_implemented` (the stale `NotImplementedError`-asserting test for `LoadClip` specifically). Add one new real-mode test (`test_load_clip_real_loads_qwen3_fixture`) that calls `LoadClip.execute()` against the `qwen3_tiny.safetensors` fixture and verifies the returned encoder is a `torch.nn.Module` with `.arch == "qwen3"` and an attached `.tokenizer`.

### Out of Scope
None. This task's `defers_to` field is `[]` (empty). No scope is deferred.

## Existing Codebase Assessment

**What already exists:** `LoadModel` (Phase 20) already has a real branch that follows the exact pattern this task will replicate: import `get_module` from the family dispatcher, call it with the dispatch key, and pass a lambda to `pipeline_cache.get_or_load()`. The CLIP dispatcher (`worker/nodes/arch/clip/__init__.py`) already imports and registers `qwen3`, so `get_module("qwen3")` will return the `qwen3` module. The `qwen3.py` module's `load(path, caps, device)` function is fully implemented (Phase 22 Group C) with all four contract steps: shape inference, dtype selection, meta-device construction, key remapping, and tokenizer loading from the vendored path.

**Established patterns:** The real branch follows a strict template — import the dispatcher at the top of the `else` block (not at module level, to keep mock-mode collection torch-free), call `get_module(key)`, guard against `None` with a `RuntimeError`, log at DEBUG level, and wrap the call in `pipeline_cache.get_or_load()` with a `key` namespace prefix (e.g. `"clip:{model_id}"`). The test pattern mirrors `test_load_model_real_loads_zit_fixture`: construct `PipelineCache()`, create `NodeContext(mock=False)`, call `execute()`, then assert the return dict has the correct key, the value is a `torch.nn.Module`, the `.arch` attribute matches, and parameters are on the real device.

**Gap between design doc and source:** The design doc (§10.3) specifies that `clip_type` is an optional dispatch hint with default `"qwen3"`. The current source already declares `clip_type` as optional in `INPUT_SLOTS` and uses `inputs.get("clip_type", "qwen3")` in the task's context — this matches the design. No gap exists.

## Resolved Dependencies

None. This task only modifies existing Python source files and tests; no new external packages or version pins are introduced. All referenced types (`NodeContext`, `PipelineCache`, `BaseNode`, `SlotSpec`, `Qwen3TextEncoder`, `get_module`, `load`) already exist in the codebase and have been verified via file inspection.

## Approach

1. **Replace `LoadClip`'s real branch in `worker/nodes/loader.py`.** In `LoadClip.execute()`, replace lines 231–244 (the `else:` block containing the `NotImplementedError` throw) with dispatch logic that follows the `LoadModel` real branch pattern exactly:
   - Import `get_module` from `worker.nodes.arch.clip` inside the `else` block (to keep mock-mode collection torch-free).
   - Call `get_module(inputs.get("clip_type", "qwen3"))` to resolve the dispatch key. The `"qwen3"` default matches the dispatcher's registered module key.
   - Guard against `None` with a `RuntimeError` that names the failed key (defensive guard, same pattern as `LoadModel` lines 84–91).
   - Log at DEBUG level: `"LoadClip: requesting model_id=%s, clip_type=%s"` (matching `LoadModel`'s debug log pattern).
   - Return `{"clip": ctx.pipeline_cache.get_or_load(f"clip:{inputs['model_id']}", lambda: module.load(inputs["model_id"], ctx.caps))}`. Note: `qwen3.load()`'s signature is `load(path, caps, device="cpu")`, so only `path` and `caps` are passed — `device` defaults to `"cpu"`, matching the test context's device.
   - Update the `REAL_PATH_VERIFIED` marker comment on line 199 to point at the new test function name.

2. **Remove the stale `NotImplementedError` test in `worker/tests/test_nodes_loader.py`.** Delete the function `test_load_clip_real_raises_not_implemented` (lines 286–307). This test asserts the old behaviour that is being replaced.

3. **Add the new real-mode fixture test.** After `test_load_clip_mock_returns_sentinel`, add `test_load_clip_real_loads_qwen3_fixture` decorated with `@pytest.mark.real_mode`. The test follows the exact structure of `test_load_model_real_loads_zit_fixture`:
   - Import `Path`, `torch`, `LoadClip`, `PipelineCache`.
   - Set `fixture_path` to `worker/tests/fixtures/qwen3_tiny.safetensors`.
   - Construct `LoadClip()` node, `NodeContext(mock=False, pipeline_cache=PipelineCache())`.
   - Call `execute(ctx, model_id=fixture_path, clip_type="qwen3")`.
   - Assert `"clip" in result`.
   - Assert `isinstance(result["clip"], torch.nn.Module)`.
   - Assert `result["clip"].arch == "qwen3"`.
   - Assert parameters are on CPU device.
   - Assert `hasattr(result["clip"], "tokenizer")` (verifies the tokenizer attachment step).
   - Update the `REAL_PATH_VERIFIED` marker on `LoadClip.execute()` to point at this new test name.

4. **Verify the marker convention.** After the edit, `LoadClip.execute()` will have:
   - `REAL_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_load_clip_real_loads_qwen3_fixture`
   - `MOCK_PATH_VERIFIED: worker/tests/test_nodes_loader.py::test_load_clip_mock_returns_sentinel`
   Both markers name collectible tests, satisfying Gate 4 (§8 of ENVIRONMENT.md) and FORGE_AGENT_RULES §5.13.

## Public API Surface

None. This task modifies `LoadClip.execute()`'s internal implementation but does not change its public signature (`execute(self, ctx: NodeContext, **inputs) -> dict`), its class attributes, or any external-facing API. The `qwen3.load()` function is already public and was introduced in P22-C2.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | worker/nodes/loader.py | Replace LoadClip's real branch with dispatch to arch.clip.get_module + pipeline_cache.get_or_load; update REAL_PATH_VERIFIED marker |
| MODIFY | worker/tests/test_nodes_loader.py | Remove test_load_clip_real_raises_not_implemented; add test_load_clip_real_loads_qwen3_fixture |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| worker/tests/test_nodes_loader.py | test_load_clip_mock_returns_sentinel (mock) | LoadClip.execute() returns sentinel dict in mock mode; MOCK_PATH_VERIFIED marker satisfied | `python -m pytest worker/tests/test_nodes_loader.py -v -m "not real_mode" -k test_load_clip_mock_returns_sentinel` exits 0 |
| worker/tests/test_nodes_loader.py | test_load_clip_real_loads_qwen3_fixture (real) | LoadClip.execute() loads qwen3_tiny.safetensors via real branch, returns torch.nn.Module with .arch="qwen3" and .tokenizer; REAL_PATH_VERIFIED marker satisfied | `python -m pytest worker/tests/test_nodes_loader.py -v -m real_mode -k test_load_clip_real_loads_qwen3_fixture` exits 0 |
| worker/tests/test_nodes_loader.py | test_load_clip_in_registry (mock) | LoadClip appears in NODE_REGISTRY after import | `python -m pytest worker/tests/test_nodes_loader.py -v -k test_load_clip_in_registry` exits 0 |
| worker/tests/test_nodes_loader.py | test_load_model_real_loads_zit_fixture (real) | LoadModel real branch still works (regression guard) | `python -m pytest worker/tests/test_nodes_loader.py -v -m real_mode -k test_load_model_real_loads_zit_fixture` exits 0 |
| worker/tests/test_nodes_loader.py | test_load_vae_real_raises_not_implemented (real) | LoadVae real branch still raises NotImplementedError (regression guard) | `python -m pytest worker/tests/test_nodes_loader.py -v -m real_mode -k test_load_vae_real_raises_not_implemented` exits 0 |

After the change, `test_nodes_loader.py` will have exactly 4 `@pytest.mark.real_mode` tests: `test_load_model_real_loads_zit_fixture`, `test_load_vae_real_raises_not_implemented`, `test_load_clip_real_loads_qwen3_fixture`, and `test_load_vae_real_cache_key_format`. The two `test_load_clip_real_*` NotImplementedError tests are removed. The acceptance criterion requires `>=4 tests` in real_mode, which is met.

## CI Impact

No CI changes required. The existing CI jobs (`worker-linux-real`, `worker-windows-real`) run `python -m pytest worker/tests/ -v -m real_mode` which will pick up the new test automatically. The removed test was already part of the same file and marker group. No new test file, file type, or test module is introduced.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The change is pure Python logic with no `#[cfg(...)]` guards, no path-separator handling (uses `str()` on a Path object, same as `qwen3.py`'s own `tokenizer_path` construction), and no line-ending differences.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The qwen3_tiny.safetensors fixture may not contain the right keys for the Qwen3TextEncoder's state_dict, causing `load_state_dict(assign=True, strict=False)` to load zero weights and leaving the model with only zero-initialized parameters. This would make the test pass structurally (no exception) but the model would be non-functional. | Low | Medium | The fixture was built by P22-B1 using the same key patterns that `qwen3.py`'s `_infer_hyperparams` and `_build_key_remapping` expect. The `load()` function already handles shape mismatches gracefully (skips non-matching keys per lines 832-840). The test verifies `.arch == "qwen3"` and `.tokenizer` existence, which are set regardless of weight loading success. If the fixture is truly incompatible, the fixture builder (not this task) needs fixing. |
| `qwen3.load()` requires `device` as a keyword argument (default `"cpu"`), but the lambda in `get_or_load` only passes `inputs["model_id"]` and `ctx.caps`. If `qwen3.load()`'s signature changes in a future task to require `device`, the lambda would need updating. | Low | Low | This is the same pattern `LoadModel` uses — it only passes two arguments to `zit.load()`. The `device` default is `"cpu"` in both `zit.py` and `qwen3.py`. If a future task changes this, the ACT agent for that task will fix all callers. |
| Removing `test_load_clip_real_raises_not_implemented` could cause a test count regression below the `>=4` acceptance threshold if other real_mode tests are also affected. | Low | Medium | The plan accounts for the final test count: 4 real_mode tests remain (LoadModel real, LoadVae real, LoadClip real new, LoadVae cache key format). Verified by counting existing tests. |

## Acceptance Criteria

- [ ] `python -m pytest worker/tests/test_nodes_loader.py -v -m real_mode` exits 0
- [ ] `grep -n "test_load_clip_real_raises_not_implemented" worker/tests/test_nodes_loader.py` returns no matches (the old NotImplementedError test is removed)
- [ ] `grep -n "test_load_clip_real_loads_qwen3_fixture" worker/tests/test_nodes_loader.py` returns a match (the new test exists)
- [ ] `grep "REAL_PATH_VERIFIED:" worker/nodes/loader.py` shows the marker pointing to `test_load_clip_real_loads_qwen3_fixture`
- [ ] `grep -c "@pytest.mark.real_mode" worker/tests/test_nodes_loader.py` returns >= 4 (the real-mode test count requirement)
