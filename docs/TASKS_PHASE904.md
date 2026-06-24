# Tasks: Phase 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects)

| Field | Value |
|-------|-------|
| Phase | 904 |
| Name | P18 D16–D20 Retrofit (Real-Path Wiring Defects) |
| Project | anvilml |
| Status | Draft |
| Depends on phases | 18 (after D19, before D20), 903 |

## Overview

Phase 904 is a retrofit correcting defects discovered against the
committed state of Phase 18 groups D16–D20 (`ClipTextEncode`,
`EmptyLatent`, `Sampler`, `VaeDecode`, and the ZiT arch module's pipeline
assembly and invocation). Defects were found by reading the live source,
by direct execution where torch/diffusers were available, and — for the
most significant defect in this phase, the HF-network-access issue Group
B exists to fully resolve — by running the real loading path against real
GPU hardware and observing its actual behavior. None were inferred from
the task descriptions or design docs alone.

The first nine defects (P904-A1 through P904-A8, counting the A6/A6b
split) are not caught by the existing `worker/tests/` suite, because
`worker/tests/conftest.py` forces `ANVILML_WORKER_MOCK=1` for every test via
an autouse fixture, and all nine live exclusively in real-mode code paths
that mock mode never reaches. This is also why they survived from D16
through D19 without being noticed: each individual task's own mock-mode
tests passed, because the defect was never on a path those tests exercised.

Closing that detection gap with ordinary CI coverage is not possible:
`worker/requirements/base.txt` (which CI's venv installs) deliberately
excludes `torch` — it is GPU-architecture-dependent and is only available
via the manual-install-only `worker/requirements/rocm-{linux,windows}.txt`
files, which target a specific AMD GFX architecture and are not wired into
`scripts/install_worker_deps.sh`. CI's runners therefore have no `torch` at
all, by design, and cannot execute any code path that actually calls into
it. Group Z (P904-Z1–Z7) instead builds a separate real-mode CPU test
suite — gated behind a new `realcpu` pytest marker — initially meant to be
run only by the OpenCode agent at ACT time (on a CPU-only WSL2 box, using
`worker/requirements/cpu-linux-agent.txt`, which already exists on `main`
and installs `torch` via PyTorch's dedicated CPU index rather than plain
PyPI) or manually by a developer. P904-Z1 through P904-Z6 build this
suite under the assumption that it stays excluded from the default
`pytest worker/tests -v` gate every other task in this project relies on
for its own Acceptance Criterion — that assumption holds for all of them.
P904-Z7 changes it: once Z7 lands, `ci.yml`'s `worker` job runs the
`realcpu` suite itself, via a separate, CI-only `cpu-runner-reqs.txt`
(intentionally not the same file the agent uses, so the two consumers can
diverge later). To keep this suite fast
and dependency-light, Group Z generates synthetic tiny-config checkpoints
at test time (a 2-layer transformer, a tiny VAE, tiny text encoders) rather
than depending on real multi-gigabyte Z-Image-Turbo/Qwen3-4B weights — the
goal is exercising the real code paths (`load_transformer`/`load_vae`'s
shape-inference and remap logic once Group B lands, `load_state_dict`, a
real `ZImagePipeline.__call__`) to prove they function, not producing a
meaningful image.

Two of the eight (P904-A7, P904-A8) are new findings from a deliberate
re-audit of D1–D15 (the loader/model groups preceding D16) for the same
defect classes already found in D16–D20 — unbound names, incorrect relative
paths, missing imports, and reading the wrong source object. That audit
surfaced a systemic device-placement gap: no real-mode loader (`LoadModel`,
`LoadVae`, `LoadClip`/`arch/clip/*`) ever moves its loaded component to
`ctx.device`, so every real generation request would silently run on CPU
regardless of the GPU the worker is bound to. This is more consequential
than any of the six crash-on-first-use defects, because it produces no
exception at all — a generation job would complete "successfully," just
far slower and on the wrong device, which is the kind of defect that is
hardest to notice in production and easiest to miss in any test that
doesn't assert on device placement explicitly.

D20 (`VaeDecode` real path) was committed after this phase was first drafted
and has since been audited. Its implementation is correct — verified line
for line against `ZImagePipeline`'s own internal decode formula
(`(latents / scaling_factor) + shift_factor`, then `vae.decode()`, then
`VaeImageProcessor.postprocess()`) — and has no coupling to anything Group
A touches (`zit.py`, `Sampler`, `EmptyLatent`, the loaders), so no new
Group A bugfix task was needed for the production code itself. However,
D20's own committed test (`test_vaedeode_real_path_returns_pil_image` in
`worker/tests/test_nodes_decode.py`) does an unguarded `import torch`
inside a file CI collects by default — and CI's venv has no `torch`
installed at all (confirmed: `diffusers`/`transformers` do not pull it in
transitively; it is only an optional `extra` on `diffusers`). This breaks
CI the moment it runs against this commit. P904-A1 fixes that. Because every
other Group A task's own commit needs working CI to validate against, and
P904-A1 has no dependency on any other P904 task (only `P18-D20`), it is
sequenced first in this phase's task ordering, with P904-A2 through
P904-Z5 each chaining sequentially off the task before it across all three
groups. Group Z's scope (Z1, Z4, Z5) has also been widened to include
`VaeDecode` in its real fixture set and chain coverage, since D20 is now
committed and the real-mode test suite should cover the full node graph
through to a decoded image, not stop short at `Sampler`'s output latent.

P904-A9 through P904-A14 address a defect found after this phase's
original drafting: console output showed `config.json: 100%` being fetched
from HuggingFace Hub during `LoadModel`'s real path, directly contradicting
the project's stated local-only-`.safetensors` design intent. Root cause,
confirmed by reading `diffusers` 0.38.0 source:
`ZImageTransformer2DModel.from_single_file()` and
`AutoencoderKL.from_single_file()` (identical `FromOriginalModelMixin` code
path) both fall through to `fetch_diffusers_config(checkpoint)` — which
guesses an HF repo id and downloads its `config.json` — whenever neither
`config=` nor `original_config=` is supplied, which neither `LoadModel`
nor `LoadVae` did. `local_files_only` is not a sufficient fix, since it
only skips the download if the guessed repo happens to already be cached
locally. The fix (P904-A10, P904-A11) bypasses `from_single_file()`
entirely: each currently-supported architecture's loading logic moves into
its own arch module (`zit.py` gains `load_transformer()`/`load_vae()`),
using locally-known default configs (already-published architecture
constants, requiring no config file at all) and reusing `diffusers`' own
internal, network-free checkpoint key-remapping functions
(`convert_z_image_transformer_checkpoint_to_diffusers`,
`convert_ldm_vae_checkpoint`) rather than reimplementing that remap logic.
`loader.py`'s three loading functions (P904-A12) become thin wrappers that
dispatch to the correct arch module by name (P904-A13, since no model
object exists yet at this dispatch point) rather than calling `diffusers`
classes directly. The deprecated HF-directory loading remnants kept "for
future use" (P904-A9) are deleted outright rather than retained, per
explicit reversal of that earlier decision. P904-A14 closes out two
further defects found in this rework's own committed code: `LoadVae`'s
call into `_load_vae_from_safetensors` was missing its required `device`
argument (a confirmed `TypeError` on first real invocation), and
`LoadClip`'s docstring still described the pre-A12 stubbed behavior.

