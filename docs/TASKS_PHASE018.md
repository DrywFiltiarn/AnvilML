# Tasks: Phase 018 — ZiT Generic Nodes

| Field | Value |
|-------|-------|
| Phase | 018 |
| Name | ZiT Generic Nodes |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 17 |

## Overview

Phase 018 implements the full set of generic inference nodes using Z-Image Turbo FP8 safetensors as the first real model. All nodes are architecture-agnostic — `LoadModel`, `LoadVae`, `LoadClip`, `ClipTextEncode`, `EmptyLatent`, `Sampler`, `VaeDecode`, `SaveImage` — with ZiT-specific dispatch living in `worker/nodes/arch/diffusion/zit.py`.

`model_id` for `LoadModel`/`LoadVae`/`LoadClip` is always a single `.safetensors` filesystem path — never an HF-style snapshot directory. `LoadClip` dispatches to `worker/nodes/arch/clip/{qwen3,clip_l,t5}.py` based on the `clip_type` input, mirroring how `Sampler`/`EmptyLatent` dispatch to `worker/nodes/arch/diffusion/{zit,flux}.py` based on the loaded model's architecture.

Each node's mock-vs-real behavior is scoped individually in that node's own task below — there is no project-wide rule that every node must have both a mock sentinel path and a real path; check each task's own `context` and acceptance criterion. The Python test suite runs in mock mode by default (`ANVILML_WORKER_MOCK=1`, set automatically in `worker/tests/conftest.py`) — this controls the *test harness*, not what any individual node's implementation is required to support.

If a task's scope excludes functionality that a different task is expected to deliver, that exclusion is recorded **only** via the task's JSON `defers_to` field (validated by The Forge at startup) — never as prose in `context`, never as a code comment with no JSON counterpart. See `FORGE_TASK_AUTHORING_SPEC.md §12a`.

After Phase 018, a real ZiT FP8 workflow submitted to a server with a GPU and the correct model files produces a PNG artifact.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | worker loader nodes | P18-A1 … P18-A3 | LoadModel, LoadVae, LoadClip |
| B | worker inference nodes | P18-B1 … P18-B3 | ClipTextEncode, EmptyLatent, Sampler, VaeDecode |
| C | worker pipeline | P18-C1 | pipeline_cache.py LRU model cache |
| D | worker arch + real paths | P18-D1 … P18-D20 | arch dispatch and real loading/sampling paths for every Group A/B node, gated on Retrofit Phase 903 |
| E | integration | P18-E1 | test_parity.py + real ZiT smoke proof doc |

## Prerequisites

Phase 017 complete. `worker/nodes/__init__.py` and `base.py` exist. `worker/worker_main.py` handles Execute with `run_graph`. The `SaveImage` node (mock) exists from Phase 014.

P18-D2 through P18-D20 additionally require Retrofit Phase 903 complete:
`anvilml-scheduler` resolves submitted `model_id` SHA256 hashes to real
filesystem paths before dispatch (`P903-A1`), and `worker_main.py` wires a
real `PipelineCache` instance into every `NodeContext` instead of an empty
dict placeholder (`P903-A2`). See `docs/TASKS_PHASE903.md`.

## Interfaces and Contracts

| Contract document | Relevant tasks | What must match |
|-------------------|---------------|-----------------|
| `ANVILML_DESIGN.md §10.3` | All | Node type names, INPUT_SLOTS, OUTPUT_SLOTS per table (note: `EmptyLatent` gained an optional `model:MODEL` input slot in P18-D17 — table updated) |
| `ANVILML_DESIGN.md §10.4` | P18-D1, P18-D2, P18-D3b, P18-D17, P18-D18a–c, P18-D19 | `can_handle()`/`get_module()`/`sample()`/`compute_latent_shape()` arch module interface (now `arch/diffusion/`); the latent shape formula is architecture-specific and must not be hardcoded in generic nodes |
| `ANVILML_DESIGN.md §10.4a` | P18-D7, P18-D8, P18-D9, P18-D10, P18-D11, P18-D12 | `arch/clip/` dispatch interface: `can_handle(clip_type:str)`/`get_module(clip_type:str)`/`load(model_id,torch_dtype)`, mirroring §10.4's diffusion contract but dispatching on the `clip_type` string, not a loaded object |
| `ANVILML_DESIGN.md §10.5` | P18-C1, P18-D1, P18-D4–D6, P18-D7–D14 | FP8 dtype handling: transformer stays float8, no upcast; text_encoder/vae stay bf16. §10.5 also specifies direct `safetensors` weight loading — `LoadModel`/`LoadVae`/`LoadClip` use single-file loading exclusively (`P18-D7`–`P18-D14`) |
| `ANVILML_DESIGN.md Appendix B` | P18-E1, P18-D4–D6 | Example workflow JSON structure; `model_id` is submitted as a hash and resolved to a path before the worker sees it (Retrofit 903); as of P18-D13/D14/D12, that resolved path is always a single `.safetensors` file, never an HF directory |

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
must be coordinated with P18-D16 (`ClipTextEncode`), which is the sole
consumer.

