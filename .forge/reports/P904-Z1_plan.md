# Plan Report: P904-Z1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P904-Z1                                     |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects) |
| Description | worker/tests/real_fixtures.py: synthetic tiny clip checkpoint fixtures (qwen3/clip_l/t5) |
| Depends on  | P904-B4                                     |
| Project     | anvilml                                     |
| Planned at  | 2026-06-24T16:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `worker/tests/real_fixtures.py` containing three pytest fixtures (`tiny_qwen3_clip`, `tiny_clip_l_clip`, `tiny_t5_clip`) that build a minimal transformer model using its real `transformers` Config class with tiny dimensions (hidden_size=32, num_hidden_layers=2), save the model's `state_dict()` to a `.safetensors` file via `safetensors.torch.save_file`, and return the file path. These fixtures give downstream real-mode tests (P904-Z1b, Z3, Z4, Z5) a valid safetensors checkpoint for the three CLIP text-encoder architectures without downloading multi-GB real weights.

## Scope

### In Scope
- Create `worker/tests/real_fixtures.py` with three `@pytest.fixture` functions:
  - `tiny_qwen3_clip(tmp_path)` — `Qwen3ForCausalLM(Qwen3Config(**tiny_values))`, save `state_dict()` to tmp_path, return path
  - `tiny_clip_l_clip(tmp_path)` — `CLIPTextModelWithProjection(CLIPTextConfig(**tiny_values))`, save `state_dict()` to tmp_path, return path
  - `tiny_t5_clip(tmp_path)` — `T5EncoderModel(T5Config(**tiny_values))`, save `state_dict()` to tmp_path, return path
