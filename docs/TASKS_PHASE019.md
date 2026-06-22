# Tasks: Phase 019 — Flux 2 Klein Nodes

| Field | Value |
|-------|-------|
| Phase | 019 |
| Name | Flux 2 Klein Nodes |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 18 |

## Overview

Phase 019 adds Flux 2 Klein FP8 support by implementing
`worker/nodes/arch/diffusion/flux.py`. The generic node set from Phase 018
(`LoadModel`, `LoadVae`, `LoadClip`, `ClipTextEncode`, `EmptyLatent`,
`Sampler`, `VaeDecode`, `SaveImage`) is reused without modification. Only
the arch dispatch module is new.

Flux 2 Klein uses the Qwen3 8B FP8-mixed text encoder and a Flux-compatible
VAE. The `LoadClip` node with `clip_type="qwen3"` loads the text encoder.
The `Sampler` node dispatches to `flux.py` when `model.arch == "flux"`. The
workflow JSON structure is identical to ZiT — only model IDs and
`clip_type` change.

Flux 2 Klein's latent shape formula packs latents into 2×2 patches
(channel count ×4, spatial dimensions ÷2 after the scale-factor division)
— structurally different from ZiT's plain scale-factor division. Its
text-encoder conditioning is single-encoder Qwen3-only, not a CLIP-L/T5
concatenation. Both were confirmed against the installed `diffusers`
package source, not assumed by analogy to ZiT or to FLUX.1 (an
architecturally distinct pipeline class — see Known Constraints).

After Phase 019, both ZiT FP8 and Flux 2 Klein FP8 produce real PNG
artifacts using the same generic node graph.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | worker arch | P19-A1 … P19-A5 | arch/diffusion/flux.py: mock dispatch (A1), latent shape formula (A2), pipeline assembly (A3), callback adapter (A4), real invocation (A5) |
| B | integration | P19-B1 | Parity test update + Flux smoke proof doc |

## Prerequisites

Phase 018 complete, including its Group D addendum (`P18-D1` … `P18-D20`):
all 9 baseline nodes have real (non-mock) paths and load from single
`.safetensors` files (not HF directories — see `docs/TASKS_PHASE018.md`'s
narrative note on the `P18-D7`–`P18-D14` insertion), `arch/__init__.py`
exposes `get_module()` (now via the `arch/diffusion/` subpackage,
`P18-D7`), `EmptyLatent` has its optional `model:MODEL` input
and dispatches latent shape computation to `compute_latent_shape()`
(`P18-D3b`) rather than any hardcoded formula, and `arch/diffusion/zit.py`
has a complete, real `sample()` path proving the dispatch pattern this
phase reuses. `P18-E1`'s parity test and smoke proof doc exist and pass.

## Interfaces and Contracts

| Contract document | Relevant tasks | What must match |
|-------------------|---------------|-----------------|
| `ANVILML_DESIGN.md §10.3` | P19-B1 | Node type names, INPUT_SLOTS, OUTPUT_SLOTS — unchanged in this phase; `test_parity.py` must still report exactly 9 node types |
| `ANVILML_DESIGN.md §10.4` | P19-A1, P19-A2, P19-A5 | `can_handle()`/`get_module()`/`sample()`/`compute_latent_shape()` arch module interface, established in Phase 018 and reused unmodified (now under `arch/diffusion/`, see `P18-D7`) |
| `ANVILML_DESIGN.md §10.5` | P19-A3 | FP8 dtype handling: transformer stays float8, no upcast; text_encoder/vae stay bf16 |
| `ANVILML_DESIGN.md Appendix B` | P19-B1 | Example workflow JSON structure, including `EmptyLatent`'s `model` input wiring established in `P18-D17`; `model_id` values are single `.safetensors` filesystem paths, never HF directories (`P18-D12`/`P18-D13`/`P18-D14`) |

## Task Descriptions

### Group A — Flux arch module

#### P19-A1: worker/nodes/arch/diffusion/flux.py: mock dispatch module

**Goal:** Create the Flux 2 Klein arch module's mock dispatch path, mirroring `arch/diffusion/zit.py`'s structure, with the real path explicitly deferred to its named successors.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/flux.py` — new file; `can_handle()` and mock `sample()`
- `worker/tests/test_arch_flux.py` — ≥3 tests

**Key implementation notes:**
- `can_handle(model_obj) -> bool` — `True` iff `model_obj.arch == "flux"`, checked via attribute comparison, not `isinstance()`, to keep arch modules decoupled from specific class hierarchies — same convention as `arch/diffusion/zit.py`.
- `sample(model, conditioning, latent, steps, cfg, seed, device, cancel_flag, emit_progress) -> tuple[Any, int]` — mock path returns `(MockLatent(), seed)` immediately, no `torch`/`diffusers` imports at module top level.
- The real path is explicitly out of scope for this task and is stubbed with `NotImplementedError`. `defers_to: ["P19-A3", "P19-A4", "P19-A5"]` — the stub site must additionally carry the code comment `# defers_to: P19-A3, P19-A4, P19-A5 — real path split across pipeline assembly/callback/invocation`, matching the JSON field exactly.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_flux.py -v` exits 0, ≥3 tests (`can_handle` returns `True` for a Flux model object, `False` for a ZiT model object; mock `sample()` returns `(MockLatent(), seed)`).