Group B (P904-B1–B4) goes one step further than A9–A14's fix: A9–A14 made
the loading path **offline** (no HF network calls), but it still depends
on `diffusers.loaders.single_file_utils`'s `convert_z_image_transformer_checkpoint_to_diffusers`
and `convert_ldm_vae_checkpoint` — private, unversioned internals with no
API stability guarantee. Group B removes that dependency entirely by
inferring each checkpoint's config directly from its own raw tensor shapes
and performing the key remap manually, the way ComfyUI's
`model_detection.py` does — detect architecture and shape from the
checkpoint's own tensor names, with no `diffusers`-internal function calls
at all. A draft derivation proposal was checked against live `diffusers`
0.38.0 source during this phase's drafting and found to contain at least
one confirmed error (see P904-B1's task notes); every task in this group
requires independently verifying its own derivations at ACT time rather
than transcribing that proposal as ground truth.

Because Group B's `load_transformer()`/`load_vae()` continue to consume
raw, pre-remap checkpoint key formats (same as the A10/A11 versions they
replace), Group Z's transformer/VAE fixture design must still produce
fixtures in that raw format — saving a model's own post-construction
`state_dict()` would never exercise the shape-inference/remap path at all.
P904-Z1b's raw-checkpoint fixtures close that gap by inverting the known
remap tables before saving (the same approach works whether the consuming
code is A10/A11's diffusers-internal reuse or B1/B2's hand-rolled
replacement, since both expect the identical raw checkpoint format).

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | retrofit | P904-A1 … P904-A14 (incl. A6b) | Seven real-path defects in D16–D19, two systemic device-placement findings from the D1–D15 re-audit, a CI-breaking `torch` import in D20's own test, an HF-network-access defect in `LoadModel`/`LoadVae`'s real path requiring offline-loading rework across `loader.py` and `zit.py`, and two further defects found in that rework's own committed code (a missing `device` argument, a stale docstring) — fifteen tasks total, sequenced linearly |
| B | offline config inference rework | P904-B1 … P904-B4 | Replaces Group A's reliance on private, unversioned `diffusers` internals (`convert_z_image_transformer_checkpoint_to_diffusers`, `convert_ldm_vae_checkpoint`) with direct, ComfyUI-style shape inference from each checkpoint's own raw tensor keys — four tasks, sequenced linearly |
| Z | test infrastructure | P904-Z1 … P904-Z7 (incl. Z1b) | Real-mode CPU test suite mirroring the mock suite's per-node and chain coverage, extended through `VaeDecode` (D20) and through raw-checkpoint-format fixtures that exercise Group B's shape-inference path, using synthetic tiny-config checkpoints; Z6 fixes a live CI failure where 5 of Group B's own tests landed unmarked in the mock-mode test file; Z7 wires the suite into `ci.yml` itself for the first time (previously excluded from CI entirely, run only manually/at ACT time) |

## Prerequisites

Phase 18 groups D16–D20 complete and committed (`ClipTextEncode` real path,
`EmptyLatent` real path, `Sampler` real path, `arch/diffusion/zit.py`'s
`sample()` with pipeline assembly + invocation, and `VaeDecode` real path). Phase 903 complete
(`PipelineCache` correctly wired into `NodeContext`, model_id resolution at
dispatch time) — several P904 tasks read `self.ctx.pipeline_cache` and
`self.ctx.device`, which only became real-mode-usable after 903.

## Interfaces and Contracts

| Contract document | Relevant tasks | What must match |
|-------------------|---------------|-----------------|
| `ANVILML_DESIGN.md §1550` | P904-A5 | `cancel_flag`'s documented type is `threading.Event` — verified directly against the live doc during this phase's authoring and confirmed current; `worker_main.py`'s `list[bool]` is the side that diverges and must be brought into line, not the other way around |
| `ANVILML_DESIGN.md §10.4` | P904-A6, P904-A6b | `sample()`'s documented signature has been pre-updated by a human to include `clip` (and to omit `vae`) ahead of this phase — these tasks bring the code into line with the doc; the doc is not edited by either task |
| `ANVILML_DESIGN.md §10.4a` | P904-A7 | `load()`'s documented signature has been pre-updated by a human to include `device` ahead of this phase — this task brings the code into line with the doc; the doc is not edited by this task |
| `worker/nodes/base.py` | P904-A7, P904-A8 | `NodeContext.device` is the single source of truth for target device placement; any real-mode loader that allocates a tensor or moves a model must read it from `self.ctx.device`, never hardcode or omit |
| diffusers 0.38.0 `ZImagePipeline` (`pipeline_z_image.py`) | P904-A6b | `__init__`'s `vae` parameter has no default in the published signature but tolerates `None` via `register_modules`; `__call__` only dereferences `self.vae` at final decode (~line 583), unreachable when `output_type="latent"` — verify this against whatever diffusers version is actually pinned in `worker/requirements/` before relying on it, since pipeline internals can change across versions |

## Task Descriptions

### Group A — Retrofit

#### P904-A1: worker/tests/test_nodes_decode.py: unconditional torch import breaks CI

**Goal:** Fix D20's own committed real-path test, which does an unguarded
`import torch` inside a mock-mode-collected test file, breaking CI on
every commit until resolved. This is the first task in this phase's
sequential ordering — every subsequent Group A task's own implementation
report needs a working CI run to validate against, and CI is broken right
now regardless of which task lands first, so this one runs before any
other.

**Files to create or modify:**
- `worker/tests/test_nodes_decode.py` — guard or relocate `test_vaedeode_real_path_returns_pil_image` and its `_MockVaeWithDecode.decode()` helper's `import torch` calls

**Key implementation notes:**
- Confirmed by direct inspection: `worker/requirements/base.txt` lists bare `diffusers>=0.38.0` with no `[torch]` extra, and `torch` is only declared as an optional extra dependency on `diffusers`, never pulled in transitively by a plain install — CI's venv genuinely has no `torch` available
- Two equally valid fixes, either is acceptable — choose one and note the choice in the implementation report: (a) add `pytest.importorskip("torch")` as the first line of the test function (and the helper class's `decode()` method), so the test is skipped rather than erroring when `torch` is absent; or (b) move this test out of `test_nodes_decode.py` entirely into Group Z's `realcpu`-marked suite (P904-Z4, which is being widened to cover `VaeDecode` regardless) and delete it from this file, since it is a real-mode test that arguably belongs there in the first place
- If choosing (b), coordinate with P904-B4's implementation — do not let the same coverage exist in two places
- This task's only prereq is `P18-D20` — it has no dependency on any other P904 task, which is why it is sequenced first

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v
# -> exits 0 in an environment with NO torch installed (this is the actual
#    regression check -- run this in a venv built from base.txt alone,
#    with no cpu-linux-agent.txt or rocm-*.txt layered on top)
```

#### P904-A2: worker/nodes/arch/clip/qwen3.py + clip_l.py: fix tokenizer asset directory depth

**Goal:** Correct the tokenizer asset directory resolution in `qwen3.py` and
`clip_l.py` so `LoadClip` can locate the bundled tokenizer files in real
mode; both currently resolve one directory level too shallow.

**Files to create or modify:**
- `worker/nodes/arch/clip/qwen3.py` — change `Path(__file__).parent.parent` to `Path(__file__).parent.parent.parent` in `load()`'s `tokenizer_dir` resolution
- `worker/nodes/arch/clip/clip_l.py` — identical change

**Key implementation notes:**
- From `worker/nodes/arch/clip/qwen3.py`, two `.parent` calls resolve to `worker/nodes/arch/`, giving `worker/nodes/arch/assets/qwen25_tokenizer` — confirmed not to exist via `find` on the live repo
- The real location is `worker/assets/qwen25_tokenizer/` and `worker/assets/clip_l_tokenizer/` — three `.parent` calls
- `worker/nodes/arch/clip/t5.py` already has the correct depth (`.parent.parent.parent`) with an inline comment explaining the correction — copy that comment's wording into both fixed files for consistency, don't invent new wording
- Do not modify `t5.py`

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py worker/tests/test_arch_clip_l.py -v
# -> exits 0, same test count as before (mock-mode tests don't exercise the path, but must not regress)
python3 -c "from pathlib import Path; p = Path('worker/nodes/arch/clip/qwen3.py').read_text(); assert '.parent.parent.parent' in p"
python3 -c "from pathlib import Path; p = Path('worker/nodes/arch/clip/clip_l.py').read_text(); assert '.parent.parent.parent' in p"
```

#### P904-A3: worker/nodes/loader.py: LoadClip.execute() missing torch import

**Goal:** Fix a `NameError` in `LoadClip.execute()`'s real-mode branch that
fires before architecture dispatch is ever reached, masking P904-A2's fix
from being observable until this is fixed first.

**Files to create or modify:**
- `worker/nodes/loader.py` — add `import torch` inside `LoadClip.execute()`'s real-mode branch

**Key implementation notes:**
- The crash is at `return module.load(model_id, torch_dtype=torch.bfloat16)` — `torch` is referenced but never imported in this method's scope
- Every sibling real-mode function in this file (`LoadVae.execute()`'s `loader_fn`, `_load_model_from_hf_directory`, `_load_clip_from_hf_directory`) already does a local `import torch` immediately before first use — match that exact placement convention, immediately after the `ANVILML_WORKER_MOCK == "1"` early-return block
- Confirmed by direct execution in an environment without `torch` installed: the traceback shows `NameError` at this exact line, independent of whether `safetensors`/`diffusers` are present

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v
# -> exits 0, same test count as before
python3 -c "
import ast
tree = ast.parse(open('worker/nodes/loader.py').read())
src = open('worker/nodes/loader.py').read()
assert 'import torch' in src.split('class LoadClip')[1].split('class ')[0]
"
```

#### P904-A4: worker/nodes/sampler.py: EmptyLatent unbound ctx reference

**Goal:** Fix a plain `NameError` in `EmptyLatent.execute()`'s real-mode
branch caused by referencing a bare `ctx` instead of `self.ctx`.

**Files to create or modify:**
- `worker/nodes/sampler.py` — change `device=ctx.device` to `device=self.ctx.device` in `EmptyLatent.execute()`

**Key implementation notes:**
- This is a one-line fix; do not alter the surrounding arch-dispatch or `compute_latent_shape()` call, which are correct
- Confirmed by direct execution: `NameError: name 'ctx' is not defined` the moment the real branch is reached with a `model` input

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py -v
# -> exits 0, same test count as before
grep -n "device=self.ctx.device" worker/nodes/sampler.py
# -> at least one match inside EmptyLatent.execute()
grep -n "device=ctx.device" worker/nodes/sampler.py
# -> zero matches
```

#### P904-A5: worker/worker_main.py: reconcile cancel_flag type contract

**Goal:** Resolve the type contract mismatch between `worker_main.py`'s
`cancel_flag` (a `list[bool]`) and `zit.py`'s `_make_callback`, which calls
`.is_set()` expecting a `threading.Event` per its own documented contract
and `ANVILML_DESIGN.md §1550`.

**Files to create or modify:**
- `worker/worker_main.py` — change `_cancel_flag: list[bool] = [False]` to `_cancel_flag = threading.Event()`; change the two `_cancel_flag[0] = False`/`True` assignments to `.clear()`/`.set()`; add `import threading` if not already present

**Key implementation notes:**
- Do not modify `arch/diffusion/zit.py` — `threading.Event` is the documented design and the correct primitive for cross-thread cancellation signaling
- `ANVILML_DESIGN.md §1550` was verified directly during this phase's authoring and still specifies `threading.Event` exactly as `NodeContext`'s docstring shows — `worker_main.py`'s `list[bool]` is confirmed to be the side that diverges, not the doc; no escalation or doc update is needed for this task, only the code fix below
- This defect is latent (no test currently reaches it) but is the highest-priority item in this phase: it blocks every real `Sampler` invocation, for every architecture, the moment any pipeline's callback is actually invoked during denoising

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v
# -> exits 0, same test count as before
python3 -c "
import ast
src = open('worker/worker_main.py').read()
assert 'threading.Event()' in src
assert '_cancel_flag[0]' not in src
"
```

#### P904-A6: worker/nodes/arch/diffusion/zit.py + sampler.py: loader_fn reads tokenizer/text_encoder off the wrong object

**Goal:** Fix `zit.py`'s `loader_fn` reading `.tokenizer`/`.text_encoder` off
`conditioning` (which never carries them) instead of off a `clip` object
(which does); widen `Sampler`'s contract to supply that `clip` object.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/zit.py` — add `clip: Any` parameter to `sample()`'s signature (positioned after `conditioning`); change `loader_fn`'s `getattr(conditioning, "tokenizer", None)` / `getattr(conditioning, "text_encoder", None)` to read from `clip` instead
- `worker/nodes/sampler.py` — add `SlotSpec("clip", "CLIP")` to `Sampler.INPUT_SLOTS`; pass `clip=inputs.get("clip")` through to `mod.sample(...)` in `Sampler.execute()`
- `worker/tests/test_arch_zit.py` — update `test_sample_real_assembles_pipeline_via_cache` and `test_sample_real_invokes_pipeline_with_correct_args` to pass a mock `clip` object to `sample()`, since both currently call it without one and will break the moment `clip` becomes a required positional parameter
- `docs/TESTS.md` — update the catalogue entries for both tests above to reflect the new `clip` argument, per `ENVIRONMENT.md §11.4`/`§5.10`'s obligation that any task modifying a test file updates its catalogue entry in the same task

**Key implementation notes:**
- `Conditioning` (`worker/nodes/encoder.py`) only ever has `.positive`/`.negative` — confirmed by reading the class definition in full; it was never going to have `.tokenizer`/`.text_encoder`
- `RealClip` (`worker/nodes/loader.py`) is the object that actually has `.tokenizer`/`.text_encoder` properties — this is the object `LoadClip` produces and the one that should flow into `Sampler`
- This traces to P18-D18a's own task context, which directed the implementing agent to pull tokenizer/text_encoder "from model/conditioning" — the original task was underspecified, not an implementation error; note this in the implementation report rather than treating it as a regression
- `Sampler`'s call into `mod.sample(...)` currently passes 9 positional arguments ending at `emit_progress`, then `pipeline_cache=` as keyword-only — insert `clip` as a new positional argument in the same position it's added to `sample()`'s signature, and update the call site to match
- `docs/TESTS.md` is task-owned documentation (unlike `ANVILML_DESIGN.md`/`ARCHITECTURE.md`/`ENVIRONMENT.md`) — updating it here is a normal ACT-session obligation, not a boundary violation
- **Do not edit `docs/ANVILML_DESIGN.md`.** Design/architecture/environment documents are human-authored only and out of any task's scope per the agent operating rules. `ANVILML_DESIGN.md §10.4`'s `sample()` signature already reflects this fix — the design doc has been pre-updated by a human ahead of this phase so it states the correct, current contract before this task lands; this task's code change brings the implementation into line with what the doc already says, not the other way around

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py worker/tests/test_nodes_sampler.py -v
# -> exits 0, same test count as before, no test calls sample() without clip
python3 -c "
import inspect, os
os.environ['ANVILML_WORKER_MOCK'] = '1'
from worker.nodes.arch.diffusion.zit import sample
assert 'clip' in inspect.signature(sample).parameters
"
grep -n "test_sample_real_assembles_pipeline_via_cache\|test_sample_real_invokes_pipeline_with_correct_args" docs/TESTS.md
# -> both entries present and updated to mention the clip argument
```

#### P904-A6b: worker/nodes/arch/diffusion/zit.py: remove vestigial vae parameter

**Goal:** Remove `sample()`'s unused-by-design `vae` parameter — `Sampler`
must never receive or forward a VAE component; `VaeDecode` (P18-D20) is its
sole owner.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/zit.py` — remove the `vae: Any = None` parameter from `sample()`'s signature; remove the `vae=vae` line from `loader_fn`'s `ZImagePipeline(...)` construction
- `worker/tests/test_arch_zit.py` — two call sites pass `vae=None` and `vae=mock_vae` respectively; remove both keyword arguments, since `sample()` no longer accepts `vae`
- `docs/TESTS.md` — update the corresponding catalogue entries to reflect the removed `vae` argument

**Key implementation notes:**
- Confirmed via `diffusers` 0.38.0 source (`pipeline_z_image.py`): `ZImagePipeline.__init__` accepts `vae=None` without raising (`register_modules` tolerates `None`; `vae_scale_factor` falls back to `8` when `self.vae is None`), and `__call__` never dereferences `self.vae` when `output_type="latent"` — only at final decode (~line 583), which `Sampler`'s call never reaches
- Do not add a `vae` `SlotSpec` to `Sampler.INPUT_SLOTS` — this is a deliberate design decision (Sampler produces a latent; `VaeDecode` is the only node that consumes a VAE), not an oversight to fix
- This task depends on P904-A6 because both touch `sample()`'s signature; sequencing avoids two agents editing the same signature in conflicting orders
- `ANVILML_DESIGN.md §10.4` was never going to document a `vae` parameter on `sample()` in the first place, and is human-authored, out of this task's scope to touch — no design-doc action is needed here at all, this is a code-only fix

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v
# -> exits 0, same test count as before, no test passes vae= to sample()
python3 -c "
import inspect, os
os.environ['ANVILML_WORKER_MOCK'] = '1'
from worker.nodes.arch.diffusion.zit import sample
assert 'vae' not in inspect.signature(sample).parameters
"
grep -n "SlotSpec(\"vae\"" worker/nodes/sampler.py
# -> zero matches (Sampler.INPUT_SLOTS must not gain a vae slot)
grep -n "vae=" worker/tests/test_arch_zit.py
# -> zero matches (no test call site passes vae anymore)
```

#### P904-A7: worker/nodes/arch/clip/{qwen3,clip_l,t5}.py: text encoders never moved to ctx.device

**Goal:** Fix all three CLIP arch modules' `load()` functions to place the
loaded text encoder on the worker's assigned device instead of silently
defaulting to CPU.

**Files to create or modify:**
- `worker/nodes/arch/clip/qwen3.py` — widen `load(model_id, torch_dtype)` to `load(model_id, torch_dtype, device: str = "cpu")`; call `model.to(device)` before returning; construct `RealClip(tokenizer, model, device=device)`
- `worker/nodes/arch/clip/clip_l.py` — identical change
- `worker/nodes/arch/clip/t5.py` — identical change
- `worker/nodes/loader.py` — `LoadClip.execute()` passes `device=self.ctx.device` explicitly into `module.load(...)` in real mode

**Key implementation notes:**
- Confirmed by grep across all three files: zero references to `.to(` or `device=` anywhere in any `load()` function; `RealClip.__init__` defaults `device="cpu"` and is never overridden by any caller
- This is a silent-degradation defect, not a crash — text encoding runs on CPU with no exception, even when `ctx.device` is `cuda:0`, making it harder to notice than the crash-on-first-use defects elsewhere in this phase
- Apply the identical fix to all three files in the same task — do not fix only one and leave the others inconsistent, which is exactly the pattern that produced P904-A2's bug (one file fixed, two left behind)
- `device` must default to `"cpu"` (matching `RealClip.__init__`'s own existing default), not be a bare required parameter — `test_arch_clip_qwen3.py`, `test_arch_clip_l.py`, and `test_arch_clip_t5.py` each call `load("/fake/path", None)` positionally with only two arguments; a required third parameter would break all three without any other change. The default preserves this call pattern unchanged while still letting `LoadClip.execute()` pass `device=self.ctx.device` explicitly in real mode
- **Do not edit `docs/ANVILML_DESIGN.md`.** Design/architecture/environment documents are human-authored only and out of any task's scope per the agent operating rules. `ANVILML_DESIGN.md §10.4a`'s `load()` signature already reflects this fix — the design doc has been pre-updated by a human ahead of this phase so it states the correct, current contract before this task lands; this task's code change brings the implementation into line with what the doc already says, not the other way around

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py worker/tests/test_arch_clip_l.py worker/tests/test_arch_clip_t5.py worker/tests/test_nodes_loader.py -v
# -> exits 0, same test count as before, no test file needed modification
for f in qwen3 clip_l t5; do
  grep -n "\.to(device)" worker/nodes/arch/clip/$f.py || exit 1
done
# -> all three files contain a .to(device) call
```

#### P904-A8: worker/nodes/loader.py: LoadModel and LoadVae never moved to ctx.device

**Goal:** Fix the same device-placement defect class as P904-A7 in
`LoadModel` and `LoadVae`'s real-mode loading paths.

**Files to create or modify:**
- `worker/nodes/loader.py` — `_load_model_from_hf_directory` gains a `device: str` parameter, calls `transformer.to(device)` before constructing `RealModel`; `LoadModel.execute()` calls it as `_load_model_from_hf_directory(model_id, model_id, self.ctx.device)`; `LoadVae.execute()`'s `loader_fn` closure captures `self.ctx.device` and calls `.to(device)` on the `AutoencoderKL` result before returning

**Key implementation notes:**
- Confirmed by grep: `_load_model_from_hf_directory` and `LoadVae.execute()`'s `loader_fn` never reference `self.ctx.device` or call `.to(` anywhere — both default to whatever device `from_single_file()` places them on (CPU, absent an explicit device map)
- `.to()` returns a new reference for some module wrapper types and mutates in place for others, depending on the `diffusers` version in use — always assign the return value (`transformer = transformer.to(device)`), never assume in-place mutation
- This is the broader-scope sibling of P904-A7 — together they close the device-placement gap across every real-mode loader in the project, not just the CLIP path

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v
# -> exits 0, same test count as before
grep -n "transformer.to(device)\|transformer = transformer.to(device)" worker/nodes/loader.py
grep -n "\.to(device)" worker/nodes/loader.py
# -> at least two matches (LoadModel's transformer, LoadVae's AutoencoderKL)
```

#### P904-A9: worker/nodes/loader.py: remove deprecated HF-directory loading remnants entirely

**Goal:** Delete `_load_from_hf_directory` and `_load_clip_from_hf_directory` outright —
both are dead code kept "for future reactivation," and that decision is reversed.

**Files to create or modify:**
- `worker/nodes/loader.py` — delete both functions and any now-unused imports they alone required

**Key implementation notes:**
- Both functions are never called anywhere in the codebase — confirmed by their own docstrings ("kept but never called") and by grep
- After deletion, check whether `CLIPTextModelWithProjection`, `CLIPTokenizer`, `Qwen2Tokenizer`, `Qwen3ForCausalLM`, `T5ForConditionalGeneration`, `T5TokenizerFast` are still imported anywhere else in this file — if not, remove those imports too
- Do not rename or touch `_load_model_from_hf_directory` in this task — that is P904-A12's job, kept in a separate task so the deletion diff here stays clean and easy to review independently of the rename/restructure diff
- This is a pure removal; no behavior change for any currently-passing test

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v
# -> exits 0, same test count as before
grep -n "_load_from_hf_directory\|_load_clip_from_hf_directory" worker/nodes/loader.py
# -> zero matches
```

#### P904-A10: worker/nodes/arch/diffusion/zit.py: add load_transformer() — offline transformer loading, no HF network access

**Goal:** Stop `LoadModel`'s real path from silently contacting HuggingFace Hub during
generation. Root cause confirmed by direct execution (console showed `config.json: 100%`
being fetched) and by reading `diffusers` 0.38.0 source: `ZImageTransformer2DModel.
from_single_file()` with no `config=`/`original_config=` kwarg falls through to
`fetch_diffusers_config(checkpoint)`, which guesses an HF repo id and downloads its
`config.json`. `local_files_only` alone does not reliably fix this — it only skips the
download if the guessed repo happens to already be in the local HF cache.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/zit.py` — add `load_transformer(model_id: str) -> ZImageTransformer2DModel`

**Key implementation notes:**
- `ZImageTransformer2DModel()` constructed with zero arguments already matches the published 6B ZiT architecture — `dim=3840, n_layers=30, n_heads=30, n_kv_heads=30, cap_feat_dim=2560` are all registered defaults, confirmed by reading the class's `@register_to_config __init__`; no separate config dict is needed for this single currently-supported architecture
- Load the raw checkpoint via `safetensors.torch.load_file(model_id)`, remap its keys via `diffusers.loaders.single_file_utils.convert_z_image_transformer_checkpoint_to_diffusers(checkpoint)` — this is `diffusers`' own tested key-remap and QKV-defuse logic (handles the fused `attention.qkv.weight` → separate `to_q`/`to_k`/`to_v` split, the `x_embedder.`/`final_layer.` → `all_x_embedder.2-1.`/`all_final_layer.2-1.` rename, and the `model.diffusion_model.` prefix strip) — reuse it as-is, do not reimplement
- Then `model.load_state_dict(remapped_checkpoint)` and return the model
- Zero network calls anywhere in this function — this is the actual fix, not a flag toggle
- `diffusers.loaders.single_file_utils` is a private module path, not part of the public API — flag in the implementation report that this may break across `diffusers` version bumps and should be re-verified on upgrade

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v
# -> exits 0, same test count as before (mock-mode tests don't exercise this function)
python3 -c "
import os
os.environ['ANVILML_WORKER_MOCK'] = '1'
from worker.nodes.arch.diffusion.zit import load_transformer
assert callable(load_transformer)
"
```

#### P904-A11: worker/nodes/arch/diffusion/zit.py: add load_vae() — offline VAE loading, no HF network access

**Goal:** Stop `LoadVae`'s real path from the same HF-contacting defect, for the VAE
component. VAE loading lives in `zit.py`, not a separate module — it is bound 1:1 to
the diffusion model's latent space, and `VAE_SCALE_FACTOR` already lives here as a
documented constant.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/zit.py` — add `load_vae(model_id: str) -> AutoencoderKL`

**Key implementation notes:**
- Same root cause as P904-A10 — `AutoencoderKL.from_single_file()` shares the identical `FromOriginalModelMixin` code path, confirmed by reading the source
- Construct `AutoencoderKL(block_out_channels=[128, 256, 512, 512], ...)` using the published config this file's own `VAE_SCALE_FACTOR` comment already documents
- Load the raw checkpoint via `safetensors.torch.load_file(model_id)`; remap keys via `diffusers.loaders.single_file_utils.convert_ldm_vae_checkpoint(checkpoint, config)` where `config` is a plain dict containing `down_block_types`/`up_block_types` matching the 4-entry `block_out_channels` length — this function only reads list *length* from `config`, not a full LDM-format YAML; confirmed by reading its source. Do not attempt to construct a YAML-shaped `original_config`
- `convert_ldm_vae_checkpoint` correctly handles both prefixed (`first_stage_model.`/`vae.`) and bare standalone-file keys — confirmed: when no recognized prefix is found, its internal `vae_key` stays `""`, and every key matches `str.startswith("")`, so a standalone file with unprefixed keys passes through unchanged. No special-casing needed for the standalone-file case this project actually uses

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v
# -> exits 0, same test count as before
python3 -c "
import os
os.environ['ANVILML_WORKER_MOCK'] = '1'
from worker.nodes.arch.diffusion.zit import load_vae
assert callable(load_vae)
"
```

#### P904-A12: worker/nodes/loader.py: rewrite LoadModel/LoadVae/LoadClip's loader functions as thin per-arch wrappers

**Goal:** Make `LoadModel` and `LoadVae`'s real paths dispatch through the arch system
consistently, the way `LoadClip` already correctly does via `arch_clip.get_module()` —
currently `LoadModel`/`LoadVae` call `diffusers` classes directly inline, bypassing the
per-architecture abstraction entirely.

**Files to create or modify:**
- `worker/nodes/loader.py` — rename `_load_model_from_hf_directory` to `_load_model_from_safetensors`; rewrite `LoadVae.execute()`'s inline `loader_fn` into a named `_load_vae_from_safetensors`; rewrite `LoadClip.execute()`'s inline dispatch into a named `_load_clip_from_safetensors`

**Key implementation notes:**
- `_load_model_from_safetensors`: keep the existing safetensors-metadata arch-detection logic exactly as-is (the `safe_open(...).metadata` read and the path-stripping fallback are unrelated to the HF-network bug) — only replace the direct `ZImageTransformer2DModel.from_single_file(...)` call with `arch.diffusion.get_module_by_name(detected_arch).load_transformer(model_id)` (P904-A10, dispatch added by P904-A13)
- `_load_vae_from_safetensors(model_id, arch)`: same dispatch pattern, calling `get_module_by_name(arch).load_vae(model_id)` (P904-A11)
- `_load_clip_from_safetensors(model_id, clip_type)`: this is a pure rename/extraction for naming symmetry with the other two — `LoadClip.execute()`'s existing dispatch via `arch_clip.get_module()` is already correct and needs no behavior change, only a name
- Update `LoadModel.execute()`/`LoadVae.execute()`/`LoadClip.execute()`'s call sites to use the new function names

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v
# -> exits 0, same test count as before
grep -n "_load_model_from_safetensors\|_load_vae_from_safetensors\|_load_clip_from_safetensors" worker/nodes/loader.py
# -> all three names present
grep -n "_load_model_from_hf_directory" worker/nodes/loader.py
# -> zero matches (old name fully retired)
```

#### P904-A13: worker/nodes/arch/diffusion/__init__.py: add an arch-by-name lookup

**Goal:** `LoadModel`/`LoadVae` (P904-A12) only have a bare architecture string at
dispatch time — no model object exists yet to call the existing `get_module(model_obj)`
with. Add a lookup that works from a name alone.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/__init__.py` — add `get_module_by_name(arch: str) -> ModuleType | None`

**Key implementation notes:**
- Reuse the same `pkgutil.iter_modules()` scan `get_module(model_obj)` already does, but match each module's `can_handle()` against a tiny shim object carrying only `.arch = arch` — `can_handle()` only ever reads `getattr(model_obj, "arch", None)` (confirmed by reading `zit.py`'s source), so a bare shim with just that one attribute satisfies it without constructing a real model
- Do not change `can_handle()`'s signature or `get_module(model_obj)`'s existing behavior — this is a pure addition alongside it, zero risk to already-passing callers
- A minimal local class or `types.SimpleNamespace(arch=arch)` both work as the shim; either is acceptable

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_init.py -v
# -> exits 0, same test count as before
python3 -c "
import os
os.environ['ANVILML_WORKER_MOCK'] = '1'
from worker.nodes.arch.diffusion import get_module_by_name
mod = get_module_by_name('zit')
assert mod is not None
assert mod.__name__.endswith('.zit')
"
```

#### P904-A14: worker/nodes/loader.py: LoadVae missing device arg (TypeError on first real call); LoadClip stale docstring

**Goal:** Fix two confirmed defects in the already-committed P904-A9–A13
implementation, found by direct inspection of the live repository at HEAD
(commit `b54f40b`, post-A13) rather than introduced by this task's own
authoring.

**Files to create or modify:**
- `worker/nodes/loader.py` — fix `LoadVae.execute()`'s call to `_load_vae_from_safetensors`; correct `LoadClip.execute()`'s stale docstring

**Key implementation notes:**
- `LoadVae.execute()` calls `_load_vae_from_safetensors(model_id, "zit")` with only 2 of the function's 3 required positional arguments — `device` is missing entirely. Confirmed via `ast` inspection: the function's signature is `_load_vae_from_safetensors(model_id: str, arch: str, device: str)`, no default on `device`. This raises `TypeError: missing 1 required positional argument: 'device'` the instant `LoadVae`'s real path executes. `LoadModel`'s equivalent call correctly passes `self.ctx.device` as the third argument — `LoadVae`'s doesn't. Fix: change the call to `_load_vae_from_safetensors(model_id, "zit", self.ctx.device)`
- `LoadClip.execute()`'s docstring still says `Raises: NotImplementedError: If called in non-mock mode. The real safetensors loading path is stubbed until P18-D1.` — stale; the real dispatch already works correctly via `_load_clip_from_safetensors` (landed in P904-A12). Update the docstring to describe actual behavior; no code change needed for this part

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v
# -> exits 0, same test count as before
grep -n '_load_vae_from_safetensors(model_id, "zit", self.ctx.device)' worker/nodes/loader.py
# -> confirms the fixed call site
grep -n "stubbed until P18-D1" worker/nodes/loader.py
# -> zero matches (stale docstring corrected)
```

### Group B — Offline Config Inference Rework

This group replaces Group A's reliance on `diffusers.loaders.single_file_utils`'s
private internal functions (`convert_z_image_transformer_checkpoint_to_diffusers`,
`convert_ldm_vae_checkpoint`) with direct, ComfyUI-style shape inference from each
checkpoint's own raw tensor keys and shapes. Group A's A9–A14 fix made the loading
path *offline* (no HF network calls); Group B makes it *self-contained* (no
dependency on unversioned `diffusers` internals that could change or move without
notice). A draft derivation proposal was provided ahead of this group's authoring;
it was checked against live `diffusers` 0.38.0 source during this phase's drafting
and found to contain at least one confirmed error (P904-B1's `in_channels`
derivation) — every task in this group requires independently verifying its own
derivations at ACT time rather than transcribing the proposal's tables as ground
truth.

#### P904-B1: worker/nodes/arch/diffusion/zit.py: load_transformer() — replace diffusers-internals reuse with shape-inferred config + manual key remap

**Goal:** Remove `load_transformer()`'s dependency on the private, unversioned
`diffusers.loaders.single_file_utils.convert_z_image_transformer_checkpoint_to_diffusers`
function by inferring the model's config directly from the raw checkpoint's own
tensor shapes, then performing the key remap manually.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/zit.py` — rewrite `load_transformer()`'s config and remap logic

**Key implementation notes:**
- Currently zero-arg-constructs `ZImageTransformer2DModel()` (relying on registered defaults matching the published config) then calls the private diffusers remap function. Replace both: infer `dim`, `head_dim`/`n_heads`, `n_layers`, `n_refiner_layers`, `cap_feat_dim`, `in_channels`, and patch sizes directly from tensor shapes present in the raw state dict, the way ComfyUI's `model_detection.py` does
- **Confirmed against a full, untruncated key scan of the real checkpoint** (sorted, all keys, not a `[:30]` sample): `dim=3840` (`*.attention.out.weight` is `[3840, 3840]`), `head_dim=128` (`*.attention.q_norm.weight`/`k_norm.weight` are `[128]`), `n_heads=n_kv_heads=30` (`dim/head_dim`, confirmed both equal via the fused QKV width: `*.attention.qkv.weight` is `[11520, 3840]`, `11520/3 = 3840 = dim`, so no GQA on the main attention), `n_layers=30` (`layers.0` through `layers.29` present, no `layers.30`), `n_refiner_layers=2` (`context_refiner.0`/`.1` and `noise_refiner.0`/`.1`, both stacks end at index 1), `cap_feat_dim=2560` (`cap_embedder.0.weight` is `[2560]`, a 1D `RMSNorm` weight whose only dimension is `cap_feat_dim`)
- `in_channels=16` — **not 64**, correcting an earlier draft derivation that claimed `final_layer.linear.weight.shape[0]` (`[64, 3840]`, so shape `[0] = 64`) equals `in_channels` directly. It does not: `FinalLayer`'s output width is `patch_size**2 * f_patch_size * out_channels` (`out_channels == in_channels`), and with the registered default patch sizes (`all_patch_size=(2,)`, `all_f_patch_size=(1,)`), `64 = 2**2 * 1 * in_channels` → `in_channels = 16`. This is independently cross-confirmed by the VAE's `latent_channels` (P904-B2), which must equal the transformer's `in_channels` exactly — the real VAE scan also shows `16`
- The fused-QKV-to-separate-Q/K/V split logic (`torch.chunk` into three equal parts along dim 0) is correct in the original draft proposal and may be implemented directly, without importing it from `diffusers`
- `norm_eps`, `rope_theta`, `t_scale`, `axes_dims`, `axes_lens`, and `qk_norm` are scalar/list hyperparameters never stored as weights — they cannot be derived from tensor shapes at all. Keep these as hardcoded constants matching `ZImageTransformer2DModel`'s registered defaults (`norm_eps=1e-5`, `rope_theta=256.0`, `t_scale=1000.0`, `axes_dims=[32,48,48]`, `axes_lens=[1024,512,512]`, `qk_norm=True`) — there is nothing to infer here, this is not a gap in the shape-inference approach, just a category of config this approach cannot and should not try to cover
- `load_transformer()` must continue to perform **zero network calls** and must continue to behave identically in mock mode (`ANVILML_WORKER_MOCK=1` returns `None` immediately, no imports)

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v
# -> exits 0, same test count as before
grep -n "convert_z_image_transformer_checkpoint_to_diffusers" worker/nodes/arch/diffusion/zit.py
# -> zero matches (private diffusers internal no longer imported)
grep -n "in_channels.*=.*16\|in_channels=16" worker/nodes/arch/diffusion/zit.py
# -> confirms the corrected in_channels value is actually used, not the wrong 64
```

#### P904-B2: worker/nodes/arch/diffusion/zit.py: load_vae() — replace diffusers-internals reuse with shape-inferred config + manual key remap

**Goal:** Remove `load_vae()`'s dependency on the private, unversioned
`diffusers.loaders.single_file_utils.convert_ldm_vae_checkpoint` function the same
way P904-B1 does for the transformer.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/zit.py` — rewrite `load_vae()`'s config and remap logic

**Key implementation notes:**
- Currently constructs `AutoencoderKL(block_out_channels=[128, 256, 512, 512])` (hardcoded from the published config) then calls the private diffusers remap function. Replace both: derive `latent_channels` from a decoder input-layer weight shape; derive `block_out_channels`'s stage count and per-stage values from the actual down/up block weights present in the checkpoint — scan for the highest block index actually present in the state dict's keys, the way ComfyUI enumerates blocks dynamically, rather than hardcoding 4 stages for every future architecture that reuses this pattern
- **Confirmed against a full, untruncated key scan of the real VAE checkpoint**: `latent_channels=16` (`decoder.conv_in.weight` is `[512, 16, 3, 3]`, dimension index 1) — this independently cross-confirms P904-B1's `in_channels=16` finding for the transformer, since the two must match exactly (same latent tensor on both sides); `block_out_channels=(128, 256, 512, 512)` (`decoder.up.{0,1,2,3}.block.0.conv1.weight` channel progression — the decoder processes stages in *reverse* of the tuple's order, deepest/smallest-spatial-resolution stage first); `in_channels=3`, `out_channels=3` (`encoder.conv_in`/`decoder.conv_out`, standard RGB)
- `layers_per_block=2` — **not 3**, despite `decoder.up.N.block.{0,1,2}` showing 3 resnet blocks per stage in the real scan. Confirmed via `diffusers`' `vae.py` source: the `Decoder` class (unlike the `Encoder`) constructs its up-blocks with `num_layers=self.layers_per_block + 1` — a documented decoder-only asymmetry. Apply this `-1` offset when inferring `layers_per_block` from the observed block count; reading the raw count of 3 directly would be wrong
- `scaling_factor` (proposal's claimed value: `0.18215`, the original Stable Diffusion 1.x default) **cannot be confirmed from tensor shapes at all** — it is a training-time scalar baked into the model, never stored as a weight tensor. There is no way to verify this value is correct for Z-Image-Turbo's actual VAE from the checkpoint alone; treat it as a best-effort default, and flag in the implementation report that an incorrect value here would produce a visibly wrong brightness/contrast in decoded images, not a crash — so this is the one part of this task that real-image visual inspection (not a shape-match test) would be needed to fully validate, beyond this phase's CPU-only test scope
- `load_vae()` must continue to perform **zero network calls** and must continue to behave identically in mock mode

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v
# -> exits 0, same test count as before
grep -n "convert_ldm_vae_checkpoint" worker/nodes/arch/diffusion/zit.py
# -> zero matches (private diffusers internal no longer imported)
grep -n "layers_per_block.*=.*2\|layers_per_block=2" worker/nodes/arch/diffusion/zit.py
# -> confirms the corrected layers_per_block value (2, not the raw observed 3) is used
```

#### P904-B3: worker/nodes/loader.py: switch arch detection from safetensors-metadata-only to key-prefix-based detection

**Goal:** Add key-prefix-based architecture detection as the primary signal in
`_load_model_from_safetensors`, matching the ComfyUI pattern, so future
architectures whose checkpoints don't carry export-tool metadata can still be
detected correctly.

**Files to create or modify:**
- `worker/nodes/loader.py` — extend `_load_model_from_safetensors`'s arch-detection logic

**Key implementation notes:**
- `_load_model_from_safetensors` currently detects arch by reading safetensors metadata (an `"arch"` key written by the export tool, if present), with a path-derived fallback. This works today but doesn't scale to architectures whose checkpoints carry no export-tool metadata at all
- Add key-prefix inspection of the raw state dict as an additional detection signal: keys starting with `model.diffusion_model.` indicate ZiT (confirmed via the real remap table in P904-B1's source reading); a future `flux.py` would add its own distinguishing prefix or key pattern
- Keep the existing metadata-based detection as a fallback for checkpoints that do carry it — this is additive to P904-A12's working dispatch flow, not a replacement of it
- Confirm this change does not alter `LoadModel`'s behavior for the currently-supported ZiT checkpoint — same `detected_arch` result, same `get_module_by_name()` dispatch outcome, as a regression check

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v
# -> exits 0, same test count as before, same detected arch for the ZiT test checkpoint
```

#### P904-B4: worker/nodes/arch/clip/{qwen3,clip_l,t5}.py: confirm no rework needed — already fully offline

**Goal:** Verify and formally close out that the three CLIP arch modules need no
equivalent rework, since they were never built on the `from_single_file()`/
diffusers-internals pattern P904-B1–B3 address.

**Files to create or modify:**
- None — this is a verification task, not a code change

**Key implementation notes:**
- Confirmed by reading all three files: each constructs its model via `Config(**pinned_values)` + `load_state_dict()` — no `from_single_file()`, no network-capable config fetch of any kind. The concern P904-B1/B2 address does not apply here
- The only `from_pretrained()` call in any of these files loads the tokenizer from a local vendored directory under `worker/assets/` — offline by virtue of the path given, not by mechanism (`from_pretrained()` itself can reach the network, but only when given a repo id, never when given a local directory path, which is all these three files ever pass)
- Re-confirm both points still hold at ACT time: no `diffusers`/`transformers` version bump has changed `from_pretrained()`'s local-path behavior, and no `pinned_values` have drifted from a real checkpoint's actual shapes. Record the confirmation in the implementation report
- If either check fails, escalate rather than silently patching — that would mean this task's premise was wrong and the scope needs re-assessing, not a quiet fix

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_qwen3.py worker/tests/test_arch_clip_l.py worker/tests/test_arch_clip_t5.py -v
# -> exits 0, same test count as before, no code changes made
```

### Group Z — Real-Mode CPU Test Infrastructure

#### P904-Z1: worker/tests/real_fixtures.py: synthetic tiny clip checkpoint fixtures (qwen3/clip_l/t5)

**Goal:** Provide pytest fixtures that produce small, fast, real (not
mocked) `.safetensors` checkpoints for every component type the real-mode
suite needs, without depending on multi-gigabyte real model downloads.

**Files to create or modify:**
- `worker/tests/real_fixtures.py` — new file; fixtures `tiny_qwen3_clip`, `tiny_clip_l_clip`, `tiny_t5_clip`, each returning a saved checkpoint file path

**Key implementation notes:**
- For each CLIP variant, reuse the real loader module's own `config_values` pattern (`qwen3.py`/`clip_l.py`/`t5.py` already show the exact `Config(**values)` → model → `load_state_dict` construction) but with drastically reduced `hidden_size`/`num_hidden_layers` (e.g. `hidden_size=32, num_hidden_layers=2`) — do not invent a different construction path from the one production code already uses
- These fixtures are unaffected by the raw-checkpoint-format concern that applies to the transformer/VAE fixtures (P904-Z1b) — `qwen3.py`/`clip_l.py`/`t5.py` call `load_state_dict()` directly on whatever is saved, with no key-remap step in between, so a model's own native `state_dict()` is exactly the correct format to save here
- Each fixture must return a file **path** (`tmp_path`-scoped), not the in-memory model object — the tests in B3–B5 are specifically exercising the file-load path (`from_single_file`, `load_state_dict(safetensors_load_file(...))`), not bypassing it

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/real_fixtures.py --collect-only
# -> collects without error (file itself has no test_ functions, just fixtures; verifies no import-time failure)
```

#### P904-Z1b: worker/tests/real_fixtures.py: raw-checkpoint-format ZiT transformer and VAE fixtures (pre-remap keys)

**Goal:** Provide tiny fixtures in the same raw, pre-remap key format real
`.safetensors` checkpoints actually use — `load_transformer()`/`load_vae()`
(P904-A10/A11) consume fused QKV weights and original (non-diffusers)
key names, and a fixture saving a model's own post-construction
`state_dict()` would skip the remap/QKV-defuse path entirely, producing a
test that passes without ever exercising the code it's meant to verify.

**Files to create or modify:**
- `worker/tests/real_fixtures.py` — add `tiny_zit_transformer_raw`, `tiny_vae_raw` fixtures

**Key implementation notes:**
- `tiny_zit_transformer_raw`: build a tiny `ZImageTransformer2DModel(dim=64, n_layers=2, n_heads=2, cap_feat_dim=64)`, take its `state_dict()`, then invert it into raw-checkpoint format before saving — fuse `to_q`/`to_k`/`to_v` into a single `qkv.weight` via `torch.cat`, rename `all_x_embedder.2-1.`/`all_final_layer.2-1.` back to `x_embedder.`/`final_layer.`, and prepend `model.diffusion_model.` to every key
- Build this inverse mapping by reading `diffusers.loaders.single_file_utils.convert_z_image_transformer_checkpoint_to_diffusers`'s own rename table directly — do not guess at the mapping independently; the two must be exact inverses of each other, or the round-trip test is meaningless (it would pass even if the real remap function were broken)
- `tiny_vae_raw`: same approach, inverting `convert_ldm_vae_checkpoint`'s `DIFFUSERS_TO_LDM_MAPPING` for a tiny `AutoencoderKL(block_out_channels=(8,16), latent_channels=4)`
- Both fixtures return a saved file **path**, matching B1's convention

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/real_fixtures.py --collect-only
# -> collects without error
```

#### P904-Z2: pytest.ini + ci.yml: register realcpu marker, exclude from CI

**Goal:** Make the new real-mode CPU suite explicitly opt-in and provably
excluded from the default CI gate.

**Files to create or modify:**
- `worker/tests/pytest.ini` — add a `markers` section registering `realcpu`
- `.github/workflows/ci.yml` — worker test job's pytest invocation gains `-m "not realcpu"`

**Key implementation notes:**
- `worker/requirements/cpu-linux-agent.txt` (`torch` via PyTorch's dedicated CPU index, `--index-url https://download.pytorch.org/whl/cpu`) **already exists on `main`** — creating it is explicitly **out of scope** for this task. Do not recreate, overwrite, or modify it; only reference its existence when describing how P904-Z3–B5 are run
- `worker/requirements/base.txt` deliberately excludes `torch` (GPU-arch-dependent); `rocm-linux.txt`/`rocm-windows.txt` are the existing manual-install-only precedent that `cpu-linux-agent.txt` already follows — `cpu-linux-agent.txt` is not wired into `scripts/install_worker_deps.sh`, and this task must not change that
- The marker exclusion in `ci.yml` is defense-in-depth, not the only thing preventing CI from running these tests — `torch`'s absence from CI's venv already causes any test importing it to fail at collection; the explicit `-m "not realcpu"` makes the exclusion legible to a human reading the CI config rather than relying on an implicit import failure

**Acceptance criterion:**
```bash
grep -n "realcpu" worker/tests/pytest.ini
# -> marker registered
grep -n 'not realcpu' .github/workflows/ci.yml
# -> exclusion flag present in the worker test job's pytest invocation
test -f worker/requirements/cpu-linux-agent.txt && grep -q "^torch" worker/requirements/cpu-linux-agent.txt
# -> confirms the file already exists (pre-condition check, not this task's output)
```

#### P904-Z3: worker/tests/test_real_loaders.py: real CPU node tests for LoadModel/LoadVae/LoadClip

**Goal:** Prove `LoadModel`, `LoadVae`, and `LoadClip`'s real-mode paths
function correctly against real (tiny) checkpoints — the first real
execution of these three loaders anywhere in the project's committed test
suite.

**Files to create or modify:**
- `worker/tests/test_real_loaders.py` — new file, all tests `@pytest.mark.realcpu`

**Key implementation notes:**
- Force `ANVILML_WORKER_MOCK=0` per-test using the same override-and-restore pattern already established in `test_arch_zit.py`'s `test_sample_real_assembles_pipeline_via_cache`
- `LoadModel.execute(model_id=<tiny_zit_transformer_raw fixture path from P904-Z1b>)` must return a `RealModel` whose `.in_channels` matches the *tiny* fixture's config (`4`), not the real architecture's published value (`16`) — this is the test's way of confirming it actually loaded the tiny checkpoint rather than silently falling back to something else
- `LoadClip.execute(model_id=<fixture path>, clip_type=...)` must be run once per clip type (`qwen3`, `clip_l`, `t5`) using each of P904-Z1's three respective fixtures — do not test only one and assume the others are equivalent, which is exactly the assumption that let P904-A2's bug exist identically in two of the three files and go unnoticed in the third
- Assert the loaded `RealClip.text_encoder`'s device matches whatever `ctx.device` was set to in the test's `NodeContext` — this is the direct regression check for P904-A7's fix
- Add `test_loadmodel_no_network_access`: monkeypatch `huggingface_hub`'s network entry points (e.g. `huggingface_hub.file_download.hf_hub_download`) to raise immediately if called, then run `LoadModel.execute()` and assert it completes without ever triggering the guard — this is the direct regression test for the entire P904-A9–A14 HF-network-access fix and its P904-B1–B3 follow-up rework, the defect this sub-effort exists to close

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/test_real_loaders.py -v -m realcpu
# -> exits 0, all real-mode loader tests pass against synthetic tiny checkpoints,
#    including test_loadmodel_no_network_access
```

#### P904-Z4: worker/tests/test_real_encoder_sampler.py: real CPU node tests for ClipTextEncode/EmptyLatent/Sampler/VaeDecode

**Goal:** Prove `ClipTextEncode`, `EmptyLatent`, `Sampler`, and `VaeDecode`'s
real-mode paths function correctly end-to-end at the per-node level,
directly exercising every defect fixed in P904-A4 through P904-A6b, plus
first-time real-mode coverage for D20's `VaeDecode` (committed after this
task was originally scoped, now folded in).

**Files to create or modify:**
- `worker/tests/test_real_encoder_sampler.py` — new file, all tests `@pytest.mark.realcpu`

**Key implementation notes:**
- Chain real outputs from P904-Z3's loaders directly as inputs here — no mocking between nodes, only the model/vae/clip *files* are synthetic
- `EmptyLatent.execute(width=128, height=128, ...)` is the direct regression test for P904-A4 (`self.ctx.device`, not bare `ctx.device`) — use 128×128 specifically, per the explicit minimal-load-bearing sizing agreed for this suite
- `Sampler.execute(..., steps=1, cfg=1.0, seed=0)` with `steps=1` is deliberate: this suite verifies the code *functions* (real `ZImagePipeline.__call__` runs, real `cancel_flag.is_set()` doesn't raise, real `loader_fn` resolves `tokenizer`/`text_encoder` from the new `clip` parameter), not that the output is a meaningful image — do not increase `steps` to "look more real," it only adds CPU runtime with no additional verification value
- This is the direct regression test for P904-A5 (`threading.Event`) and P904-A6/A6b (`clip` parameter, no `vae` parameter) together — a failure here pinpoints whether the wiring fix or the underlying pipeline call itself is the problem
- `VaeDecode.execute(vae=<real tiny_vae_raw fixture from P904-Z1b>, latent=<a real tensor whose shape matches the tiny VAE's expected input>)` must return a real `PIL.Image.Image` — D20's own production code was verified correct against `ZImagePipeline`'s own decode formula during this phase's D20 audit, so this is coverage, not a bugfix target; if it fails, the bug is most likely in this test's fixture shapes, not in `decode.py` itself, and should be debugged with that prior in mind
- If P904-A1 chose to relocate `test_vaedeode_real_path_returns_pil_image` into this file rather than guard it in place, incorporate it here rather than authoring a duplicate — check what P904-A1 actually did before writing this task's `VaeDecode` test from scratch

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/test_real_encoder_sampler.py -v -m realcpu
# -> exits 0, all four node tests (including VaeDecode) pass against synthetic tiny checkpoints
```

#### P904-Z5: worker/tests/test_real_chain.py: full real-mode node chain on tiny CPU weights, through VaeDecode

**Goal:** Prove the full real-mode node chain (`LoadModel`→`LoadVae`→
`LoadClip`→`ClipTextEncode`→`EmptyLatent`→`Sampler`→`VaeDecode`) functions
end-to-end when wired together exactly as a real job graph would, catching
any cross-node wiring gap that per-node isolation in B3/B4 could miss —
now extended through `VaeDecode` since D20 has landed.

**Files to create or modify:**
- `worker/tests/test_real_chain.py` — new file, one test, `@pytest.mark.realcpu`

**Key implementation notes:**
- Construct and call each node directly in sequence (do not call `worker/executor.py`'s `run_graph()` itself) — this keeps the test's pass/fail signal specific to the nodes' own real-mode logic, independent of `executor.py`'s separate correctness
- 128×128, `steps=1`, single batch — same minimal-load-bearing sizing as B4, for the same reason (verify function, not output quality)
- The chain now runs all the way through `VaeDecode` and asserts the final output is a real `PIL.Image.Image`, not a `MockImage` — this is the project's first test exercising the complete real-mode node graph from model load through to a decoded image
- This is also a device-consistency check by construction: if `LoadVae` (P904-A8) places the VAE on `ctx.device` but `Sampler`'s output latent ends up on a different device for any reason, `vae.decode()` will raise a device-mismatch error here even if B3/B4's per-node tests each individually passed in isolation — treat such a failure as evidence of a cross-node device-placement gap, not a flaky test, and report it rather than loosening assertions to make it pass

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/test_real_chain.py -v -m realcpu
# -> exits 0, full chain completes without exception, final output is a real PIL.Image.Image
```

#### P904-Z6: worker/tests/test_arch_zit.py: fix CI failures — 5 B1/B2 tests unmarked, running torch-less in CI

**Goal:** Fix a live CI failure confirmed via the actual GitHub Actions log:
5 tests added during P904-B1/B2's own implementation do a bare `import torch`
with no `realcpu` marker, in a file CI collects by default in a venv with no
`torch` installed. `ModuleNotFoundError` on every run since P904-Z2 landed —
the marker was registered, but these five tests were never tagged with it.
Sequenced last in this phase, after the rest of Group Z, since it is a
standalone fix to an existing file rather than something the Z1–Z5 chain's
own fixtures or infrastructure depend on.

**Files to create or modify:**
- `worker/tests/test_arch_zit.py` — mark one test `@pytest.mark.realcpu`; rewrite four others to drop their `torch` dependency entirely

**Key implementation notes:**
- This is a **per-test decision, not a blanket fix** — the five tests do not all have the same underlying need for `torch`, confirmed by reading the production functions each one exercises:
  - `test_remap_key_transformations` genuinely needs `torch`: it tests `_remap_z_image_keys()`, which calls `torch.chunk(fused_qkv, 3, dim=0)` internally to defuse the QKV projection — there is no way to exercise this function without a real `torch.Tensor`. Add `@pytest.mark.realcpu` to this test. This will be the **first** `realcpu`-marked test in the codebase; no existing pattern to follow beyond the marker registration itself from P904-Z2
  - `test_infer_vae_config_from_checkpoint`, `test_infer_vae_config_missing_key_raises`, `test_remap_ldm_vae_keys`, `test_remap_ldm_vae_keys_first_stage_model_prefix` test `_infer_vae_config_from_checkpoint()` and `_remap_ldm_vae_keys()` — confirmed by reading both functions, neither ever calls anything `torch`-specific; both only do `.shape[N]` integer indexing and plain string/dict key manipulation. These four tests' own docstrings already say `torch`/`diffusers` import is "not strictly required" — that claim is correct for these two functions (it is *not* correct for `_remap_z_image_keys`, despite a near-identical docstring sentence appearing on `test_remap_key_transformations` too — don't assume the docstrings are reliable signals on their own, verify against the actual function each test calls)
- For the four torch-free rewrites: replace every `torch.ones(...)` checkpoint-fixture entry with a tiny local stand-in exposing only `.shape` as a plain tuple (e.g. a small class or even a bare tuple subclass) — the production functions never read anything else off these objects. Remove the `import torch` line from each of these four test bodies entirely
- Do not change the synthetic shape values used in any test's fixture data (e.g. `decoder.conv_out.weight` using `(64, 64, 3, 3)` rather than the real checkpoint's actual `(3, 128, 3, 3)`) — this is deliberate, exercising the function's arithmetic independently of real-world values, not a bug to "correct" to match the real scan

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v -m "not realcpu"
# -> exits 0, the 4 rewritten tests pass with no torch installed in this venv
grep -n "@pytest.mark.realcpu" worker/tests/test_arch_zit.py
# -> exactly one match, on test_remap_key_transformations
grep -n "^    import torch" worker/tests/test_arch_zit.py
# -> only test_remap_key_transformations still has this line among the five tests in question
```

#### P904-Z7: create cpu-runner-reqs.txt for CI use; wire mock-then-realcpu sequencing into ci.yml's worker job

**Goal:** Give CI its own CPU-only `torch` requirements file, separate from
`cpu-linux-agent.txt` (which remains exclusively for The Forge agents on
WSL2, unchanged), and close the last gap in actually running Group Z's
`realcpu` suite anywhere: `ci.yml`'s `worker` job has never installed
either file or run any `realcpu`-marked test, on either OS, since P904-Z2
only excluded them.

**Files to create or modify:**
- `worker/requirements/cpu-runner-reqs.txt` — new file, for CI's exclusive use
- `.github/workflows/ci.yml` — restructure the `worker` job's test step into the mock-then-realcpu sequence described below

**Key implementation notes:**
- `cpu-linux-agent.txt` is **not renamed, not touched, not referenced as deprecated** — it stays exactly as it is, for exactly its existing purpose (The Forge agents at ACT time on WSL2). No document that currently references it needs updating
- `cpu-runner-reqs.txt` is a deliberate duplicate, not a symlink or an alias: same content today (`--index-url https://download.pytorch.org/whl/cpu`, `torch`, `torchaudio`, `torchvision`), but a genuinely separate file so CI's pins can diverge from the agent's later without the two consumers fighting over one file's contents
- The PyTorch CPU index itself serves both Windows and Linux wheels — `pip` resolves the correct one automatically — so `cpu-runner-reqs.txt` needs no platform-specific variant and the new CI steps need no `if: runner.os == 'Linux'` gate
- Restructure `ci.yml`'s `worker` job from a single "Run worker tests" step into three: (1) "Run worker tests (mock mode)" — unchanged from today, `base.txt` only, `ANVILML_WORKER_MOCK=1`, `-m "not realcpu"`, runs first so the mock suite is provably torch-free at the moment it executes, exactly as it always has been; (2) "Install cpu-runner-reqs.txt" — `pip install -r worker/requirements/cpu-runner-reqs.txt`, runs only after step 1 has already passed; (3) "Run worker tests (realcpu)" — `ANVILML_WORKER_MOCK=0`, `-m realcpu`. All three steps run on **both** `ubuntu-latest` and `windows-latest` in the existing matrix
- As of this task, zero or very few tests carry `@pytest.mark.realcpu` (only `test_remap_key_transformations` from P904-Z6) — step 3 reporting "0 selected" or a small count is expected and not a failure; the bulk of `realcpu` coverage lands with P904-Z3–Z5

**Acceptance criterion:**
```bash
test -f worker/requirements/cpu-runner-reqs.txt && grep -q "^torch" worker/requirements/cpu-runner-reqs.txt
test -f worker/requirements/cpu-linux-agent.txt
# -> both files present; the original is untouched, the new one exists alongside it
diff worker/requirements/cpu-linux-agent.txt worker/requirements/cpu-runner-reqs.txt
# -> no diff today (deliberately identical at creation time; expected to be edited
#    independently of each other in the future, not kept in sync mechanically)
grep -n "runner.os == 'Linux'" .github/workflows/ci.yml | grep -v "Provision worker venv"
# -> zero matches outside the pre-existing venv-provisioning steps (no new OS gate added)
```


## Files Affected

| Action | Path | Description |
|--------|------|--------------|
| MODIFY | `worker/tests/test_nodes_decode.py` | Guard or relocate the unconditional `import torch` in D20's real-path test (A1) |
| MODIFY | `worker/nodes/arch/clip/qwen3.py` | Fix tokenizer path depth (A2); add device param, `.to(device)`, pass device to RealClip (A7) |
| MODIFY | `worker/nodes/arch/clip/clip_l.py` | Fix tokenizer path depth (A2); add device param, `.to(device)`, pass device to RealClip (A7) |
| MODIFY | `worker/nodes/arch/clip/t5.py` | Add device param, `.to(device)`, pass device to RealClip (A7) — path depth already correct, not touched |
| MODIFY | `worker/nodes/loader.py` | Add missing `import torch` in `LoadClip.execute()` (A3); pass `device=self.ctx.device` into `module.load()` (A7); add device param + `.to(device)` to loader functions (A8); delete deprecated `_load_from_hf_directory`/`_load_clip_from_hf_directory` (A9); rename and rewrite `_load_model_from_hf_directory`/`LoadVae`'s inline loader/`LoadClip`'s inline dispatch into `_load_model_from_safetensors`/`_load_vae_from_safetensors`/`_load_clip_from_safetensors` thin arch-dispatch wrappers (A12); fix `LoadVae`'s missing `device` argument and `LoadClip`'s stale docstring (A14); add key-prefix-based arch detection alongside the existing metadata-based detection (B3) |
| MODIFY | `worker/nodes/sampler.py` | Fix `ctx` → `self.ctx` in `EmptyLatent` (A4); add `clip` input slot and pass-through in `Sampler` (A6) |
| MODIFY | `worker/worker_main.py` | Replace `list[bool]` cancel flag with `threading.Event()` (A5) |
| MODIFY | `worker/nodes/arch/diffusion/zit.py` | Add `clip` parameter, fix loader_fn's tokenizer/text_encoder source (A6); remove vestigial `vae` parameter (A6b); add `load_transformer()` (A10) and `load_vae()` (A11) — both offline, no HF network access; replace both functions' reliance on private `diffusers` internals with direct shape-inferred config + manual key remap (B1, B2) |
| MODIFY | `worker/nodes/arch/diffusion/__init__.py` | Add `get_module_by_name(arch: str)` lookup alongside the existing `get_module(model_obj)` (A13) |
| MODIFY | `worker/tests/test_arch_zit.py` | Update two existing tests to pass `clip`/drop `vae=` keyword args (A6, A6b); mark `test_remap_key_transformations` `@pytest.mark.realcpu` and rewrite four other B1/B2 tests to drop their unneeded `torch` dependency, fixing a live CI failure (Z6) |
| CREATE | `worker/tests/real_fixtures.py` | Tiny CLIP checkpoint fixtures (qwen3/clip_l/t5), native `state_dict()` format (Z1); tiny ZiT transformer/VAE fixtures in raw, pre-remap checkpoint format — required to actually exercise Group B's shape-inference remap path (Z1b) |
| MODIFY | `worker/tests/pytest.ini` | Register `realcpu` marker (Z2) |
| MODIFY | `.github/workflows/ci.yml` | Add `-m "not realcpu"` to the worker test job's pytest invocation (Z2) |
| CREATE | `worker/tests/test_real_loaders.py` | Real CPU tests for LoadModel/LoadVae/LoadClip (Z3) |
| CREATE | `worker/tests/test_real_encoder_sampler.py` | Real CPU tests for ClipTextEncode/EmptyLatent/Sampler/VaeDecode (Z4) |
| CREATE | `worker/tests/test_real_chain.py` | Full real-mode node chain test through to decoded image, 128×128, 1 step (Z5) |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-------------------|----------------|--------|-------------------|---------------------|
| `test_real_loaders.py` | `test_loadmodel_real_tiny_checkpoint` | LoadModel's real path loads a real (tiny, raw-format) checkpoint without the A2/A3-class defects or the A9–A13 HF-network-access defect | `ANVILML_WORKER_MOCK=0`, `realcpu` marker, `tiny_zit_transformer_raw` fixture (P904-Z1b) | `LoadModel.execute(model_id=<fixture path>)` | `RealModel.in_channels == 4` (the tiny config's value); zero `huggingface_hub` network calls | `ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/test_real_loaders.py -k loadmodel -v -m realcpu` |
| `test_real_loaders.py` | `test_loadclip_all_three_types_correct_device` | LoadClip works for qwen3/clip_l/t5 and places the text encoder on `ctx.device` | `ANVILML_WORKER_MOCK=0`, `realcpu` marker, all three tiny clip fixtures (P904-Z1) | `LoadClip.execute(model_id=<fixture>, clip_type=<type>)` for each type | `RealClip.text_encoder`'s device matches `ctx.device` for all three | `ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/test_real_loaders.py -k loadclip -v -m realcpu` |
| `test_real_encoder_sampler.py` | `test_emptylatent_real_self_ctx` | EmptyLatent's real path doesn't reference unbound `ctx` (A4 regression) | `ANVILML_WORKER_MOCK=0`, `realcpu` marker, `tiny_zit_transformer_raw` fixture (P904-Z1b) | `EmptyLatent.execute(width=128, height=128, model=<real RealModel>)` | Real `torch.Tensor` matching `compute_latent_shape()` | `ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/test_real_encoder_sampler.py -k emptylatent -v -m realcpu` |
| `test_real_encoder_sampler.py` | `test_sampler_real_one_step` | Sampler's real path runs a real `ZImagePipeline.__call__` without A5/A6/A6b's defects | `ANVILML_WORKER_MOCK=0`, `realcpu` marker, full B1/B1b fixture set | `Sampler.execute(..., steps=1, cfg=1.0, seed=0)` | Unchanged-shape latent tensor, non-negative resolved seed | `ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/test_real_encoder_sampler.py -k sampler -v -m realcpu` |
| `test_real_encoder_sampler.py` | `test_vaedecode_real_tiny_vae` | VaeDecode's real path (D20) decodes a real latent to a real image | `ANVILML_WORKER_MOCK=0`, `realcpu` marker, `tiny_vae_raw` fixture (P904-Z1b) | `VaeDecode.execute(vae=<real tiny_vae_raw>, latent=<matching-shape tensor>)` | Real `PIL.Image.Image`, not `MockImage` | `ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/test_real_encoder_sampler.py -k vaedecode -v -m realcpu` |
| `test_real_chain.py` | `test_full_chain_tiny_weights_128px` | The full seven-node real-mode chain functions end-to-end, model load through decoded image | `ANVILML_WORKER_MOCK=0`, `realcpu` marker, full Z1/Z1b fixture set | LoadModel→LoadVae→LoadClip→ClipTextEncode→EmptyLatent→Sampler→VaeDecode, 128×128, 1 step | No exception; final output is a real `PIL.Image.Image` | `ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/test_real_chain.py -v -m realcpu` |
| `test_real_loaders.py` | `test_loadmodel_no_network_access` | LoadModel's real path makes zero `huggingface_hub` calls (A9–A13 regression) | `ANVILML_WORKER_MOCK=0`, `realcpu` marker, `tiny_zit_transformer_raw` fixture, `huggingface_hub` network entry points monkeypatched to raise | `LoadModel.execute(model_id=<fixture path>)` | Completes without raising; the monkeypatched network guard is never triggered | `ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/test_real_loaders.py -k no_network -v -m realcpu` |

## CI Impact

`.github/workflows/ci.yml`'s worker test job gains `-m "not realcpu"` on its
mock-mode pytest invocation (P904-Z2) — at that point, and through P904-Z6,
Group Z's `realcpu`-marked tests were excluded from CI entirely, in
addition to being naturally uncollectable in CI's venv before this since
`torch` was not installed there (`base.txt` deliberately excludes it).
`rust-linux`/`rust-windows`/`config-drift`/`openapi-drift` are unaffected —
no Rust-side changes in this phase.

P904-Z7 ends that exclusion. The `worker` job's single test step becomes
three: mock-mode tests run first (unchanged, still `torch`-free, still the
same enforcement the venv boundary always provided); `cpu-runner-reqs.txt`
is installed into that same job's venv only after the mock-mode step has
already passed; then the `realcpu` suite runs for real, in CI, on both
`ubuntu-latest` and `windows-latest`. `cpu-runner-reqs.txt` is a deliberate
duplicate of `worker/requirements/cpu-linux-agent.txt` — kept as a separate
file rather than the same one, since the agent's file (manually installed,
ACT-time only, WSL2-only in practice) and CI's file (provisioned fresh on
a throwaway runner every run) are different consumers that may need to
diverge later. From P904-Z7 onward, the `realcpu` suite is no longer a
manual-only, never-automated check — it is part of the same CI run as the
mock-mode suite, sequenced after it specifically so the mock-mode
guarantee is never weakened by `torch`'s later presence in the same job.

Despite Z2 landing the marker correctly, CI broke anyway between Z2 and Z6:
five tests added during P904-B1/B2's own implementation landed directly in
`test_arch_zit.py` (the mock-mode file) with a bare `import torch` and no
`realcpu` tag, confirmed by a live CI log showing `ModuleNotFoundError: No
module named 'torch'` on every one of them. P904-Z6 fixes this — one test
genuinely needs `torch` and is retroactively marked `realcpu`; the other
four needed no `torch` dependency at all and are rewritten without it.
This is the first concrete evidence in this phase that registering a
marker is necessary but not sufficient — every PR touching a mock-mode
test file still needs a human or agent to apply the marker correctly at
the point a new real-mode test is added. P904-Z7's ordering (mock tests
before `torch` is even installed) is a stronger, structural version of the
same guarantee — it does not depend on every future commit remembering to
do anything correctly, since `torch` genuinely is not present yet at that
point in the job.

