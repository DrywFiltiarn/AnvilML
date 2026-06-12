# Plan Report: P21-A5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P21-A5                                      |
| Phase       | 021 — Real Python Worker — ZiT              |
| Description | worker: nodes/zit.py real ZiT nodes + nodes/common.py SaveImage |
| Depends on  | P21-A4                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-12T21:55:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `worker/nodes/zit.py` with four real ZiT diffusers pipeline nodes (ZitLoadPipeline, ZitTextEncode, ZitSampler, ZitDecode) and `worker/nodes/common.py` with SaveImage, each implementing the exact INPUT_SLOTS / OUTPUT_SLOTS declared in `ANVILML_DESIGN.md §14.6` and mirrored in `crates/anvilml-scheduler/src/nodes.rs`. Preserve `ANVILML_WORKER_MOCK=1` sentinel paths so CI stays hermetic.

## Scope

### In Scope
- `worker/nodes/zit.py`: four node classes (`ZitLoadPipeline`, `ZitTextEncode`, `ZitSampler`, `ZitDecode`) with real diffusers integration and mock sentinel fallbacks.
- `worker/nodes/common.py`: `SaveImage` node that encodes a PIL Image to PNG, emits `ImageReady` via `ctx.emit_fn` with `ctx.job_id`.
- `worker/tests/test_nodes_zit.py`: pytest file testing mock-mode output slots and SaveImage ImageReady emission.
- Auto-import of `zit` and `common` modules via `nodes/__init__.py` (already handles `pkgutil.iter_modules`, so no code change needed — both new `.py` files will self-register on import).

### Out of Scope
- SDXL nodes (reserved for Phase 22).
- Rust-side changes (KNOWN_NODE_TYPES already declared in `nodes.rs`; parity test P21-A6 handles it).
- Executor changes (already handles SaveImage fallback in mock path).
- Logging additions beyond what is necessary for the new code paths (per §11.1, every non-trivial code path gets DEBUG logging).

## Approach

1. **Create `worker/nodes/zit.py`** with four `@register`-decorated node classes:

   **ZitLoadPipeline** (`INPUT_SLOTS=["model_id"]`, `OUTPUT_SLOTS=["pipeline"]`):
   - Mock: returns sentinel `{"pipeline": "zit_pipeline_mock"}`.
   - Real: uses `diffusers.ZitsPipeline.from_pretrained(model_id, torch_dtype=torch.bfloat16)` via `pipeline_cache.get_or_load()`, returns `{"pipeline": pipeline}`.

   **ZitTextEncode** (`INPUT_SLOTS=["pipeline", "prompt"]`, `OUTPUT_SLOTS=["conditioning"]`):
   - Mock: returns `{"conditioning": "zit_cond_mock"}`.
   - Real: calls `pipeline(text=prompt, ...)` returning the conditioning tensor pair `(embeds, pooled)`.

   **ZitSampler** (`INPUT_SLOTS=["pipeline", "conditioning", "steps", "seed"]`, `OUTPUT_SLOTS=["latents", "seed"]`):
   - Mock: returns `{"latents": "zit_latents_mock", "seed": -1}`.
   - Real: resolves `seed=-1` to `random.randint(0, 2**63-1)`; sets `generator = torch.Generator(device=device_str).manual_seed(seed)`; calls `pipeline(prompt_embeds=conditioning[0], ...)` with `callback_on_step_end` that checks `ctx.cancel_flag.is_set()` and raises `CancelledError`; returns `{"latents": latents, "seed": actual_seed}`.

   **ZitDecode** (`INPUT_SLOTS=["pipeline", "latents"]`, `OUTPUT_SLOTS=["image"]`):
   - Mock: returns `{"image": "zit_image_mock"}`.
   - Real: calls `pipeline.vae.decode(latents / 0.1842, ...)` → `Image.from_tensor()` → PIL `Image`; returns `{"image": pil_image}`.