#### P19-A2: worker/nodes/arch/diffusion/flux.py: compute_latent_shape()

**Goal:** Add the Flux 2 Klein latent shape formula, verified against `diffusers` source rather than assumed identical to ZiT's — proving the architecture-dispatched design from Phase 018 was necessary, not premature.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/flux.py` — `compute_latent_shape()`
- `worker/tests/test_arch_flux.py` — ≥2 new tests

**Key implementation notes:**
- `compute_latent_shape(batch_size, height, width, num_channels_latents) -> tuple[int, ...]`, implementing `Flux2KleinPipeline.prepare_latents`'s formula — confirmed against installed `diffusers` source, not assumed identical to ZiT's (`P18-D3b`).
- Flux 2 Klein packs latents into 2×2 patches: the formula multiplies the channel count by 4 and halves each spatial dimension after the scale-factor division — structurally different from ZiT's plain scale-factor division.
- The VAE spatial scale factor for Flux 2 Klein's VAE must be confirmed against its real config at implementation time, not assumed equal to ZiT's value of 8.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_flux.py -v` exits 0, ≥2 new tests (correct shape for known dimensions; a non-divisible-by-scale-factor edge case).

#### P19-A3: worker/nodes/arch/diffusion/flux.py: assemble Flux2KleinPipeline

**Goal:** Begin `sample()`'s real path by assembling and caching the `Flux2KleinPipeline` object, without yet invoking it.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/flux.py` — pipeline assembly inside `sample()`'s real path
- `worker/tests/test_arch_flux.py` — 1 new test

**Key implementation notes:**
- Assemble `diffusers.Flux2KleinPipeline(scheduler, vae, text_encoder, tokenizer, transformer, is_distilled=...)` via `pipeline_cache.get_or_load(f"{model_id}:pipeline", ...)` — identical caching pattern to `arch/diffusion/zit.py` (`P18-D18a`).
- `defers_to: ["P19-A5"]` — this task assembles the pipeline but does not invoke it; the stub site must carry the code comment `# defers_to: P19-A5 — pipeline assembled, not yet invoked`, matching the JSON field.
- **Do not use `diffusers.FluxPipeline`** — it is a real class, but is the pipeline for FLUX.1, an earlier and architecturally different model family. `FluxPipeline.from_pretrained()` against Flux 2 Klein weights does not raise an `ImportError` — it either fails later in a confusing way or silently constructs an incorrect pipeline. The correct class is `Flux2KleinPipeline`. `diffusers` also exposes `Flux2Pipeline` for the non-Klein Flux 2 Dev variant — do not confuse the two.
- The `is_distilled` constructor argument's correct value for the Flux 2 Klein Turbo baseline must be confirmed against the model's actual configuration at implementation time, not assumed to be `False`.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_flux.py -v` exits 0, same mock tests pass, plus 1 new test asserting `get_or_load` is called with the pipeline cache key.

#### P19-A4: worker/nodes/arch/diffusion/flux.py: callback_on_step_end adapter

**Goal:** Add the step-callback adapter for Flux 2 Klein, verifying rather than assuming its signature matches ZiT's.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/flux.py` — `_make_callback()` helper and `_SamplingCancelled` sentinel
- `worker/tests/test_arch_flux.py` — ≥2 new tests

**Key implementation notes:**
- `_make_callback(emit_progress, cancel_flag, total_steps) -> Callable`. `Flux2KleinPipeline`'s real callback signature is `(self, i, t, callback_kwargs) -> dict` — confirmed via source to be identical in shape to `ZImagePipeline`'s (`P18-D18b`); this was checked independently, not assumed, since P19-A2 found a genuine divergence elsewhere (the latent shape formula) and a divergence in one place does not imply a divergence in another.
- Reuse the same adapter pattern as `arch/diffusion/zit.py`, with a separate, module-private sentinel exception class (`_SamplingCancelled`) — arch modules must not share private exception types across files.
- This task fully implements the adapter itself and carries no `defers_to`; like `P18-D18b`, the adapter being unused until `P19-A5` wires it in is ordinary sequencing, not deferred scope.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_flux.py -v` exits 0, same mock tests pass, plus ≥2 new tests (`emit_progress` called with correct arguments; cancellation raises `_SamplingCancelled`).

#### P19-A5: worker/nodes/arch/diffusion/flux.py: invoke pipeline, return latent

**Goal:** Complete `sample()`'s real path by invoking the pipeline assembled in P19-A3, using the callback adapter built in P19-A4.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/flux.py` — `sample()`'s real path, pipeline invocation

