# Tasks: Phase 25 — Flux 2 Klein 4B Diffusion + Flux 2 VAE

**Phase:** 25
**Name:** Flux 2 Klein 4B Diffusion + Flux 2 VAE
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 9, 10, 19, 20, 21, 22, 23, 24

---

## Overview

This phase adds the project's second diffusion architecture —
`worker/nodes/arch/diffusion/flux2klein.py` (4B variant) — and its corresponding
VAE, `worker/nodes/arch/vae/flux2_vae.py`, following the exact same loading
contract every prior arch module has followed. This phase exists specifically to
serve as `ANVILML_DESIGN.md §20`'s explicit confirmation point: **adding a second
diffusion architecture must require zero changes to the generic node layer** —
`loader.py`, `sampler.py`, `encoder.py`, `decode.py`, `image.py` are all touched by
zero tasks in this phase. If any of them genuinely needed a change, that would be a
design defect to report, not something to silently patch around.

This phase also confirms a second, smaller claim: Flux 2 Klein's text encoder is the
**same** Qwen3 4B module already built in Phase 22 — no new CLIP arch module is
needed for this row of the model matrix. Only the diffusion transformer and the VAE
are genuinely new per-architecture work; everything else (the generic nodes, the
dispatch mechanism, the CLIP module) is proven to be correctly architecture-agnostic
by this phase reusing it unchanged.

At the start of this phase, only ZiT exists as a diffusion architecture. At the end:
`flux2klein.py` and `flux2_vae.py` both pass the full loading contract against their
own fixtures, `arch/diffusion/__init__.py` and `arch/vae/__init__.py` each correctly
disambiguate between two registered modules, and submitting a Flux 2 Klein 4B
generation graph through the exact same `POST /v1/jobs` pipeline Phase 24 used for
ZiT produces a real artifact — with the phase's own report explicitly confirming
zero generic-node-layer changes were needed, or flagging a design defect if any
were.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Fixtures | P25-A1 | Flux 2 Klein 4B and Flux 2 VAE fixture builders |
| B | Diffusion shape inference & dispatch | P25-B1 … P25-B2 | `_infer_hyperparams()`, then `can_handle()` + dispatch confirming dual-module disambiguation |
| C | Diffusion construction & loading | P25-C1 … P25-C2 | Meta construction + dtype, then key remap + load |
| D | Diffusion sampling | P25-D1 | `compute_latent_shape()` + `sample()`, combined into one task |
| E | VAE | P25-E1 | The full VAE contract in a single task — the established second-module pattern |
| F | Proof | P25-F1 | The phase's Runnable Proof, with explicit confirmation of the zero-change claim |

---

## Prerequisites

The full ZiT/Qwen3/ZiT-VAE chain must work end to end per Phase 24 (P24-F1), since
this phase reuses the generic node layer, the dispatch mechanism, and `qwen3.py`
entirely unchanged.

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §20` roadmap note | P25-B2, P25-F1 | The explicit zero-generic-node-layer-change confirmation requirement |
| `ANVILML_DESIGN.md §11.3` | P25-B1, P25-C1, P25-C2 | The same four-step contract every prior arch module followed |
| `ANVILML_DESIGN.md §10.4` | P25-D1, P25-E1 | Fixed method names, identical across both architecture families |
| `ANVILML_DESIGN.md` Appendix B.1/B.2 | P25-F1 | Flux 2 Klein reuses Qwen3 4B for its text encoder — confirmed, not assumed |

---

## Task Descriptions

### Group A — Fixtures

#### P25-A1: worker/tests/fixtures/: Flux 2 Klein 4B + Flux 2 VAE fixture builders

**Goal:** Create the tiny synthetic checkpoints this phase's remaining tasks test
against, following the same convention as every prior fixture.

**Files to create or modify:**
- `worker/tests/fixtures/build_flux2klein_fixture.py`,
  `build_flux2_vae_fixture.py` — new builder scripts.
- Four generated `.safetensors` fixture files (4B + VAE, each with a no-metadata
  variant), committed.

**Key implementation notes:**
- Same discipline as every prior fixture: structurally valid for the respective
  shape-inference formula, never a miniaturized copy of real shapes.

**Acceptance criterion:**
```bash
# Both build scripts exit 0, all 4 files under 10MB combined, all load via
# safetensors.safe_open
```

---

### Group B — Diffusion shape inference & dispatch

#### P25-B1: worker/nodes/arch/diffusion/flux2klein.py: shape inference from header (4B)

**Goal:** Implement the contract's first step for this second diffusion module.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/flux2klein.py` — new; `_infer_hyperparams()`.

