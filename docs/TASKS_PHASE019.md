# Tasks: Phase 019 — Flux 2 Klein Nodes

| Field | Value |
|-------|-------|
| Phase | 019 |
| Name | Flux 2 Klein Nodes |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 18 |

## Overview

Phase 019 adds Flux 2 Klein FP8 support by implementing `worker/nodes/arch/flux.py`. The generic node set from Phase 018 (`LoadModel`, `LoadVae`, `LoadClip`, `ClipTextEncode`, `Sampler`, `VaeDecode`, `SaveImage`) is reused without modification. Only the arch dispatch module is new.

Flux 2 Klein uses the Qwen3 8B FP8-mixed text encoder and a Flux-compatible VAE. The `LoadClip` node with `clip_type="qwen3"` loads the text encoder. The `Sampler` node dispatches to `flux.py` when `model.arch == "flux"`. The workflow JSON structure is identical to ZiT — only model IDs and `clip_type` change.

After Phase 019, both ZiT FP8 and Flux 2 Klein FP8 produce real PNG artifacts using the same generic node graph.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | worker arch | P19-A1 | arch/flux.py Flux 2 Klein dispatch |
| B | integration | P19-B1 | Parity test update + Flux smoke proof doc |

## Prerequisites

Phase 018 complete: all 9 baseline nodes implemented, parity test passing, `arch/` registry established.

## Task Descriptions

### Group A — Flux arch module

#### P19-A1: worker/nodes/arch/flux.py: Flux 2 Klein FP8 dispatch module

**Goal:** Implement `worker/nodes/arch/flux.py`:
- `can_handle(model_obj) -> bool` — returns True when `model_obj.arch == "flux"`
- `sample(model, conditioning, latent, steps, cfg, seed, device, cancel_flag, emit_progress) -> tuple[latent_tensor, int]`

Real path: use `diffusers.FluxPipeline` components (transformer, scheduler) in FP8 precision. Qwen3 conditioning is already encoded in `conditioning` before `sample()` is called — the arch module receives the conditioning tensor, not raw text. Per-step callback checks `cancel_flag.is_set()`. Every FP8 precision choice and Flux-specific behaviour has an inline comment.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 pytest worker/tests/test_arch_flux.py -v` exits 0 ≥ 3 tests; `can_handle` returns False for ZiT model object.

### Group B — Integration

#### P19-B1: parity test update + Flux smoke proof documentation

**Goal:** No new node types are added in Phase 019 — `test_parity.py` should still pass with the same 9 nodes. Create `docs/example_workflows/flux_klein_fp8.json` (same structure as `zit_fp8.json`, different model IDs and `clip_type: "qwen3"`). Create `docs/PROOF_phase019.md` documenting the Flux manual smoke proof.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 pytest worker/tests/test_parity.py -v` exits 0 (unchanged); `docs/PROOF_phase019.md` exists with complete commands.

## Phase Acceptance Criteria

```bash
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v
cargo test --workspace --features mock-hardware
# Real hardware proof (manual, requires Flux 2 Klein FP8 safetensors in models/):
# Submit flux_klein_fp8.json; verify Completed + PNG artifact
```

## Known Constraints and Gotchas

- The Flux conditioning tensor is a concatenation of CLIP-L and T5/Qwen3 embeddings in the original Flux architecture. If `LoadClip` with `clip_type="qwen3"` is used alone (as in our baseline workflow), the conditioning object must handle the single-encoder case. Document the design choice in an inline comment in `flux.py`.
- `can_handle()` must check `model_obj.arch == "flux"` not `isinstance()` to keep arch modules decoupled from specific class hierarchies.
