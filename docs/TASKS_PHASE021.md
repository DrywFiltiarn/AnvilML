# Tasks: Phase 21 — ZiT Diffusion Arch Module: Sampling & Latent Shape

**Phase:** 21
**Name:** ZiT Diffusion Arch Module: Sampling & Latent Shape
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 9, 10, 19, 20

---

## Overview

This phase completes `zit.py` with its remaining two fixed-contract methods —
`compute_latent_shape()` and `sample()` — and creates the generic `Sampler` node,
giving it a real branch from this same phase rather than the deferred-raise pattern
the loader nodes used in Phase 19. This closes out ZiT as a fully functional
diffusion arch module for everything except VAE decoding, which remains out of
scope (a separate arch family per `ANVILML_DESIGN.md §11.4`, covered in a later
phase).

This phase exists right after Phase 20's loading work because `sample()` and
`compute_latent_shape()` are the two remaining fixed-name methods
`ANVILML_DESIGN.md §10.4`'s table requires of every diffusion arch module — `load()`
alone (Phase 20) only gets a model into memory; it can't yet produce a denoised
latent or tell `EmptyLatent` what shape to allocate. Unlike the loader nodes in
Phase 19, `Sampler`'s real branch is built correctly **from the start** in this
phase, since `zit.py`'s `sample()` already exists by the time `Sampler` needs it —
there's no analogous "groundwork before concrete architecture" gap to bridge here.

At the start of this phase, `zit.py` only has `load()`; `Sampler` doesn't exist at
all. At the end: `compute_latent_shape()` implements ZiT's actual patch-packing
formula (not a generic downscale assumption); `sample()` assembles and caches a
runnable pipeline from `load()`'s already-cached components, runs denoising, and
correctly resolves a `-1` seed; and the generic `Sampler` node dispatches to it with
both `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` markers pointing at genuinely
passing tests from the same task.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Latent shape | P21-A1 | `compute_latent_shape()`'s architecture-specific formula |
| B | Sampling | P21-B1 … P21-B2 | Pipeline assembly/caching, then the denoising loop + seed resolution |
| C | Sampler node | P21-C1 … P21-C2 | Mock branch, then the real branch dispatching to `zit.py` |
| D | Proof | P21-D1 | The phase's Runnable Proof |

---

## Prerequisites

`zit.py`'s `load()` must be complete per Phase 20 (P20-C3), including
`_infer_hyperparams()` (P20-B1) and `can_handle()`/dispatch registration (P20-B2).
`pipeline_cache.py` must exist per Phase 19 (P19-B1).

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §10.3` EmptyLatent row | P21-A1 | `compute_latent_shape()`'s exact name and the "shape formula, not just a scale factor" requirement |
| `ANVILML_DESIGN.md §10.4` | P21-A1, P21-B1, P21-B2, P21-C2 | Fixed method names — `compute_latent_shape`, `sample` — never a family-prefixed or "clearer" variant |
| `ANVILML_DESIGN.md §11.6` | P21-B1 | Pipeline assembly happens inside `sample()`, from already-cached components — never a hub-aware pipeline loader |
| `ANVILML_DESIGN.md` Sampler row, seed note | P21-B2 | Seed `-1` resolves to a random integer before denoising; the resolved seed is what's returned |
| `ANVILML_DESIGN.md §10.6` | P21-C2 | Marker pair requirement — both must point at genuinely passing tests |

---

## Task Descriptions

### Group A — Latent shape

#### P21-A1: worker/nodes/arch/diffusion/zit.py: compute_latent_shape() formula

**Goal:** Implement ZiT's actual latent-shape formula, derived from the same
hyperparameters shape inference already extracts — not a generic assumption
borrowed from a different architecture.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/zit.py` — adds `compute_latent_shape()`.

**Key implementation notes:**
- The method name is fixed per `ANVILML_DESIGN.md §10.4`'s table —
  `compute_latent_shape` exactly, never `latent_shape()` or `get_latent_dims()`.
- Non-multiple-of-patch-size dimensions are handled per a documented rounding
  rule, with the rule explained in a code comment — never silently truncated
  without explanation.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_zit.py -v
# -> >=25 tests total in the file, exits 0
```

---

### Group B — Sampling

#### P21-B1: worker/nodes/arch/diffusion/zit.py: sample() pipeline assembly + caching

**Goal:** Implement the pipeline-assembly half of `sample()` — turning
`load()`'s already-cached raw component into a runnable pipeline object, cached
separately under its own key.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/zit.py` — adds pipeline assembly to `sample()`.

**Key implementation notes:**
- The pipeline is assembled **directly from the already-loaded `nn.Module`** — not
  via `diffusers.DiffusionPipeline.from_pretrained()`, which would violate
  `ANVILML_DESIGN.md §11.2`'s library boundary.
- Cached under `f"{model_id}:pipeline"`, distinct from the raw component's own
  cache key — this is the second, separate cache entry `ANVILML_DESIGN.md §11.6`
  describes.
