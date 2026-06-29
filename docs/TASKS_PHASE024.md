# Tasks: Phase 24 — Generic Conditioning/Sampling/Decode Nodes, Real Mode

**Phase:** 24
**Name:** Generic Conditioning/Sampling/Decode Nodes, Real Mode
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 9, 10, 14, 15, 19, 20, 21, 22, 23

---

## Overview

This phase gives every remaining generic node a real branch — `ClipTextEncode`,
`VaeDecode`, `EmptyLatent`, `SaveImage`, `ImageResize` — now that ZiT, Qwen3, and
the ZiT VAE all exist to dispatch to. It closes with the first full real (non-mock,
non-`PassThrough`) generation job submitted through the actual HTTP API and
dispatch pipeline, completing "ZiT Diffusion + Qwen3 CLIP + ZiT VAE" as a fully
closed roadmap group.

This phase exists last in this architecture-loading arc because every node it
completes needed a real arch module to dispatch to before its real branch could be
anything but another deferred-raise placeholder — `ClipTextEncode` needed Qwen3
(Phase 22), `VaeDecode` needed the ZiT VAE (Phase 23), `EmptyLatent` needed
`compute_latent_shape()` (Phase 21), `SaveImage` needed a real `PIL.Image` to encode
(downstream of all three). This phase's final task is also deliberately distinct
from Phase 23's `P23-F1` proof: that phase chained the underlying arch modules
directly; this phase chains the **generic node layer** — the actual
`POST /v1/jobs` → dispatch → execute → artifact pipeline every real job will use.

At the start of this phase, only `Sampler` (Phase 21) and the loader nodes have
real branches; `ClipTextEncode`, `VaeDecode`, `EmptyLatent`, `SaveImage`, and
`ImageResize` are either nonexistent or mock-only. At the end: every generic node in
the MVP baseline set has a working real branch, and submitting
`ANVILML_DESIGN.md` Appendix B.2's exact example graph via `POST /v1/jobs` against
the fixture checkpoints produces a real, retrievable PNG artifact — end to end,
through the real dispatch pipeline, not a direct arch-module test.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | ClipTextEncode | P24-A1 … P24-A2 | Mock branch, then real tokenize+encode |
| B | VaeDecode | P24-B1 … P24-B2 | Mock branch, then real dispatch to the VAE module |
| C | EmptyLatent | P24-C1 … P24-C2 | Mock branch, then real `compute_latent_shape()` dispatch |
| D | SaveImage & ImageResize | P24-D1 … P24-D3 | `SaveImage` mock, then real PNG encode + `ImageReady`; `ImageResize` |
| E | Full graph integration | P24-E1 | The complete generic-node graph through real dispatch |
| F | Proof | P24-F1 | The first end-to-end real generation Runnable Proof |

---

## Prerequisites

`LoadClip`/`LoadModel`/`LoadVae`'s real branches must all exist per Phases 20, 22,
23. `Sampler`'s real branch and `compute_latent_shape()` must exist per Phase 21.
`zit_vae.py`'s `decode()` must exist per Phase 23 (P23-D1). The dispatch/event-loop
pipeline from Phases 14–16 must be fully functional.

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §10.3` | All node tasks | Exact slot shapes for every node in this phase |
| `ANVILML_DESIGN.md §10.4` | P24-B2, P24-C2 | Dispatch via `arch.vae.get_module()`/`arch.diffusion.get_module()` — generic nodes never reimplement arch-specific logic |
| `ANVILML_DESIGN.md §14.6` | P24-D1 | Mock `SaveImage`'s exact 64×64 black PNG spec |
| `ANVILML_DESIGN.md` Appendix B.2 | P24-E1, P24-F1 | The exact example graph this phase's integration proof submits |
| `ANVILML_DESIGN.md §10.6` | All real-branch tasks | Both markers required, both pointing at genuinely passing tests |

---

## Task Descriptions

### Group A — ClipTextEncode

#### P24-A1: worker/nodes/encoder.py: ClipTextEncode node, mock branch only

**Goal:** Create the generic text-conditioning node with its mock branch working,
before the real branch (needing a real loaded encoder) is added.

**Files to create or modify:**
- `worker/nodes/encoder.py` — new; `ClipTextEncode`, mock branch only.

**Key implementation notes:**
- Architecture-agnostic by design — no per-architecture dispatch lives in this
  node; the `clip` input object already carries its own tokenizer and model from
  `LoadClip`.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_nodes_encoder.py -v
# -> >=3 tests, exits 0
```

#### P24-A2: worker/nodes/encoder.py: ClipTextEncode real branch tokenizes + encodes

**Goal:** Complete `ClipTextEncode` with real tokenization and encoding, calling
the already-loaded encoder object directly.

**Files to create or modify:**
- `worker/nodes/encoder.py` — completes the real branch.

**Key implementation notes:**
- This node does **not** dispatch through `arch.clip.get_module()` again — the
  `clip` input already **is** the fully-loaded module `LoadClip`'s real branch
  (Phase 22) returned. Re-dispatching here would be redundant and risks resolving a
  different module than the one actually loaded.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_nodes_encoder.py -v -m real_mode
