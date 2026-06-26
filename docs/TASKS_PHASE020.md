# Tasks: Phase 20 — ZiT Diffusion Arch Module: Shape Inference & Construction

**Phase:** 20
**Name:** ZiT Diffusion Arch Module: Shape Inference & Construction
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 9, 10, 19

---

## Overview

This phase implements the project's first real, concrete architecture module —
`worker/nodes/arch/diffusion/zit.py` — built up incrementally through the exact
four-step loading contract `ANVILML_DESIGN.md §11.3` specifies: shape inference from
the safetensors header, meta-device construction with correct dtype selection, and
finally key remapping and weight loading. This phase is scoped to **loading only**
— `sample()` and `compute_latent_shape()` are explicitly out of scope, reserved for
a later counterpart phase, since a generation pipeline needs `LoadClip`/`zit_vae.py`/
`Sampler` machinery this phase doesn't touch.

This phase exists right after the loading-contract groundwork (Phase 19) because
every piece Phase 19 built — the fixture convention, `pipeline_cache.py`, the
deliberately-raising `LoadModel` real branch — exists specifically to be filled in
by exactly this kind of phase. Building this phase in four sequential steps (shape
inference → dispatch registration → meta construction → dtype selection → key
remap/load) mirrors the design document's own four-step contract directly, rather
than attempting all of it in one task — each step is independently testable against
the same fixture, and each was the site of a specific, named, real incident (`P904`)
in the project's own history that this granularity exists to prevent recurring.

At the start of this phase, `zit.py` doesn't exist and `LoadModel`'s real branch
unconditionally raises `NotImplementedError`. At the end: a tiny synthetic ZiT-shaped
fixture checkpoint loads successfully end-to-end through the real path — shape
inference, meta-device construction, dtype selection, key remapping, and weight
loading all genuinely exercised, with `LoadModel`'s stale placeholder test removed
and replaced by one that actually passes.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Fixture | P20-A1 | The tiny synthetic ZiT-shaped checkpoint, plus a metadata-fallback variant |
| B | Shape inference & dispatch | P20-B1 … P20-B2 | `_infer_hyperparams()`, then `can_handle()` + registration |
| C | Construction & loading | P20-C1 … P20-C3 | Meta-device construction, dtype selection, then key remap + load |
| D | Loader integration | P20-D1 | `LoadModel`'s real branch finally calls something real |
| E | Proof | P20-E1 | The phase's Runnable Proof |

---

## Prerequisites

`worker/tests/fixtures/README.md`'s convention must exist per Phase 19 (P19-D1).
`LoadModel`'s mock/real-placeholder structure must exist per Phase 19 (P19-C1,
P19-C2). The `arch/diffusion/__init__.py` dispatcher must exist (with zero
registered modules) per Phase 10 (P10-B1).

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §11.3` | P20-B1, P20-C1, P20-C3 | The exact four-step loading contract, including the "read every key" and dtype-cast-before-`assign=True` rules |
| `ANVILML_DESIGN.md §11.2` | P20-C1 | The library boundary — layer/block classes only, never a hub-aware loader |
| `ANVILML_DESIGN.md §11.5` | P20-C2 | The fixed fp8→bf16→fp16→fp32 dtype precedence |
| `ANVILML_DESIGN.md §17.5` | P20-A1 | Fixture sizing and the mandatory metadata-fallback regression case |
| `ANVILML_DESIGN.md §10.6` | P20-D1 | Marker hygiene — a stale marker pointing at a removed test is a real defect |

---

## Task Descriptions

### Group A — Fixture

#### P20-A1: worker/tests/fixtures/: ZiT diffusion fixture safetensors builder

**Goal:** Create the tiny synthetic checkpoint every subsequent task in this
phase tests against, following the convention Phase 19 documented.

**Files to create or modify:**
- `worker/tests/fixtures/build_zit_fixture.py` — new; the builder script.
- `worker/tests/fixtures/zit_tiny.safetensors`,
  `zit_tiny_no_metadata.safetensors` — the generated fixtures, committed.

**Key implementation notes:**
- Shapes are structurally valid for the shape-inference formula `zit.py` will
  implement — **not** a miniaturized copy of the real model's actual shapes, per
  `ANVILML_DESIGN.md §17.5`.
- The second variant (`zit_tiny_no_metadata.safetensors`) has a non-recognizable
  key prefix and no `arch` metadata key — the mandatory regression case that
  exercises the metadata-fallback path.

**Acceptance criterion:**
```bash
python worker/tests/fixtures/build_zit_fixture.py
# -> exits 0, both files under 10MB combined, both load via safetensors.safe_open
```

---

### Group B — Shape inference & dispatch

#### P20-B1: worker/nodes/arch/diffusion/zit.py: shape inference from safetensors header

**Goal:** Implement the contract's first step — inferring every architecture
hyperparameter from the checkpoint's tensor shapes alone, reading every key.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/zit.py` — new; `_infer_hyperparams()`.