- The denoising call itself is explicitly deferred to the next task.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_zit.py -v
# -> >=29 tests total in the file, exits 0
```

#### P21-B2: worker/nodes/arch/diffusion/zit.py: sample() denoising loop + seed resolution

**Goal:** Complete `sample()` with the actual denoising call and the seed
resolution logic that makes a `-1` seed reproducible once resolved.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/zit.py` — completes `sample()`.

**Key implementation notes:**
- `seed == -1` resolves to a cryptographically random integer via `secrets`, not
  `random.random` — resolved **before** denoising runs, so the value returned is
  the actual seed used, reproducible if logged.
- The return value never contains `-1` — it's always the resolved, concrete seed.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_zit.py -v
# -> >=34 tests total in the file, exits 0
```

---

### Group C — Sampler node

#### P21-C1: worker/nodes/sampler.py: Sampler generic node, mock branch only

**Goal:** Create the generic `Sampler` node with its mock branch fully working,
establishing the node's shape before the real branch is wired in.

**Files to create or modify:**
- `worker/nodes/sampler.py` — new; `Sampler`, mock branch only.

**Key implementation notes:**
- The mock branch's seed resolution is **deterministic** (`-1` → `0`), not
  random — mock-mode tests need reproducible output, unlike the real branch's
  cryptographically random resolution.
- The real branch is a bare placeholder for this task only — completed next.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_nodes_sampler.py -v
# -> >=3 tests, exits 0
```

#### P21-C2: worker/nodes/sampler.py: Sampler real branch dispatches to arch module

**Goal:** Complete `Sampler` with its real branch, dispatching to `zit.py`'s
`sample()` — correctly from the start, with both required markers pointing at
genuinely passing tests.

**Files to create or modify:**
- `worker/nodes/sampler.py` — completes the real branch.

**Key implementation notes:**
- Dispatches via `arch.diffusion.get_module(model.arch).sample(...)`, per
  `ANVILML_DESIGN.md §10.4`'s dispatch table.
- This is the first arch-dispatching node in the project where **both** markers
  point at passing tests from the same task — unlike the loader nodes' Phase 19/20
  split (a real-raises marker first, replaced later), `zit.py`'s `sample()` already
  exists by this point, so there's no equivalent groundwork gap to bridge.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_nodes_sampler.py -v -m real_mode
# -> >=8 tests total in the file, exits 0
```

---

### Group D — Proof

#### P21-D1: Runnable Proof: Sampler node denoises the ZiT fixture latent for real

**Goal:** Produce this phase's Runnable Proof, a real-mode pytest invocation
exercising the full chain this phase built.

**Files to create or modify:**
- None. This task runs the existing real-mode test suites; see Acceptance
  Criterion.

**Key implementation notes:**
- Like Phase 20, this phase's proof is a pytest invocation, not a live HTTP
  request — no `VaeDecode`/`ClipTextEncode` exists yet to turn the denoised latent
  into a viewable image. That's a later phase's scope.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_zit.py worker/tests/test_nodes_sampler.py -v -m real_mode
# -> exits 0, zero skips, zero xfails in this invocation
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v
python -m pytest worker/tests -v -m real_mode

# Runnable Proof (manual): see P21-D1 — the full real-mode chain (load, pipeline
# assembly, denoising, seed resolution, Sampler's real branch) succeeds end to end
# against the ZiT fixture checkpoint.
python -m pytest worker/tests/test_arch_zit.py worker/tests/test_nodes_sampler.py -v -m real_mode
```

---

## Known Constraints and Gotchas

- `compute_latent_shape()` and `sample()` are fixed method names per
  `ANVILML_DESIGN.md §10.4` — no family-prefixed or "clearer" alternative name is
  ever acceptable, in this module or any future one.
- The pipeline cache (keyed `f"{model_id}:pipeline"`) is distinct from the
  component cache `load()` uses — confirm both exist as separate `pipeline_cache`
  entries, not one entry overwriting the other.
- Mock-mode seed resolution is deterministic (`-1` → `0`); real-mode seed
  resolution is cryptographically random via `secrets`. These are deliberately
  different and must not be unified into one shared resolution function.
- `VaeDecode` does not exist yet — this phase's denoised latent has no path to
  becoming a viewable image until a later phase builds the VAE arch module and the
  generic decode node.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 21 — ZiT Diffusion Arch Module: Sampling & Latent Shape

**Capability proved:** The full real-mode sampling chain — pipeline assembly,
denoising, and seed resolution — succeeds end to end against the ZiT fixture
checkpoint, with the generic `Sampler` node's real branch dispatching correctly to
`zit.py`'s `sample()` from the same task that introduced it.

\`\`\`bash
# Runnable Proof (manual):
python -m pytest worker/tests/test_arch_zit.py worker/tests/test_nodes_sampler.py -v -m real_mode
# -> exits 0, zero skips, zero xfails
\`\`\`
```
