# Plan Report: P904-A9

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P904-A9                                       |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects) |
| Description | worker/nodes/loader.py: remove deprecated HF-directory loading remnants entirely |
| Depends on  | P904-A8                                       |
| Project     | anvilml                                       |
| Planned at  | 2026-06-24T10:30:00Z                          |
| Attempt     | 1                                             |

## Objective

Delete two dead-code helper functions (`_load_from_hf_directory` and `_load_clip_from_hf_directory`) from `worker/nodes/loader.py` that were preserved "for future reactivation" but are no longer needed per explicit project decision. This is a pure removal task with zero behavioral change to any live code path — the functions are never called anywhere in the codebase, and all existing tests continue to pass unchanged.

## Scope

### In Scope
- Delete the `_load_from_hf_directory` function (lines 645–672 in `loader.py`), which loads a VAE from an HF-style directory using `AutoencoderKL.from_pretrained()`.
- Delete the `_load_clip_from_hf_directory` function (lines 756–831 in `loader.py`), which loads a text encoder from an HF-style directory using `from_pretrained()` on various transformers classes.
- No top-level imports need removal — all transformers imports (`CLIPTextModelWithProjection`, `CLIPTokenizer`, `Qwen2Tokenizer`, `Qwen3ForCausalLM`, `T5ForConditionalGeneration`, `T5TokenizerFast`) are local to `_load_clip_from_hf_directory` and are removed automatically with the function body. `AutoencoderKL` and `torch` are still used by `LoadVae.execute()` and `_load_model_from_hf_directory`, so they remain.
- Confirm all existing mock-mode tests in `worker/tests/test_nodes_loader.py` continue to pass.

### Out of Scope
- No renaming or modification of `_load_model_from_hf_directory` — that is P904-A10's job, sequenced after this one.
- No changes to `__all__` exports (these functions are module-private, underscore-prefixed, and not exported).
- No new tests — this is a pure removal with no behavioral change.
- No changes to design docs, architecture docs, or ENVIRONMENT.md.

## Existing Codebase Assessment

The `worker/nodes/loader.py` file (831 lines) defines three loader node classes (`LoadModel`, `LoadVae`, `LoadClip`) along with mock sentinel classes and three module-level helper functions for real-mode safetensors loading. The two functions targeted for deletion were created during Phase 018 (P18-D12, P18-D13, P18-D14) as preservation of the old `from_pretrained` code paths. Both functions are documented in their own docstrings as "kept but never called" and "may be reactivated in a future task."

Codebase inspection confirms: neither function is imported or called anywhere in the codebase — not in any other Python module, not in tests, not in the Rust supervisor. The only references to them are in task descriptions, design documentation, and previous plan/implement reports (historical).

The module follows a lazy-import convention: `torch`, `diffusers`, and `safetensors` are never imported at the top level; they are imported inside the non-mock code path. Both deleted functions follow this convention with local imports.

No gap between design doc and source: the design doc (ANVILML_DESIGN.md §1334) mentions these functions by name as deprecated remnants, consistent with their current state.

## Resolved Dependencies

None. This task introduces no new dependencies and removes none. The transformers imports consumed by `_load_clip_from_hf_directory` are local to that function and are removed with it — no top-level import block changes.

| Type   | Name    | Version verified | MCP source | Feature flags confirmed |
|--------|---------|-----------------|------------|------------------------|
| (none) | (none)  | (none)          | (none)     | (none)                 |

## Approach

1. **Open `worker/nodes/loader.py` and locate the two functions to delete.**
   - `_load_from_hf_directory` starts at line 645 (function definition) and ends at line 672 (closing `)` of `from_pretrained` call).
   - `_load_clip_from_hf_directory` starts at line 756 (function definition) and ends at line 831 (closing `)` of `loader_fn()` call).

2. **Delete `_load_from_hf_directory` (lines 645–672).**
   - Remove the entire function body including its docstring, local imports (`from diffusers import AutoencoderKL`, `import torch`), and the `AutoencoderKL.from_pretrained()` call.
   - Rationale: This function loads a VAE via `from_pretrained(model_id, subfolder="vae")` — the active path in `LoadVae.execute()` now uses `from_single_file()` (P18-D14), making this function unreachable dead code.

3. **Delete `_load_clip_from_hf_directory` (lines 756–831).**
   - Remove the entire function body including its docstring, local imports (`from transformers import CLIPTextModelWithProjection, CLIPTokenizer, Qwen2Tokenizer, Qwen3ForCausalLM, T5ForConditionalGeneration, T5TokenizerFast`, `import torch`), the `if/elif/else` clip_type dispatch, and the `loader_fn` closure.
   - Rationale: This function contains the original inline dispatch logic that was replaced by `arch_clip.get_module()` in P18-D12. The active path in `LoadClip.execute()` now uses the arch dispatcher, making this function unreachable dead code.

