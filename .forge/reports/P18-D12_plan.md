# Plan Report: P18-D12

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-D12                                       |
| Phase       | 018 — ZiT Generic Nodes                       |
| Description | worker/nodes/loader.py: LoadClip dispatches via arch.clip.get_module(), fixes ctx bug |
| Depends on  | P18-D9, P18-D10, P18-D11                      |
| Project     | anvilml                                       |
| Planned at  | 2026-06-23T07:25:00Z                          |
| Attempt     | 1                                             |

## Objective

Replace `LoadClip.execute()`'s inline `if/elif/else` clip-type dispatch with the `arch.clip.get_module(clip_type)` dispatcher (mirroring `Sampler`'s `arch.get_module(model)` pattern), and fix the pre-existing `ctx` bug where `execute()` reads bare `ctx.pipeline_cache` instead of `self.ctx.pipeline_cache` — a `NameError` at runtime. The old branch bodies are preserved in an unreachable `_load_from_hf_directory()` function for future reactivation.

## Scope

### In Scope
- `worker/nodes/loader.py` — refactor `LoadClip.execute()` real path to use `arch.clip.get_module(clip_type)`, fix bare `ctx` → `self.ctx` on `pipeline_cache` access, add `_load_from_hf_directory(model_id, clip_type)` stub function.

### Out of Scope
None. `defers_to (from JSON): []` — this task implements its full scope. No functionality is deferred to another task.

## Existing Codebase Assessment

The codebase already has a fully functional `arch.clip` dispatcher (`worker/nodes/arch/clip/__init__.py`) with `get_module(clip_type)` and `can_handle(clip_type)`, and three arch modules (`qwen3.py`, `clip_l.py`, `t5.py`) each providing `can_handle()` and `load(model_id, torch_dtype)`. These were implemented by P18-D9 through P18-D11 and are the prerequisites for this task.

The `Sampler` node in `sampler.py` already uses the `arch.get_module(model)` pattern for diffusion dispatch — this is the reference pattern to mirror. The `arch.clip` dispatcher has the identical structure but dispatches on a string (`clip_type`) instead of a model object.

The bug is clear: `LoadModel.execute()` and `LoadVae.execute()` both reference bare `ctx.pipeline_cache` (lines 304, 398) instead of `self.ctx.pipeline_cache`. `BaseNode.__init__` stores the context as `self.ctx` — there is no local `ctx` variable. The same pattern exists in `LoadClip.execute()` at line 553. `SaveImage.execute()` correctly uses `self.ctx.emit`.

Established patterns to follow:
- Mock mode check via `os.environ.get("ANVILML_WORKER_MOCK") == "1"` at the top of the real path.
- Lazy imports inside the non-mock code path.
- `RealClip(tokenizer, text_encoder)` wrapper for the return value.
- Google-style docstrings on classes and functions.

## Resolved Dependencies

None. This task introduces no new external dependencies — it only refactors internal dispatch to use existing `arch.clip.get_module()`.

## Approach

### Step 1: Add import for `arch.clip` at module level

Add `from worker.nodes.arch import clip as arch_clip` near the top of `loader.py`, alongside the existing `from worker.nodes.base import BaseNode, NodeContext, SlotSpec, register` import. This is a module-level import of a pure-Python dispatch registry — no torch/diffusers/safetensors dependencies are pulled in at import time because the arch modules defer all heavy imports to their `load()` functions.

### Step 2: Replace LoadClip.execute()'s real-path dispatch

In `LoadClip.execute()`, replace the entire `if clip_type == "qwen3": ... elif clip_type == "clip_l": ... elif clip_type == "t5": ... else: raise ValueError(...)` block (lines 489–528) with the new dispatch pattern:

```python
module = arch_clip.get_module(clip_type)
if module is None:
    raise ValueError(f"unsupported clip_type: {clip_type!r}")
return module.load(model_id, torch_dtype=torch.bfloat16)
```

This mirrors `Sampler`'s dispatch exactly: call `get_module()`, check for `None`, then call the matched module's entry point. The `torch.bfloat16` dtype matches the existing code's `torch_dtype=torch.bfloat16` on line 538.

### Step 3: Fix the ctx bug in LoadClip.execute()

