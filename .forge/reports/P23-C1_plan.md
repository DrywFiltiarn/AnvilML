# Plan Report: P23-C1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P23-C1                                            |
| Phase       | 23 — ZiT VAE Arch Module                          |
| Description | worker/nodes/arch/vae/zit_vae.py: meta construction + dtype selection |
| Depends on  | P23-B2                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-17T02:45:00Z                              |
| Attempt     | 1                                                 |

## Objective

Implement step 2 of the four-step loading contract (`ANVILML_DESIGN.md §11.3`) for the
ZiT VAE arch module: construct a `ZiTVaeModel` nn.Module on `torch.device("meta")` using
only the hyperparameters inferred by `_infer_hyperparams()` (P23-B1), with the compute
dtype chosen per the fixed precedence chain in §11.5 (fp8 → bf16 → fp16 → fp32, reading
`ctx.caps`). This is the meta-construction step — no memory is allocated for parameters,
preventing the ~15 GB crash that P904 experienced. Step 3 (materialize via `to_empty()`,
build key remapping, `load_state_dict(assign=True)`) and the `.arch` attribute are the
next task's scope (P23-C3). The `load()` function is a partial stub: it returns the
meta-constructed module with dtype applied but no weights loaded yet.

## Scope

### In Scope
- Create `ZiTVaeModel(nn.Module)` class in `zit_vae.py` that constructs the VAE encoder,
  mid-block, and decoder from P23-B1's inferred hyperparameters (`encoder_channels`,
  `decoder_channels`, `latent_channels`) using `torch.nn.Conv2d`, `torch.nn.GroupNorm`,
  and `torch.nn.SiLU` — diffusers'/torch's layer classes per §11.2's library boundary.
- Create `_select_dtype(caps: dict, native_dtype: str) -> torch.dtype` implementing the
  fixed precedence from §11.5 (fp8 → bf16 → fp16 → fp32). This is the dtype selection
  function; the actual application of dtype to the module construction is done in this
  task. (P23-C2's scope is extending `load()` to read `ctx.caps` and pass it through —
  the precedence logic itself is implemented here as a reusable function.)
- Create `load(path: str, caps: dict, device: str = "cpu") -> ZiTVaeModel` partial stub:
  step 1 (infer hyperparams via `_infer_hyperparams()`, already exists) → step 2
  (select dtype via `_select_dtype()`, construct on meta, apply dtype). Returns the
  meta-constructed module with dtype applied. Does NOT materialize, remap keys, or load
  weights.
- Add `try/except ImportError` guard for torch/nn imports (per §11.2) so the module stays
  importable in mock-mode collection.
- Add `logging` import and a module-level `logger` instance.
- Write >=3 new tests in `test_arch_vae_zit.py` (>=10 total in file).

### Out of Scope
- Step 3: materialize via `to_empty()`, build checkpoint-key → module-key remapping,
  call `load_state_dict(assign=True)`, and return a fully-loaded module. Deferred to
  P23-C3.
- Step 4: set `.arch` attribute on the returned module. Deferred to P23-C3.
- The dual-mode parity markers (`REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED`) on `load()`
  — these require a fully functional `load()` that can be exercised in both mock and real
  mode. P23-C1's `load()` is a partial stub (meta construction only) that cannot produce
  a usable model, so markers are premature. P23-C3 will add them when `load()` is complete.
  (P23-C2 also defers to P23-C3 for markers, per §10.6.)
- `decode()` — the VAE family's second fixed method. Deferred to P23-D1.
- `can_handle()` and dispatch registration — already implemented by P23-B2.

## Existing Codebase Assessment

**What already exists:** `zit_vae.py` (280 lines) implements step 1 of the loading
contract: `_infer_hyperparams()` reads the safetensors header, infers
`encoder_channels`, `decoder_channels`, `latent_channels`, `arch`, and `native_dtype`.
`can_handle()` is also implemented. The VAE dispatcher in `arch/vae/__init__.py` already
registers `zit_vae` as its first entry. The existing test file `test_arch_vae_zit.py` has
7 tests covering `_infer_hyperparams`, `ARCH`, `can_handle()`, and `get_module()`.