**Acceptance criterion:** Existing mock tests in
`worker/tests/test_nodes_loader.py` continue to pass unchanged.

#### P18-D7: worker/nodes/arch/: restructure into arch/diffusion/ + arch/clip/ siblings

**Goal:** Move `zit.py` (and Phase 19's `flux.py`) into a new
`worker/nodes/arch/diffusion/` subpackage; `arch/__init__.py` becomes a
thin re-export shim, with all `pkgutil.iter_modules()` scanning logic moved
into `arch/diffusion/__init__.py`, now scanning its own directory instead
of `arch/`'s. No behavior change — `get_module()`/`can_handle()` still work
identically; only the module `__name__` returned by `get_module()` changes
(`worker.nodes.arch.zit` → `worker.nodes.arch.diffusion.zit`). This sets up
structural symmetry for the new `arch/clip/` package (P18-D8) added
alongside it. `test_arch_init.py` and `test_arch_zit.py` updated for the
new import paths; `ANVILML_DESIGN.md §10.4` updated for the new paths.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_init.py worker/tests/test_arch_zit.py -v` exits 0.

#### P18-D8: worker/nodes/arch/clip/__init__.py: clip dispatcher

**Goal:** Create `worker/nodes/arch/clip/__init__.py` mirroring
`arch/diffusion/__init__.py`'s `can_handle()`/`get_module()` contract
exactly, but dispatching on the `clip_type` **string** directly rather than
a loaded model object — no object exists yet at the point `LoadClip` needs
to decide which loader to call. No clip arch modules exist yet; this task
only adds the dispatcher itself, tested against a temporary dummy module
removed before commit.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_init.py -v` exits 0.

#### P18-D9: worker/nodes/arch/clip/qwen3.py: single-file Qwen3 text encoder loading

**Goal:** `can_handle(clip_type) -> bool` returns `clip_type == "qwen3"`.
`load(model_id: str, torch_dtype) -> RealClip`: `model_id` is always a
single `.safetensors` file — never an HF directory, never passed to
`from_pretrained`. Import `RealClip` lazily inside `load()` from
`worker.nodes.loader` (avoids a circular import, since `loader.py` calls
`arch.clip.get_module()`; mirrors `sampler.py`'s existing
lazy-import-of-arch pattern).

**Tokenizer:** `Qwen2Tokenizer.from_pretrained("worker/assets/qwen25_tokenizer")`
— vendored locally, verified against `Qwen/Qwen2.5-VL-7B-Instruct`, no
network call. Resolve this path via `pathlib` relative to this file, not
the process working directory.

**Weights:** `Qwen3ForCausalLM.from_config(config)` then
`load_state_dict(safetensors.torch.load_file(model_id))`. Confirm
`Qwen3Config` field values against the installed `transformers` source
AND the actual checkpoint's tensor shapes before writing the config —
do not assume defaults match the checkpoint.

Return `RealClip(tokenizer, text_encoder)`.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py -v` exits 0, ≥3 tests added.

#### P18-D10: worker/nodes/arch/clip/clip_l.py: single-file CLIP-L text encoder loading

**Goal:** `can_handle(clip_type) -> bool` returns `clip_type == "clip_l"`.
`load(model_id: str, torch_dtype) -> RealClip`: `model_id` is always a
single `.safetensors` file — never an HF directory, never passed to
`from_pretrained`. Import `RealClip` lazily inside `load()` from
`worker.nodes.loader` (same circular-import avoidance as P18-D9).

**Tokenizer:** `CLIPTokenizer.from_pretrained("worker/assets/clip_l_tokenizer")`
— vendored locally, verified against `openai/clip-vit-large-patch14`, no
network call. Resolve this path via `pathlib` relative to this file.

**Weights:** `CLIPTextModelWithProjection.from_config(config)` then
`load_state_dict(safetensors.torch.load_file(model_id))`. Confirm
`CLIPTextConfig` field values (`hidden_size`, `num_hidden_layers`,
`projection_dim`) against the installed `transformers` source AND the
actual checkpoint's tensor shapes before writing the config.

Return `RealClip(tokenizer, text_encoder)`.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_l.py -v` exits 0, ≥3 tests added.

#### P18-D11: worker/nodes/arch/clip/t5.py: single-file T5-XXL text encoder loading

**Goal:** `can_handle(clip_type) -> bool` returns `clip_type == "t5"`.
`load(model_id: str, torch_dtype) -> RealClip`: `model_id` is always a
single `.safetensors` file — never an HF directory, never passed to
`from_pretrained`. Import `RealClip` lazily inside `load()` from
`worker.nodes.loader` (same circular-import avoidance as P18-D9).

**Tokenizer:** `T5TokenizerFast.from_pretrained("worker/assets/t5_tokenizer")`
— this task switches `LoadClip`'s `t5` branch from the slow `T5Tokenizer`
to `T5TokenizerFast`; update `loader.py`'s stale `T5Tokenizer` import
accordingly. The vendored `tokenizer.json` was verified against
`InvokeAI/t5-v1_1-xxl`'s `bfloat16/tokenizer_2/` (Apache-2.0, an
explicitly-licensed re-host of the same `FLUX.1-schnell` `text_encoder_2`
tokenizer content). `google/t5-v1_1-xxl` is not used as the source — it
ships only a SentencePiece `spiece.model`, no `tokenizer.json`.
`black-forest-labs/FLUX.1-dev`'s own `tokenizer_2/` is not used either — it
is a gated repo requiring HF auth and license acceptance, unsuitable for
an unauthenticated asset fetch. See `worker/tools/seed_tokenizers.sh`'s
header comments for the full sourcing rationale.

