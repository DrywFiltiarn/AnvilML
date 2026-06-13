# Plan Report: P21-A7

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P21-A7                                            |
| Phase       | 021 — Real Python Worker — ZiT                    |
| Description | anvilml: real ZiT end-to-end smoke proof (manual, real hardware) |
| Depends on  | P21-A6                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-06-13T01:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Document and execute a manual end-to-end smoke proof that a real ZiT (Zero-Iteration) image generation pipeline runs on real hardware — from venv provisioning through job submission, WebSocket progress streaming, and artifact retrieval — producing a genuine generated PNG image (not a black placeholder). No new source code is written; the deliverable is a proof document at `docs/PROOF_phase021.md`.

## Scope

### In Scope
- Document the full manual smoke-proof procedure in `docs/PROOF_phase021.md`
- Steps include: venv provisioning, placing a real ZiT model, starting the binary without mock, submitting a ZiT job graph via curl, observing WebSocket progress events, fetching the produced PNG artifact, and verifying it is a real generated image
- Include troubleshooting guidance and human-verification checklist
- Reference the existing `valid_zit_job.json` job payload (5-node ZiT DAG)
- Note the expected WebSocket event sequence: `job.queued → job.started → job.progress (×5) → job.image_ready → job.completed`
- Note the expected ZiT defaults: 8 steps, guidance_scale=0.0, 1024×1024, bf16

### Out of Scope
- Writing any new source code, tests, config files, or CI changes
- Creating the `install_worker_deps.sh` script (Phase 21 tasks A1–A6 handle the worker code; provisioning script creation is out of scope for this proof task)
- Automating the proof (it is manual by design)
- SDXL pipeline verification (that is a separate proof)
- CI integration — this proof is manual, real-hardware only

## Approach

1. **Verify prerequisites exist.** Confirm that phases P21-A1 through P21-A6 are complete: the worker code (`worker/nodes/base.py`, `worker/executor.py`, `worker/pipeline_cache.py`, `worker/defaults.py`, `worker/nodes/zit.py`, `worker/nodes/common.py`), the parity test (`backend/tests/known_node_types.json` + `worker/tests/test_parity.py`), and that `cargo test` and `pytest` pass in mock mode.

2. **Provision the Python venv.** Run `bash backend/scripts/install_worker_deps.sh` (or manually create a venv with `python3.12 -m venv ./venv` and install dependencies from `worker/requirements/base.txt` plus the appropriate backend requirements file — `cuda.txt`, `rocm-linux.txt`, or `cpu.txt` depending on hardware). The venv must contain `torch`, `diffusers`, `transformers`, `pillow`, `msgpack`, `numpy`, and `safetensors`. Verify with `./venv/bin/python3 -c "import torch; import diffusers; print('OK')"`.

3. **Place a real ZiT model.** Download a real ZiT model (e.g. `stabilityai/zits` or a compatible distilled/turbo model) into `models/diffusion/` as a `.safetensors` file, or configure the job to reference a HuggingFace Hub model ID (the `ZitLoadPipeline` node accepts `model_id` strings that resolve via `ZitsPipeline.from_pretrained()`). The existing `model-fp16.safetensors` in `models/diffusion/` may be used if it is a real ZiT-compatible model; otherwise the proof document will note the need to place a real model file or use a Hub ID.

4. **Start the server without mock.** Run `ANVILML_VENV_PATH=./venv cargo run --release` (or `cargo run --features mock-hardware` is NOT used — this is real hardware). Ensure `ANVILML_WORKER_MOCK` is **unset** (not set to `1`). The server binds on `127.0.0.1:8488` and spawns the real Python worker.

5. **Submit a ZiT job.** Use the existing `valid_zit_job.json` (5-node DAG: ZitLoadPipeline → ZitTextEncode → ZitSampler → ZitDecode → SaveImage) via curl:
   ```bash
   curl -s -X POST http://127.0.0.1:8488/v1/jobs \
     -H 'content-type: application/json' \
     -d @valid_zit_job.json
   ```

