# Plan Report: P904-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P904-A2                                           |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects) |
| Description | Fix tokenizer asset directory depth in qwen3.py and clip_l.py |
| Depends on  | P18-D9, P18-D10, P904-A1                          |
| Project     | anvilml                                           |
| Planned at  | 2026-06-23T19:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Correct the tokenizer asset directory resolution in `worker/nodes/arch/clip/qwen3.py` and `worker/nodes/arch/clip/clip_l.py` so that `Qwen2Tokenizer.from_pretrained(tokenizer_dir)` and `CLIPTokenizer.from_pretrained(tokenizer_dir)` resolve to the actual bundled tokenizer directories at `worker/assets/qwen25_tokenizer/` and `worker/assets/clip_l_tokenizer/`. Both files currently resolve one directory level too shallow (`worker/nodes/arch/assets/`), which does not exist — the real assets live at `worker/assets/` (one level higher).

## Scope

### In Scope
- Modify `worker/nodes/arch/clip/qwen3.py`: change `Path(__file__).parent.parent / "assets" / "qwen25_tokenizer"` to `Path(__file__).parent.parent.parent / "assets" / "qwen25_tokenizer"` on line 102, and add the same inline comment t5.py uses.
- Modify `worker/nodes/arch/clip/clip_l.py`: change `Path(__file__).parent.parent / "assets" / "clip_l_tokenizer"` to `Path(__file__).parent.parent.parent / "assets" / "clip_l_tokenizer"` on line 102, and add the same inline comment t5.py uses.
- Verify existing mock-mode tests (`test_arch_clip_qwen3.py`, `test_arch_clip_l.py`) still pass after the change.

### Out of Scope

defers_to (from JSON): absent. This task may not defer any scope.

None. This task implements its full scope in full — no stubs, no deferred functionality.

## Existing Codebase Assessment

Three CLIP architecture dispatch modules exist in `worker/nodes/arch/clip/`: `qwen3.py`, `clip_l.py`, and `t5.py`. Each follows an identical pattern: a `can_handle(clip_type)` string comparison, a `load(model_id, torch_dtype)` function that checks `ANVILML_WORKER_MOCK` at runtime, returns a `RealClip` sentinel in mock mode, and constructs a real `transformers` model from verbatim config values in real mode.

`t5.py` (line 99–105) already has the correct tokenizer path resolution using `Path(__file__).parent.parent.parent / "assets" / "t5_tokenizer"`, accompanied by an inline comment explaining that the plan originally specified `parent.parent` but the actual asset layout places tokenizers at `worker/assets/` (one level higher). This comment was added as a lesson learned and serves as the reference wording for this task.

`qwen3.py` (line 99–102) and `clip_l.py` (line 99–102) both use `Path(__file__).parent.parent / "assets" / ...`, which resolves to `worker/nodes/arch/assets/` — a directory that does not exist. The actual tokenizer assets are at `worker/assets/qwen25_tokenizer/` and `worker/assets/clip_l_tokenizer/`. This was confirmed by listing the `worker/assets/` directory contents, which contain all three tokenizer directories (`qwen25_tokenizer`, `clip_l_tokenizer`, `t5_tokenizer`) with their `tokenizer_config.json` files.

The existing tests in `worker/tests/test_arch_clip_qwen3.py` and `worker/tests/test_arch_clip_l.py` only exercise the mock-mode code path (returning `RealClip(MockTokenizer(), MockTextEncoder())`), so they never touch the filesystem path resolution. The change is safe for mock-mode tests — it only affects the real-mode code path that these tests do not reach.

No external dependencies are introduced. No new public API is added. The `load()` function signature, return type, and behavior remain unchanged.

## Resolved Dependencies

None. This task modifies only two Python source files and does not introduce or reference any external crate, package, or library. All imports (`pathlib.Path`, `transformers.Qwen2Tokenizer`, `transformers.CLIPTokenizer`, `safetensors.torch`) already exist in the codebase and are not modified.

## Approach

1. **Read t5.py's inline comment verbatim** (lines 99–105) to capture the exact wording used for the correction explanation. The comment is:
   ```
   # Resolve the tokenizer directory relative to this module.
   # The tokenizer assets live in worker/assets/t5_tokenizer/
   # (three levels up from this file's parent, then into assets).
   # Note: the plan originally specified parent.parent, but the
   # actual asset layout places tokenizers at worker/assets/
   # (one level higher than parent.parent would resolve).
   ```