## Platform Considerations

`threading.Event()` (P904-A5) is cross-platform standard library and
behaves identically on Linux and Windows — no platform-specific handling
needed. Device placement (`P904-A7`, `P904-A8`) uses the same `ctx.device`
string convention already proven cross-platform-safe by `EmptyLatent`'s
existing (correct, once A3 lands) `torch.randn(..., device=self.ctx.device)`
call.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|-------------|
| `ANVILML_DESIGN.md §1550`, `§10.4`, and `§10.4a` have all been verified/pre-corrected ahead of this phase's execution (see Interfaces and Contracts above) — the residual risk is that the human-authored doc and this task list's understanding of "current" drift apart again before P904 actually executes (e.g. if `ANVILML_DESIGN.md` is edited again for unrelated reasons between this phase's authoring and its execution) | Low | Medium | Each affected task (`P904-A5`, `A6`, `A6b`, `A7`) explicitly states it must not edit the design doc itself; if an ACT agent finds the doc no longer matches what this task list describes, that is a `Status=BLOCKED` condition (per the agent operating rules — design-doc drift is outside any task's authority to self-resolve), not a license to edit the doc or silently proceed |
| Fixing P904-A7/A8's device placement could change memory behaviour (model now actually resides on GPU) in ways not previously exercised by any test, surfacing a downstream OOM or shape issue that was latent while everything silently ran on CPU | Medium | Medium | `PipelineCache`'s existing OOM-retry-with-eviction logic (P18-C1) is designed for exactly this; no new mitigation needed beyond confirming P18-C1's tests still pass post-fix |
| P904-A6's `clip` parameter addition to `Sampler.INPUT_SLOTS` is a public node contract change — any already-authored example workflow JSON (`docs/example_workflows/zit_fp8.json`) referencing `Sampler` without a `clip` input will need updating | High | Low | `SlotSpec` does not currently support a way to distinguish "newly required" from "always required" — confirm with the ACT agent whether `clip` should be `optional=True` with a deprecation path, or a hard-required breaking change; check `docs/example_workflows/zit_fp8.json` and update it in the same task if it references Sampler |
| Group Z's synthetic tiny-config checkpoints (P904-Z1/Z1b) may not faithfully reproduce every shape-dependent code path a real-size checkpoint would exercise — e.g. attention head dimension edge cases that only manifest at the real architecture's actual `n_heads`/`dim` ratio | Low | Low | Group Z's stated purpose is proving the code *functions* (no crash, correct shape propagation, correct object wiring), not full numerical/architectural fidelity |
| A CPU-only `torch` install (P904-Z2's `cpu-linux-agent.txt`) running real `diffusers`/`transformers` inference, even at `steps=1` and 128×128, could still be slow enough to make routine ACT-time runs impractical | Medium | Low | Tiny config (2-layer transformer, `dim=64`) keeps per-test runtime in the low seconds on CPU; if ACT-time runtime proves impractical in practice, the fixture configs in Z1/Z1b can be shrunk further without losing coverage of the code paths being verified |
| Group B's shape-inference derivations (P904-B1/B2) were checked against both live `diffusers` 0.38.0 source and a full, untruncated key scan of the real ZiT transformer and VAE checkpoints during this phase's drafting — every numeric config value (`dim`, `head_dim`, `n_heads`, `n_layers`, `n_refiner_layers`, `cap_feat_dim`, `in_channels`, `latent_channels`, `block_out_channels`, `layers_per_block`) is now confirmed against real data, with two errors caught and corrected before authoring (`in_channels` and `layers_per_block`, both off by a derivable factor rather than a guess). The residual risk is `scaling_factor`, which cannot be confirmed from tensor shapes at all and remains a best-effort default | Low | Medium | `scaling_factor` errors produce a visibly wrong brightness/contrast in decoded output, not a crash — this would surface during the manual real-hardware verification pass described in this phase's Acceptance Criteria, not silently; if it proves wrong, the fix is a one-line constant change with no structural impact |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v -m "not realcpu"` exits 0 (no regressions across the full mock-mode suite, with the one genuinely-real-mode test in `test_arch_zit.py` explicitly excluded — this invocation is also CI's actual command as of P904-Z2)
- [ ] `grep -n "device=ctx.device" worker/nodes/sampler.py` returns no hits
- [ ] `grep -n "_cancel_flag\[0\]" worker/worker_main.py` returns no hits
- [ ] `python3 -c "import inspect,os; os.environ['ANVILML_WORKER_MOCK']='1'; from worker.nodes.arch.diffusion.zit import sample; s=inspect.signature(sample); assert 'clip' in s.parameters and 'vae' not in s.parameters"` exits 0
- [ ] `docs/example_workflows/zit_fp8.json` (if it references `Sampler`) updated to include a `clip` input, or confirmed not to need updating
- [ ] `grep -n 'not realcpu' .github/workflows/ci.yml` confirms Group Z is excluded from the default CI gate
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_decode.py -v` exits 0 in a venv built from `base.txt` alone, with no `torch` installed at all (the actual regression check for A1 — confirms CI is no longer broken by D20's committed test)
- [ ] `grep -n "_load_from_hf_directory\|_load_clip_from_hf_directory" worker/nodes/loader.py` returns no hits (A9 — deprecated remnants deleted)
- [ ] `grep -n "_load_model_from_safetensors\|_load_vae_from_safetensors\|_load_clip_from_safetensors" worker/nodes/loader.py` shows all three present (A12)
- [ ] `python3 -c "import os; os.environ['ANVILML_WORKER_MOCK']='1'; from worker.nodes.arch.diffusion import get_module_by_name; assert get_module_by_name('zit') is not None"` exits 0 (A13)
- [ ] `grep -n '_load_vae_from_safetensors(model_id, "zit", self.ctx.device)' worker/nodes/loader.py` confirms A14's argument fix
- [ ] `grep -n "convert_z_image_transformer_checkpoint_to_diffusers\|convert_ldm_vae_checkpoint" worker/nodes/arch/diffusion/zit.py` returns no hits (B1, B2 — private `diffusers` internals fully removed)
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v -m "not realcpu"` exits 0 in a venv with no `torch` installed (the actual regression check for Z6 — confirms CI is no longer broken by the five unmarked B1/B2 tests)
- [ ] `grep -n "Install cpu-runner-reqs.txt" .github/workflows/ci.yml` confirms Z7's new step exists in the `worker` job, after the mock-mode test step and before the `realcpu` test step
- [ ] `ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/ -v -m realcpu` exits 0 when run manually/at ACT time with `cpu-linux-agent.txt` installed — this remains how the OpenCode agent verifies its own work locally; from P904-Z7 onward, CI runs the equivalent check itself in `ci.yml`'s `worker` job using `cpu-runner-reqs.txt`, so this manual run and the CI run should agree

