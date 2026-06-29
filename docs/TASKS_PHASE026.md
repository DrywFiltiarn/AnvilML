# Tasks: Phase 26 — Flux 2 Klein 9B + Qwen3-8B CLIP Variant

**Phase:** 26
**Name:** Flux 2 Klein 9B + Qwen3-8B CLIP Variant
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 9, 10, 19, 22, 25

---

## Overview

This phase confirms `ANVILML_DESIGN.md §20`'s second architecture-confirmation
point: the same `flux2klein.py` and `qwen3.py` files already written in Phases 25
and 22 can serve a larger model size (9B diffusion, 8B text encoder) through shape
inference alone — `§11.3` step 1 — with **no second file, no size-specific branch,
and no hardcoded size enum** anywhere in either module. This phase adds no new arch
module files; it extends existing ones with new fixtures and confirms the existing
shape-inference logic generalizes correctly.

This phase exists as the final per-architecture confirmation phase because it closes
the full MVP model matrix from `ANVILML_DESIGN.md §2.3`'s table — all three rows
(ZiT+Qwen3-4B, Flux2Klein-4B+Qwen3-4B, Flux2Klein-9B+Qwen3-8B) will produce real
artifacts through the real pipeline after this phase. It also introduces one
genuinely new technical wrinkle neither prior phase needed to handle: Qwen3-8B's
FP8-mixed precision — some tensors natively FP8, others not, within the same
checkpoint — which `§11.5`'s per-module dtype precedence, as originally stated,
assumes is a single decision for the whole module. This phase's tasks document
exactly how per-tensor native dtype interacts with that module-level precedence.

At the start of this phase, `flux2klein.py` and `qwen3.py` have only been exercised
against their 4B fixtures. At the end: both modules are confirmed to correctly infer
and load the larger variant from their existing, unmodified shape-inference logic
(or, if any change was genuinely needed, that change is verified to be strictly
shape-driven, never size-branching), and a 9B/8B generation graph produces a real
artifact through the exact same pipeline every prior architecture phase used.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Fixtures | P26-A1 | Flux 2 Klein 9B and Qwen3-8B (FP8-mixed) fixture builders |
| B | Flux 2 Klein 9B | P26-B1 … P26-B2 | Shape inference confirmation, then full load/sample confirmation |
| C | Qwen3-8B | P26-C1 … P26-C2 | Shape inference + FP8-mixed detection, then load with per-tensor dtype handling |
| D | Proof | P26-D1 | The full MVP model matrix's final Runnable Proof |

---

## Prerequisites

`flux2klein.py`'s full loading and sampling contract must work per Phase 25 (P25-C2,
P25-D1). `qwen3.py`'s full loading contract must work per Phase 22 (P22-C2). The
existing Flux 2 VAE fixture from Phase 25 is reused unchanged — the VAE has no size
variant in the model matrix.

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §20` roadmap note | P26-B1, P26-C1 | "Confirms the same arch module can serve two model sizes via shape inference... rather than needing a second file" |
| `ANVILML_DESIGN.md §11.3` step 1 | P26-B1, P26-C1 | Shape inference alone determines hyperparameters — no hardcoded size assumption |
| `ANVILML_DESIGN.md §11.5` | P26-C2 | The fixed dtype precedence, extended (not replaced) to handle per-tensor native dtype within one mixed checkpoint |
| `ANVILML_DESIGN.md §2.3` | P26-D1 | The complete MVP model matrix — this phase's proof closes the final row |

---

## Task Descriptions

### Group A — Fixtures

#### P26-A1: worker/tests/fixtures/: Flux 2 Klein 9B + Qwen3-8B fixture builders

**Goal:** Create the fixtures this phase's confirmation tasks test against —
shaped to imply larger hyperparameters than the existing 4B fixtures, using the
exact same formula code.

**Files to create or modify:**
- `worker/tests/fixtures/build_flux2klein_9b_fixture.py`,
  `build_qwen3_8b_fixture.py` — new builder scripts.
- Two generated `.safetensors` fixtures, committed.

**Key implementation notes:**
- No no-metadata variant is needed for either fixture — that regression case is a
  **per-family**, not per-size, requirement per `ANVILML_DESIGN.md §17.5`, and was
  already covered by the 4B/4B fixtures.
- The Qwen3-8B fixture specifically needs some tensors at native FP8 and others not
  — the mixed-precision wrinkle this phase's Group C confirms.

**Acceptance criterion:**
```bash
# Both build scripts exit 0, both files under 10MB, both load via
# safetensors.safe_open
```

---

### Group B — Flux 2 Klein 9B

#### P26-B1: worker/nodes/arch/diffusion/flux2klein.py: 9B variant via shape inference

**Goal:** Confirm the existing `_infer_hyperparams()` correctly scales to the 9B
fixture with no size-specific branching.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/flux2klein.py` — confirmed, not duplicated; any
  change must remain shape-driven.

