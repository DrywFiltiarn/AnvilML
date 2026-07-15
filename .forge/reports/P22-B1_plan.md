# Plan Report: P22-B1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P22-B1                                            |
| Phase       | 22 — Qwen3 CLIP Arch Module                       |
| Description | worker/tests/fixtures/: Qwen3 CLIP fixture safetensors builder |
| Depends on  | P22-A1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-15T10:45:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create a Python builder script (`worker/tests/fixtures/build_qwen3_fixture.py`) that
generates a tiny synthetic `.safetensors` checkpoint file with Qwen3 text-encoder-shaped
tensor keys, following the fixture conventions documented in
`worker/tests/fixtures/README.md` (established Phase 19, P19-D1). Run the script to
produce `worker/tests/fixtures/qwen3_tiny.safetensors` (committed to git). The file must
be under 10 MB and load successfully via `safetensors.safe_open`. This fixture provides
the structural shape validation that subsequent Phase 22 tasks (B2–E1) will exercise
through the real-mode loading contract.

## Scope

### In Scope
- Create `worker/tests/fixtures/build_qwen3_fixture.py` — a builder script following
  the README.md conventions (no arguments, idempotent, writes to the fixtures directory,
  uses structurally valid but small tensor shapes).
- Run the script to produce `worker/tests/fixtures/qwen3_tiny.safetensors`.
- The fixture tensors use Qwen3 text-encoder key patterns
  (`model.embed_tokens.weight`, `model.layers.N.self_attn.*_proj.weight`,
  `model.layers.N.mlp.*_proj.weight`, `model.layers.N.*_layernorm.weight`,
  `model.norm.weight`) with a consistent hidden dimension of 64, 2 layers, and a small
  vocab size of 128 — structurally valid for the shape-inference formula `qwen3.py` will
  implement in P22-B2, NOT a miniaturized copy of the real model's actual shapes.
- The fixture includes an `arch: "qwen3"` metadata key in the safetensors header so the
  loader can identify the architecture from metadata (a separate no-metadata variant is
  deferred to a later task that implements the loader's real branch and needs the
  metadata-fallback regression case — P22-D1 or P22-C2, as appropriate).

### Out of Scope
- The shape-inference function (`_infer_hyperparams()` in `qwen3.py`) — implemented in
  P22-B2.
- The `can_handle()` and dispatch registration — implemented in P22-B3.
- The metadata-fallback regression fixture (non-recognizable key prefix, no `arch`
  metadata) — deferred to the task that implements the loader's real branch, per the
  convention that building a fixture is part of the same task that implements the
  corresponding `load()` function.
- Tests for the fixture — the fixture itself has no tests beyond the acceptance criterion
  (script exits 0, file under 10 MB, loads via `safe_open`); test coverage comes in
  P22-B2 through P22-D1 when the actual loading code is exercised against this fixture.

## Existing Codebase Assessment

The `worker/tests/fixtures/` directory already contains three fixture files and two builder
scripts from Phase 19 (P19-D1) and Phase 20 (P20-A1): `build_zit_fixture.py`,
`build_zit_fp8_fixture.py`, `zit_tiny.safetensors`, `zit_tiny_no_metadata.safetensors`,
and `zit_tiny_fp8.safetensors`. The builder scripts follow a consistent pattern: they
resolve the fixtures directory relative to the script's own location, define a tensor
factory function returning a `dict[str, torch.Tensor]`, and a `build()` function that
calls `save_file()` with the appropriate metadata.

The `worker/nodes/arch/clip/__init__.py` dispatcher exists with zero registered modules
and a `get_module(key)` function that iterates `_REGISTERED_MODULES` calling
`can_handle(key)` on each. The `qwen3.py` arch module does not yet exist — it will be
created in P22-B2.

The established patterns to follow: (1) resolve paths relative to `__file__` for
idempotency; (2) use `from __future__ import annotations`; (3) include a module-level
docstring explaining the fixture's purpose and usage; (4) use `torch.randn()` for
deterministic random tensors; (5) keep tensor shapes small but structurally consistent
(consistent hidden dim across all tensors).

No gap between design doc and current source affects this task — the fixture conventions
are well-documented and the existing zit fixtures serve as a clear reference pattern.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source     | Feature flags confirmed |
|--------|-------------|-----------------|----------------|------------------------|
| python | safetensors | 0.8.0           | pypi-query MCP | n/a                    |
| python | torch       | (from venv)     | —              | n/a                      |

`safetensors` 0.8.0 is confirmed compatible with Python 3.12 (`requires_python >= 3.10`,
verified via MCP). The `safetensors.torch.save_file()` function and
`safetensors.safe_open()` function are the standard public API — confirmed by the
existing zit fixture scripts which use the same imports.

## Approach