```bash
# Runnable Proof (manual): once P904 lands, re-confirm end-to-end against
# real ZiT FP8 weights that every fix in this phase holds together. There is
# no committed harness for this in this repository -- the steps below describe
# what a manual verification pass should check, for whoever performs it:
#   LoadModel/LoadVae/LoadClip   -> all succeed, no NameError/OSError/TypeError,
#                                   and CRITICALLY: no HF Hub network activity at
#                                   all (no config.json fetch, no huggingface_hub
#                                   warnings about unauthenticated requests) --
#                                   this is the actual regression check for the
#                                   A9-A14 offline-loading fix and the B1-B3
#                                   shape-inference rework that removes the
#                                   remaining diffusers-internals dependency
#   ClipTextEncode               -> hidden_dim matches the real text encoder's config
#   EmptyLatent                  -> shape matches compute_latent_shape()
#   Sampler                      -> denoised latent shape == input latent shape
#   VaeDecode                    -> decoded image size matches the original
#                                   EmptyLatent request
#
# Group Z's own suite is the committed equivalent of the network-activity check
# above, including test_loadmodel_no_network_access (Z3). Through P904-Z6 this
# ran only manually/at ACT time, never in CI. From P904-Z7 onward, ci.yml's
# worker job runs this same command itself, on both ubuntu-latest and
# windows-latest, after the mock-mode suite has already passed and
# cpu-runner-reqs.txt has been installed into that job's venv:
ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/ -v -m realcpu
# -> exits 0 against synthetic tiny checkpoints; run manually or by the
#    OpenCode agent at ACT time on a CPU-capable box, AND (from Z7 onward) by CI
```