**Established patterns:** `zit.py` (Phase 20) is the reference implementation. It follows
the exact same discipline: guarded torch imports at module scope, a model class that
inherits from `nn.Module` (or `object` when torch is absent), meta-device construction,
dtype selection via `_select_dtype()`, and a partial `load()` that defers materialization
and key remapping. The `try/except ImportError` guard pattern uses `_ModuleBase =
nn.Module if nn is not None else object` as the conditional base class.

**Gap between design doc and current source:** The VAE architecture is structurally
different from the ZiT diffusion transformer. `zit.py` uses `nn.Linear`,
`nn.MultiheadAttention`, `nn.LayerNorm`, and `nn.GELU` for a transformer architecture.
The VAE uses `nn.Conv2d`, `nn.GroupNorm`, and `nn.SiLU` for a UNet-style encoder/decoder.
The channel dimensions from `_infer_hyperparams()` (encoder_channels, decoder_channels,
latent_channels) map to convolutional layer in/out channels, not transformer hidden
dimensions. This requires a new `ZiTVaeModel` class with a different internal structure.

## Resolved Dependencies

| Type   | Name          | Version verified | MCP source | Feature flags confirmed |
|--------|---------------|-----------------|------------|------------------------|
| python | torch         | via pip freeze  | pypi-query | n/a                    |
| python | diffusers     | via pip freeze  | pypi-query | n/a                    |
| python | safetensors   | via pip freeze  | pypi-query | n/a                    |

No new external dependencies are introduced. All types used (`torch`, `torch.nn`,
`torch.device`, `torch.dtype`) are from the existing `torch` package, which is already
a dependency of this project (installed via `requirements/cpu-linux-agent.txt` or
matching hardware-specific requirements file). The `try/except ImportError` guard is
already established in `zit.py` and `qwen3.py`.

## Approach

