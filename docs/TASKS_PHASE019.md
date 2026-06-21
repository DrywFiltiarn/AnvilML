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
`worker/nodes/arch/flux.py`. The generic node set from Phase 018
(`LoadModel`, `LoadVae`, `LoadClip`, `ClipTextEncode`, `EmptyLatent`,
`Sampler`, `VaeDecode`, `SaveImage`) is reused without modification. Only
the arch dispatch module is new.

Flux 2 Klein uses the Qwen3 8B FP8-mixed text encoder and a Flux-compatible
VAE. The `LoadClip` node with `clip_type="qwen3"` loads the text encoder.
The `Sampler` node dispatches to `flux.py` when `model.arch == "flux"`. The
workflow JSON structure is identical to ZiT — only model IDs and
`clip_type` change.

After Phase 019, both ZiT FP8 and Flux 2 Klein FP8 produce real PNG
artifacts using the same generic node graph.

**This phase is structured to avoid the defect found and corrected in
Phase 018 (see Retrofit Phase 903 and `TASKS_PHASE018.md`'s Group D
addendum), and uses `FORGE_TASK_AUTHORING_SPEC.md §4`'s `defers_to` field
— validated by The Forge at startup — rather than a prose-only deferral
for every task below that intentionally leaves real-path scope for a
named successor.** `P19-A1` is scoped as mock-only by design, exactly
mirroring what `P18-D1` should have been, with `defers_to: ["P19-A3",
"P19-A4", "P19-A5"]`. `P19-A3` similarly carries `defers_to: ["P19-A5"]`.
No task in this phase defers to itself, to a task that won't touch the
relevant file, or to an unnamed "future phase" — and per §12a, none of
these deferrals exist only as prose; each is a structural JSON field
entry, checked for existence and downstream position before any agent
session runs.

Several Flux 2 Klein specifics were verified against the installed
`diffusers==0.38.0` package source in the same session that produced these
tasks, rather than assumed identical to ZiT (Phase 018) or to FLUX.1 (an
architecturally distinct, explicitly-warned-against pipeline class — see
Known Constraints). Notably, the latent shape formula differs structurally
from ZiT's (2×2 patch packing vs. a plain spatial scale factor), and the
text-encoder conditioning scheme is single-encoder Qwen3-only, not a
CLIP-L/T5 concatenation as Phase 019's original draft incorrectly assumed
by analogy to FLUX.1.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | worker arch | P19-A1 … P19-A5 | arch/flux.py: mock dispatch (A1), latent shape formula (A2), pipeline assembly (A3), callback adapter (A4), real invocation (A5) |
| B | integration | P19-B1 | Parity test update + Flux smoke proof doc |

## Prerequisites

Phase 018 complete, including its Group D addendum (`P18-D1` … `P18-D11`):
all 9 baseline nodes have real (non-mock) paths, `arch/__init__.py`
exposes `get_module()`, `EmptyLatent` has its optional `model:MODEL` input
and dispatches latent shape computation to `compute_latent_shape()`
(`P18-D3b`) rather than any hardcoded formula, and `arch/zit.py` has a
complete, real `sample()` path proving the dispatch pattern this phase
reuses. `P18-E1`'s parity test and smoke proof doc exist and pass.

## Interfaces and Contracts

| Contract document | Relevant tasks | What must match |
|-------------------|---------------|-----------------|
| `ANVILML_DESIGN.md §10.3` | P19-B1 | Node type names, INPUT_SLOTS, OUTPUT_SLOTS — unchanged in this phase; `test_parity.py` must still report exactly 9 node types |
| `ANVILML_DESIGN.md §10.4` | P19-A1, P19-A2, P19-A5 | `can_handle()`/`get_module()`/`sample()`/`compute_latent_shape()` arch module interface, established in Phase 018 and reused unmodified |
| `ANVILML_DESIGN.md §10.5` | P19-A3 | FP8 dtype handling: transformer stays float8, no upcast; text_encoder/vae stay bf16 |
| `ANVILML_DESIGN.md Appendix B` | P19-B1 | Example workflow JSON structure, including `EmptyLatent`'s `model` input wiring established in `P18-D8` |

## Task Descriptions

### Group A — Flux arch module

#### P19-A1: worker/nodes/arch/flux.py: mock dispatch module

