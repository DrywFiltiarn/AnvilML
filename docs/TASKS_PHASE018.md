# Tasks: Phase 018 — ZiT Generic Nodes

| Field | Value |
|-------|-------|
| Phase | 018 |
| Name | ZiT Generic Nodes |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 17 |

## Overview

Phase 018 implements the full set of generic inference nodes using Z-Image Turbo FP8 safetensors as the first real model. All nodes are architecture-agnostic — `LoadModel`, `LoadVae`, `LoadClip`, `ClipTextEncode`, `EmptyLatent`, `Sampler`, `VaeDecode`, `SaveImage` — with ZiT-specific dispatch living in `worker/nodes/arch/zit.py`.

Every node has both a mock path (fast sentinel, `ANVILML_WORKER_MOCK=1`) and a real path (loads actual FP8 safetensors). The mock path must never import `torch` or `diffusers`. The real path uses `safetensors` for weight loading, `diffusers` for pipeline components, and `torch` for inference.

After Phase 018, a real ZiT FP8 workflow submitted to a server with a GPU and the correct model files produces a PNG artifact.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | worker loader nodes | P18-A1 … P18-A3 | LoadModel, LoadVae, LoadClip |
| B | worker inference nodes | P18-B1 … P18-B3 | ClipTextEncode, EmptyLatent, Sampler, VaeDecode |
| C | worker pipeline | P18-C1 | pipeline_cache.py LRU model cache |
| D | worker arch | P18-D1 | arch/zit.py ZiT FP8 dispatch module |
| E | integration | P18-E1 | test_parity.py + real ZiT smoke proof doc |

## Prerequisites

Phase 017 complete. `worker/nodes/__init__.py` and `base.py` exist. `worker/worker_main.py` handles Execute with `run_graph`. The `SaveImage` node (mock) exists from Phase 014.

## Interfaces and Contracts

| Contract document | Relevant tasks | What must match |
|-------------------|---------------|-----------------|
| `ANVILML_DESIGN.md §10.3` | All | Node type names, INPUT_SLOTS, OUTPUT_SLOTS per table |
| `ANVILML_DESIGN.md §10.4` | P18-D1 | `can_handle()` and `sample()` arch module interface |
| `ANVILML_DESIGN.md §10.5` | P18-C1, P18-D1 | FP8 dtype handling: transformer stays float8, no upcast; text_encoder/vae stay bf16 |
| `ANVILML_DESIGN.md Appendix B` | P18-E1 | Example workflow JSON structure |

## Task Descriptions

### Group A — Loader nodes

#### P18-A1: worker/nodes/loader.py: LoadModel node

**Goal:** Implement `LoadModel` node: `INPUT_SLOTS=[SlotSpec("model_id","STRING")]`, `OUTPUT_SLOTS=[SlotSpec("model","MODEL")]`. Mock: return `{"model": MockModel(arch="zit")}`. Real: use `safetensors.safe_open` to load FP8 safetensors; detect arch from metadata; load UNet/DiT weights into appropriate diffusers component via `pipeline_cache.get_or_load()`. Every public function and class needs a doc comment. Every decision point needs an inline comment.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0 with ≥ 4 tests.

#### P18-A2: worker/nodes/loader.py: LoadVae node

**Goal:** Add `LoadVae` node to `loader.py`: `INPUT_SLOTS=[SlotSpec("model_id","STRING")]`, `OUTPUT_SLOTS=[SlotSpec("vae","VAE")]`. Mock: return `{"vae": MockVae()}`. Real: load VAE safetensors via `pipeline_cache`. `LoadModel` outputs only `MODEL` — it never provides a VAE.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::TestLoadVae -v` exits 0 ≥ 3 tests.

#### P18-A3: worker/nodes/loader.py: LoadClip node

**Goal:** Add `LoadClip` node to `loader.py`: `INPUT_SLOTS=[SlotSpec("model_id","STRING"), SlotSpec("clip_type","STRING",optional=True)]`, `OUTPUT_SLOTS=[SlotSpec("clip","CLIP")]`. Mock: return `{"clip": MockClip(clip_type=clip_type or "qwen3")}`. Real: load text encoder safetensors. `clip_type` hint selects tokeniser (`"qwen3"`, `"clip_l"`, `"t5"`).

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::TestLoadClip -v` exits 0 ≥ 3 tests.

### Group B — Inference nodes

#### P18-B1: worker/nodes/encoder.py: ClipTextEncode node

**Goal:** Implement `ClipTextEncode` node: inputs `clip:CLIP, text:STRING, negative_text:STRING(optional)`, outputs `conditioning:CONDITIONING`. Mock: return `{"conditioning": MockConditioning(text=text)}`. Real: call `clip.encode(text)` (arch-agnostic interface on the CLIP object).

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_encoder.py -v` exits 0 ≥ 3 tests.

#### P18-B2: worker/nodes/sampler.py: EmptyLatent and Sampler nodes

**Goal:** Implement `EmptyLatent`: inputs `width:INT, height:INT, batch_size:INT(optional)`, outputs `latent:LATENT`. Mock: return `{"latent": MockLatent(width,height)}`. Implement `Sampler`: inputs `model:MODEL, conditioning:CONDITIONING, latent:LATENT, steps:INT, cfg:FLOAT, seed:INT`, outputs `latent:LATENT, seed:INT`. Mock: emit 3 Progress events, return `{"latent": MockLatent, "seed": resolved_seed}`. Real: call arch dispatch `arch.sample(model, conditioning, latent, steps, cfg, seed, ...)`.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py -v` exits 0 ≥ 5 tests (seed=-1 resolves; progress events emitted; latent returned).