**Key implementation notes:**
- **Do not create a second function or code path for the 9B size.** Any genuinely
  needed change must be entirely shape-driven (e.g., correctly reading a key
  pattern that only exists at this larger size) — never a branch like `if
  num_layers > X: ...9b-specific...`.
- `can_handle()`/`get_module()` continue routing both fixture sizes to this **one**
  registered module — no second registration entry for the 9B size.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_flux2klein.py -v
# -> >=29 tests total in the file, exits 0
```

#### P26-B2: worker/nodes/arch/diffusion/flux2klein.py: 9B load/sample end-to-end

**Goal:** Confirm the full existing loading and sampling contract works correctly
against the 9B fixture.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/flux2klein.py` — primarily verification; fixes only
  if a genuine defect surfaces.

**Key implementation notes:**
- This task is **primarily verification**, not new implementation — if the
  verification surfaces a genuine defect, fix it under the same shape-driven-only
  constraint as P26-B1, with the reason documented.
- Dtype selection (`§11.5`) is unaffected by model size — the same precedence
  applies regardless of 4B or 9B.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_flux2klein.py -v
# -> >=35 tests total in the file, exits 0
```

---

### Group C — Qwen3-8B

#### P26-C1: worker/nodes/arch/clip/qwen3.py: 8B FP8-mixed variant via shape inference

**Goal:** Confirm shape inference scales to the 8B fixture, and correctly detects
the genuinely new wrinkle this size introduces: mixed FP8 precision within one
checkpoint.

**Files to create or modify:**
- `worker/nodes/arch/clip/qwen3.py` — confirmed/extended for mixed-dtype detection.

**Key implementation notes:**
- Mixed precision within one checkpoint — some tensors natively FP8, others not —
  is a genuinely new wrinkle the 4B fixture didn't exercise.
- `can_handle("qwen3")` is unaffected by size — both 4B and 8B route through the
  same string match; size is determined entirely by shape inference, never a
  second `clip_type` string like `"qwen3-8b"`.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_clip_qwen3.py -v
# -> >=21 tests total in the file, exits 0
```

#### P26-C2: worker/nodes/arch/clip/qwen3.py: 8B load end-to-end + FP8-mixed handling

**Goal:** Confirm `load()` correctly handles per-tensor native dtype within the
mixed checkpoint, extending (not replacing) the module-level dtype precedence.

**Files to create or modify:**
- `worker/nodes/arch/clip/qwen3.py` — extends `load()`'s dtype handling.

**Key implementation notes:**
- FP8-native tensors load at their native FP8 dtype, with no upcast — only
  non-FP8 tensors in the same checkpoint follow the normal `§11.5` precedence.
- This interaction between per-tensor native dtype and the module-level fallback
  chain is documented explicitly in a code comment, since `§11.5`'s precedence as
  originally written assumes a single whole-module decision.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_clip_qwen3.py -v
# -> >=27 tests total in the file, exits 0
```

---

### Group D — Proof

#### P26-D1: Runnable Proof: Flux 2 Klein 9B + Qwen3-8B graph produces a real artifact

**Goal:** Produce this phase's Runnable Proof, closing the complete MVP model
matrix.

**Files to create or modify:**
- None. This task runs the already-built binary against the new fixtures; see
  Acceptance Criterion.

**Key implementation notes:**
- Reuses the existing Flux 2 VAE fixture from Phase 25 unchanged — the VAE has no
  size variant per the model matrix.
- This is the exact same generic-node pipeline every prior architecture phase
  used, with zero changes — the only genuinely new inputs are the 9B/8B
  `model_id` values.
- This closes `ANVILML_DESIGN.md §2.3`'s full model matrix: all three rows now
  produce real artifacts through the real pipeline.

**Acceptance criterion:**
```bash
# Full submit -> poll -> retrieve sequence in real mode, using the 9B/8B graph,
# producing a retrievable, valid PNG.
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v
python -m pytest worker/tests -v -m real_mode

