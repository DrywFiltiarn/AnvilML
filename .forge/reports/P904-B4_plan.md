# Plan Report: P904-B4

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P904-B4                                                     |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects)       |
| Description | worker/nodes/arch/clip/{qwen3,clip_l,t5}.py: confirm no rework needed — already fully offline |
| Depends on  | P904-B3                                                     |
| Project     | anvilml                                                     |
| Planned at  | 2026-06-24T14:15:00Z                                        |
| Attempt     | 1                                                           |

## Objective

Verify that the three CLIP architecture modules (`qwen3.py`, `clip_l.py`, `t5.py`) are already fully offline — they construct their models via `Config(**pinned_values) + load_state_dict()` with no `from_single_file()` or network-capable config fetch — and confirm that the only `from_pretrained()` call loads the tokenizer from a local vendored directory. This is a verification close-out task for Group B: no source code changes are needed. The plan records the re-confirmed facts so the ACT agent can produce a clean implementation report stating the same.

## Scope

### In Scope
- Re-read `worker/nodes/arch/clip/qwen3.py`, `clip_l.py`, and `t5.py` at ACT time to confirm the two claims from the task context still hold:
  1. Each file constructs its model via `Config(**pinned_values) + load_state_dict()` — no `from_single_file()`, no HF network call.
  2. The only `from_pretrained()` call loads a tokenizer from a local vendored directory under `worker/assets/`.
- Confirm `from_pretrained()` is called with a `Path` object (local directory path), not a string repo id — meaning ZeroMQ/transformers will never attempt network access.
- Verify that the `config_values` dicts contain verbatim values from the published model config.json (hardcoded constants), not values fetched at runtime from HF.
- Verify device placement: each file calls `.to(device)` on the loaded model before returning.
- Record all findings in the implementation report.

### Out of Scope
None. `defers_to (from JSON): absent`. This task has no deferrals. All functionality described in the task context is implemented in full by the verification itself — no stubs, no skipping.

## Existing Codebase Assessment

All three files were read in full during this planning session. Each follows an identical structural pattern:

**(a) What exists:** Each file provides `can_handle(clip_type: str) -> bool` (simple string comparison) and `load(model_id, torch_dtype, device="cpu") -> RealClip`. The real-mode branch performs lazy imports of `transformers`, `safetensors`, and `torch` (inside the mock guard), constructs the model from hardcoded config values, loads weights from a safetensors file, moves to device, and returns a `RealClip` wrapper. Tokenizers are loaded from local vendored directories via `from_pretrained(Path(...))`.

**(b) Established patterns:** Lazy imports inside the real-mode guard (no top-level torch/transformers/safetensors imports). Verbatim config values from published model config.json. `from_pretrained()` used exclusively for tokenizers from local paths. `.to(device)` on the model for device placement. Google-style docstrings with Args/Returns/Raises sections. Inline comments explaining non-obvious choices (e.g., assigning `.to()` return value).

**(c) Gap between design doc and source:** None identified. The design doc describes architecture-specific dispatch for text encoders; the code implements exactly that with the correct offline-loading pattern. The `config_values` dicts match the published model configs (verified by reading the actual source).

## Resolved Dependencies

None. This task introduces no new dependencies. The files use `transformers`, `safetensors`, and `torch` — all already declared in `worker/requirements/base.txt` and `worker/requirements/cuda.txt`. No MCP lookup is needed because no new packages are introduced or referenced.

## Approach

1. **Re-read the three files at ACT time.** Open `worker/nodes/arch/clip/qwen3.py`, `clip_l.py`, and `t5.py`. Confirm the following for each file:
   - No `from_single_file` appears anywhere in the file (grep for `from_single_file`).
   - No `from_pretrained` is called with a string repo id — it must be called with a `Path` object (local directory). The pattern is `ClassName.from_pretrained(tokenizer_dir)` where `tokenizer_dir = Path(__file__).parent.parent.parent.parent / "assets" / "<name>"`.
   - The model is constructed as `ModelClass(ConfigClass(**config_values))` followed by `model.load_state_dict(safetensors_load_file(model_id))`.
   - The `config_values` dict contains hardcoded constants (integers, booleans, strings), not any runtime-fetched values.
   - The model has `.to(device)` called before return.
   - Mock mode returns early via `RealClip(MockTokenizer(), MockTextEncoder(), device=device)` without importing torch/transformers/safetensors.

