# Tasks: Phase 23 — ZiT VAE Arch Module

**Phase:** 23
**Name:** ZiT VAE Arch Module
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 9, 10, 19, 20, 21

---

## Overview

This phase implements `worker/nodes/arch/vae/zit_vae.py` — the ZiT-compatible VAE,
the third and final arch module needed before a complete generation chain exists.
It follows the same shape-inference-through-loading contract as `zit.py` (Phase 20)
and `qwen3.py` (Phase 22), then adds the VAE family's second fixed method, `decode()`,
turning a denoised latent into a real PIL image. `LoadVae`'s real branch — the last
of the three loader nodes still deliberately raising since Phase 19 — finally calls
something real, closing that gap entirely.

This phase exists as a genuinely independent arch module from `zit.py`, per
`ANVILML_DESIGN.md §11.4`'s explicit decision to split VAE into its own family
rather than nesting it under diffusion — despite both modules sharing the "ZiT" name
informally, their shape-inference formulas, key namespaces, and `nn.Module`
families are completely independent, and this phase's tasks are written to keep
that independence real rather than accidentally coupling the two through shared
code. This phase also closes the loop on the loader nodes Phase 19 deliberately
left incomplete — after this phase, `LoadModel`, `LoadClip`, and `LoadVae` are all
genuinely real.

At the start of this phase, `zit_vae.py` doesn't exist and `LoadVae`'s real branch
unconditionally raises `NotImplementedError`. At the end: a tiny synthetic
ZiT-VAE-shaped fixture loads and decodes successfully end-to-end, `LoadVae` is
fully real, and — for the first time in the project — a real-mode integration test
chains `LoadModel` → `Sampler` → `decode()` directly against their respective
fixtures to produce an actual `PIL.Image`, the first genuinely complete generation
output, even before the generic node layer is wired through it.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Fixture | P23-A1 | The tiny synthetic ZiT-VAE-shaped checkpoint, plus a metadata-fallback variant |
| B | Shape inference & dispatch | P23-B1 … P23-B2 | `_infer_hyperparams()`, then `can_handle()` + registration |
| C | Construction & loading | P23-C1 … P23-C3 | Meta construction, dtype selection, then key remap + load |
| D | Decoding | P23-D1 | `decode()` — the VAE family's second fixed method |
| E | Loader integration | P23-E1 | `LoadVae`'s real branch — the third and final loader to go real |
| F | Proof | P23-F1 | The first complete real-mode generation chain |

---

## Prerequisites

`worker/tests/fixtures/README.md`'s convention must exist per Phase 19 (P19-D1).
`LoadVae`'s mock/real-placeholder structure must exist per Phase 19 (P19-C3). The
`arch/vae/__init__.py` dispatcher must exist (with zero registered modules) per
Phase 10 (P10-B2). `zit.py`'s `compute_latent_shape()` must exist per Phase 21
(P21-A1), since this phase's decode latent shape must be consistent with it.

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §11.4` | P23-B1 | VAE is its own arch family, independent of `zit.py`'s diffusion module — no shared inference logic |
| `ANVILML_DESIGN.md §11.3` | P23-B1, P23-C1, P23-C2, P23-C3 | The same four-step contract every prior arch module followed |
| `ANVILML_DESIGN.md §10.4` | P23-B2, P23-D1 | Fixed method names — `load()`, `decode()` — never a family-prefixed variant |
| `ANVILML_DESIGN.md §17.5` | P23-A1 | Fixture sizing and the mandatory metadata-fallback regression case, required per-family |
| `ANVILML_DESIGN.md §10.6` | P23-E1 | Marker hygiene — a stale marker pointing at a removed test is a real defect |

---

## Task Descriptions

### Group A — Fixture

#### P23-A1: worker/tests/fixtures/: ZiT VAE fixture safetensors builder

**Goal:** Create the tiny synthetic VAE checkpoint every subsequent task in this
phase tests against.

**Files to create or modify:**
- `worker/tests/fixtures/build_zit_vae_fixture.py` — new; the builder script.
- `worker/tests/fixtures/zit_vae_tiny.safetensors`,
  `zit_vae_tiny_no_metadata.safetensors` — the generated fixtures, committed.

**Key implementation notes:**
- The metadata-fallback regression case is required for VAE fixtures too, per
  `ANVILML_DESIGN.md §17.5`'s per-family requirement — not just for diffusion
  modules.

**Acceptance criterion:**
```bash
python worker/tests/fixtures/build_zit_vae_fixture.py
# -> exits 0, both files under 10MB combined, both load via safetensors.safe_open
```

---

### Group B — Shape inference & dispatch

#### P23-B1: worker/nodes/arch/vae/zit_vae.py: shape inference from safetensors header

**Goal:** Implement the contract's first step for this independent VAE module.

**Files to create or modify:**
- `worker/nodes/arch/vae/zit_vae.py` — new; `_infer_hyperparams()`.

**Key implementation notes:**
- **Do not import or reuse `zit.py`'s hyperparameter inference logic** — despite
  both modules relating to "ZiT" informally, `ANVILML_DESIGN.md §11.4` is explicit
  that VAE is its own independent arch family with its own shape-inference formula.
- `can_handle()` and dispatch registration are explicitly deferred to the next
  task.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_vae_zit.py -v
# -> >=4 tests, exits 0
```