**Key implementation notes:**
- Reads **every** key via `f.keys()` — never a truncated or sliced sample. This is
  the exact regression `P904` produced: a partial key scan silently missed two of
  three layer stacks while still looking complete.
- `can_handle()` and dispatch registration are explicitly deferred to the next task.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_zit.py -v
# -> >=4 tests, exits 0
```

#### P20-B2: worker/nodes/arch/diffusion/zit.py: can_handle() + dispatch registration

**Goal:** Connect `zit.py` to the dispatch mechanism Phase 10 built, giving it
its first real entry.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/zit.py` — adds `can_handle()`.
- `worker/nodes/arch/diffusion/__init__.py` — registers `zit.py`.

**Key implementation notes:**
- This is the first real entry in `_REGISTERED_MODULES`, which has been correctly
  empty since Phase 10 — confirm the existing `get_module()` dispatcher (P10-B1)
  now actually finds this module without any change to the dispatcher itself.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_zit.py -v
# -> >=7 tests total in the file, exits 0
```

---

### Group C — Construction & loading

#### P20-C1: worker/nodes/arch/diffusion/zit.py: meta-device construction

**Goal:** Implement the contract's second step — constructing the target module
on `torch.device("meta")` so no real memory is allocated yet, using only the
shape-inferred hyperparameters.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/zit.py` — adds meta-device construction.

**Key implementation notes:**
- This is the exact step that fixed `P904`'s ~15GB-on-construction crash — no real
  parameter memory exists until materialization (a later task).
- Uses `diffusers`'/`transformers`' layer/block classes as building blocks only,
  per `ANVILML_DESIGN.md §11.2`'s library boundary — never a hub-aware loader.
- Dtype selection is explicitly deferred to the next task.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_zit.py -v
# -> >=10 tests total in the file, exits 0
```

#### P20-C2: worker/nodes/arch/diffusion/zit.py: dtype selection per InferenceCaps

**Goal:** Implement the fixed dtype precedence every architecture module follows
identically, applied to this module's meta-device construction.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/zit.py` — adds dtype selection.