Change `ctx.pipeline_cache.get_or_load(...)` to `self.ctx.pipeline_cache.get_or_load(...)` on line 553. The `ctx` bare name is a `NameError` — the context is stored as `self.ctx` by `BaseNode.__init__`. This is the same fix applied in `LoadModel.execute()` (line 304) and `LoadVae.execute()` (line 398).

### Step 4: Add _load_from_hf_directory stub function

After the `LoadClip` class definition, add a module-level function:

```python
def _load_from_hf_directory(model_id: str, clip_type: str) -> RealClip:
    """(Deprecated) Load a text encoder from an HF-style directory.

    This function preserves the original from_pretrained-based loading
    path that was replaced by the arch.clip.get_module() dispatcher
    in P18-D12. It is kept but never called — it may be reactivated
    in a future task if HF-directory loading is needed again.

    Args:
        model_id: Path to the model directory.
        clip_type: The clip type string (e.g. "qwen3", "clip_l", "t5").

    Returns:
        A RealClip instance with tokenizer and text_encoder.

    Raises:
        ValueError: If clip_type is not one of the supported types.
    """
    # This function preserves the original inline dispatch logic
    # that was replaced by arch.clip.get_module() in P18-D12.
    # It is intentionally never called — kept for future reactivation.
    ...
```

The body copies the original if/elif/else dispatch logic (lines 489–528 of current file) verbatim, including the `from_pretrained` calls inside the loader_fn closure and the `RealClip(tokenizer, text_encoder)` return. The function is never called by any code path.

### Step 5: Verify existing mock tests still pass

The acceptance criterion is: `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0. All existing tests in this file exercise only the mock code path (lines 242, 364, 468), which is unchanged by this task. The real path changes (dispatch refactor + ctx fix) are unreachable in mock mode.

## Public API Surface

No new public items are introduced. The `_load_from_hf_directory` function is module-private (leading underscore) and never called. No changes to any existing `pub` (Python `def`) signatures.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/loader.py` | Refactor `LoadClip.execute()` real path; add `_load_from_hf_directory()` stub; fix `ctx` → `self.ctx` |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `worker/tests/test_nodes_loader.py` | All existing tests | Existing mock-mode tests for LoadModel, LoadVae, LoadClip continue to pass after the refactoring | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0 |

No new tests are added. The acceptance criterion explicitly states "Existing mock tests ... continue to pass unchanged." The mock path is untouched, and the real path changes are unreachable in mock mode.

## CI Impact

No CI changes required. The modified file (`worker/nodes/loader.py`) is already part of the existing `py_compile` and `pytest` steps in CI (`worker-linux` and `worker-windows` jobs). No new test files or modules are introduced.

## Platform Considerations

None identified. The change is purely Python dispatch logic with no platform-specific behavior. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `arch_clip.get_module(clip_type)` returns `None` for a clip_type that the old code handled — e.g. if `arch.clip` hasn't imported all modules yet or a module's `can_handle()` is broken. | Low | High | The existing `arch.clip.get_module()` iterates over all sibling modules and catches exceptions in `can_handle()` (line 124 of `arch/clip/__init__.py`). If a module raises, it is skipped. The `None` check produces a clear `ValueError` message. |
| The `_load_from_hf_directory` stub function body copies the old dispatch logic with `from_pretrained` calls that may have subtle differences from the new dispatch path (e.g. tokenizer path resolution differs between `arch/clip/qwen3.py` which uses `Path(__file__).parent.parent / "assets"` and the old code which passes `model_id` directly to `from_pretrained`). | Low | Medium | The stub is intentionally dead code — it is never called. Documenting it as deprecated makes its status explicit. If reactivation is needed, the stub will be rewritten then. |
| The `self.ctx` fix in `LoadClip.execute()` could interact with the `pipeline_cache` type annotation (`dict[str, Any]` in `NodeContext`) — if `pipeline_cache` is still a plain dict at runtime (pre-P903-A2), `.get_or_load()` would fail with `AttributeError`. | Low | Medium | P903-A2 (prerequisite of this task's prerequisite chain) ensures `pipeline_cache` is a `PipelineCache` instance. The existing comment on line 302–303 of `loader.py` documents this. If it fails, the error is immediate and obvious. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0
- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/loader.py` exits 0 (syntax check before test run)