2. **Fix `worker/nodes/arch/clip/qwen3.py`**:
   - Replace line 99–102 (the four-line comment block + tokenizer_dir assignment) with the same comment structure as t5.py, adapted for qwen3's tokenizer name (`qwen25_tokenizer`) and the correct depth (`parent.parent.parent`):
     ```python
     # Resolve the tokenizer directory relative to this module.
     # The tokenizer assets live in worker/assets/qwen25_tokenizer/
     # (three levels up from this file's parent, then into assets).
     # Note: the plan originally specified parent.parent, but the
     # actual asset layout places tokenizers at worker/assets/
     # (one level higher than parent.parent would resolve).
     tokenizer_dir = Path(__file__).parent.parent.parent / "assets" / "qwen25_tokenizer"
     ```
   - This is a single-line functional change (`parent.parent` → `parent.parent.parent`) plus a comment block update for consistency.

3. **Fix `worker/nodes/arch/clip/clip_l.py`**:
   - Replace line 99–102 with the same comment structure as t5.py, adapted for clip_l's tokenizer name (`clip_l_tokenizer`) and the correct depth:
     ```python
     # Resolve the tokenizer directory relative to this module.
     # The tokenizer assets live in worker/assets/clip_l_tokenizer/
     # (three levels up from this file's parent, then into assets).
     # Note: the plan originally specified parent.parent, but the
     # actual asset layout places tokenizers at worker/assets/
     # (one level higher than parent.parent would resolve).
     tokenizer_dir = Path(__file__).parent.parent.parent / "assets" / "clip_l_tokenizer"
     ```

4. **Verify existing tests pass**: Run the mock-mode test suite for both files to confirm no regression:
   ```bash
   ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py worker/tests/test_arch_clip_l.py -v
   ```
   These tests exercise only the mock-mode path and will not be affected by the path change.

5. **Verify the path resolves correctly**: Confirm the corrected path expression resolves to an existing directory:
   ```bash
   python3 -c "
   from pathlib import Path
   p = Path('worker/nodes/arch/clip/qwen3.py')
   tokenizer_dir = p.parent.parent.parent / 'assets' / 'qwen25_tokenizer'
   assert tokenizer_dir.is_dir(), f'{tokenizer_dir} is not a directory'
   assert (tokenizer_dir / 'tokenizer_config.json').exists()
   "
   python3 -c "
   from pathlib import Path
   p = Path('worker/nodes/arch/clip/clip_l.py')
   tokenizer_dir = p.parent.parent.parent / 'assets' / 'clip_l_tokenizer'
   assert tokenizer_dir.is_dir(), f'{tokenizer_dir} is not a directory'
   assert (tokenizer_dir / 'tokenizer_config.json').exists()
   "
   ```

**Rationale for copying t5.py's comment verbatim**: The task context explicitly states to "copy that comment's wording into both fixed files for consistency, don't invent new wording." The comment explains the historical reason for the correction (the original plan specified `parent.parent`), which is useful context for future maintainers who might wonder why three `.parent` calls are needed instead of two.

## Public API Surface

None. The `load()` function signature (`def load(model_id: str, torch_dtype: Any) -> RealClip`) remains unchanged. The `can_handle()` function is unmodified. No new types, functions, or re-exports are introduced.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/clip/qwen3.py` | Change `parent.parent` to `parent.parent.parent` in `tokenizer_dir` resolution; update comment block to match t5.py's wording |
| MODIFY | `worker/nodes/arch/clip/clip_l.py` | Change `parent.parent` to `parent.parent.parent` in `tokenizer_dir` resolution; update comment block to match t5.py's wording |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_arch_clip_qwen3.py` | `test_can_handle_qwen3` | `can_handle("qwen3")` returns `True` — unchanged by path fix | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_can_handle_qwen3 -v` exits 0 |
| `worker/tests/test_arch_clip_qwen3.py` | `test_can_handle_non_qwen3` | `can_handle()` returns `False` for non-qwen3 types — unchanged | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_can_handle_non_qwen3 -v` exits 0 |
| `worker/tests/test_arch_clip_qwen3.py` | `test_load_mock_returns_realclip` | `load()` returns `RealClip` with sentinels in mock mode — unchanged | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_load_mock_returns_realclip -v` exits 0 |
| `worker/tests/test_arch_clip_qwen3.py` | `test_load_mock_no_torch_import` | No top-level torch import — unchanged | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py::test_load_mock_no_torch_import -v` exits 0 |
| `worker/tests/test_arch_clip_l.py` | `test_can_handle_clip_l` | `can_handle("clip_l")` returns `True` — unchanged by path fix | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_l.py::test_can_handle_clip_l -v` exits 0 |
| `worker/tests/test_arch_clip_l.py` | `test_can_handle_non_clip_l` | `can_handle()` returns `False` for non-clip_l types — unchanged | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_l.py::test_can_handle_non_clip_l -v` exits 0 |
| `worker/tests/test_arch_clip_l.py` | `test_load_mock_returns_realclip` | `load()` returns `RealClip` with sentinels in mock mode — unchanged | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_l.py::test_load_mock_returns_realclip -v` exits 0 |
| `worker/tests/test_arch_clip_l.py` | `test_load_mock_no_torch_import` | No top-level torch import — unchanged | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_l.py::test_load_mock_no_torch_import -v` exits 0 |