#### P23-B2: worker/nodes/arch/vae/zit_vae.py: can_handle() + dispatch registration

**Goal:** Connect `zit_vae.py` to the VAE dispatch mechanism Phase 10 built.

**Files to create or modify:**
- `worker/nodes/arch/vae/zit_vae.py` — adds `can_handle()`.
- `worker/nodes/arch/vae/__init__.py` — registers `zit_vae.py`.

**Key implementation notes:**
- Uses the same metadata-or-path-derived dispatch pattern as `zit.py` — distinct
  from `qwen3.py`'s `clip_type`-string dispatch, since VAE and diffusion share one
  dispatch shape while CLIP uses another, per `ANVILML_DESIGN.md §10.4`.
- VAE modules implement `load()` and `decode()` only — no `sample()` or
  `compute_latent_shape()`, which are diffusion-only.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_vae_zit.py -v
# -> >=7 tests total in the file, exits 0
```

---

### Group C — Construction & loading

#### P23-C1: worker/nodes/arch/vae/zit_vae.py: meta construction

**Goal:** Implement meta-device construction for the VAE module, identical
discipline to every prior arch module.

**Files to create or modify:**
- `worker/nodes/arch/vae/zit_vae.py` — adds meta-device construction.

**Key implementation notes:**
- Uses `diffusers`'/torch's layer/block classes (conv, normalization, attention if
  present in this VAE's architecture) per `ANVILML_DESIGN.md §11.2`'s library
  boundary.
- Dtype selection is explicitly deferred to the next task.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_vae_zit.py -v
# -> >=10 tests total in the file, exits 0
```

#### P23-C2: worker/nodes/arch/vae/zit_vae.py: dtype selection per InferenceCaps

**Goal:** Implement the same fixed dtype precedence every arch module follows,
restated for this module rather than assumed shared via inheritance.

**Files to create or modify:**
- `worker/nodes/arch/vae/zit_vae.py` — adds dtype selection.

**Key implementation notes:**
- The fp8→bf16→fp16→fp32 precedence is identical to every other arch module's —
  this restatement, rather than a shared base class, is intentional per the
  project's per-module contract-following discipline.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_vae_zit.py -v
# -> >=14 tests total in the file, exits 0
```

#### P23-C3: worker/nodes/arch/vae/zit_vae.py: key remap, load_state_dict, .arch attribute

**Goal:** Complete `load()` with the final two contract steps.

**Files to create or modify:**
- `worker/nodes/arch/vae/zit_vae.py` — completes `load()`.

**Key implementation notes:**
- The key remap table is built against **this phase's own fixture** — never
  assumed from `zit.py`'s diffusion key mapping, which belongs to a completely
  different key namespace.
- The same mandatory cast-before-`assign=True` ordering applies, restated here as
  it has been for every prior arch module.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_vae_zit.py -v
# -> >=20 tests total in the file, exits 0
```

---

### Group D — Decoding

#### P23-D1: worker/nodes/arch/vae/zit_vae.py: decode() latent-to-image

**Goal:** Implement the VAE family's second fixed method — turning a denoised
latent into a real, viewable image — the actual point of integration between this
module and `zit.py`'s sampling output.

**Files to create or modify:**
- `worker/nodes/arch/vae/zit_vae.py` — adds `decode()`.