**Goal:** Create `worker/nodes/arch/flux.py` with:
- `can_handle(model_obj) -> bool` — `True` iff `model_obj.arch == "flux"`
  (checked via attribute comparison, not `isinstance()`, to keep arch
  modules decoupled from specific class hierarchies — same convention as
  `arch/zit.py`)
- `sample(model, conditioning, latent, steps, cfg, seed, device,
  cancel_flag, emit_progress) -> tuple[Any, int]` — mock path returns
  `(MockLatent(), seed)` immediately, no `torch`/`diffusers` imports at
  module top level

The real path is explicitly out of scope for this task and is stubbed
with `NotImplementedError`. **This task's JSON `defers_to` field is
`["P19-A3", "P19-A4", "P19-A5"]`** — per
`FORGE_TASK_AUTHORING_SPEC.md §12a`, this is the only legitimate way to
record the deferral; the stub site must additionally carry the code
comment `# defers_to: P19-A3, P19-A4, P19-A5 — real path split across
pipeline assembly/callback/invocation`, matching the JSON field exactly.
The Forge validates at startup that all three targets exist and are
genuinely downstream of this task in the prereq graph — this task must not
list itself, an unnamed future phase, or a target with no structural
relationship to it. This is the exact defect class Retrofit Phase 903 and
Phase 018's Group D addendum corrected, and the reason `defers_to` exists
at all (`FORGE_TASK_AUTHORING_SPEC.md §12a`); this task must not
reintroduce a prose-only version of it.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m
pytest worker/tests/test_arch_flux.py -v` exits 0 ≥ 3 tests (`can_handle`
returns `True` for a Flux model object, `False` for a ZiT model object;
mock `sample()` returns `(MockLatent(), seed)`).

#### P19-A2: worker/nodes/arch/flux.py: compute_latent_shape()

**Goal:** Add `compute_latent_shape(batch_size, height, width,
num_channels_latents) -> tuple[int, ...]`, implementing
`Flux2KleinPipeline.prepare_latents`'s verified formula — confirmed against
installed `diffusers` source, **not assumed identical to ZiT's**
(`P18-D3b`). Flux 2 Klein packs latents into 2×2 patches: the formula
multiplies the channel count by 4 and halves each spatial dimension after
the scale-factor division, which is structurally different from ZiT's
plain scale-factor division. This is exactly the divergence that motivated
making `compute_latent_shape()` an architecture-dispatched function in
Phase 018 rather than a single shared formula or constant — this task is
the proof that the generalization was necessary, not premature.

The VAE spatial scale factor for Flux 2 Klein's VAE must be confirmed
against its real config at implementation time, not assumed equal to
ZiT's value of 8.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m
pytest worker/tests/test_arch_flux.py -v` exits 0 with ≥ 2 new tests
(correct shape for known dimensions; a non-divisible-by-scale-factor edge
case).

#### P19-A3: worker/nodes/arch/flux.py: assemble Flux2KleinPipeline

**Goal:** Begin `sample()`'s real path. Assemble `diffusers.Flux2KleinPipeline(scheduler, vae,
text_encoder, tokenizer, transformer, is_distilled=...)` via
`pipeline_cache.get_or_load(f"{model_id}:pipeline", ...)`, identical
caching pattern to `arch/zit.py` (`P18-D9a`). **This task's JSON
`defers_to` field is `["P19-A5"]`** — it assembles the pipeline but does
not invoke it; per `FORGE_TASK_AUTHORING_SPEC.md §12a`, the stub site must
carry the code comment `# defers_to: P19-A5 — pipeline assembled, not yet
invoked`, matching the JSON field. This mirrors `P18-D9a`'s identical
deferral to `P18-D9c` in Phase 018.

**⚠️ CRITICAL — do not use `diffusers.FluxPipeline`.** Confirmed via
source: `FluxPipeline` is a real class in `diffusers`, but it is the
pipeline for FLUX.1, an earlier and architecturally different model
family. `FluxPipeline.from_pretrained()` against Flux 2 Klein weights does
not raise an `ImportError` — it either fails later in a confusing way or
silently constructs an incorrect pipeline. The correct class is
`Flux2KleinPipeline`. `diffusers` also exposes `Flux2Pipeline` for the
non-Klein Flux 2 Dev variant — do not confuse the two.

The `is_distilled` constructor argument's correct value for the Flux 2
Klein Turbo baseline must be confirmed against the model's actual
configuration at implementation time, not assumed to be `False`.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m
pytest worker/tests/test_arch_flux.py -v` exits 0, same mock tests pass,
plus one new test asserting `get_or_load` is called with the pipeline
cache key.

