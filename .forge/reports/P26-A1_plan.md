# Plan Report: P26-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P26-A1                                      |
| Phase       | 26 — Flux 2 Klein 9B + Qwen3-8B CLIP Variant |
| Description | worker/tests/fixtures/: Flux 2 Klein 9B + Qwen3-8B fixture builders |
| Depends on  | P25-F1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-23T17:55:00Z                        |
| Attempt     | 1                                           |

## Objective

Create two Python builder scripts (`build_flux2klein_9b_fixture.py` and `build_qwen3_8b_fixture.py`) in `worker/tests/fixtures/` that generate tiny synthetic `.safetensors` checkpoints shaped to imply the Flux 2 Klein 9B diffusion architecture and the Qwen3-8B FP8-mixed text encoder respectively. The fixtures use structurally valid tensor shapes and key naming conventions matching `flux2klein.py`'s and `qwen3.py`'s existing shape-inference formulas — only the hyperparameter VALUES differ from the 4B/4B fixtures, with no code changes required in the arch modules.

## Scope

### In Scope
- `worker/tests/fixtures/build_flux2klein_9b_fixture.py` — builder script for the Flux 2 Klein 9B fixture
- `worker/tests/fixtures/flux2klein9b_tiny.safetensors` — generated fixture file (with `arch: "flux2klein"` metadata)
- `worker/tests/fixtures/build_qwen3_8b_fixture.py` — builder script for the Qwen3-8B fixture
- `worker/tests/fixtures/qwen3_8b_tiny.safetensors` — generated fixture file (with `arch: "qwen3"` metadata, some tensors at native FP8 dtype)
- Both builder scripts accept no arguments, are idempotent, and write to the fixtures directory relative to their own location

### Out of Scope
None. `defers_to (from JSON): []`. This task must implement its full scope. No no-metadata variant is needed per §17.5's per-family requirement (already covered by 4B fixtures). No test code changes — those are handled by later tasks (P26-B1, P26-C1). No changes to existing `flux2klein.py` or `qwen3.py` — the shape-inference formulas already generalize.

## Existing Codebase Assessment

The fixture directory at `worker/tests/fixtures/` already contains builder scripts and generated `.safetensors` files for the ZiT, Flux 2 Klein 4B, Qwen3 4B, and Flux 2 VAE families. The builder scripts follow a consistent pattern:

1. **Path resolution**: Each script resolves `_FIXTURE_DIR` relative to its own location and prepends the repo root to `sys.path` so `import worker...` works regardless of invocation directory.
2. **Tensor construction**: A `_tensors()` function returns a `dict[str, torch.Tensor]` with keys matching the real checkpoint naming convention (e.g., `double_blocks.0.img_attn.qkv`).
3. **No-metadata variant** (where needed): A `_no_metadata_tensors()` function uses `xyz_`-prefixed keys and omits the `arch` metadata key to exercise the metadata-fallback code path.
4. **`build()` function**: Calls `save_file()` with the tensor dict and optional `metadata={"arch": "..."}`, prints the written path.

The `flux2klein.py` shape-inference (`_infer_hyperparams_inner`) reads ALL keys via `f.keys()` (P904 regression prevention), infers `hidden_dim` from `time_text_embed.timestep_embedder.0.weight` shape[0], counts double/single blocks via regex on `double_blocks[_.](\d+)` and `single_blocks[_.](\d+)`, and detects architecture from metadata or key patterns.

The `qwen3.py` shape-inference similarly reads ALL keys, infers `hidden_dim` from `self_attn.{q,k,v,o}_proj.weight` shape[0], counts layers from `model.layers.(\\d+)`, infers `intermediate_size` from `mlp.gate_proj.weight` shape[0], and infers `vocab_size` from `embed_tokens.weight` shape[0].