# -> >=8 tests total in the file, exits 0
```

---

### Group B — VaeDecode

#### P24-B1: worker/nodes/decode.py: VaeDecode node, mock branch only

**Goal:** Create the generic VAE-decode node's mock branch.

**Files to create or modify:**
- `worker/nodes/decode.py` — new; `VaeDecode`, mock branch only.

**Key implementation notes:**
- Unlike `Sampler` (Phase 21) and `zit_vae.py`'s `decode()` (Phase 23) — which had
  no groundwork gap to bridge — this node's mock/real split across two tasks is
  purely for keeping each task's `context` field within the authoring spec's size
  limit, not a design distinction.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_nodes_decode.py -v
# -> >=3 tests, exits 0
```

#### P24-B2: worker/nodes/decode.py: VaeDecode real branch dispatches to vae module

**Goal:** Complete `VaeDecode` with real dispatch to the loaded VAE's `decode()`.

**Files to create or modify:**
- `worker/nodes/decode.py` — completes the real branch.

**Key implementation notes:**
- Dispatches via `arch.vae.get_module(vae.arch).decode(...)` — the `vae` input is
  already the fully-loaded module from `LoadVae`'s real branch (Phase 23).

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_nodes_decode.py -v -m real_mode
# -> >=8 tests total in the file, exits 0
```

---

### Group C — EmptyLatent

#### P24-C1: worker/nodes/loader.py: EmptyLatent node, mock branch only

**Goal:** Create `EmptyLatent`'s mock branch, which correctly ignores the
optional `model` input entirely.

**Files to create or modify:**
- `worker/nodes/loader.py` — adds `EmptyLatent`, mock branch only.

**Key implementation notes:**
- Co-located in `loader.py` per `ANVILML_DESIGN.md §10.3`'s `Loaders` category
  grouping, even though this node creates a latent rather than loading a model.
- Mock mode ignores `model` entirely, per the design doc's explicit note — this is
  correct behavior, not an oversight to "fix" by making mock mode also dispatch.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_nodes_loader.py -v
# -> >=3 tests, exits 0
```

#### P24-C2: worker/nodes/loader.py: EmptyLatent real branch via compute_latent_shape

**Goal:** Complete `EmptyLatent` with real dispatch to the loaded model's
`compute_latent_shape()`.

**Files to create or modify:**
- `worker/nodes/loader.py` — completes `EmptyLatent`'s real branch.

**Key implementation notes:**
- `model` is **required** in real mode — its absence raises a clear error, per
  `ANVILML_DESIGN.md §10.3`'s explicit "required in real mode" note. This is the
  opposite of mock mode's behavior, and both are correct for their respective mode.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_nodes_loader.py -v -m real_mode
# -> exits 0
```

---

### Group D — SaveImage & ImageResize

#### P24-D1: worker/nodes/image.py: SaveImage node, mock branch only

**Goal:** Create the output node's mock branch, matching the design doc's exact
mock-mode specification.

**Files to create or modify:**
- `worker/nodes/image.py` — new; `SaveImage`, mock branch only.

**Key implementation notes:**
- The mock branch emits `ImageReady` with a 64×64 black PNG — `ANVILML_DESIGN.md
  §14.6`'s exact specified value, not an arbitrary placeholder size.
- `SaveImage` has no output slots — it emits an event instead, per
  `ANVILML_DESIGN.md §10.3`'s table.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_nodes_image.py -v
# -> >=3 tests, exits 0
```

#### P24-D2: worker/nodes/image.py: SaveImage real branch encodes PNG, emits ImageReady

**Goal:** Complete `SaveImage` with real PNG encoding and event emission.

**Files to create or modify:**
- `worker/nodes/image.py` — completes `SaveImage`'s real branch.

**Key implementation notes:**
- This node only **emits the event** — actual artifact persistence happens
  Rust-side in `event_loop.rs` (Phase 15) on receipt. `SaveImage` itself never
  writes to disk or to the artifact store directly.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_nodes_image.py -v -m real_mode
# -> >=8 tests total in the file, exits 0
```

#### P24-D3: worker/nodes/image.py: ImageResize node, mock + real (lanczos default)

**Goal:** Complete the image node set with resizing — a trivial PIL operation
needing no actual mock/real behavioral split.

**Files to create or modify:**
- `worker/nodes/image.py` — adds `ImageResize`.

**Key implementation notes:**
- Both branches can call the **same** real PIL resize — resizing has no GPU/model
  dependency to mock around. Only the `ctx.mock` branching structure itself is
  required per `ANVILML_DESIGN.md §14.6`'s general node pattern, even though the
  underlying behavior doesn't differ.
- `method` defaults to `"lanczos"` per the design doc's exact wording.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_nodes_image.py -v -m real_mode
# -> >=13 tests total in the file, exits 0
```

---

### Group E — Full graph integration

#### P24-E1: anvilml-server: real end-to-end ZiT generation graph via POST /v1/jobs