# Runnable Proof (manual): see P26-D1 — a Flux 2 Klein 9B + Qwen3-8B generation
# graph produces a real, retrievable PNG artifact through the exact same pipeline
# used for every prior architecture, closing the full MVP model matrix.
```

---

## Known Constraints and Gotchas

- **No new arch module files exist after this phase** — `flux2klein.py` and
  `qwen3.py` are the same two files extended in Phase 25/22, now confirmed to serve
  two sizes each.
- Any code change in this phase must be **strictly shape-driven** — a branch keyed
  on a hardcoded size threshold (layer count, hidden dim, etc.) is exactly the
  defect class this phase exists to prevent, and must be flagged rather than
  silently introduced.
- FP8-mixed precision is a genuinely new per-tensor concern this phase introduces —
  `§11.5`'s precedence still applies, but now per-tensor for tensors not already at
  their native FP8 dtype, not uniformly across the whole module.
- This phase closes the full MVP model matrix from `§2.3` — no further
  per-architecture phase is expected in the MVP scope after this one.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 26 — Flux 2 Klein 9B + Qwen3-8B CLIP Variant

**Capability proved:** A Flux 2 Klein 9B + Qwen3-8B (FP8-mixed) generation graph
produces a real, retrievable PNG artifact through the exact same generic-node
pipeline used for every prior architecture — confirming `flux2klein.py` and
`qwen3.py` serve two model sizes each via shape inference alone, with no second
file and no size-specific branching. This closes the full MVP model matrix from
`ANVILML_DESIGN.md §2.3`.

\`\`\`bash
# Runnable Proof (manual): identical to Phases 24/25's sequence, with model_id
# values pointing at the Flux 2 Klein 9B + Qwen3-8B fixtures (Phase 25's Flux 2
# VAE fixture is reused unchanged — VAE has no size variant per the model matrix).
cargo build --release -p anvilml
./target/release/anvilml &
sleep 2
DIFF_ID=$(sha256sum worker/tests/fixtures/flux2klein9b_tiny.safetensors | head -c1048576 | cut -d' ' -f1)
VAE_ID=$(sha256sum worker/tests/fixtures/flux2_vae_tiny.safetensors | head -c1048576 | cut -d' ' -f1)
CLIP_ID=$(sha256sum worker/tests/fixtures/qwen3_8b_tiny.safetensors | head -c1048576 | cut -d' ' -f1)
JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d "{\"graph\":{\"nodes\":[
    {\"id\":\"model\",\"type\":\"LoadModel\",\"inputs\":{\"model_id\":\"$DIFF_ID\"}},
    {\"id\":\"vae\",\"type\":\"LoadVae\",\"inputs\":{\"model_id\":\"$VAE_ID\"}},
    {\"id\":\"encoder\",\"type\":\"LoadClip\",\"inputs\":{\"model_id\":\"$CLIP_ID\",\"clip_type\":\"qwen3\"}},
    {\"id\":\"latent\",\"type\":\"EmptyLatent\",\"inputs\":{\"width\":64,\"height\":64,\"model\":{\"node_id\":\"model\",\"output_slot\":\"model\"}}},
    {\"id\":\"cond\",\"type\":\"ClipTextEncode\",\"inputs\":{\"clip\":{\"node_id\":\"encoder\",\"output_slot\":\"clip\"},\"positive_text\":\"a photograph of a red fox in a snowy forest\"}},
    {\"id\":\"sampled\",\"type\":\"Sampler\",\"inputs\":{\"model\":{\"node_id\":\"model\",\"output_slot\":\"model\"},\"conditioning\":{\"node_id\":\"cond\",\"output_slot\":\"conditioning\"},\"clip\":{\"node_id\":\"encoder\",\"output_slot\":\"clip\"},\"latent\":{\"node_id\":\"latent\",\"output_slot\":\"latent\"},\"steps\":4,\"cfg\":1.0,\"seed\":-1}},
    {\"id\":\"decoded\",\"type\":\"VaeDecode\",\"inputs\":{\"vae\":{\"node_id\":\"vae\",\"output_slot\":\"vae\"},\"latent\":{\"node_id\":\"sampled\",\"output_slot\":\"latent\"}}},
    {\"id\":\"saved\",\"type\":\"SaveImage\",\"inputs\":{\"image\":{\"node_id\":\"decoded\",\"output_slot\":\"image\"},\"seed\":{\"node_id\":\"sampled\",\"output_slot\":\"seed\"}}}
  ]},\"settings\":{}}" \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['job_id'])")
sleep 5
HASH=$(curl -s "http://127.0.0.1:8488/v1/jobs/$JOB_ID" | python3 -c "
import sys,json
d=json.load(sys.stdin)
assert d['status']=='Completed'
print(d.get('artifact_hash') or d.get('result',{}).get('artifact_hash'))
")
curl -s -o saved_proof.png "http://127.0.0.1:8488/v1/artifacts/$HASH"
python3 -c "from PIL import Image; im=Image.open('saved_proof.png'); assert im.size==(64,64)"
# -> exits 0; a real, retrievable 64x64 PNG was produced, closing all three rows
#    of the MVP model matrix
kill %1
rm -f saved_proof.png
\`\`\`
```