**Key implementation notes:**
- The precedence is fixed and universal: `fp8` (if both `caps.fp8` and the
  checkpoint's native dtype agree) → `bf16` → `fp16` → `fp32`. This is the same
  logic every future arch module (Flux 2 Klein, etc.) will implement identically.
- On CPU, step 1 always fails and step 2 succeeds — CPU real-mode tests always
  land on `bfloat16`, which is correct, not a workaround.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_zit.py -v
# -> >=14 tests total in the file, exits 0
```

#### P20-C3: worker/nodes/arch/diffusion/zit.py: key remap, load_state_dict, .arch attribute

**Goal:** Complete `load()` with the final two contract steps — materializing
real weights into the meta-constructed module and tagging the result with its
architecture string.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/zit.py` — completes `load()`.

**Key implementation notes:**
- Tensors are cast to the already-selected dtype **before** calling
  `load_state_dict(..., assign=True)` — this exact ordering is mandatory.
  `assign=True` bypasses dtype coercion, so casting afterward silently fails to
  take effect. This is `P904`'s exact dtype-safety incident, reproduced here as the
  rule that prevents it from recurring.
- The key remap table is built by inspecting both key sets directly against this
  phase's own fixture — never assumed from a prior model version or another
  architecture's mapping.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_zit.py -v
# -> >=20 tests total in the file, exits 0
```

---

### Group D — Loader integration

#### P20-D1: worker/nodes/loader.py: LoadModel real branch calls zit.py via dispatch

**Goal:** Close the gap Phase 19 deliberately left open — `LoadModel`'s real
branch finally does something real, end to end.

**Files to create or modify:**
- `worker/nodes/loader.py` — replaces the `NotImplementedError` placeholder.

**Key implementation notes:**
- The stale marker pointing at the old `NotImplementedError`-asserting test must be
  updated to point at the new passing test — **not** left alongside it. A marker
  pointing at a removed or stale test is the exact `P902` false-mechanical-guarantee
  failure mode: a checkable claim that turns out false.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_nodes_loader.py -v -m real_mode
# -> exits 0
```

---

### Group E — Proof

#### P20-E1: Runnable Proof: LoadModel node loads the ZiT fixture checkpoint for real

**Goal:** Produce this phase's Runnable Proof — a real-mode pytest invocation
exercising the entire chain this phase built, rather than a live-server HTTP proof.

**Files to create or modify:**
- None. This task runs the existing real-mode test suites; see Acceptance
  Criterion.

**Key implementation notes:**
- This phase's proof is intentionally a pytest invocation, not a live HTTP request
  — no `Sampler`/`VaeDecode` exists yet to complete an actual end-to-end generation
  job. That requires a later counterpart phase building `sample()` and
  `compute_latent_shape()`, explicitly out of this phase's scope.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_zit.py worker/tests/test_nodes_loader.py -v -m real_mode
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

# Runnable Proof (manual): see P20-E1 — the full real-mode chain (shape inference,
# meta construction, dtype selection, key remap, load, LoadModel's real branch)
# succeeds end to end against the ZiT fixture checkpoint, with zero
# NotImplementedError anywhere in the chain.
python -m pytest worker/tests/test_arch_zit.py worker/tests/test_nodes_loader.py -v -m real_mode
```

---

## Known Constraints and Gotchas

- `zit.py`'s `sample()` and `compute_latent_shape()` are **not** implemented in this
  phase — only `load()`, across its four contract steps. A later counterpart phase
  completes the diffusion module.
- Tensors must be cast to the target dtype **before** `load_state_dict(...,
  assign=True)`, never after — this exact ordering mistake is `P904`'s recorded
  dtype-safety incident, and every future arch module must follow the same rule.
- Shape inference must read every key in the safetensors file, never a truncated
  sample — `P904`'s exact shape-inference regression silently missed two of three
  layer stacks using a sliced key list that looked complete.
- The `REAL_PATH_VERIFIED` marker on `LoadModel` must point at a currently-passing
  test after P20-D1 — the prior marker, pointing at a now-removed
  `NotImplementedError`-asserting test, must not be left in place alongside the new
  one.
- This phase's fixture checkpoint is deliberately tiny and structurally synthetic —
  never a real downloaded model file, and never scaled up "for realism" beyond what
  the shape-inference formula needs to construct correctly.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 20 — ZiT Diffusion Arch Module: Shape Inference & Construction

**Capability proved:** The full real-mode model-loading chain — shape inference,
meta-device construction, dtype selection, key remapping, and weight loading —
succeeds end to end against a tiny synthetic ZiT-shaped fixture checkpoint, with
`LoadModel`'s real branch calling genuinely real code for the first time in the
project.

\`\`\`bash
# Runnable Proof (manual):
python -m pytest worker/tests/test_arch_zit.py worker/tests/test_nodes_loader.py -v -m real_mode
# -> exits 0, zero skips, zero xfails
\`\`\`
```