1. **Add import guards and logger to `zit_vae.py`.** Add `import logging` and a
   module-level `logger = logging.getLogger(__name__)` at the top of the file. Add the
   torch/nn guarded import block (following `zit.py`'s pattern): wrap `import torch`
   and `import torch.nn as nn` in a `try/except ImportError`, setting each to `None` on
   failure. This ensures the module remains importable during mock-mode test collection
   (where torch is not installed), per `ANVILML_DESIGN.md §11.2`.

2. **Create `ZiTVaeModel(nn.Module)` class.** Define a new class that constructs the VAE
   encoder, mid-block, and decoder from hyperparameters:
   - `_ModuleBase = nn.Module if nn is not None else object` (conditional base class,
     same pattern as `zit.py` line 93).
   - `__init__(self, hyperparams: dict[str, Any]) -> None`: extract
     `encoder_channels`, `decoder_channels`, `latent_channels` from hyperparams.
   - Construct encoder blocks: a list of `nn.ModuleDict` entries, each containing a
     `conv` (Conv2d with `in_channels=encoder_channels→out_channels`, kernel 3, pad 1)
     and `norm` (GroupNorm with 8 groups and matching out_channels). Apply SiLU after
     each conv. The number of encoder blocks is derived from the fixture's structure
     (the fixture has `encoder.blocks.0.conv.weight` — one block per the tiny fixture).
   - Construct mid-block: a single `nn.ModuleDict` with conv + norm (uses
     `latent_channels` as both in and out channels).
   - Construct decoder blocks: a list of `nn.ModuleDict` entries, each containing a
     `conv` (Conv2d with `in_channels=decoder_channels→out_channels`, kernel 3, pad 1)
     and `norm`. The output of the final decoder block produces the decoded image.
   - Use `torch.nn.functional.silu()` as the activation (SiLU is the standard VAE
     activation in diffusers' AutoencoderKL).

   The exact number of blocks is derived from the fixture's key patterns: the fixture
   has `encoder.blocks.0.conv.weight` and `decoder.blocks.0.conv.weight`, indicating at
   least one block each. For the tiny fixture, one block per encoder/decoder is
   sufficient to exercise the construction. The constructor iterates over the
   hyperparams dict to determine block counts — in practice, the fixture has 1 encoder
   block and 1 decoder block, so the model will have exactly that.

   The class stores `.arch = "zit_vae"` as a plain attribute (set in `__init__`), so
   it is available on the meta-constructed module. The `.arch` attribute is set here
   rather than in `load()` because it is a property of the model class, not the loading
   step — P23-C3 will also verify it persists after materialization.

3. **Create `_select_dtype(caps: dict, native_dtype: str) -> torch.dtype`.** Implement
   the fixed precedence from §11.5:
   - Branch 1: `caps.get("fp8", False) and native_dtype == "fp8"` → `torch.float8_e4m3fn`
   - Branch 2: `caps.get("bf16", False)` → `torch.bfloat16`
   - Branch 3: `caps.get("fp16", False)` → `torch.float16`
   - Branch 4: default → `torch.float32`

   This is a pure function with no side effects. It mirrors `_select_dtype()` from
   `zit.py` (line 1086) exactly, but is defined independently in `zit_vae.py` per the
   per-module contract-following discipline stated in P23-C2's context.

4. **Create partial `load(path: str, caps: dict, device: str = "cpu") -> ZiTVaeModel`.**
   The stub implements steps 1-2 of the loading contract:
   - Guard: if `torch is None`, raise `RuntimeError` with a clear message (same pattern
     as `zit.py` line 480-485).
   - Step 1: `hyperparams = _infer_hyperparams(path)` (already exists).
   - Step 2a: `target_dtype = _select_dtype(caps, hyperparams["native_dtype"])`.
   - Step 2b: `with torch.device("meta"): model = ZiTVaeModel(hyperparams)`.
   - Step 2c: `model.to(target_dtype)` — apply dtype to the meta-constructed module.
     On meta device, `.to(dtype)` changes parameter dtype metadata without allocating
     real memory.
   - Return `model` immediately. Do NOT call `to_empty()`, do NOT build key remapping,
     do NOT call `load_state_dict()`. These are P23-C3's scope.

   Log the dtype selection at DEBUG level with the selected dtype and device:
   `logger.debug("selected dtype=%s for VAE on device=%s", target_dtype, device)`.

5. **Write tests in `test_arch_vae_zit.py`.** Add >=3 new tests:
   - `test_load_meta_construction_succeeds`: call `load()` against the regular fixture
     path with a caps dict (bf16=True), assert the returned module is a `ZiTVaeModel`,
     its parameters are on `torch.device("meta")` (verify via `param.device`), and no
     real memory was allocated (the meta device means `param.numel()` > 0 but actual
     memory is zero).
   - `test_load_meta_construction_no_metadata_fixture`: call `load()` against the
     no-metadata fixture variant, assert it succeeds and returns a valid `ZiTVaeModel`
     with meta-device parameters.
   - `test_load_dtype_selection_applied`: call `load()` with caps that select fp32
     (all False), assert the model's parameters have `dtype == torch.float32` (on meta
     device, this checks the dtype metadata, not actual tensor data).

   All three tests need `real_mode` marker because `load()` imports torch (guarded,
   but the test body calls `load()` which requires torch to be importable). These tests
   are collected in mock-mode CI (the guard prevents import errors) but only run in
   real-mode.

## Public API Surface

| Item | Module Path | Signature / Description |
|------|-------------|------------------------|
| `ZiTVaeModel` | `worker.nodes.arch.vae.zit_vae` | `class ZiTVaeModel(_ModuleBase)` — nn.Module subclass, `__init__(self, hyperparams: dict[str, Any]) -> None`, sets `.arch = "zit_vae"` |
| `_select_dtype` | `worker.nodes.arch.vae.zit_vae` | `_select_dtype(caps: dict, native_dtype: str) -> torch.dtype` — pure function, fixed precedence fp8→bf16→fp16→fp32 |
| `load` | `worker.nodes.arch.vae.zit_vae` | `load(path: str, caps: dict, device: str = "cpu") -> ZiTVaeModel` — partial stub (steps 1-2 of §11.3) |

Note: `load()` is not `pub` in the Rust sense — it is the Python arch module's
entry point, called by `LoadVae`'s real branch (P23-E1). It follows the fixed
method-name contract from `ANVILML_DESIGN.md §10.4`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/arch/vae/zit_vae.py` | Add torch/nn guarded imports, logger, `ZiTVaeModel` class, `_select_dtype()`, partial `load()` stub |
| Modify | `worker/tests/test_arch_vae_zit.py` | Add >=3 new tests for meta construction, dtype application, and no-metadata fixture variant |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `test_arch_vae_zit.py` | `test_load_meta_construction_succeeds` | `load()` against regular fixture returns a `ZiTVaeModel` with meta-device parameters (zero real memory) | `python -m pytest worker/tests/test_arch_vae_zit.py::test_load_meta_construction_succeeds -v` |
| `test_arch_vae_zit.py` | `test_load_meta_construction_no_metadata_fixture` | `load()` against no-metadata fixture variant succeeds, returns `ZiTVaeModel` with meta-device parameters | `python -m pytest worker/tests/test_arch_vae_zit.py::test_load_meta_construction_no_metadata_fixture -v` |
| `test_arch_vae_zit.py` | `test_load_dtype_selection_applied` | Model parameters have the dtype selected by `_select_dtype()` (fp32 when all caps are False) | `python -m pytest worker/tests/test_arch_vae_zit.py::test_load_dtype_selection_applied -v` |

Acceptance: `python -m pytest worker/tests/test_arch_vae_zit.py -v` exits 0 with >=10
total tests (7 existing + >=3 new).

## CI Impact

No CI changes required. The new tests use `real_mode` marker (they call `load()` which
requires torch), so they run in the `worker-linux-real` and `worker-windows-real` CI
jobs but not in the mock-only jobs. The guarded torch imports ensure the module stays
importable during mock-mode collection (the CI `worker-*-mock` jobs install only
`requirements/base.txt`, no torch).

## Platform Considerations

None identified. The meta-device construction is platform-neutral — `torch.device("meta")`
works identically on Linux and Windows. No `# cfg` guards are needed. The Windows
cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The VAE architecture (encoder blocks, mid-block, decoder blocks) may not match the actual ZiT-VAE checkpoint's structure, causing key remapping in P23-C3 to fail because the constructed module's state_dict keys don't align with the checkpoint keys. | Medium | High | The P23-A1 fixture is a synthetic checkpoint with known keys (`encoder.blocks.0.conv.weight`, etc.). P23-C3's key remapping will be built against this fixture's actual key set, so even if the architecture doesn't perfectly match a production checkpoint, the remapping will work for the fixture. The P23-C3 task explicitly states "key remap table is built against this phase's own fixture." |
| `model.to(dtype)` on a module with meta-device parameters may not change dtype metadata in the torch version installed on the CI runner. | Low | Medium | This is standard PyTorch behavior confirmed in `zit.py` (line 523). If it fails, the ACT agent will verify the torch version via `python -c "import torch; print(torch.__version__)"` and adjust. |
| The guarded torch import pattern may not work correctly if `torch.nn.SiLU` or `torch.nn.GroupNorm` is referenced before the guard sets `nn = None`. | Low | Medium | The guard wraps `import torch` and `import torch.nn as nn` together, and `_ModuleBase` is set immediately after. The `ZiTVaeModel` class only references `nn` inside `__init__`, which is never called without torch present. Follow `zit.py`'s exact pattern. |

## Acceptance Criteria

- [ ] `python -m py_compile worker/nodes/arch/vae/zit_vae.py` exits 0 (syntax check)
- [ ] `python -m pytest worker/tests/test_arch_vae_zit.py --collect-only -q` outputs >=10 test items
- [ ] `python -m pytest worker/tests/test_arch_vae_zit.py -v` exits 0 (>=10 total tests pass)
- [ ] `python -c "from worker.nodes.arch.vae.zit_vae import ZiTVaeModel, _select_dtype, load; print('OK')"` exits 0 (imports work with torch absent — mock-mode collection)
- [ ] `python -c "import torch; from worker.nodes.arch.vae.zit_vae import load; m = load('worker/tests/fixtures/zit_vae_tiny.safetensors', {'bf16': True, 'fp16': True, 'fp8': False, 'fp32': True}, 'cpu'); assert all(p.device.type == 'meta' for p in m.parameters()); print('meta OK')"` exits 0 (meta construction verified)
