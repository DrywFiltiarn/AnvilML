# Plan Report: P904-A3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P904-A3                                           |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects) |
| Description | worker/nodes/loader.py: LoadClip.execute() missing torch import causes NameError before dispatch |
| Depends on  | P18-D12, P904-A2                                  |
| Project     | anvilml                                           |
| Planned at  | 2026-06-23T20:55:00Z                              |
| Attempt     | 1                                                 |

## Objective

Fix a `NameError` in `LoadClip.execute()`'s real-mode branch that fires before architecture dispatch is ever reached. The method references `torch.bfloat16` on line 618 but never imports the `torch` package in that scope. Every sibling real-mode function in `loader.py` (`LoadVae.execute()`'s `loader_fn`, `_load_model_from_hf_directory`, `_load_clip_from_hf_directory`) already performs a local `import torch` immediately before first use, following the module's documented lazy-import convention. Adding this import allows the real-mode code path to reach the architecture dispatch and subsequently the tokenizer-path fix from P904-A2.

## Scope

### In Scope
- Add `import torch` inside `LoadClip.execute()`'s real-mode branch (after the `ANVILML_WORKER_MOCK == "1"` early return at line 596–601, before `module.load(model_id, torch_dtype=torch.bfloat16)` at line 618).
- Add an inline comment explaining the lazy-import rationale, matching the convention used by sibling functions in the same file.

### Out of Scope
defers_to (from JSON): absent. This task has no deferrals.

- P904-A2's tokenizer directory depth fix (separate task, fixes `qwen3.py`/`clip_l.py`).
- P904-A7's device-placement fix (separate task, adds `device` parameter to `module.load()`).
- Any changes to `LoadModel`, `LoadVae`, or other loader functions.
- Any test additions — this is a one-line fix that does not change mock-mode behavior.

## Existing Codebase Assessment

The `worker/nodes/loader.py` module implements three loader nodes (`LoadModel`, `LoadVae`, `LoadClip`) with a strict lazy-import convention: `torch`, `diffusers`, and `safetensors` must never be imported at the module level, because doing so would cause the worker to fail on systems without GPU hardware. Every real-mode code path guards its imports inside the `if os.environ.get("ANVILML_WORKER_MOCK") == "1":` early-return block, importing lazily only in the non-mock branch.

Three of the four real-mode functions in this file follow this convention correctly:
- `LoadVae.execute()` (line 502): `import torch` after the mock check, before `loader_fn` uses `torch.bfloat16`.
- `_load_model_from_hf_directory()` (line 679): `import torch` after the mock check, before `ZImageTransformer2DModel.from_single_file()` uses `torch.float16`.
- `_load_clip_from_hf_directory()` (line 747): `import torch` after the mock check, before the encoder uses `torch.bfloat16`.

`LoadClip.execute()` (line 618) is the outlier: it references `torch.bfloat16` directly in the `module.load()` call without a preceding local import. The module-level imports (lines 29–33) include `os` and `typing.Any` but not `torch`. This is a straightforward omission — the fix is a single `import torch` line placed immediately after the mock-mode early return and before the first use of `torch`.

The test file `worker/tests/test_nodes_loader.py` only tests mock-mode behavior (all tests run with `ANVILML_WORKER_MOCK=1`), so this fix does not require any test changes — the real-mode branch is never exercised by the existing test suite.

## Resolved Dependencies

None. No new external dependencies are introduced. `torch` is already referenced by other functions in this file and is expected to be available in the real-mode environment.

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| (none) | | | | |

## Approach

defers_to (from JSON): absent.

**Step 1.** Open `worker/nodes/loader.py`. Locate the `LoadClip.execute()` method (lines 561–618).

**Step 2.** After the mock-mode early return block (lines 596–601 — the `if os.environ.get("ANVILML_WORKER_MOCK") == "1":` block ending with `return {"clip": MockClip(clip_type=clip_type)}`), insert a local `import torch` statement.

**Step 3.** Add an inline comment on the import line explaining the lazy-import rationale, matching the convention used by sibling functions. The comment should reference the module-level docstring's requirement (lines 17–22) that `torch` must never be imported at the top level. The exact comment wording from `LoadVae.execute()` (line 498–500) is:

```python
        # Real mode: lazy imports — these packages are not available
        # in mock mode (no torch installed), so importing them here
        # keeps the worker importable when ANVILML_WORKER_MOCK=1.
        import torch
```

**Step 4.** Verify that the `import torch` line appears before line 618 (`return module.load(model_id, torch_dtype=torch.bfloat16)`), ensuring `torch` is defined at the point of use.

**Step 5.** Run the existing mock-mode test suite to confirm no regression:
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v
```

**Step 6.** Run the acceptance criterion verification:
```bash
python3 -c "
import ast
tree = ast.parse(open('worker/nodes/loader.py').read())
src = open('worker/nodes/loader.py').read()
assert 'import torch' in src.split('class LoadClip')[1].split('class ')[0]
"
```

## Public API Surface

None. This task modifies only the internal implementation of `LoadClip.execute()` — no public types, functions, or method signatures are changed.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/loader.py` | Add `import torch` inside `LoadClip.execute()`'s real-mode branch after mock-mode early return |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_loader.py` | (existing tests) | No regression in mock-mode behavior | `ANVILML_WORKER_MOCK=1` | N/A | All existing tests pass | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0 |
| (manual verification) | import torch in LoadClip | `import torch` exists within `LoadClip` class scope | Source file read | N/A | Assertion passes | `python3 -c "..."` exits 0 |

## CI Impact

No CI changes required. This is a one-line fix inside a real-mode branch that is never exercised by mock-mode tests (the default CI gate). The fix does not add, modify, or remove any test files, and does not change any public API.

## Platform Considerations

None identified. `import torch` is a standard Python import that works identically on Linux and Windows. The `torch` package's availability is determined by the worker's requirements file (CPU, CUDA, or ROCm variant), not by platform-specific code paths. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|-------------|
| The `import torch` statement inside `LoadClip.execute()` could shadow a module-level `torch` import if one is added elsewhere in the file, but no such import exists — this is not a current risk. | Low | Low | No mitigation needed; confirmed via grep that `torch` is not imported at module level. |
| The fix is a one-line addition inside a real-mode branch that is never reached by any committed test (all tests use `ANVILML_WORKER_MOCK=1`). A syntax or indentation error in the edit could silently break the real-mode path without being caught by tests. | Low | High | Run `python -m py_compile worker/nodes/loader.py` after the edit to catch any syntax error before running tests. The acceptance criterion also verifies the import is present in the correct scope. |
| The module docstring (lines 17–22) states that `torch`, `diffusers`, and `safetensors` must never be imported at the top level. Adding `import torch` inside the method body is consistent with this convention, but a future developer might mistakenly move it to module level. | Low | Medium | The inline comment at the import site references the module-level docstring's requirement, making the convention explicit at the point of use. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0
- [ ] `python3 -c "import ast; tree = ast.parse(open('worker/nodes/loader.py').read()); src = open('worker/nodes/loader.py').read(); assert 'import torch' in src.split('class LoadClip')[1].split('class ')[0]"` exits 0
- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/loader.py` exits 0