- Each fixture uses the real `transformers` Config class for its model type, overriding only `hidden_size`/`d_model` to 32 and `num_hidden_layers`/`num_layers` to 2, while keeping all other config parameters at their registered defaults (as the real arch modules do via verbatim dicts — but using the Config class is the task's explicit instruction)
- Each fixture returns a `pathlib.Path` to the saved `.safetensors` file
- The file follows the project's test conventions: Google-style docstrings, `from __future__ import annotations`, module-level docstring explaining purpose

### Out of Scope
None. `defers_to (from JSON): absent`. This task implements its full scope with no deferrals.

## Existing Codebase Assessment

The project's test suite lives in `worker/tests/`, with `conftest.py` providing an autouse `mock_mode` fixture that forces `ANVILML_WORKER_MOCK=1` for every test. The existing CLIP architecture tests (`test_arch_clip_qwen3.py`, `test_arch_clip_l.py`, `test_arch_clip_t5.py`) follow a consistent pattern: module docstring, `from __future__ import annotations`, imports of `can_handle`/`load` from the arch module, and test functions with Google-style docstrings containing Preconditions, Tests, and Expected output sections.

The three CLIP arch modules (`qwen3.py`, `clip_l.py`, `t5.py`) each construct their model via a verbatim config dict passed to the Config class constructor (e.g. `Qwen3Config(**config_values)`), then call `model.load_state_dict(safetensors_load_file(model_id))`. The real `load_state_dict()` call accepts any state dict with matching tensor shapes — it does not require a specific key format because no key remap is applied to CLIP text encoders (confirmed: the P904-A9..A14 remap concern only affects transformer/VAE, not CLIP).

The `worker/requirements/base.txt` already includes `safetensors>=0.8` and `transformers>=5.12`. `torch` is not listed in base.txt (it's a transitive dependency of diffusers/transformers but not guaranteed to be present in CI's base venv). This is consistent with the project's design: real-mode code paths are lazy-imported and unreachable in mock mode.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source     | Feature flags confirmed |
|--------|-------------|-----------------|----------------|------------------------|
| python | safetensors | 0.8.0           | pypi-query MCP | n/a                    |
| python | transformers| 5.12.1          | pypi-query MCP | n/a                    |

Both packages are already declared in `worker/requirements/base.txt`. No new dependencies are introduced. `torch` is required at fixture runtime but is a transitive dependency of `transformers` (listed as `torch>=2.4; extra == "torch"` in transformers' requires_dist) and will be present when the real-mode CPU test suite installs torch via `cpu-linux-agent.txt`.

## Approach

1. **Create `worker/tests/real_fixtures.py`** with a module-level docstring explaining that these fixtures generate synthetic tiny-config checkpoints for CLIP text encoders (qwen3, clip_l, t5) to avoid downloading multi-GB real weights in tests. Include `from __future__ import annotations`.

2. **Implement `tiny_qwen3_clip(tmp_path)` fixture**:
   - Import `torch` and `safetensors.torch` lazily inside the fixture body (not at module level, to preserve mock-mode import isolation — if torch is absent, the fixture raises a clear error)
   - Import `Qwen3Config, Qwen3ForCausalLM` from `transformers`
   - Construct config with `hidden_size=32, num_hidden_layers=2` plus all other parameters at their registered defaults (the Config class constructor accepts keyword overrides; unspecified fields use registered defaults)
   - Instantiate `Qwen3ForCausalLM(config)` and call `.state_dict()` to get the raw tensor dict
   - Save via `safetensors.torch.save_file(state_dict, str(tmp_path / "qwen3_clip.safetensors"))`
   - Return the Path object

3. **Implement `tiny_clip_l_clip(tmp_path)` fixture** (same pattern):
   - Import `CLIPTextConfig, CLIPTextModelWithProjection` from `transformers`
   - Construct with `hidden_size=32, num_hidden_layers=2`
   - Save state_dict to tmp_path, return Path

4. **Implement `tiny_t5_clip(tmp_path)` fixture** (same pattern):
   - Import `T5Config, T5EncoderModel` from `transformers`
   - Construct with `d_model=32, num_layers=2` (T5 uses `d_model`/`num_layers` naming, not `hidden_size`/`num_hidden_layers`)
   - Save state_dict to tmp_path, return Path

5. **Rationale on lazy torch import**: Each fixture body imports `torch` and `safetensors.torch` inside the function rather than at module level. This matches the established pattern in the CLIP arch modules where `torch`/`transformers`/`safetensors` are never top-level imports (they would break mock-mode tests). If `torch` is not installed when a fixture runs, the import failure will produce a clear error message.

## Public API Surface

```python
# worker/tests/real_fixtures.py (new file)

@pytest.fixture
def tiny_qwen3_clip(tmp_path: pathlib.Path) -> pathlib.Path:
    """Build a tiny Qwen3 text-encoder checkpoint and return its path."""

@pytest.fixture
def tiny_clip_l_clip(tmp_path: pathlib.Path) -> pathlib.Path:
    """Build a tiny CLIP-L text-encoder checkpoint and return its path."""

@pytest.fixture
def tiny_t5_clip(tmp_path: pathlib.Path) -> pathlib.Path:
    """Build a tiny T5-XXL text-encoder checkpoint and return its path."""
```

These are pytest fixtures (not `pub` items in the Rust sense), but they form the public API surface that downstream tasks import.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/tests/real_fixtures.py` | Three pytest fixtures generating tiny-config checkpoint files for qwen3, clip_l, and t5 text encoders |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/real_fixtures.py` | `test_fixtures_exist_and_return_path` | All three fixtures are importable and return a `pathlib.Path` pointing to an existing `.safetensors` file | `torch` installed (real-mode CPU venv) | None | Three file paths, each pointing to a valid safetensors file | `worker/.venv/bin/python -c "from worker.tests.real_fixtures import tiny_qwen3_clip, tiny_clip_l_clip, tiny_t5_clip; print('OK')"` exits 0 |
| `worker/tests/real_fixtures.py` | `test_qwen3_checkpoint_loadable` | The qwen3 fixture's safetensors file can be loaded with `safetensors.torch.load_file` and contains tensors with expected shapes | `torch` installed, fixture ran | `tiny_qwen3_clip` fixture | Tensors exist with shapes consistent with hidden_size=32, num_hidden_layers=2 | `worker/.venv/bin/python -m pytest worker/tests/real_fixtures.py::test_qwen3_checkpoint_loadable -v` exits 0 |
| `worker/tests/real_fixtures.py` | `test_clip_l_checkpoint_loadable` | The clip_l fixture's safetensors file is loadable with correct shapes | `torch` installed, fixture ran | `tiny_clip_l_clip` fixture | Tensors consistent with CLIPTextConfig hidden_size=32 | Same pattern as above |
| `worker/tests/real_fixtures.py` | `test_t5_checkpoint_loadable` | The t5 fixture's safetensors file is loadable with correct shapes | `torch` installed, fixture ran | `tiny_t5_clip` fixture | Tensors consistent with T5Config d_model=32 | Same pattern as above |

Acceptance command for the full suite (in a venv with torch):
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/real_fixtures.py -v
```

## CI Impact

No CI changes required. This task only creates a new test file in `worker/tests/`. The existing CI invocation (`pytest worker/tests -v`) will discover and run it. The fixtures require `torch`, which is absent from CI's base venv — when torch is missing, the fixture body's lazy `import torch` will raise `ImportError`, causing the test to fail. This is acceptable for now; P904-Z2 will register the `realcpu` marker and exclude this file from CI's default pytest invocation, providing opt-in real-mode testing.

## Platform Considerations

None identified. The fixtures use only Python standard library (`pathlib`, `tmp_path` from pytest), `torch`, `safetensors`, and `transformers` — all cross-platform. No `#[cfg(...)]` guards or path-separator handling needed beyond what `pathlib` already provides. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `transformers` 5.12.1 may have changed Config class parameter names from earlier versions (e.g. `hidden_size` → `d_model` for Qwen3) | Low | Medium | Verify each Config class constructor at ACT time by reading the transformers source or running a quick `python -c "import inspect; from transformers import Qwen3Config; print(inspect.signature(Qwen3Config.__init__))"` before writing the fixture body. The plan uses the parameter names confirmed in the existing arch modules' verbatim dicts (which are the source of truth for the model's actual config schema). |
| Fixture execution time / memory during CI if torch is present | Low | Low | The models are tiny (hidden_size=32, 2 layers) — each state_dict is a few hundred KB at most. No meaningful risk of slowdown. |
| `tmp_path` fixture scope — pytest's `tmp_path` creates a per-test temporary directory that is cleaned up after the test function returns | Low | Medium | The fixture returns a Path, not the data itself. Downstream tests receive the Path and must use the file before the test function exits (or copy it). This is the standard pytest pattern and matches the task's instruction ("each fixture returns a saved file path"). If a downstream test needs the file to persist across test functions, it should use `tmp_path_factory` instead — that is the responsibility of the consuming task, not this one. |
| `safetensors.torch.save_file` API shape changed between 0.8 and current version | Low | Medium | Verified via MCP: `safetensors.torch.save_file` accepts `(tensors: dict, filename: str)` — confirmed by the package info showing the torch usage example. No feature flags needed. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/tests/real_fixtures.py` exits 0
- [ ] `worker/.venv/bin/python -c "from worker.tests.real_fixtures import tiny_qwen3_clip, tiny_clip_l_clip, tiny_t5_clip; print('import OK')"` exits 0 (module imports cleanly without torch in mock mode — lazy imports preserve isolation)
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/real_fixtures.py -v` exits 0 in a venv where torch is installed (real-mode CPU venv with `cpu-linux-agent.txt`)
- [ ] Each fixture returns a `pathlib.Path` pointing to a file that exists and is a valid `.safetensors` file loadable via `safetensors.torch.load_file`