## Known Constraints and Gotchas

- P18-D18a's task context underspecified the source of `tokenizer`/
  `text_encoder` ("from model/conditioning") — P904-A6 is not correcting an
  implementation mistake so much as resolving an ambiguity the original
  task left open. Future task authoring should name the exact object a
  value is read from, not a disjunction of plausible objects.
- The device-placement gap (P904-A7, P904-A8) was not caught by any prior
  phase's acceptance criteria because no existing test asserts on tensor or
  model `.device` — every mock-mode test uses sentinel objects with no
  real device concept. `test_real_chain.py` (P904-Z5) is the first test
  that would actually catch a device mismatch end-to-end, since `VaeDecode`
  calling `vae.decode(latents, ...)` with the VAE and latent on different
  devices raises immediately — this is a useful side-effect of extending
  the chain test through D20, not something that had to be separately
  engineered.
- The `vae=None`-is-tolerated finding (P904-A6b) is specific to `diffusers`
  0.38.0's `ZImagePipeline` implementation; if the pinned `diffusers`
  version changes, re-verify `register_modules`' `None`-tolerance and
  `__call__`'s `self.vae` dereference point before assuming this still holds.
- Group Z's real-mode suite requires a separate Python environment with
  `worker/requirements/cpu-linux-agent.txt` installed (`torch` via PyTorch's
  dedicated CPU index, not plain PyPI) layered on top of the existing
  `worker/.venv` — it is not the same venv CI provisions via
  `install_worker_deps.sh`, and the two should not be conflated. A
  developer or the OpenCode agent must explicitly create or extend a venv
  with both `base.txt` and `cpu-linux-agent.txt` installed before Group Z's
  tests can run at all; this is by design, not an oversight to streamline
  away.