**Key implementation notes:**
- Same discipline as `zit.py` — reads every key, never a truncated sample.
- `can_handle()` and dispatch registration are explicitly deferred to the next
  task.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_flux2klein.py -v
# -> >=3 tests, exits 0
```

#### P25-B2: worker/nodes/arch/diffusion/flux2klein.py: can_handle() + dispatch (4B)

**Goal:** Connect `flux2klein.py` to the diffusion dispatch mechanism, and
confirm — for the first time — that two registered diffusion modules coexist
correctly without either's `can_handle()` accidentally matching the other's
fixture.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/flux2klein.py` — adds `can_handle()`.
- `worker/nodes/arch/diffusion/__init__.py` — registers `flux2klein.py` as its
  **second** real entry, alongside `zit.py`.

**Key implementation notes:**
- This is `ANVILML_DESIGN.md §20`'s explicit confirmation point: registering a
  second diffusion module requires zero changes to `loader.py`, `sampler.py`,
  `encoder.py`, `decode.py`, or `image.py`. If any change to those files seems
  necessary, that's a design defect to report, not something to silently patch.
- The cross-check is bidirectional: `flux2klein.py`'s `can_handle()` rejects
  `zit.py`'s fixture, **and** `zit.py`'s `can_handle()` rejects `flux2klein.py`'s
  fixture (a new test added to `test_arch_zit.py` too).

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_flux2klein.py -v
# -> >=7 tests total in the file, exits 0
```

---

### Group C — Diffusion construction & loading

#### P25-C1: worker/nodes/arch/diffusion/flux2klein.py: meta construction + dtype (4B)

**Goal:** Implement meta-device construction and dtype selection, identical
discipline to `zit.py`.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/flux2klein.py` — adds meta construction + dtype
  selection.

**Key implementation notes:**
- Uses `diffusers`'/`transformers`' layer/block classes per `§11.2`'s library
  boundary — never a hub-aware loader, restated for this second module the same as
  every prior one.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_flux2klein.py -v
# -> >=11 tests total in the file, exits 0
```

#### P25-C2: worker/nodes/arch/diffusion/flux2klein.py: key remap, load, .arch (4B)

**Goal:** Complete `load()` with the final contract steps, against a genuinely
different key namespace from `zit.py`'s.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/flux2klein.py` — completes `load()`.

**Key implementation notes:**
- The key remap table is built against **this module's own fixture** — never
  assumed from `zit.py`'s mapping, since Flux 2 Klein is a genuinely different
  model family.
- The same mandatory cast-before-`assign=True` ordering applies, restated again.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_flux2klein.py -v
# -> >=17 tests total in the file, exits 0
```

---

### Group D — Diffusion sampling

#### P25-D1: worker/nodes/arch/diffusion/flux2klein.py: sample() + compute_latent_shape (4B)

**Goal:** Complete `flux2klein.py` with its sampling contract, combined into one
task since the scope is smaller than `zit.py`'s was as the first arch module
written through this same contract.

**Files to create or modify:**
- `worker/nodes/arch/diffusion/flux2klein.py` — adds `compute_latent_shape()`,
  `sample()`.

**Key implementation notes:**
- `compute_latent_shape()` implements Flux 2 Klein's **own** patch-packing
  formula — architecture-specific, never reused from `zit.py`.
- `sample()`'s pipeline assembly/caching pattern matches `zit.py`'s exactly, since
  this part of the contract is genuinely shared in shape (though not in code) across
  every diffusion module.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_flux2klein.py -v
# -> >=25 tests total in the file, exits 0
```

---

### Group E — VAE

#### P25-E1: worker/nodes/arch/vae/flux2_vae.py: full load() + decode() (single task)

**Goal:** Implement the second VAE module in a single task — unlike `zit_vae.py`'s
careful multi-task introduction in Phase 23, the contract pattern is now
established and doesn't need the same step-by-step granularity for its second
instance.

**Files to create or modify:**
- `worker/nodes/arch/vae/flux2_vae.py` — new; the complete loading + decoding
  contract.