The existing 4B fixtures use `hidden_dim=128` (Flux 2 Klein) and `hidden_dim=64` (Qwen3). The new 9B/8B fixtures must use larger values to be distinguishable while staying under 10 MB.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source     | Feature flags confirmed |
|--------|-------------|-----------------|----------------|------------------------|
| python | safetensors | 0.5.3+ (via base.txt) | pypi-query MCP | n/a |
| python | torch       | (via requirements/*.txt) | pypi-query MCP | n/a |

No new dependencies introduced. Both `safetensors` and `torch` are already in the project's `worker/requirements/` files. PyTorch's `float8_e4m3fn` dtype is available in torch 2.5+ (the project's current torch build), which supports native FP8 tensor creation via `.to(torch.float8_e4m3fn)`.

## Approach

### Step 1: Write `build_flux2klein_9b_fixture.py`

Create the builder script following the exact pattern from `build_flux2klein_fixture.py`:

1. **Header and imports**: Same shebang, docstring explaining the fixture's purpose, `from __future__ import annotations`, imports for `os`, `sys`, `torch`, and `safetensors.torch.save_file`.

2. **Path resolution**: Identical pattern to the existing fixture — `_FIXTURE_DIR` from `os.path.dirname(os.path.abspath(__file__))` and `_REPO_ROOT` three levels up, inserted into `sys.path`.

3. **Hyperparameters**: Set `hidden_dim=256` (distinguishable from 4B's 128), `context_dim=4096` (distinguishable from 4B's 768), `latent_channels=4`, `patch_size=2`, `out_channels=4`. Keep block counts at 1 (minimal — shape inference counts max block index + 1, so a single block implies count=1, which is structurally valid).

4. **`_flux2klein_9b_tensors()` function**: Return a tensor dict with the same key patterns as the 4B fixture but with shapes scaled to the new hyperparameters:
   - `time_text_embed.timestep_embedder.0.weight`: `(256, 256)` — 256 KB
   - `time_text_embed.context_embedder`: `(256, 4096)` — 4 MB
   - `double_blocks.0.img_mod.lin`: `(256*6,)` = `(1536,)` — 6 KB
   - `double_blocks.0.txt_mod.lin`: `(1536,)` — 6 KB
   - `double_blocks.0.img_attn.qkv`: `(256, 256*3)` = `(256, 768)` — 768 KB
   - `double_blocks.0.img_attn.norm`: `(256,)` — 1 KB
   - `double_blocks.0.img_attn.proj`: `(256, 256)` — 256 KB
   - `double_blocks.0.txt_attn.qkv`: `(4096, 768)` — 12 MB — **TOO LARGE**
   
   **Adjustment**: The text attention QKV tensor at `(context_dim, hidden_dim*3)` = `(4096, 768)` would be ~12 MB alone, exceeding the 10 MB total limit. I need to reduce `context_dim` to something that keeps the total under 10 MB. With `context_dim=512`:
   - `time_text_embed.context_embedder`: `(256, 512)` = 512 KB
   - `double_blocks.0.txt_attn.qkv`: `(512, 768)` = 1.5 MB
   
   **Final hyperparameters**: `hidden_dim=256`, `context_dim=512`. Total file size ≈ 5.1 MB.

5. **`build()` function**: Save the tensors to `flux2klein9b_tiny.safetensors` with `metadata={"arch": "flux2klein"}`. No no-metadata variant needed (per-family requirement already covered).

### Step 2: Write `build_qwen3_8b_fixture.py`

Create the builder script following the pattern from `build_qwen3_fixture.py`:

1. **Header and imports**: Same structure as existing fixtures.

2. **Path resolution**: Identical pattern.

3. **Hyperparameters**: Set `hidden_dim=128` (distinguishable from 4B's 64), `num_hidden_layers=1`, `intermediate_size=256` (distinguishable from 4B's 128), `vocab_size=128` (same as 4B for size control).

4. **`_qwen3_8b_tensors()` function**: Return a tensor dict with Qwen3 key naming convention (`model.` prefix, separate `self_attn.{q,k,v,o}_proj` keys):
   - `model.layers.0.self_attn.q_proj.weight`: `(128, 128)` — 64 KB
   - `model.layers.0.self_attn.k_proj.weight`: `(128, 128)` — 64 KB
   - `model.layers.0.self_attn.v_proj.weight`: `(128, 128)` — 64 KB
   - `model.layers.0.self_attn.o_proj.weight`: `(128, 128)` — 64 KB
   - `model.layers.0.mlp.gate_proj.weight`: `(256, 128)` — 128 KB
   - `model.layers.0.mlp.up_proj.weight`: `(256, 128)` — 128 KB
   - `model.layers.0.mlp.down_proj.weight`: `(128, 256)` — 128 KB
   - `model.layers.0.input_layernorm.weight`: `(128,)` — 512 B
   - `model.layers.0.post_attention_layernorm.weight`: `(128,)` — 512 B
   - `model.norm.weight`: `(128,)` — 512 B
   - `model.embed_tokens.weight`: `(128, 128)` — 64 KB
   - **FP8 tensors** (to demonstrate mixed-precision): `model.layers.0.self_attn.q_proj.weight_fp8`, `model.layers.0.mlp.gate_proj.weight_fp8`, `model.embed_tokens.weight_fp8` — each at `torch.float8_e4m3fn` dtype (1 byte per element, ~16 KB total for FP8 tensors).

5. **`build()` function**: Save tensors to `qwen3_8b_tiny.safetensors` with `metadata={"arch": "qwen3"}`. No no-metadata variant needed.

### Step 3: Run both builder scripts

Execute each script from the repo root to generate the `.safetensors` files:
```bash
worker/.venv/bin/python worker/tests/fixtures/build_flux2klein_9b_fixture.py
worker/.venv/bin/python worker/tests/fixtures/build_qwen3_8b_fixture.py
```

### Step 4: Verify acceptance criteria

```bash
# Check file sizes
ls -la worker/tests/fixtures/flux2klein9b_tiny.safetensors worker/tests/fixtures/qwen3_8b_tiny.safetensors

# Verify both load via safetensors.safe_open
worker/.venv/bin/python -c "
from safetensors import safe_open
with safe_open('worker/tests/fixtures/flux2klein9b_tiny.safetensors', framework='np') as f:
    print(f'Flux 2 Klein 9B: {len(list(f.keys()))} tensors, metadata={f.metadata()}')
with safe_open('worker/tests/fixtures/qwen3_8b_tiny.safetensors', framework='np') as f:
    print(f'Qwen3-8B: {len(list(f.keys()))} tensors, metadata={f.metadata()}')
"
```

## Public API Surface

None. These are internal builder scripts (not imported by any production or test code). They have `build()` functions called from `if __name__ == "__main__"` blocks, and helper functions (`_tensors()`) that are internal to the script.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/tests/fixtures/build_flux2klein_9b_fixture.py` | Builder script for Flux 2 Klein 9B fixture |
| CREATE | `worker/tests/fixtures/flux2klein9b_tiny.safetensors` | Generated Flux 2 Klein 9B checkpoint (~5 MB) |
| CREATE | `worker/tests/fixtures/build_qwen3_8b_fixture.py` | Builder script for Qwen3-8B fixture |
| CREATE | `worker/tests/fixtures/qwen3_8b_tiny.safetensors` | Generated Qwen3-8B checkpoint (~320 KB) |

## Tests

This task does not add test code — tests for the new fixtures are handled by P26-B1 and P26-C1. The acceptance criteria are the build scripts' exit codes and file validation.

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| (acceptance) | build_flux2klein_9b exits 0 | Script runs without error | `worker/.venv/bin/python worker/tests/fixtures/build_flux2klein_9b_fixture.py` exits 0 |
| (acceptance) | build_qwen3_8b exits 0 | Script runs without error | `worker/.venv/bin/python worker/tests/fixtures/build_qwen3_8b_fixture.py` exits 0 |
| (acceptance) | flux2klein9b under 10 MB | File size constraint | `stat -c%s worker/tests/fixtures/flux2klein9b_tiny.safetensors | awk '{exit ($1 > 10485760)}'` exits 0 |
| (acceptance) | qwen3_8b under 10 MB | File size constraint | `stat -c%s worker/tests/fixtures/qwen3_8b_tiny.safetensors | awk '{exit ($1 > 10485760)}'` exits 0 |
| (acceptance) | flux2klein9b loads via safe_open | Structurally valid safetensors | `worker/.venv/bin/python -c "from safetensors import safe_open; safe_open('worker/tests/fixtures/flux2klein9b_tiny.safetensors', framework='np')"` exits 0 |
| (acceptance) | qwen3_8b loads via safe_open | Structurally valid safetensors | `worker/.venv/bin/python -c "from safetensors import safe_open; safe_open('worker/tests/fixtures/qwen3_8b_tiny.safetensors', framework='np')"` exits 0 |

## CI Impact

No CI changes required. The new fixture files are under `worker/tests/fixtures/` which is already included in the workspace. No new file types, gates, or test modules are introduced. The existing CI jobs (`worker-linux-mock`, `worker-linux-real`, `worker-windows-mock`, `worker-windows-real`) will pick up the new files automatically when later tasks (P26-B1, P26-C1) add tests that reference them.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. Both builder scripts use `os.path` for path resolution (cross-platform safe) and `torch.randn()` / `torch.save_file()` which are platform-neutral. The FP8 dtype (`torch.float8_e4m3fn`) is available on all torch builds that support it (CUDA, ROCm, CPU), and the fixture's FP8 tensors are only exercised by real-mode tests on CPU (torch CPU builds support FP8 tensor construction even if the hardware doesn't have FP8 compute units).

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `torch.float8_e4m3fn` not available in the agent's torch version | Low | Medium | Check torch version at the start of the builder script; if < 2.5, fall back to creating the fixture without FP8 tensors and note that FP8-mixed precision will be verified when the fixture is loaded by the real arch module in a later task. |
| Flux 2 Klein 9B tensor shapes exceed 10 MB due to context_dim scaling | Medium | High | Use `context_dim=512` (reduced from the real 4096) to keep total file size at ~5 MB. The shape inference formulas only care about the first dimension of projection keys being `hidden_dim`, not the absolute scale of `context_dim`. |
| `_infer_hyperparams()` reads ALL keys (P904 prevention) and the fixture has fewer keys than real models | Low | Low | The shape inference has fallback paths for all dimensions (block counts, latent dimensions, architecture detection). A fixture with 1 double block and 1 single block is structurally valid — the inference just returns `double_block_count=1`, `single_block_count=1`. |
| Qwen3-8B fixture's FP8 tensors produce numerical warnings during save | Low | Low | `save_file()` may warn about FP8 precision loss when converting from float32. This is expected and harmless — the fixture is synthetic. Suppress with `warnings.filterwarnings()` if needed. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python worker/tests/fixtures/build_flux2klein_9b_fixture.py` exits 0
- [ ] `worker/.venv/bin/python worker/tests/fixtures/build_qwen3_8b_fixture.py` exits 0
- [ ] `stat -c%s worker/tests/fixtures/flux2klein9b_tiny.safetensors` is less than 10485760 (10 MB)
- [ ] `stat -c%s worker/tests/fixtures/qwen3_8b_tiny.safetensors` is less than 10485760 (10 MB)
- [ ] `worker/.venv/bin/python -c "from safetensors import safe_open; f=safe_open('worker/tests/fixtures/flux2klein9b_tiny.safetensors', framework='np'); keys=list(f.keys()); print(f'{len(keys)} tensors'); del f"` exits 0
- [ ] `worker/.venv/bin/python -c "from safetensors import safe_open; f=safe_open('worker/tests/fixtures/qwen3_8b_tiny.safetensors', framework='np'); keys=list(f.keys()); print(f'{len(keys)} tensors'); del f"` exits 0