## CI Impact

No CI changes required. This task modifies only Python source files in the worker module. The existing CI `worker` job runs `pytest worker/tests/ -v` which collects and runs the test files above. Since the path change only affects the real-mode code path (which CI's mock-mode tests never exercise), the existing CI behavior is unchanged.

## Platform Considerations

None identified. The `pathlib.Path` module is cross-platform and resolves parent directories correctly on Linux, macOS, and Windows. The asset directory structure (`worker/assets/`) is the same across all platforms. No `#[cfg(...)]` or platform-specific handling is needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The comment block wording from t5.py mentions "three levels up" which is correct for t5.py's file location; if the comment is copied verbatim except for the tokenizer name, it remains accurate for qwen3.py and clip_l.py since all three files are at the same depth (`worker/nodes/arch/clip/`) | Low | Low | Verify the comment is semantically identical for all three files: all three reside in `worker/nodes/arch/clip/`, so `parent.parent.parent` from any of them resolves to `worker/`. The comment is already written this way in t5.py and applies identically to qwen3 and clip_l. |
| A downstream task (P904-A7) will later add a `device` parameter to `load()` — if P904-A2's edit touches the same lines that P904-A7 will modify, two sequential ACT agents editing the same region could conflict | Medium | Medium | P904-A2 only modifies lines 99–102 (the comment + `tokenizer_dir` assignment). P904-A7's scope (per TASKS_PHASE904.md) modifies the function signature (line 48) and the return statement (line 132). These are non-overlapping line ranges, so no edit conflict is possible. |
| The existing tests only exercise the mock-mode path, so the real-mode path fix cannot be validated by the existing test suite | Medium | Low | The acceptance criteria include a direct filesystem verification (`tokenizer_dir.is_dir()` and `tokenizer_config.json` exists) that proves the corrected path resolves to a real directory. This is sufficient for a path-depth fix since the real-mode code path is only exercised in a manual harness or Group B's real-mode test suite (P904-B3). |

## Acceptance Criteria

- [ ] `python3 -c "from pathlib import Path; p = Path('worker/nodes/arch/clip/qwen3.py').read_text(); assert '.parent.parent.parent' in p"` exits 0
- [ ] `python3 -c "from pathlib import Path; p = Path('worker/nodes/arch/clip/clip_l.py').read_text(); assert '.parent.parent.parent' in p"` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py worker/tests/test_arch_clip_l.py -v` exits 0 with same test count as before
- [ ] `python3 -c "from pathlib import Path; p = Path('worker/nodes/arch/clip/qwen3.py'); td = p.parent.parent.parent / 'assets' / 'qwen25_tokenizer'; assert td.is_dir() and (td / 'tokenizer_config.json').exists()"` exits 0
- [ ] `python3 -c "from pathlib import Path; p = Path('worker/nodes/arch/clip/clip_l.py'); td = p.parent.parent.parent / 'assets' / 'clip_l_tokenizer'; assert td.is_dir() and (td / 'tokenizer_config.json').exists()"` exits 0
- [ ] `grep -n "the plan originally specified parent.parent" worker/nodes/arch/clip/qwen3.py` exits 0 (comment from t5.py present)
- [ ] `grep -n "the plan originally specified parent.parent" worker/nodes/arch/clip/clip_l.py` exits 0 (comment from t5.py present)