**Weights:** `T5EncoderModel.from_config(config)` then
`load_state_dict(safetensors.torch.load_file(model_id))`. Confirm
`T5Config` field values against the actual checkpoint's tensor shapes
before writing the config.

Return `RealClip(tokenizer, text_encoder)`.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_t5.py -v` exits 0, ≥3 tests added.

#### P18-D12: worker/nodes/loader.py: LoadClip dispatches via arch.clip

**Goal:** Replace `LoadClip`'s inline `if clip_type == "qwen3": ... elif
...` dispatch with `arch.clip.get_module(clip_type)`, mirroring
`Sampler`'s existing `arch.get_module(model)` pattern. Fixes a pre-existing
bug where `execute()` reads the bare name `ctx` instead of `self.ctx` when
accessing `pipeline_cache` (a `NameError` at runtime, uncaught by the
existing test suite since it runs in mock mode only). The original
`from_pretrained`-based branch bodies move unchanged into a new
`_load_from_hf_directory(model_id, clip_type)` function — kept, never
called, preserved for future reactivation.

**Acceptance criterion:** Existing mock tests in
`worker/tests/test_nodes_loader.py` continue to pass unchanged.

#### P18-D13: worker/nodes/loader.py: LoadModel single-file path

**Goal:** Replace `from_pretrained(model_id, subfolder="unet")` with
`ZImageTransformer2DModel.from_single_file(model_id, torch_dtype=torch.float16)`
— confirmed registered in `diffusers.loaders.single_file_model.
SINGLE_FILE_LOADABLE_CLASSES`, which infers the model's config from the
checkpoint's own tensor keys, no `config.json` needed. The existing
`safe_open()` metadata read for `arch` detection is unchanged; the result
is still wrapped in `RealModel(transformer, arch=arch)` exactly as before
— `Sampler`/`EmptyLatent` depend on `.arch`/`.in_channels` via this
wrapper. The original directory-based code moves into
`_load_from_hf_directory(model_id, arch)`, kept but never called. Fixes
the `self.ctx` vs bare `ctx` bug.

**Acceptance criterion:** Existing mock tests in
`worker/tests/test_nodes_loader.py` continue to pass unchanged.

#### P18-D14: worker/nodes/loader.py: LoadVae single-file path

**Goal:** Replace `from_pretrained(model_id, subfolder="vae")` with
`AutoencoderKL.from_single_file(model_id, torch_dtype=torch.bfloat16)` —
same `SINGLE_FILE_LOADABLE_CLASSES` mechanism as P18-D13. `LoadVae`
continues to return the bare `AutoencoderKL` instance unwrapped (it
already exposes `.config.block_out_channels` etc., no wrapper needed, per
the original P18-A2/D5 design). The original directory-based code moves
into `_load_from_hf_directory(model_id)`, kept but never called. Fixes the
`self.ctx` vs bare `ctx` bug.

**Acceptance criterion:** Existing mock tests in
`worker/tests/test_nodes_loader.py` continue to pass unchanged.

#### P18-D16: worker/nodes/encoder.py: ClipTextEncode real path

**Goal:** Replace `ClipTextEncode`'s real-path `NotImplementedError` (this
TODO currently carries no task reference at all — this task is its
correct owner). Call the loaded CLIP object's encode method to produce
`prompt_embeds`/`negative_prompt_embeds` matching
`ZImagePipeline.__call__`'s expected shape. Return a conditioning object
exposing `.positive`/`.negative` — the exact contract P18-D18a–c consume.

**Acceptance criterion:** Existing mock tests in
`worker/tests/test_nodes_encoder.py` continue to pass unchanged.

#### P18-D17: worker/nodes/sampler.py: EmptyLatent real path + new model input

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

#### P18-D18a / P18-D18b / P18-D18c: worker/nodes/arch/diffusion/zit.py: real sample()

Split into three atomic sub-tasks; the original single-task scope (pipeline
assembly + callback adapter + invocation + cancellation) was too large for
one task.

**Goal (D18a):** Assemble the `ZImagePipeline` from cached components via
`pipeline_cache.get_or_load(f"{model_id}:pipeline", ...)`, per the
component/pipeline caching split already specified in P18-D1's own goal
above. Also corrects this module's docstring, which currently misdescribes
the real callback shape. **`defers_to: ["P18-D18c"]`** — this task assembles
the pipeline but does not invoke it; per
`FORGE_TASK_AUTHORING_SPEC.md §12a`, the stub site must carry the code
comment `# defers_to: P18-D18c — pipeline assembled, not yet invoked`.