**Key implementation notes:**
- Wrap the pipeline call (received via `defers_to` from P19-A3) in `try`/`except _SamplingCancelled` for clean cancellation handling.
- Call `pipeline(prompt_embeds=conditioning.positive, negative_prompt_embeds=conditioning.negative, latents=latent, num_inference_steps=steps, guidance_scale=cfg, output_type="latent", callback_on_step_end=_make_callback(...), return_dict=False)`.
- This task is the named recipient of both P19-A1's and P19-A3's `defers_to` entries — its own implementation report should confirm the assembled pipeline is actually invoked here, closing both links.
- `Flux2KleinPipeline.__call__` derives the pipeline-internal `text_ids` value itself from `prompt_embeds` (via its own `encode_prompt`/`_prepare_text_ids`) — callers never construct or pass `text_ids` directly. `ClipTextEncode`'s existing `.positive`/`.negative` conditioning contract (`P18-D16`) requires no Flux-specific changes — confirmed via source before drafting this task, since the divergence in `encode_prompt`'s return signature turned out to be fully internal to `__call__`.
- `output_type="latent"` is required, exactly as in `arch/diffusion/zit.py` (`P18-D18c`), so `VaeDecode` (unchanged from Phase 018) remains the sole node responsible for decoding. Return `(result[0], seed)`.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_flux.py -v` exits 0, same mock tests pass.

### Group B — Integration

#### P19-B1: parity test update + Flux smoke proof documentation

**Goal:** Confirm no new node types were introduced in Phase 019, and document a manual, real-hardware end-to-end run for Flux 2 Klein.

**Files to create or modify:**
- `worker/tests/test_parity.py` — unchanged; re-run to confirm still passing with the same 9 nodes
- `docs/example_workflows/flux_klein_fp8.json` — new Flux 2 Klein workflow JSON
- `docs/PROOF_phase019.md` — Flux manual smoke proof documentation

**Key implementation notes:**
- `flux_klein_fp8.json` has the same structure as `zit_fp8.json`, including `EmptyLatent`'s `model` input wiring established in `P18-D17` — different model IDs and `clip_type: "qwen3"`.
- `model_id` values are single `.safetensors` filesystem paths, never HF directories, per `P18-D12`/`P18-D13`/`P18-D14`.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_parity.py -v` exits 0 (unchanged); `docs/PROOF_phase019.md` exists with complete commands.

## Phase Acceptance Criteria

```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v
cargo test --workspace --features mock-hardware
# Runnable Proof (manual): a real Flux 2 Klein FP8 workflow produces a PNG artifact on real hardware
cargo run --features real-hardware
curl -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' -d @docs/example_workflows/flux_klein_fp8.json
# poll /v1/jobs/:id until Completed; curl /v1/artifacts/:hash
# -> Completed, artifact is a real image/png
```

Requires Flux 2 Klein FP8 safetensors in `models/` — not runnable in CI. Full documented command sequence and expected output: `docs/PROOF_phase019.md`.

## Known Constraints and Gotchas

- **Corrected from this phase's original draft:** Flux 2 Klein's
  conditioning is **not** a concatenation of CLIP-L and T5/Qwen3
  embeddings. That description applies to FLUX.1's conditioning scheme,
  not to `Flux2KleinPipeline`, which uses Qwen3 as its sole text encoder
  (`text_encoder: Qwen3ForCausalLM`, `tokenizer: Qwen2TokenizerFast` in its
  constructor — confirmed via source). There is no dual-encoder handling
  to implement or document in `flux.py`; `LoadClip(clip_type="qwen3")` is
  the only encoder this pipeline uses.
- `can_handle()` must check `model_obj.arch == "flux"`, not `isinstance()`,
  to keep arch modules decoupled from specific class hierarchies.
- `diffusers.FluxPipeline` is a real class but is the FLUX.1 pipeline, not
  Flux 2 Klein. Using it against Flux 2 Klein weights does not raise an
  import error — it silently targets the wrong model. Use
  `diffusers.Flux2KleinPipeline`.
- `diffusers>=0.36.0` is required (`worker/requirements/base.txt`) — this
  release introduced `Flux2Pipeline`/`Flux2KleinPipeline` alongside
  `ZImagePipeline` (Phase 018).
- `arch/diffusion/flux.py` must not call `Flux2KleinPipeline.from_pretrained(model_id)`
  inside `sample()`. Reuse the transformer already cached by `LoadModel`;
  assemble and cache the full pipeline once per `model_id`, same pattern as
  `arch/diffusion/zit.py`.
- Flux 2 Klein's latent shape formula (`P19-A2`) is **not** identical to
  ZiT's (`P18-D3b`) — it packs latents into 2×2 patches (channel count
  ×4, spatial dimensions ÷2 after the scale-factor division). Do not
  copy ZiT's formula by analogy; it was independently verified against
  `diffusers` source for this task.
- `Flux2KleinPipeline`'s `callback_on_step_end` signature **is** identical
  in shape to `ZImagePipeline`'s (`(self, i, t, callback_kwargs) -> dict`)
  — this was verified, not assumed, and the two pipelines should not be
  presumed identical or divergent on any other point without checking
  source directly; Phase 019 explicitly found one divergence (latent
  shape) and one non-divergence (callback signature) by checking each
  independently.