#### P18-B3: worker/nodes/decode.py: VaeDecode node

**Goal:** Implement `VaeDecode`: inputs `vae:VAE, latent:LATENT`, outputs `image:IMAGE`. VAE is always an explicit required input. Mock: return `{"image": MockImage()}`. Real: call `vae.decode(latent)` → PIL Image. Update SaveImage in image.py to emit real ImageReady event with PNG base64.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_decode.py -v` exits 0 ≥ 3 tests.

### Group C — Pipeline cache

#### P18-C1: worker/pipeline_cache.py: LRU model cache

**Goal:** Implement `pipeline_cache.py` with `PipelineCache(max_entries=2)`: `get_or_load(model_id, dtype, loader_fn)` — return cached value or call `loader_fn()` and cache result. Evict LRU entry when max_entries exceeded. Log eviction at INFO. This module has no dependency on `arch/` — it is a generic keyed cache used by the loader nodes (P18-A1–A3) for raw components and, from P18-D1 onward, by arch modules for assembled pipeline objects.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_pipeline_cache.py -v` exits 0 ≥ 4 tests (cache hit, cache miss, LRU eviction, max_entries=1).

### Group D — ZiT architecture module

#### P18-D1: worker/nodes/arch/zit.py: ZiT FP8 sampling dispatch

**Goal:** Create `worker/nodes/arch/__init__.py` (architecture registry) and `worker/nodes/arch/zit.py` implementing:
- `can_handle(model_obj) -> bool` — returns True if `model_obj.arch == "zit"`
- `sample(model, conditioning, latent, steps, cfg, seed, device, cancel_flag, emit_progress) -> tuple[latent_tensor, int]`

**Real class:** `diffusers.ZImagePipeline` — confirmed present in `diffusers` from release 0.36.0 onward (PRs huggingface/diffusers#12703, #12715). There is no `diffusers.ZitPipeline`; that name does not exist anywhere in the library.

**Component/pipeline caching split:** `model` is the diffusion transformer component, already loaded and cached by `LoadModel` (P18-A1) via `pipeline_cache.get_or_load(model_id, ...)`. `arch/zit.py` does not call `ZImagePipeline.from_pretrained(model_id)` inside `sample()` — that would reload the full model from disk on every sampling call. Instead, on the first `sample()` call for a given `model_id`, `arch/zit.py` assembles a `ZImagePipeline` instance from the already-loaded `transformer`, `vae`, and `text_encoder` components and caches the **assembled pipeline object itself** under a separate cache key (`f"{model_id}:pipeline"`) via `pipeline_cache.get_or_load()`. Subsequent calls for the same `model_id` reuse the cached pipeline.

Per-step callback checks `cancel_flag.is_set()` and calls `emit_progress(step, total_steps)` via the pipeline's `callback_on_step_end` hook. Every FP8 decision point in the real path has an inline comment: the transformer stays at `float8` dtype (no upcast) when `InferenceCaps.fp8` is `True`; the text encoder and VAE remain at `bf16`, since only the diffusion transformer is distributed as FP8 in the Z-Image Turbo release.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v` exits 0; mock `can_handle` + `sample` tests pass.

### Group E — Integration

#### P18-E1: test_parity.py + ZiT smoke proof documentation

**Goal:** Create `worker/tests/test_parity.py` verifying that NODE_REGISTRY contains exactly the 9 baseline node types from `ANVILML_DESIGN.md §10.3`. Create `docs/PROOF_phase018.md` documenting the manual real-hardware runnable proof: exact curl commands to submit the Appendix B ZiT workflow JSON and observe JobCompleted + PNG artifact.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_parity.py -v` exits 0; PROOF_phase018.md documents all commands and expected output.

## Phase Acceptance Criteria

```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v
cargo test --workspace --features mock-hardware
# Real hardware proof (manual, requires ZiT FP8 safetensors in models/):
# cargo run --features real-hardware
# curl -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' -d @docs/example_workflows/zit_fp8.json
# poll /v1/jobs/:id until Completed; curl /v1/artifacts/:hash -> image/png
```

## Known Constraints and Gotchas

- Nodes must NEVER import `torch`, `diffusers`, or `safetensors` at module top level. All real-hardware imports are inside `if not _mock:` guards so CI (mock mode) never touches them.
- The `pipeline_cache.py` OOM handler: if `torch.cuda.OutOfMemoryError` is raised during `loader_fn()`, evict all cached entries and retry once before propagating the error.
- FP8 safetensors require `torch >= 2.1` and a GPU with FP8 compute capability (Ada Lovelace+ for NVIDIA, RDNA3+ for AMD). The worker checks `InferenceCaps.fp8` before attempting FP8 loading.
- `diffusers>=0.36.0` is required (`worker/requirements/base.txt`) — this is the release that introduced both `ZImagePipeline` and the Flux 2 family (`Flux2Pipeline`, `Flux2KleinPipeline`). Earlier releases do not have these classes.
- There is no `diffusers.ZitPipeline`. The correct class is `diffusers.ZImagePipeline`.
- `diffusers.FluxPipeline` **does exist**, but it is the FLUX.1 class and is architecturally incompatible with Flux 2 Klein weights (see Phase 019). Do not use it for Flux 2 Klein — use `Flux2KleinPipeline`.
- Arch modules must not call `<Pipeline>.from_pretrained(model_id)` inside `sample()`. The diffusion transformer is already loaded and cached by `LoadModel`; arch modules assemble the full pipeline from cached components once per `model_id` and cache the assembled pipeline separately.