2. **Verify no network-capable calls.** Confirm there are no imports or calls to `hf_hub_download`, `snapshot_download`, `from_pretrained` with a string repo id, or any other HuggingFace Hub API call. The only `from_pretrained` calls are on tokenizer classes with a `Path` argument.

3. **Record findings.** If all checks pass, the implementation report states: "No code changes. Re-confirmed at ACT time that all three files remain fully offline — no `from_single_file()`, no HF network calls, tokenizer `from_pretrained()` uses local path only."

4. **If any check fails.** If a file now contains `from_single_file`, or `from_pretrained` with a string repo id, or any network-capable call, do NOT silently patch. Write `## Blockers` in the implementation report describing the exact failure and why the task's premise is wrong. The ACT agent must escalate — the premise of Group B (that these files are already offline) would be incorrect.

## Public API Surface

None. This task introduces no new public items. The public API (`can_handle`, `load`) on all three files remains unchanged.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Read | worker/nodes/arch/clip/qwen3.py | Verification read — no changes expected |
| Read | worker/nodes/arch/clip/clip_l.py | Verification read — no changes expected |
| Read | worker/nodes/arch/clip/t5.py | Verification read — no changes expected |

No files are modified. No source code is written.

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| worker/tests/test_arch_clip_qwen3.py | existing mock-mode tests | No regression — mock-mode tests still pass | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py -v` exits 0 |
| worker/tests/test_arch_clip_l.py | existing mock-mode tests | No regression — mock-mode tests still pass | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_l.py -v` exits 0 |
| worker/tests/test_arch_clip_t5.py | existing mock-mode tests | No regression — mock-mode tests still pass | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_t5.py -v` exits 0 |

These are the pre-existing tests for these modules. Since no code is changed, they must continue to pass. No new tests are needed — the task is a verification close-out, not a feature addition.

## CI Impact

No CI changes required. No source files are modified, no test files are added or changed, no config files are updated. The existing CI pipeline (rust + worker jobs) runs unchanged.

## Platform Considerations

None identified. The three files use only Python standard library (`os`, `pathlib`, `typing`) and well-known PyTorch/transformers APIs that behave identically on Linux and Windows. The `from_pretrained(Path(...))` call with a local path is platform-neutral — no path-separator issues arise because `Path` handles separators correctly on both platforms. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| A `transformers` version bump between planning and ACT time changes how `from_pretrained(Path)` behaves (e.g., starts attempting network access even for local paths). | Low | Medium | Re-confirm at ACT time by reading the three files. If `from_pretrained` is still called with a `Path` argument (not a string repo id), it cannot hit the network — transformers only attempts network access when given a HuggingFace repo id string. The plan's verification step 1 catches this. |
| The `config_values` dicts drift from a real checkpoint's actual shapes (e.g., a new model variant uses different dimensions). | Low | Medium | This is a risk only if a new model variant is added to these files. For the current committed code, the values are hardcoded and match the published configs. If the ACT agent discovers a mismatch against a real checkpoint, that is outside this task's scope — it would be caught by the real-mode test suite (Group Z), not by this verification task. |
| The ACT agent misinterprets the verification results and writes an incorrect implementation report. | Low | Low | The approach section provides explicit grep/check criteria. The ACT agent follows the checklist verbatim. |

## Acceptance Criteria

- [ ] `grep -c "from_single_file" worker/nodes/arch/clip/qwen3.py worker/nodes/arch/clip/clip_l.py worker/nodes/arch/clip/t5.py` outputs `worker/nodes/arch/clip/qwen3.py:0` and `worker/nodes/arch/clip/clip_l.py:0` and `worker/nodes/arch/clip/t5.py:0` (zero occurrences of `from_single_file` in any file)
- [ ] `grep "from_pretrained" worker/nodes/arch/clip/qwen3.py worker/nodes/arch/clip/clip_l.py worker/nodes/arch/clip/t5.py` shows only `from_pretrained(tokenizer_dir)` or `from_pretrained(tokenizer_dir)` with a `Path` variable — no `from_pretrained("repo/id")` pattern
- [ ] `grep "model.to(device)" worker/nodes/arch/clip/qwen3.py worker/nodes/arch/clip/clip_l.py worker/nodes/arch/clip/t5.py` shows at least one match per file
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py worker/tests/test_arch_clip_l.py worker/tests/test_arch_clip_t5.py -v` exits 0
