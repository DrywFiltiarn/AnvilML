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
| D | worker arch + real paths | P18-D1 … P18-D11 | arch/zit.py mock dispatch (D1), then real paths for every Group A/B/D node plus real ZiT sampling, gated on Retrofit Phase 903 |
| E | integration | P18-E1 | test_parity.py + real ZiT smoke proof doc |

P18-D2 through P18-D11 were added after P18-D1 completed, to correct a
defect found while auditing P18-D1's deferred real-`sample()` path ahead of
P18-E1 planning: every "real path" stub across Groups A–D (`LoadModel`,
`LoadVae`, `LoadClip`, `ClipTextEncode`, `EmptyLatent`, `Sampler`, and
`arch/zit.py`'s own `sample()`) deferred to either itself, a task that never
touched the file, or no task at all. See `docs/TASKS_PHASE903.md` for the
two prerequisite fixes (model path resolution, real pipeline cache wiring)
these tasks depend on.

## Prerequisites

Phase 017 complete. `worker/nodes/__init__.py` and `base.py` exist. `worker/worker_main.py` handles Execute with `run_graph`. The `SaveImage` node (mock) exists from Phase 014.

P18-D2 through P18-D11 additionally require Retrofit Phase 903 complete:
`anvilml-scheduler` resolves submitted `model_id` SHA256 hashes to real
filesystem paths before dispatch (`P903-A1`), and `worker_main.py` wires a
real `PipelineCache` instance into every `NodeContext` instead of an empty
dict placeholder (`P903-A2`). See `docs/TASKS_PHASE903.md`.

## Interfaces and Contracts

| Contract document | Relevant tasks | What must match |
|-------------------|---------------|-----------------|
| `ANVILML_DESIGN.md §10.3` | All | Node type names, INPUT_SLOTS, OUTPUT_SLOTS per table (note: `EmptyLatent` gained an optional `model:MODEL` input slot in P18-D8 — table updated) |
| `ANVILML_DESIGN.md §10.4` | P18-D1, P18-D2, P18-D3b, P18-D8, P18-D9a–c, P18-D10 | `can_handle()`/`get_module()`/`sample()`/`compute_latent_shape()` arch module interface; the latent shape formula is architecture-specific and must not be hardcoded in generic nodes |
| `ANVILML_DESIGN.md §10.5` | P18-C1, P18-D1, P18-D4–D6 | FP8 dtype handling: transformer stays float8, no upcast; text_encoder/vae stay bf16 |
| `ANVILML_DESIGN.md Appendix B` | P18-E1, P18-D4–D6 | Example workflow JSON structure; `model_id` is submitted as a hash and resolved to a path before the worker sees it (Retrofit 903) |

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

> **Note on P18-D1's outcome:** the goal above describes the intended real
> `sample()` path. What was actually implemented is the mock path only —
> the real path was stubbed with `NotImplementedError` and a TODO comment
> deferring to "a future phase," without a genuine task ever being
> assigned to complete it. P18-D2 through P18-D11 below correct this,
> after Retrofit Phase 903 closes two prerequisite gaps discovered during
> that audit (model path resolution, real pipeline cache wiring — see
> `docs/TASKS_PHASE903.md`). The same kind of dangling or self-referential
> deferral was also found in `LoadModel`, `LoadVae`, `LoadClip` (Group A),
> `EmptyLatent`, `Sampler` (Group B) — each gets its own task below.
>
> The incident described above predates `FORGE_TASK_AUTHORING_SPEC.md`'s
> `defers_to` field and §12a authoring procedure, both added specifically
> in response to it. None of the tasks below repeat the original prose-only
> deferral pattern; where a task genuinely defers scope (`P18-D9a`), the
> deferral is recorded in the task's JSON `defers_to` field, validated by
> The Forge at startup, not merely stated in prose.

#### P18-D2: worker/nodes/arch/__init__.py: get_module() dispatcher

**Goal:** Add `get_module(model_obj) -> ModuleType | None`, returning the
actual arch module that claims a given model object, not just a boolean.
`can_handle()` is preserved and refactored to delegate to `get_module()`
internally so there is exactly one iteration implementation. This is
required because `Sampler` (P18-D10) needs to call `.sample()` on the
actual matching module, and `EmptyLatent` (P18-D8) needs to read an
arch-specific constant off it — neither is possible with a boolean-only
`can_handle()`.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m
pytest worker/tests/test_arch_init.py -v` exits 0 with ≥ 3 new tests.

#### P18-D3: worker/nodes/arch/zit.py: VAE_SCALE_FACTOR constant

**Goal:** Add `VAE_SCALE_FACTOR: int = 8`, sourced from Z-Image-Turbo's
published VAE config (`block_out_channels=[128,256,512,512]`, 4 entries,
giving `2**(4-1)=8` per `ZImagePipeline.__init__`'s own formula;
independently corroborated as 8× spatial compression, 1024×1024 image →
128×128 latent grid). Consumed by `EmptyLatent`'s real path (P18-D8) via
the P18-D2 dispatcher — wiring that consumer is out of scope here.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m
pytest worker/tests/test_arch_zit.py -v` exits 0 with 1 new test.

#### P18-D3b: worker/nodes/arch/zit.py: compute_latent_shape() function

**Goal:** Add `compute_latent_shape(batch_size, height, width,
num_channels_latents) -> tuple[int, ...]`, implementing
`ZImagePipeline.prepare_latents`'s exact shape formula. This function — not
a bare `VAE_SCALE_FACTOR` lookup — is `EmptyLatent`'s (P18-D8) actual
dispatch target. The shape *formula itself*, not just one scale-factor
constant, must be architecture-specific: Flux 2 Klein (Phase 19) uses a
structurally different formula involving 2×2 latent patch packing
(`num_channels * 4`, `height // 2`, `width // 2` after the scale-factor
division), not a plain scale-factor division. Computing the formula inside
`EmptyLatent` itself — as originally drafted before this task was added —
would have required `EmptyLatent` to know about every architecture's
packing scheme, defeating the purpose of architecture-agnostic generic
nodes. `VAE_SCALE_FACTOR` (P18-D3) remains available as a documented
constant for any other purpose but is no longer `EmptyLatent`'s direct
dispatch target.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m
pytest worker/tests/test_arch_zit.py -v` exits 0 with ≥ 2 new tests (correct
shape for a known height/width pair; a non-divisible-by-scale-factor edge
case).

#### P18-D4: worker/nodes/loader.py: LoadModel real path

**Goal:** Replace `LoadModel`'s `NotImplementedError` real path. `model_id`
is now a real filesystem path (Retrofit 903 resolves the submitted hash
before dispatch — no hash decoding happens here). Detect architecture from
safetensors metadata or the `models/` directory naming convention; load the
diffusion transformer via `pipeline_cache.get_or_load()`. The returned model
object must expose `.arch` (str) and `.in_channels` (int) so `EmptyLatent`
(P18-D8) and `arch.sample()` (P18-D9a–c) can read them.

**Acceptance criterion:** Existing mock tests in
`worker/tests/test_nodes_loader.py` continue to pass unchanged (this task
implements the real path only; mock mode is untouched).

#### P18-D5: worker/nodes/loader.py: LoadVae real path

**Goal:** Replace `LoadVae`'s `NotImplementedError` real path. Load
`diffusers.AutoencoderKL` via `pipeline_cache.get_or_load()`, matching the
component type `ZImagePipeline.__init__` expects for its `vae=` argument.

**Acceptance criterion:** Existing mock tests in
`worker/tests/test_nodes_loader.py` continue to pass unchanged.

#### P18-D6: worker/nodes/loader.py: LoadClip real path

**Goal:** Replace `LoadClip`'s `NotImplementedError` real path. Load the
tokenizer/text-encoder pair matching the `clip_type` hint (Qwen2Tokenizer +
Qwen3Model for `"qwen3"`, the Z-Image-Turbo baseline) via
`pipeline_cache.get_or_load()`. The returned object's exact attribute names
must be coordinated with P18-D7 (`ClipTextEncode`), which is the sole
consumer.

**Acceptance criterion:** Existing mock tests in
`worker/tests/test_nodes_loader.py` continue to pass unchanged.

#### P18-D7: worker/nodes/encoder.py: ClipTextEncode real path

**Goal:** Replace `ClipTextEncode`'s real-path `NotImplementedError` (this
TODO currently carries no task reference at all — this task is its
correct owner). Call the loaded CLIP object's encode method to produce
`prompt_embeds`/`negative_prompt_embeds` matching
`ZImagePipeline.__call__`'s expected shape. Return a conditioning object
exposing `.positive`/`.negative` — the exact contract P18-D9a–c consume.

**Acceptance criterion:** Existing mock tests in
`worker/tests/test_nodes_encoder.py` continue to pass unchanged.

#### P18-D8: worker/nodes/sampler.py: EmptyLatent real path + new model input

**Goal:** Add an optional `model:MODEL` input slot to `EmptyLatent`
(`ANVILML_DESIGN.md §10.3` updated accordingly). Real path dispatches via
`mod = arch.get_module(model)` (P18-D2), reads `num_channels_latents` from
`model.in_channels` (P18-D4), then calls
`mod.compute_latent_shape(batch_size, height, width, num_channels_latents)`
(P18-D3b) to obtain the noise tensor shape — the shape formula itself is
not computed inline here, since it is architecture-specific (see P18-D3b's
rationale). This input is required (not optional) in real mode — if
absent, the node raises rather than guessing a channel count or a shape
formula.

**Acceptance criterion:** Existing mock tests in
`worker/tests/test_nodes_sampler.py` continue to pass unchanged (mock mode
ignores the new optional input).

#### P18-D9a / P18-D9b / P18-D9c: worker/nodes/arch/zit.py: real sample()

Split into three atomic sub-tasks; the original single-task scope (pipeline
assembly + callback adapter + invocation + cancellation) was too large for
one task.

**Goal (D9a):** Assemble the `ZImagePipeline` from cached components via
`pipeline_cache.get_or_load(f"{model_id}:pipeline", ...)`, per the
component/pipeline caching split already specified in P18-D1's own goal
above. Also corrects this module's docstring, which currently misdescribes
the real callback shape. **`defers_to: ["P18-D9c"]`** — this task assembles
the pipeline but does not invoke it; per
`FORGE_TASK_AUTHORING_SPEC.md §12a`, the stub site must carry the code
comment `# defers_to: P18-D9c — pipeline assembled, not yet invoked`.

**Goal (D9b):** Build a `callback_on_step_end` adapter closure bridging
diffusers' real 4-argument callback signature (`self, i, t,
callback_kwargs`) to the simpler 2-argument `emit_progress(step, total)`
interface `sample()`'s own public signature exposes, plus cooperative
cancellation via a private sentinel exception. This task fully implements
the adapter function itself — it does not carry a `defers_to`; the adapter
being unused until D9c wires it in is ordinary sequencing, not deferred
scope.

**Goal (D9c):** Invoke the assembled pipeline (received via `defers_to`
from `P18-D9a`) with `output_type="latent"` — returning the raw denoised
latent rather than a decoded image, since `VaeDecode` (P18-D11) remains the
sole node responsible for decoding, per the explicit-VAE-input contract
already established in this document. Return `(latent, seed)`. This task
is the named recipient of `P18-D9a`'s `defers_to` entry — its own
implementation report should confirm the assembled pipeline is actually
invoked here, closing that link.

**Acceptance criteria:** Each sub-task's existing mock tests in
`worker/tests/test_arch_zit.py` continue to pass; D9a and D9b each add new
tests per their task `context`.

#### P18-D10: worker/nodes/sampler.py: Sampler real dispatch

**Goal:** Replace `Sampler.execute()`'s real-path `NotImplementedError`
(currently mislabeled as deferred to P18-C1, which is `pipeline_cache.py`
and never touched this file — this task is the correct owner). Dispatch
via `arch.get_module(model)` (P18-D2); raise `NodeError("unsupported model
architecture")` per `ANVILML_DESIGN.md §10.4` if no module matches.

**Acceptance criterion:** Existing mock tests in
`worker/tests/test_nodes_sampler.py` continue to pass unchanged.

#### P18-D11: worker/nodes/decode.py: VaeDecode real path

**Goal:** Replace `VaeDecode`'s real-path `NotImplementedError` (this TODO
currently carries no task reference at all — this task is its correct
owner). Invert the encode-time scaling (`scaling_factor`/`shift_factor`),
call `vae.decode()`, and postprocess to a real PIL Image for the existing,
unchanged `SaveImage` node to encode to PNG.

**Acceptance criterion:** Existing mock tests in
`worker/tests/test_nodes_decode.py` continue to pass unchanged.

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
- `model_id` as received by the Python worker (Group A/D loader nodes) is always a real filesystem path, never the SHA256 hash the job submitter provides. The scheduler rewrites the graph's `LoadModel`/`LoadVae`/`LoadClip` `model_id` inputs in place immediately before dispatch (Retrofit Phase 903, `P903-A1`). Loader nodes must not attempt to decode or look up a hash themselves.
- `NodeContext.pipeline_cache` is a real `PipelineCache` instance (Retrofit Phase 903, `P903-A2`), not an empty dict. All loader and arch real paths can rely on `ctx.pipeline_cache.get_or_load(...)` working correctly.
- `diffusers.ZImagePipeline.__call__()` performs denoising and VAE decoding together by default. Passing `output_type="latent"` (a genuinely supported value) returns the raw denoised latent instead, which is required for `Sampler` and `VaeDecode` to remain two separate, single-purpose nodes per this document's node graph.
- `diffusers.ZImagePipeline`'s real `callback_on_step_end` signature is `(self, i, t, callback_kwargs) -> dict`, not the simpler 2-argument `emit_progress(step, total)` shape `arch/zit.py`'s public `sample()` interface exposes to the rest of the codebase. An adapter closure bridges the two (P18-D9b).
- `ZImagePipeline.prepare_latents()` strictly validates the shape of any pre-supplied `latents=` tensor against `(batch_size, num_channels_latents, height_scaled, width_scaled)` and raises `ValueError` on mismatch. The exact shape formula is architecture-specific (e.g. Flux 2 Klein's 2×2 latent patch packing produces a structurally different formula, not just a different scale factor — see Phase 019) and therefore lives inside each arch module as `compute_latent_shape()` (`P18-D3b`), not inline in `EmptyLatent` (`P18-D8`) or as a single shared constant.