2. **Create `worker/nodes/common.py`** with one node class:

   **SaveImage** (`INPUT_SLOTS=["image", "prompt", "seed", "steps"]`, `OUTPUT_SLOTS=[]`):
   - Mock: encodes a 64×64 black PNG, emits `ImageReady` with `ctx.job_id` (same as executor fallback).
   - Real: converts PIL image to PNG bytes via `BytesIO`, base64-encodes, emits `ImageReady { job_id, image_b64, width, height, format: "png", seed, steps, prompt }`.

3. **Create `worker/tests/test_nodes_zit.py`**:
   - Mock-mode tests for each of the four ZiT nodes verifying output slot names match declarations.
   - SaveImage test verifying `ImageReady` event emission with correct fields.
   - All tests use `ANVILML_WORKER_MOCK=1` sentinel paths (torch/diffusers absent).

4. **Verify auto-import**: `nodes/__init__.py` uses `pkgutil.iter_modules` to import every `.py` in the directory. Since `zit.py` and `common.py` use `@register`, they will self-register on import. No code change needed to `__init__.py`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `worker/nodes/zit.py` | Four ZiT node classes with real diffusers + mock sentinel paths |
| Create | `worker/nodes/common.py` | SaveImage node with PNG encode + ImageReady emission |
| Create | `worker/tests/test_nodes_zit.py` | pytest: mock output slots correct; SaveImage emits ImageReady |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `worker/tests/test_nodes_zit.py` | `test_zit_load_pipeline_output_slots` | ZitLoadPipeline returns `{"pipeline": ...}` (mock sentinel) |
| `worker/tests/test_nodes_zit.py` | `test_zit_text_encode_output_slots` | ZitTextEncode returns `{"conditioning": ...}` (mock sentinel) |
| `worker/tests/test_nodes_zit.py` | `test_zit_sampler_output_slots` | ZitSampler returns `{"latents": ..., "seed": ...}` (mock sentinel) |
| `worker/tests/test_nodes_zit.py` | `test_zit_decode_output_slots` | ZitDecode returns `{"image": ...}` (mock sentinel) |
| `worker/tests/test_nodes_zit.py` | `test_saveimage_emits_imageready` | SaveImage encodes PNG, emits ImageReady with job_id, width, height, format, seed, steps, prompt |

## CI Impact

No CI workflow file changes. The new pytest file is picked up automatically by `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v`. The Rust `KNOWN_NODE_TYPES` already includes all four ZiT types + SaveImage (9 types in `nodes.rs`), so parity test P21-A6 will pass once these nodes register.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| diffusers API changes between versions | Low | Medium | Pin diffusers>=0.27 in `base.txt`; mock mode never imports diffusers so CI is unaffected |
| `ZitsPipeline` not available in older diffusers | Low | Medium | Guard with `try/except ImportError`; fall back to mock sentinel on import failure |
| CancelledError not caught by executor | Medium | High | Executor already catches `CancelledError` at two levels (per-node and outer try); verified in `test_executor.py` cancel tests |
| SaveImage b64 exceeds 64 MiB IPC cap | Low | Medium | 1024×1024 PNG b64 ≈ 1.4 MiB, well under 64 MiB cap; no mitigation needed for MVP |

## Acceptance Criteria

- [ ] `worker/nodes/zit.py` exists with four `@register`-decorated node classes matching slots from `nodes.rs`
- [ ] `worker/nodes/common.py` exists with `SaveImage` node that emits `ImageReady`
- [ ] `worker/tests/test_nodes_zit.py` exists and `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_nodes_zit.py -v` exits 0
- [ ] Output slots for all four ZiT nodes match declarations exactly (no extra, no missing keys)
- [ ] SaveImage test verifies ImageReady event contains `job_id`, `image_b64`, `width`, `height`, `format`, `seed`, `steps`, `prompt`
- [ ] Mock branches (ANVILML_WORKER_MOCK=1) return sentinels/black image without importing torch/diffusers
