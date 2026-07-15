# Implementation Report: P22-C1

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P22-C1                                      |
| Phase         | 22 — Qwen3 CLIP Arch Module                 |
| Description   | worker/nodes/arch/clip/qwen3.py: meta construction + dtype selection |
| Implemented   | 2026-07-15T17:00:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Implemented the `load()` function for the Qwen3 CLIP text-encoder architecture module (`worker/nodes/arch/clip/qwen3.py`), adding three key components: (1) `_select_dtype()` — a pure function implementing the fixed precedence chain from ANVILML_DESIGN.md §11.5 (fp8 → bf16 → fp16 → fp32), (2) `Qwen3TextEncoder(nn.Module)` — the target model class constructed on meta-device using PyTorch's layer classes (Embedding, Linear, LayerNorm, MultiheadAttention), and (3) `load()` — the entry point that infers hyperparameters, selects dtype, constructs the model on meta-device, applies dtype metadata, and loads the Qwen3 tokenizer from the vendored local asset directory with zero network calls. Added 10 new tests bringing the total to 16 (from 6 original), covering all dtype precedence branches, meta construction, tokenizer loading, and error propagation. Both REAL_PATH_VERIFIED and MOCK_PATH_VERIFIED dual-mode parity markers are present on `load()`.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| python | transformers | (vendored)    | N/A (local assets) |

No new external dependencies added. The tokenizer is already vendored at `worker/assets/qwen3_tokenizer/` (P22-A1). The `transformers.AutoTokenizer.from_pretrained()` call uses the existing `transformers` dependency already in `worker/requirements/cpu-runner-reqs.txt`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/arch/clip/qwen3.py` | Added torch import guard, `_select_dtype()`, `Qwen3TextEncoder`, `Qwen3DecoderLayer`, `_Qwen3MLP` classes, `load()` function with dual-mode markers, tokenizer loading, logging |
| Modify | `worker/tests/test_arch_clip_qwen3.py` | Added 10 new tests: 4 dtype selection unit tests, 2 load() real-mode tests, 1 load() mock-mode test, 1 error propagation test, 1 tokenizer network-blocking test |
| Modify | `docs/TESTS.md` | Added 11 new test entries covering all new tests |

## Commit Log

```
 .forge/reports/P22-C1_plan.md        | 206 ++++++++++++++++++
 .forge/state/CURRENT_TASK.md         |   6 +-
 .forge/state/state.json              |  13 +-
 docs/TESTS.md                        | 120 ++++++++++
 worker/nodes/arch/clip/qwen3.py      | 409 ++++++++++++++++++++++++++++++++++-
 worker/tests/test_arch_clip_qwen3.py | 255 ++++++++++++++++++++++
 6 files changed, 993 insertions(+), 16 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 16 items

worker/tests/test_arch_clip_qwen3.py::test_infer_hyperparams_qwen3_fixture PASSED [  6%]
worker/tests/test_arch_clip_qwen3.py::test_infer_hyperparams_nonexistent_path_raises PASSED [ 12%]
worker/tests/test_arch_clip_qwen3.py::test_infer_hyperparams_truncated_header_raises PASSED [ 18%]
worker/tests/test_arch_clip_qwen3.py::test_can_handle_matches_qwen3 PASSED [ 25%]
worker/tests/test_arch_clip_qwen3.py::test_can_handle_rejects_other_keys PASSED [ 31%]
worker/tests/test_arch_clip_qwen3.py::test_get_module_returns_qwen3_for_matching_key PASSED [ 37%]
worker/tests/test_arch_clip_qwen3.py::test_dtype_selection_fp8_caps_and_native PASSED [ 43%]
worker/tests/test_arch_clip_qwen3.py::test_dtype_selection_bf16_real PASSED [ 50%]
worker/tests/test_arch_clip_qwen3.py::test_dtype_selection_bf16_mock PASSED [ 56%]
worker/tests/test_arch_clip_qwen3.py::test_dtype_selection_fp16_only PASSED [ 62%]
worker/tests/test_arch_clip_qwen3.py::test_dtype_selection_fp32_fallback PASSED [ 68%]
worker/tests/test_arch_clip_qwen3.py::test_load_real_qwen3_fixture PASSED [ 75%]
worker/tests/test_arch_clip_qwen3.py::test_load_mock_qwen3_fixture PASSED [ 81%]
worker/tests/test_arch_clip_qwen3.py::test_load_raises_invalid_hyperparams PASSED [ 87%]
worker/tests/test_arch_clip_qwen3.py::test_load_raises_runtime_error_without_torch PASSED [ 93%]
worker/tests/test_arch_clip_qwen3.py::test_tokenizer_loads_from_vendored_path_no_network PASSED [100%]

============================= 16 passed in 10.28s ==============================
```

Mock-mode suite (115 passed, 72 deselected):
```
====================== 115 passed, 72 deselected in 6.59s ======================
```

Real-mode suite (72 passed, 115 deselected):
```
===================== 72 passed, 115 deselected in 11.95s ======================
```

## Format Gate

```
(No output — cargo fmt --all -- --check exited 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.90s

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 53.36s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.41s

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.02s
```

All four platform cross-checks exit 0.

## Project Gates

Gate 1 (config_reference):
```
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out
```

Gate 4 (Mock/Real Parity Markers):
- `REAL_PATH_VERIFIED` marker names `test_load_real_qwen3_fixture` — collectible ✓
- `MOCK_PATH_VERIFIED` marker names `test_load_mock_qwen3_fixture` — collectible ✓
- `grep -L "REAL_PATH_VERIFIED:"` on worker/nodes/**/*.py — empty ✓
- `grep -L "MOCK_PATH_VERIFIED:"` on worker/nodes/**/*.py — empty ✓

## Public API Delta

```
+def _select_dtype(caps: dict, native_dtype: str) -> "torch.dtype":
+class Qwen3TextEncoder(_ModuleBase):
+class Qwen3DecoderLayer(_ModuleBase):
+class _Qwen3MLP(_ModuleBase):
+def load(path: str, caps: dict, device: str = "cpu") -> "Qwen3TextEncoder":
```

New public items:
- `Qwen3TextEncoder` (class) — `worker.nodes.arch.clip.qwen3` — The target model class
- `Qwen3DecoderLayer` (class) — `worker.nodes.arch.clip.qwen3` — Single decoder layer (public for potential external use)
- `_select_dtype` (function) — `worker.nodes.arch.clip.qwen3` — Private dtype selection helper
- `_Qwen3MLP` (class) — `worker.nodes.arch.clip.qwen3` — Private gated MLP block
- `load` (function) — `worker.nodes.arch.clip.qwen3` — The main entry point for loading a Qwen3 text encoder

## Deviations from Plan

- **Tokenizer path resolution:** The plan specified `Path(__file__).parent.parent.parent / "assets" / "qwen3_tokenizer"` but the actual module path (`worker/nodes/arch/clip/qwen3.py`) requires 4 `.parent` traversals to reach `worker/` (not 3). The correct path is `Path(__file__).parent.parent.parent.parent / "assets" / "qwen3_tokenizer"`.
- **Additional tests beyond plan minimum:** The plan required >=4 new tests; I added 10 new tests (16 total) to cover all dtype precedence branches, both real and mock load paths, error propagation, and tokenizer network-blocking verification. This exceeds the minimum but ensures comprehensive coverage.
- **Gate 4 compliance:** The plan's MOCK_PATH_VERIFIED marker named `test_load_mock_qwen3_fixture` which did not exist at plan time. I created this test during implementation to satisfy the gate.

## Blockers

None.