**Goal (D18b):** Build a `callback_on_step_end` adapter closure bridging
diffusers' real 4-argument callback signature (`self, i, t,
callback_kwargs`) to the simpler 2-argument `emit_progress(step, total)`
interface `sample()`'s own public signature exposes, plus cooperative
cancellation via a private sentinel exception. This task fully implements
the adapter function itself — it does not carry a `defers_to`; the adapter
being unused until D18c wires it in is ordinary sequencing, not deferred
scope.

**Goal (D18c):** Invoke the assembled pipeline (received via `defers_to`
from `P18-D18a`) with `output_type="latent"` — returning the raw denoised
latent rather than a decoded image, since `VaeDecode` (P18-D20) remains the
sole node responsible for decoding, per the explicit-VAE-input contract
already established in this document. Return `(latent, seed)`. This task
is the named recipient of `P18-D18a`'s `defers_to` entry — its own
implementation report should confirm the assembled pipeline is actually
invoked here, closing that link.

**Acceptance criteria:** Each sub-task's existing mock tests in
`worker/tests/test_arch_zit.py` continue to pass; D18a and D18b each add new
tests per their task `context`. Note: after P18-D7's restructure, this file
is `worker/nodes/arch/diffusion/zit.py`, not `worker/nodes/arch/zit.py`.

#### P18-D19: worker/nodes/sampler.py: Sampler real dispatch

**Goal:** Replace `Sampler.execute()`'s real-path `NotImplementedError`
(currently mislabeled as deferred to P18-C1, which is `pipeline_cache.py`
and never touched this file — this task is the correct owner). Dispatch
via `arch.get_module(model)` (P18-D2); raise `NodeError("unsupported model
architecture")` per `ANVILML_DESIGN.md §10.4` if no module matches.

**Acceptance criterion:** Existing mock tests in
`worker/tests/test_nodes_sampler.py` continue to pass unchanged.

#### P18-D20: worker/nodes/decode.py: VaeDecode real path

**Goal:** Replace `VaeDecode`'s real-path `NotImplementedError` (this TODO
currently carries no task reference at all — this task is its correct
owner). Invert the encode-time scaling (`scaling_factor`/`shift_factor`),
call `vae.decode()`, and postprocess to a real PIL Image for the existing,
unchanged `SaveImage` node to encode to PNG.