**Goal:** Verify the complete generic-node graph — every node built across
Phases 19–24 — executes correctly end to end through the real dispatch pipeline,
not a direct arch-module test.

**Files to create or modify:**
- New integration test file (language to be confirmed as more natural for this
  specific cross-language proof — Rust or Python).

**Key implementation notes:**
- Submits exactly `ANVILML_DESIGN.md` Appendix B.2's example graph
  (`LoadModel`+`LoadVae`+`LoadClip`+`EmptyLatent`+`ClipTextEncode`+`Sampler`+
  `VaeDecode`+`SaveImage`) against the fixture checkpoints.
- This is the moment "ZiT Diffusion + Qwen3 CLIP + ZiT VAE" closes as a fully
  completed roadmap group — distinct from Phase 23's `P23-F1`, which proved the
  underlying arch modules compose correctly but bypassed the generic node layer
  entirely.

**Acceptance criterion:**
```bash
# Integration test suite (language TBD) asserting the full graph reaches
# Completed with a real, retrievable artifact.
# -> exits 0
```

---

### Group F — Proof

#### P24-F1: Runnable Proof: full graph submitted via POST /v1/jobs produces a real artifact

**Goal:** Produce this phase's Runnable Proof — the first end-to-end real
generation job in the project, closing the gap Phase 14's `PassThrough`-only proof
deliberately left open.

**Files to create or modify:**
- None. This task runs the already-built binary against real fixture-registered
  models; see Acceptance Criterion.

**Key implementation notes:**
- Run in **real** mode (not `mock-hardware`) — this is the first Runnable Proof in
  the project that doesn't use the mock feature flag, since it specifically proves
  real dispatch against real (if tiny, synthetic) checkpoints.
- The retrieved artifact's dimensions are checked against the requested
  width/height — the same end-to-end shape-contract verification Phase 23's `P23-F1`
  did directly, now confirmed through the full HTTP/dispatch pipeline.

**Acceptance criterion:**
```bash
# Full submit -> poll -> retrieve sequence against a live server in real mode:
# POST /v1/jobs with the Appendix B.2 graph (fixture model_id values)
# poll GET /v1/jobs/:id until Completed
# GET /v1/artifacts/:hash returns a real, valid PNG matching the requested dimensions
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v
python -m pytest worker/tests -v -m real_mode

# Runnable Proof (manual): see P24-F1 — a full ZiT/Qwen3/ZiT-VAE generation graph
# submitted via POST /v1/jobs against fixture checkpoints completes and produces a
# real, retrievable PNG artifact through the actual dispatch pipeline.
```

---

## Known Constraints and Gotchas

- `ClipTextEncode` and `VaeDecode` never re-dispatch through `arch.clip.get_module()`
  / `arch.vae.get_module()` on their own — their `clip`/`vae` inputs are already
  fully-loaded modules; re-dispatching risks resolving a different module than the
  one actually loaded for this job.
- `EmptyLatent`'s `model` input requirement flips between mock (ignored) and real
  (required) — both are correct for their respective mode, not an inconsistency.
- `SaveImage` never writes to the artifact store directly — it only emits
  `ImageReady`; persistence is entirely Rust-side, in `event_loop.rs` (Phase 15).
- `P24-E1`'s integration proof is distinct from Phase 23's `P23-F1` — this phase's
  proof specifically exercises the generic node layer and the real dispatch
  pipeline, not a direct arch-module chain.
- This phase's Runnable Proof (`P24-F1`) runs without the `mock-hardware` feature
  flag — confirm the real, non-mock detection and dispatch path is what's actually
  exercised, not an accidentally-still-mocked component.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 24 — Generic Conditioning/Sampling/Decode Nodes, Real Mode

**Capability proved:** The first end-to-end real (non-mock, non-`PassThrough`)
generation job in the project — a full ZiT/Qwen3/ZiT-VAE graph submitted via
`POST /v1/jobs`, dispatched through the real pipeline, producing a real, retrievable
PNG artifact matching the requested dimensions. This closes "ZiT Diffusion + Qwen3
CLIP + ZiT VAE" as a fully completed roadmap group.

\`\`\`bash
# Runnable Proof (manual): real (non-mock-hardware) mode, fixture checkpoints
# already registered in the model registry under their SHA256 ids.
cargo build --release -p anvilml
./target/release/anvilml &
sleep 2
ZIT_ID=$(sha256sum worker/tests/fixtures/zit_tiny.safetensors | head -c1048576 | cut -d' ' -f1)
VAE_ID=$(sha256sum worker/tests/fixtures/zit_vae_tiny.safetensors | head -c1048576 | cut -d' ' -f1)
CLIP_ID=$(sha256sum worker/tests/fixtures/qwen3_tiny.safetensors | head -c1048576 | cut -d' ' -f1)
JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d "{\"graph\":{\"nodes\":[
    {\"id\":\"model\",\"type\":\"LoadModel\",\"inputs\":{\"model_id\":\"$ZIT_ID\"}},
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
# -> exits 0; a real, retrievable 64x64 PNG was produced
kill %1
rm -f saved_proof.png
\`\`\`
```