#### P19-A4: worker/nodes/arch/flux.py: callback_on_step_end adapter

**Goal:** Add `_make_callback(emit_progress, cancel_flag, total_steps) ->
Callable`. Confirmed via source: `Flux2KleinPipeline`'s real callback
signature is `(self, i, t, callback_kwargs) -> dict` — **identical in
shape** to `ZImagePipeline`'s (`P18-D9b`); this was verified, not assumed,
precisely because P19-A2 found a genuine divergence elsewhere (the latent
shape formula) and a divergence in one place does not imply a divergence
in another. Reuse the same adapter pattern as `arch/zit.py`, with a
separate, module-private sentinel exception class (`_SamplingCancelled`)
— arch modules must not share private exception types across files. This
task fully implements the adapter itself and carries no `defers_to`; like
`P18-D9b`, the adapter being unused until `P19-A5` wires it in is ordinary
sequencing, not deferred scope.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m
pytest worker/tests/test_arch_flux.py -v` exits 0, same mock tests pass,
plus ≥ 2 new tests (`emit_progress` called with correct arguments;
cancellation raises `_SamplingCancelled`).

#### P19-A5: worker/nodes/arch/flux.py: invoke pipeline, return latent

**Goal:** Complete `sample()`'s real path: wrap the pipeline call
(received via `defers_to` from `P19-A3`) in `try`/`except
_SamplingCancelled` for clean cancellation handling. Call
`pipeline(prompt_embeds=conditioning.positive,
negative_prompt_embeds=conditioning.negative, latents=latent,
num_inference_steps=steps, guidance_scale=cfg, output_type="latent",
callback_on_step_end=_make_callback(...), return_dict=False)`. This task
is the named recipient of both `P19-A1`'s and `P19-A3`'s `defers_to`
entries — its own implementation report should confirm the assembled
pipeline is actually invoked here, closing both links.

Confirmed via source: `Flux2KleinPipeline.__call__` derives the
pipeline-internal `text_ids` value itself from `prompt_embeds` (via its
own `encode_prompt`/`_prepare_text_ids`) — callers never construct or pass
`text_ids` directly. `ClipTextEncode`'s existing `.positive`/`.negative`
conditioning contract (established architecture-agnostically in
`P18-D7`) requires **no Flux-specific changes**; this was confirmed before
drafting this task rather than assumed, since the pipeline's `encode_prompt`
return signature differs from `ZImagePipeline`'s in a way that could have
required a contract change, but does not in practice because the
divergence is fully internal to `__call__`.

`output_type="latent"` is required, exactly as in `arch/zit.py`
(`P18-D9c`), so `VaeDecode` (unchanged from Phase 018) remains the sole
node responsible for decoding. Return `(result[0], seed)`.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m
pytest worker/tests/test_arch_flux.py -v` exits 0, same mock tests pass.

### Group B — Integration

#### P19-B1: parity test update + Flux smoke proof documentation

**Goal:** No new node types are added in Phase 019 — `test_parity.py`
should still pass with the same 9 nodes. Create
`docs/example_workflows/flux_klein_fp8.json` (same structure as
`zit_fp8.json`, including `EmptyLatent`'s `model` input wiring established
in `P18-D8` — different model IDs and `clip_type: "qwen3"`). Create
`docs/PROOF_phase019.md` documenting the Flux manual smoke proof.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m
pytest worker/tests/test_parity.py -v` exits 0 (unchanged); `docs/PROOF_phase019.md`
exists with complete commands.

## Phase Acceptance Criteria

```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v
cargo test --workspace --features mock-hardware
# Real hardware proof (manual, requires Flux 2 Klein FP8 safetensors in models/):
# Submit flux_klein_fp8.json; verify Completed + PNG artifact
```

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
- `arch/flux.py` must not call `Flux2KleinPipeline.from_pretrained(model_id)`
  inside `sample()`. Reuse the transformer already cached by `LoadModel`;
  assemble and cache the full pipeline once per `model_id`, same pattern as
  `arch/zit.py`.
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