**Key implementation notes:**
- Registers as the **second** real entry in `arch/vae/__init__.py`'s dispatcher,
  alongside `zit_vae.py` — same disambiguation confirmation as the diffusion
  family's Group B.
- This task's larger scope (a full contract in one task, rather than split across
  five as Phase 23 did) is a deliberate sizing decision reflecting that the pattern
  no longer needs careful step-by-step introduction — not a relaxation of the
  underlying contract's requirements.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_arch_vae_flux2.py -v
# -> exits 0
```

---

### Group F — Proof

#### P25-F1: Runnable Proof: Flux 2 Klein 4B graph via generic nodes produces a real artifact

**Goal:** Produce this phase's Runnable Proof, explicitly confirming the
zero-generic-node-layer-change claim that motivated this phase's existence.

**Files to create or modify:**
- None. This task runs the already-built binary against the new fixtures; see
  Acceptance Criterion.

**Key implementation notes:**
- Reuses **exactly** the same generic node code path Phase 24 used for ZiT — the
  only things genuinely new in this submission are the `model_id` values pointing
  at Flux 2 Klein 4B and Flux 2 VAE fixtures instead of ZiT ones.
- Also confirms Flux 2 Klein's text encoder reuses Qwen3 4B (Phase 22) unchanged —
  no new CLIP module exists or is needed for this row of the model matrix.
- The implementation report must state explicitly whether any generic-node-layer
  file needed a change — and if so, flag it as a design defect rather than treating
  the change as routine.

**Acceptance criterion:**
```bash
# Full submit -> poll -> retrieve sequence against a live server in real mode,
# using the Appendix B.2 graph with Flux 2 Klein 4B / Flux 2 VAE fixture model_id
# values, producing a retrievable, valid PNG matching the requested dimensions.
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v
python -m pytest worker/tests -v -m real_mode

# Runnable Proof (manual): see P25-F1 — a Flux 2 Klein 4B + Flux 2 VAE generation
# graph, submitted through the exact same generic-node pipeline Phase 24 used for
# ZiT, produces a real, retrievable PNG artifact with zero changes to the generic
# node layer.
```

---

## Known Constraints and Gotchas

- **Zero changes to `loader.py`, `sampler.py`, `encoder.py`, `decode.py`, or
  `image.py` are expected in this phase.** If any task in this phase finds itself
  needing one, stop and report it as a design defect rather than silently making
  the change — this is the entire point of the phase.
- `flux2klein.py`'s key remap table must be built against its own fixture, never
  assumed from `zit.py`'s — despite both being diffusion modules, they are
  different model families with independent key namespaces.
- `flux2_vae.py`'s single-task scope (Group E) reflects the established pattern's
  maturity for a second instance, not a relaxation of the underlying four-step
  contract or the cast-before-`assign=True` dtype rule, both of which still apply in
  full.
- No new CLIP module exists for Flux 2 Klein — `qwen3.py` (Phase 22) serves both
  diffusion architectures' text-encoding needs, confirmed rather than assumed by
  this phase's proof.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 25 — Flux 2 Klein 4B Diffusion + Flux 2 VAE

**Capability proved:** A Flux 2 Klein 4B + Flux 2 VAE generation graph, submitted
through the exact same generic-node `POST /v1/jobs` pipeline Phase 24 used for ZiT,
produces a real, retrievable PNG artifact — confirming the generic node layer is
genuinely architecture-agnostic, with zero changes needed to add this second
diffusion architecture.

\`\`\`bash
# Runnable Proof (manual): identical to Phase 24's sequence, with model_id values
# pointing at the Flux 2 Klein 4B + Flux 2 VAE fixtures, reusing Qwen3 4B
# (Phase 22) for the text encoder unchanged.
cargo build --release -p anvilml
./target/release/anvilml &
sleep 2
DIFF_ID=$(sha256sum worker/tests/fixtures/flux2klein4b_tiny.safetensors | head -c1048576 | cut -d' ' -f1)
VAE_ID=$(sha256sum worker/tests/fixtures/flux2_vae_tiny.safetensors | head -c1048576 | cut -d' ' -f1)
CLIP_ID=$(sha256sum worker/tests/fixtures/qwen3_tiny.safetensors | head -c1048576 | cut -d' ' -f1)
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
# -> exits 0; a real, retrievable 64x64 PNG was produced via the unmodified
#    generic node pipeline, now serving a second diffusion architecture
kill %1
rm -f saved_proof.png
\`\`\`
```