- P904-Z7's CI-side venv is a different situation from the bullet above,
  not a contradiction of it: CI's runner is thrown away after every job,
  so there is no persistent venv to conflate with anything. `base.txt` and
  `cpu-runner-reqs.txt` are installed into the *same* job's venv, in
  sequence — `cpu-runner-reqs.txt` only after the mock-mode tests have
  already run and passed against a `torch`-free environment. This is
  deliberately a different file from `cpu-linux-agent.txt` even though
  both currently have identical contents — they serve different consumers
  (a long-lived agent environment vs. a one-shot CI job) and are free to
  diverge later without one change forcing an update to the other.
- D20's `VaeDecode` real path was found to be correctly implemented on
  first audit — the only defect traced to it was in its own committed
  test file (P904-A1), not in `decode.py` itself. This is the first node
  in this phase's audit history where the production code needed no fix;
  worth noting for calibration on how much scrutiny future similarly-sized
  groups warrant before assuming a defect must exist somewhere.
- The P904-A9–A14 HF-network-access defect was found by neither static
  code reading nor mock-mode tests — both had already passed for this
  code path. It surfaced only by actually running the loading code against
  real inputs and observing a side effect (an HF `config.json` download)
  that no functional assertion in this codebase would have caught on its
  own. This is the core argument for Group Z's existence:
  `test_loadmodel_no_network_access` (P904-Z3) makes that same
  observation — network activity during a supposedly-offline operation —
  into a committed, repeatable assertion rather than something that can
  only be noticed by a human watching console output during an ad hoc
  manual run.