**Acceptance criterion:** Existing mock tests in
`worker/tests/test_nodes_decode.py` continue to pass unchanged.

### Group E — Integration

#### P18-E1: test_parity.py + ZiT smoke proof documentation

**Goal:** Create `worker/tests/test_parity.py` verifying that NODE_REGISTRY contains exactly the 9 baseline node types from `ANVILML_DESIGN.md §10.3`. Create `docs/PROOF_phase018.md` documenting the manual real-hardware runnable proof: exact curl commands to submit the Appendix B ZiT workflow JSON and observe JobCompleted + PNG artifact. `model_id` values in the workflow JSON are single `.safetensors` filesystem paths (post P18-D7–D20's single-file-only loading), never HF directory paths.

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
- `model_id` as received by the Python worker (Group A/D loader nodes) is always a real filesystem path, never the SHA256 hash the job submitter provides. The scheduler rewrites the graph's `LoadModel`/`LoadVae`/`LoadClip` `model_id` inputs in place immediately before dispatch (Retrofit Phase 903, `P903-A1`). Loader nodes must not attempt to decode or look up a hash themselves. As of P18-D12/D13/D14, that resolved path is always a single `.safetensors` **file**, never an HF-style snapshot directory — the original directory-based `from_pretrained` code is preserved but unreachable in `_load_from_hf_directory(...)` functions, not deleted.
- `NodeContext.pipeline_cache` is a real `PipelineCache` instance (Retrofit Phase 903, `P903-A2`), not an empty dict. All loader and arch real paths can rely on `ctx.pipeline_cache.get_or_load(...)` working correctly.
- `diffusers.ZImagePipeline.__call__()` performs denoising and VAE decoding together by default. Passing `output_type="latent"` (a genuinely supported value) returns the raw denoised latent instead, which is required for `Sampler` and `VaeDecode` to remain two separate, single-purpose nodes per this document's node graph.
- `diffusers.ZImagePipeline`'s real `callback_on_step_end` signature is `(self, i, t, callback_kwargs) -> dict`, not the simpler 2-argument `emit_progress(step, total)` shape `arch/diffusion/zit.py`'s public `sample()` interface exposes to the rest of the codebase. An adapter closure bridges the two (P18-D18b).
- `ZImagePipeline.prepare_latents()` strictly validates the shape of any pre-supplied `latents=` tensor against `(batch_size, num_channels_latents, height_scaled, width_scaled)` and raises `ValueError` on mismatch. The exact shape formula is architecture-specific (e.g. Flux 2 Klein's 2×2 latent patch packing produces a structurally different formula, not just a different scale factor — see Phase 019) and therefore lives inside each arch module as `compute_latent_shape()` (`P18-D3b`), not inline in `EmptyLatent` (`P18-D17`) or as a single shared constant.
- `worker/nodes/arch/` is split into `arch/diffusion/` (diffusion model dispatch — `zit.py`, `flux.py`) and `arch/clip/` (text-encoder dispatch — `qwen3.py`, `clip_l.py`, `t5.py`) as of P18-D7/D8. Both follow the same `can_handle()`/`get_module()` contract shape; `arch/diffusion/` dispatches on a loaded model object's `.arch` attribute, `arch/clip/` dispatches on the `clip_type` string directly (no object exists yet at that point).
- `transformers` text-encoder classes (`Qwen3ForCausalLM`, `CLIPTextModelWithProjection`, `T5EncoderModel`) have no `from_single_file()` equivalent — unlike `diffusers` model classes (`ZImageTransformer2DModel`, `AutoencoderKL`), which do (confirmed via `diffusers.loaders.single_file_model.SINGLE_FILE_LOADABLE_CLASSES`). `LoadModel`/`LoadVae` (P18-D13/D14) use `from_single_file()`; `arch/clip/*.py` modules (P18-D9/D10/D11) use manual `from_config()` + `load_state_dict()` instead, since no shortcut exists on the `transformers` side.
- Vendored tokenizer assets live under `worker/assets/{qwen25_tokenizer,clip_l_tokenizer,t5_tokenizer}/`, committed to git (not gitignored — these are small, static, redistributable files, not user model weights). Re-seed via `worker/tools/seed_tokenizers.sh`/`.ps1`; see that script's header comments for exact source-repo provenance and why certain obvious-seeming sources (`google/t5-v1_1-xxl`, `black-forest-labs/FLUX.1-dev`) were deliberately not used.