6. **Observe progress over WebSocket.** Connect to `ws://127.0.0.1:8488/v1/events` (using `websocat` or similar) and watch for the event sequence: `job.queued`, `job.started`, 5× `job.progress` (one per DAG node), `job.image_ready` (with artifact hash), `job.completed`.

7. **Fetch the artifact.** Extract the `artifact_hash` from the `job.image_ready` event and download the PNG:
   ```bash
   curl -s -o real.png http://127.0.0.1:8488/v1/artifacts/<hash>
   ```

8. **Verify the image.** Confirm `real.png` is a valid PNG file with non-trivial content (not a 64×64 black image). Check file size > 0, dimensions 1024×1024 (or the configured resolution), and that pixel data varies (not all black). A human visually inspects the image to confirm it is a genuine generated image.

9. **Document results.** Record the proof document at `docs/PROOF_phase021.md` with the full command sequence, expected outputs, actual outputs, and the verification conclusion.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `docs/PROOF_phase021.md` | Runnable proof document for real ZiT end-to-end smoke test |

No source code, test, config, or CI files are modified. No crate version bumps are needed.

## Tests

None. This task produces no test files. The existing parity test (P21-A6) and the mock-mode pytest suite serve as the hermetic CI proof; this proof is manual and real-hardware only.

## CI Impact

No CI changes required. The existing CI gates (`cargo test --workspace --features mock-hardware`, `ANVILML_WORKER_MOCK=1 pytest worker/tests/ -v`) remain unchanged. This proof is a manual, real-hardware exercise that does not affect CI.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `install_worker_deps.sh` does not exist yet (Phase 21 may not have created it) | Medium | Medium | The proof document will include a manual venv provisioning alternative: `python3.12 -m venv ./venv && ./venv/bin/pip install -r worker/requirements/base.txt && ./venv/bin/pip install -r worker/requirements/<backend>.txt` |
| Real ZiT model download is large (several GB) and slow | Medium | Low | Use a small/distilled ZiT model; document the expected download time; allow the user to pre-download |
| GPU VRAM insufficient for 1024×1024 bf16 ZiT inference | Low | High | Document VRAM requirements (~4–6 GB minimum); suggest reducing resolution if needed; fall back to CPU (slow but functional) |
| Worker fails to connect or crashes on startup | Low | Medium | Check `GET /v1/system/env` for provisioning status; inspect worker logs in `./logs/`; verify `import torch` works in the venv |
| The generated image is all-black (model misconfiguration) | Low | Medium | Verify the model is a real ZiT-compatible model (not SDXL); check that `ZitsPipeline` loads correctly; confirm `guidance_scale=0.0` (ZiT is CFG-free) |
| Artifact hash mismatch between WebSocket event and REST API | Low | Low | Use the exact `artifact_hash` from the `job.image_ready` event; if REST returns 404, wait a moment and retry (artifact write is async) |

## Acceptance Criteria

- [ ] `docs/PROOF_phase021.md` exists and contains the full manual smoke-proof procedure with commands, expected outputs, and verification steps
- [ ] The venv is provisioned with torch, diffusers (with `ZitsPipeline`), pillow, msgpack, and safetensors
- [ ] A real ZiT model is placed in `models/diffusion/` or referenced as a Hub model ID
- [ ] The server is started with `ANVILML_WORKER_MOCK` unset (real hardware path)
- [ ] A ZiT job is submitted via curl using the 5-node DAG graph
- [ ] WebSocket events show the full lifecycle: `job.queued → job.started → job.progress (×5) → job.image_ready → job.completed`
- [ ] The artifact PNG is fetched via `GET /v1/artifacts/:hash` and saved as `real.png`
- [ ] `real.png` is verified as a non-trivial generated image (not 64×64 black, not empty)
- [ ] A human has visually confirmed the generated image is real end-to-end