1. **Create `worker/tests/fixtures/build_qwen3_fixture.py`.**
   - Follow the exact structure of `build_zit_fixture.py`: module-level docstring,
     `from __future__ import annotations`, path resolution via `os.path.dirname(os.path.abspath(__file__))`,
     a tensor factory function, and a `build()` function.
   - The tensor factory function returns a dict of Qwen3 text-encoder-shaped tensors:
     - `model.embed_tokens.weight` — `(128, 64)` — vocab_size × hidden_dim embedding
     - `model.layers.0.self_attn.q_proj.weight` — `(64, 64)` — query projection
     - `model.layers.0.self_attn.k_proj.weight` — `(64, 64)` — key projection
     - `model.layers.0.self_attn.v_proj.weight` — `(64, 64)` — value projection
     - `model.layers.0.self_attn.o_proj.weight` — `(64, 64)` — output projection
     - `model.layers.0.mlp.gate_proj.weight` — `(128, 64)` — MLP gate projection
     - `model.layers.0.mlp.up_proj.weight` — `(128, 64)` — MLP up projection
     - `model.layers.0.mlp.down_proj.weight` — `(64, 128)` — MLP down projection
     - `model.layers.0.input_layernorm.weight` — `(64,)` — layer norm scale
     - `model.layers.0.post_attention_layernorm.weight` — `(64,)` — post-attention layer norm
     - `model.layers.1.self_attn.q_proj.weight` — `(64, 64)` — second layer (same pattern)
     - `model.layers.1.self_attn.k_proj.weight` — `(64, 64)`
     - `model.layers.1.self_attn.v_proj.weight` — `(64, 64)`
     - `model.layers.1.self_attn.o_proj.weight` — `(64, 64)`
     - `model.layers.1.mlp.gate_proj.weight` — `(128, 64)`
     - `model.layers.1.mlp.up_proj.weight` — `(128, 64)`
     - `model.layers.1.mlp.down_proj.weight` — `(64, 128)`
     - `model.layers.1.input_layernorm.weight` — `(64,)`
     - `model.layers.1.post_attention_layernorm.weight` — `(64,)`
     - `model.norm.weight` — `(64,)` — final normalization
   - All tensors use `torch.randn(...)` (float32 default). Hidden dimension is 64
     (consistent across all tensors), num_hidden_layers is 2, intermediate_size is 128,
     vocab_size is 128. These are structurally valid dimensions for a transformer-based
     text encoder shape-inference formula — not a miniaturized copy of the real 4B
     model's actual shapes.
   - The `build()` function calls `save_file()` with `metadata={"arch": "qwen3"}` so the
     loader can identify the architecture from the safetensors header metadata.
   - Include a Google-style docstring on the tensor factory function explaining the key
     patterns and shape rationale.

2. **Run the builder script to produce the fixture file.**
   - Execute `python worker/tests/fixtures/build_qwen3_fixture.py` from the repo root.
   - Verify the output file exists and is under 10 MB.
   - Verify the file loads successfully via `safetensors.safe_open`.

3. **The generated `qwen3_tiny.safetensors` file is committed to git** by The Forge
   orchestrator (PLAN agents do not commit).

## Public API Surface

None. This task creates a builder script and a data file — no library code, no pub
functions, no types. The script's `build()` function is a private entry point called
only from `if __name__ == "__main__"`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/tests/fixtures/build_qwen3_fixture.py` | Builder script generating the Qwen3 tiny fixture |
| CREATE | `worker/tests/fixtures/qwen3_tiny.safetensors` | Generated fixture file (tiny synthetic checkpoint) |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| (manual) | Script execution and file validation | `build_qwen3_fixture.py` exits 0, produces a file under 10 MB, loads via `safe_open` | `python worker/tests/fixtures/build_qwen3_fixture.py && stat -c%s worker/tests/fixtures/qwen3_tiny.safetensors | grep -qE '^[0-9]{1,6}$' && python -c "from safetensors import safe_open; safe_open('worker/tests/fixtures/qwen3_tiny.safetensors', framework='pt')"` |

The acceptance criterion is the task's own runnable proof: the script exits 0, the
file is under 10 MB, and it loads via `safetensors.safe_open`. No separate test file
is needed — this is a data-generation task, not a code-modification task.

## CI Impact

No CI changes required. The fixture file is a small data file added to the repository.
It is consumed by real-mode tests in later Phase 22 tasks (P22-B2 through P22-E1),
but those tasks will add their own test files and markers. The fixture builder script
itself is not invoked by any CI job — it is a one-time generation tool run during
development.

## Platform Considerations

None identified. The builder script uses only `torch` and `safetensors`, both of which
are platform-neutral at the tensor creation level. The generated `.safetensors` file
is a binary format independent of host platform. The Windows cross-check in
ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The Qwen3 text encoder key patterns I choose may not match the actual key structure that `qwen3.py`'s `_infer_hyperparams()` will expect, causing shape inference to fail when P22-B2 reads this fixture. | Medium | High | The tensor keys follow the standard Qwen/Qwen3 transformer text-encoder safetensors key naming convention (`model.embed_tokens.*`, `model.layers.N.*`). If P22-B2 discovers a different key pattern, the fixture can be regenerated by re-running this builder script with updated keys — the script is idempotent and the change is local to this file. |
| The fixture file exceeds 10 MB due to unexpected tensor sizes. | Low | Medium | All tensors use small dimensions (hidden_dim=64, 2 layers, vocab=128). A rough estimate: ~20 tensors averaging 64×64 float32 = 16 KB each ≈ 320 KB total — well under 10 MB. The acceptance command explicitly checks file size. |
| `safetensors` API shape differs between the training-data version and the MCP-resolved 0.8.0. | Low | High | MCP-verified: `safetensors.torch.save_file()` and `safetensors.safe_open()` are the standard public API confirmed by the existing zit fixture scripts which use the same imports and have been running in CI. |

## Acceptance Criteria

- [ ] `python worker/tests/fixtures/build_qwen3_fixture.py` exits 0
- [ ] `stat -c%s worker/tests/fixtures/qwen3_tiny.safetensors` prints a number ≤ 10485760 (10 MB)
- [ ] `python -c "from safetensors import safe_open; safe_open('worker/tests/fixtures/qwen3_tiny.safetensors', framework='pt')"` exits 0 (no exception)