4. **Verify no top-level imports need removal.**
   - The transformers imports (`CLIPTextModelWithProjection`, `CLIPTokenizer`, `Qwen2Tokenizer`, `Qwen3ForCausalLM`, `T5ForConditionalGeneration`, `T5TokenizerFast`) are all local to `_load_clip_from_hf_directory` (inside the function body). Deleting the function removes them automatically.
   - `AutoencoderKL` is imported locally in both deleted functions but is also used in `LoadVae.execute()` (line 506: `from diffusers import AutoencoderKL`), so it must remain.
   - `torch` is imported locally in both deleted functions but is used throughout the file in `LoadVae.execute()`, `LoadClip.execute()`, and `_load_model_from_hf_directory`, so it must remain.
   - No changes needed to the module-level import block (lines 27–33).

5. **Verify no other code references the deleted functions.**
   - Grep confirms: `_load_from_hf_directory` and `_load_clip_from_hf_directory` appear only in `loader.py` (function definitions), task descriptions, design docs, and historical reports. No live call sites.

6. **Run mock-mode tests to confirm no regression.**
   - Run `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` — all 11 tests should pass. These tests exercise mock-mode paths only; the deleted functions are in real-mode code paths never reached in mock mode.

## Public API Surface

None. Both functions are module-private (underscore-prefixed) and not exported in `__all__`. No public class, method, or module-level symbol is affected.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/loader.py` | Delete `_load_from_hf_directory` (lines 645–672) and `_load_clip_from_hf_directory` (lines 756–831); no import changes needed |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_loader.py` | All 11 existing tests | No regression — mock-mode paths still work after deletion of real-mode dead code | `ANVILML_WORKER_MOCK=1`; NODE_REGISTRY cleared by fixture | Default test inputs (model_id="test-model", etc.) | All 11 tests pass, same test count as before | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0 |

No new tests are needed: this is a pure removal of dead code that no test exercises. The existing mock-mode test suite (11 tests) covers all live code paths and will confirm no regression.

## CI Impact

No CI changes required. This task modifies only one Python source file and removes dead code. The existing `worker` CI job (`ANVILML_WORKER_MOCK=1 <matrix-python> -m pytest worker/tests -v`) will continue to run the same test suite with the same results. No new test files, markers, or CI configuration changes are introduced.

## Platform Considerations

None identified. The deleted code is pure Python with no `os.path`/`Path` usage that would differ between Unix and Windows (both functions use `from_pretrained()` which handles path resolution internally). The `ANVILML_WORKER_MOCK=1` mock-mode tests that verify no regression run identically on both platforms. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|-------------|
| A future task (P904-A10+) that needs the old `from_pretrained` code path will have to re-implement it from scratch since the source is now deleted | Low | Low | The task context explicitly reverses the "keep for future reactivation" decision. If reactivation is needed later, the code must be written fresh — this is an intentional trade-off to keep the codebase clean rather than accumulating dead code. |
| A test or script outside the committed test suite (e.g. the manual harness `01_loaders.py` referenced in P904-A14) imports or calls one of these functions by name, causing an `ImportError` at runtime | Low | Low | Grep confirmed no call sites exist anywhere in the codebase. The manual harness (P904-A14) references the *new* offline loading path (A10–A13), not the old deprecated functions. If a future harness needs HF-directory loading, it should use the new arch-dispatched path instead. |
| Deleting lines 645–672 and 756–831 could cause an off-by-one error in the edit, accidentally removing or corrupting adjacent code (e.g. the end of `_load_model_from_hf_directory` or the module's final line) | Low | Medium | Read the file before editing to confirm exact line boundaries. After deletion, verify the file still has valid Python syntax by running `python3 -m py_compile worker/nodes/loader.py`. The acceptance criterion `grep -n "_load_from_hf_directory\|_load_clip_from_hf_directory" worker/nodes/loader.py` returning zero matches serves as a final sanity check. |

## Acceptance Criteria

- [ ] `grep -n "_load_from_hf_directory\|_load_clip_from_hf_directory" worker/nodes/loader.py` returns zero matches (both functions fully deleted)
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0 with the same 11 test count as before
- [ ] `python3 -m py_compile worker/nodes/loader.py` exits 0 (file has valid Python syntax after deletion)
- [ ] `grep -n "_load_model_from_hf_directory" worker/nodes/loader.py` returns at least one match (the function that is NOT being deleted is still present)