**Key implementation notes:**
- Method name fixed per `ANVILML_DESIGN.md §10.4` — `decode` exactly, never
  `vae_decode()` or `to_image()`.
- The input latent's shape is consistent with `zit.py`'s `compute_latent_shape()`
  output (Phase 21) — even though the two modules are independent per `§11.4`, this
  shared shape contract is the genuine integration point this task's tests verify.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_vae_zit.py -v
# -> >=25 tests total in the file, exits 0
```

---

### Group E — Loader integration

#### P23-E1: worker/nodes/loader.py: LoadVae real branch calls zit_vae.py via dispatch

**Goal:** Close the last remaining loader gap from Phase 19 — `LoadVae`'s real
branch finally does something real, completing all three loader nodes.

**Files to create or modify:**
- `worker/nodes/loader.py` — replaces `LoadVae`'s `NotImplementedError`
  placeholder.

**Key implementation notes:**
- This is the **third and final** loader to gain a real branch, after `LoadModel`
  (Phase 20) and `LoadClip` (Phase 22) — after this task, no loader node has a
  remaining `NotImplementedError` placeholder anywhere in `loader.py`.
- The same stale-marker discipline applies: the old placeholder-asserting test is
  removed, not left alongside the new passing one.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_nodes_loader.py -v -m real_mode
# -> exits 0
```

---

### Group F — Proof

#### P23-F1: Runnable Proof: full load+sample+decode chain produces a real PIL image

**Goal:** Produce this phase's Runnable Proof — the first genuinely complete
real-mode generation chain in the project, chaining the underlying arch modules
directly.

**Files to create or modify:**
- `worker/tests/test_e2e_zit_pipeline.py` — new (if not already present).

**Key implementation notes:**
- This integration test chains `LoadModel` → `Sampler` → `zit_vae.py`'s `decode()`
  **directly**, bypassing the generic `VaeDecode`/`ClipTextEncode` nodes — wiring
  those generic nodes through this chain is explicitly a later phase's scope. This
  proof confirms the underlying arch modules genuinely compose correctly before
  that wiring work begins.
- The produced image's dimensions are checked against the requested width/height —
  confirming the shape contract between `compute_latent_shape()` and `decode()`
  actually holds end to end, not just in each module's own isolated tests.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_e2e_zit_pipeline.py -v -m real_mode
# -> exits 0, asserts a real, non-mock PIL Image with correct dimensions is produced
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v
python -m pytest worker/tests -v -m real_mode

# Runnable Proof (manual): see P23-F1 — the first complete real-mode generation
# chain (LoadModel -> Sampler -> decode()) produces a real PIL.Image with correct
# dimensions, against the respective fixture checkpoints.
python -m pytest worker/tests/test_e2e_zit_pipeline.py -v -m real_mode
```

---

## Known Constraints and Gotchas

- `zit_vae.py` must never import or reuse `zit.py`'s shape-inference logic, even
  though both relate to "ZiT" informally — `ANVILML_DESIGN.md §11.4` requires VAE to
  be a genuinely independent arch family.
- `decode()` is the VAE family's second and final fixed method — no `sample()` or
  `compute_latent_shape()` exists or is ever added to a VAE module.
- After this phase, **no loader node has a remaining real-mode placeholder** —
  `LoadModel`, `LoadClip`, and `LoadVae` are all genuinely real. Any future task
  reintroducing a `NotImplementedError` in `loader.py` without a stated reason is a
  regression.
- The Phase 23 Runnable Proof deliberately bypasses the generic `VaeDecode` node —
  that node's real branch, and the rest of the generic conditioning/sampling/decode
  node layer, is explicitly a later phase's scope.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 23 — ZiT VAE Arch Module

**Capability proved:** The first genuinely complete real-mode generation chain in
the project — `LoadModel` → `Sampler` → `zit_vae.py`'s `decode()` — produces a real
`PIL.Image` with correct dimensions, chained directly against the respective
fixture checkpoints, ahead of the generic node layer being wired through it.

\`\`\`bash
# Runnable Proof (manual):
python -m pytest worker/tests/test_e2e_zit_pipeline.py -v -m real_mode
# -> exits 0, asserts a real, non-mock PIL Image with correct dimensions is produced
\`\`\`
```