- Group B's shape-inference rework was scoped against a draft derivation
  proposal supplied ahead of this group's authoring. Checking that
  proposal against live `diffusers` 0.38.0 source, and later against a
  full, untruncated key scan of the real ZiT transformer and VAE
  checkpoints, surfaced two real errors before any task touched the
  code: the proposal's claim that `final_layer.linear.weight.shape[0]`
  equals `in_channels` directly is wrong (the real relationship requires
  dividing out `patch_size**2 * f_patch_size` first — actual value 16,
  not the proposal's 64), and its claim that `layers_per_block` can be
  read directly from the count of resnet blocks per decoder stage is
  also wrong (`diffusers`' `Decoder` builds up-blocks with
  `layers_per_block + 1` resnets — actual value 2, not the raw observed
  3). Both are recorded here, not just in P904-B1/B2's task notes, as a
  general caution: a confident-sounding external derivation document is
  a starting hypothesis to verify, not a substitute for reading the
  actual model source and real checkpoint data before writing code that
  depends on it. The first scan provided for this verification was
  itself incomplete (`list(f.keys())[:30]`, missing most of the
  transformer's layer stacks) — a second, untruncated, sorted scan was
  needed before `n_layers`/`n_refiner_layers`/the VAE's block structure
  could be confirmed with confidence; an incomplete scan can look
  complete enough to trust if you don't already know what's missing.
- Registering the `realcpu` marker (P904-Z2) did not, by itself, prevent
  CI from breaking — confirmed by a live CI log showing five tests added
  during P904-B1/B2's own implementation landed directly in
  `test_arch_zit.py` with a bare `import torch` and no marker applied at
  all, fixed in P904-Z6. A marker is only as good as every future commit
  remembering to apply it; this is a process gap, not a tooling gap, and
  no amount of CI configuration alone closes it. It's also worth noting
  that not every test needing `torch.ones(...)` in its fixture data
  actually needs `torch` for the function it's testing — two of the five
  failing tests had near-identical "torch not strictly required" docstring
  language, but only one of the underlying production functions
  (`_remap_z_image_keys`, via `torch.chunk`) was actually `torch`-dependent;
  the others used `torch.ones()` for fixture convenience on functions that
  only ever read `.shape[N]`. Don't assume a docstring's claim about its
  own test is reliable without checking the production function itself.

## docs/RUNNABLE_PROOF.md update

Phase 904 has no new HTTP-, WebSocket-, or CLI-observable surface of its
own — it is a pure correctness retrofit to code paths whose only external
observable is "did a real ZiT generation job succeed," which is already
Phase 18's own Runnable Proof (`docs/RUNNABLE_PROOF.md`'s Phase 018 entry).
Add the following entry rather than a new standalone proof:

```markdown
## Phase 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects)

**Runnable Proof:** not applicable — pure correctness retrofit to code paths
already covered by Phase 18's own Runnable Proof; no new HTTP-, WebSocket-,
or CLI-observable surface is introduced. Re-run Phase 18's Runnable Proof
(`docs/PROOF_phase018.md`) after this phase lands to confirm the underlying
real ZiT FP8 workflow still produces a real PNG artifact, now via fixed
code paths rather than by coincidence of never having reached them.
```