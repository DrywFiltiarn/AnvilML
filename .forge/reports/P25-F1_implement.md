# Implementation Report: P25-F1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P25-F1                          |
| Phase         | 25 — Flux 2 Klein 4B Diffusion + Flux 2 VAE |
| Description   | Runnable Proof: Flux 2 Klein 4B graph via generic nodes produces a real artifact |
| Implemented   | 2026-07-23T14:20:00Z            |
| Status        | BLOCKED                         |

## Summary

Executed the Runnable Proof for Flux 2 Klein 4B: built the release binary, started the server in real mode, computed fixture hashes, submitted the Appendix B.2 generation graph via POST /v1/jobs, and polled the job. The job was dispatched but failed at the LoadModel node because the generic node layer hardcodes `"zit"` as the dispatch key (`get_module("zit")` in `worker/nodes/loader.py`), which routes to the ZiT module's `load()` function. The ZiT module's `_infer_hyperparams_inner()` looks for ZiT-specific keys (`input_proj.weight`, `time_text_emb.weight`, `c_crossattn_dim`) that do not exist in the Flux 2 Klein checkpoint, causing a `ValueError`. The proof cannot succeed without modifying the generic node layer to perform architecture-aware dispatch — i.e., reading the architecture string from the safetensors checkpoint header (or the filesystem path) and passing it to `get_module()`. This is a design defect in the generic node layer, not a bug in the Flux 2 Klein module itself.

## Resolved Dependencies

None. This task introduces no new dependencies.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | anvilml.toml | Uncommented `[[model_dirs]]` entries (`./models/diffusion`, `./models/text_encoders`, `./models/vae`) — required for the model scanner to discover fixtures and populate the model registry. Without this, `resolve_model_ids()` would find no entries and fail the job with `UnknownModelId`. This was a known gap (ADDENDUM_P903) that was manually fixed at runtime in prior phases but never committed. |
| Create | models/diffusion/flux2klein4b_tiny.safetensors | Copied fixture to model directory for scanner discovery. |
| Create | models/vae/flux2_vae_tiny.safetensors | Copied fixture to model directory for scanner discovery. |

Note: `models/text_encoders/qwen3_tiny.safetensors` already existed as a symlink from a prior phase.

## Commit Log

```
 anvilml.toml                                    | 18 +++++++++---------
 models/diffusion/flux2klein4b_tiny.safetensors  | Bin 0 -> 3886464 bytes
 models/vae/flux2_vae_tiny.safetensors           | Bin 0 -> 7016 bytes
 3 files changed, 9 insertions(+), 9 deletions(-)
```

## Runnable Proof Transcript

### Step 1: Build release binary
```
$ cd /home/dryw/AnvilML && cargo build --release -p anvilml 2>&1 | tail -5
Finished `release` profile [optimized] target(target(1.19s)
```

### Step 2: Start server in real mode
```
$ ./target/release/anvilml > /tmp/anvilml_server.log 2>&1 &
[1] 2968290
$ sleep 3 && curl -s http://127.0.0.1:8488/health
{"status":"ok","version":"0.1.28","uptime_s":12}
```

### Step 3: Verify model registry
```
$ curl -s http://127.0.0.1:8488/v1/models | python3 -m json.tool | head -40
[
    {
        "id": "e037f5ee838139d01fbc06db3caf13d9d088a62483dba92f22a57d080f6c3a56",
        "name": "zit_tiny.safetensors",
        "path": "./models/diffusion/zit_tiny.safetensors",
        "kind": "diffusion",
        ...
    },
    {
        "id": "8eecf4c7f5fba8ce0cf61da61ba4ae588ef0a3649075e8fa344b68163561eb58",
        "name": "flux2klein4b_tiny.safetensors",
        "path": "./models/diffusion/flux2klein4b_tiny.safetensors",
        "kind": "diffusion",
        ...
    },
    {
        "id": "069211c06b13a5989965f30308669e721d44be31a87a7c1236f4dc4e531fe321",
        "name": "qwen3_tiny.safetensors",
        "path": "./models/text_encoders/qwen3_tiny.safetensors",
        "kind": "text_encoder",
        ...
    },
    {
        "id": "2e71f7dd351bb13618a96937a32b00d69a5d4e8dfa533f5253f03ad38a1d0a2e",
        "name": "flux2_vae_tiny.safetensors",
        "path": "./models/vae/flux2_vae_tiny.safetensors",
        "kind": "vae",
        ...
    }
]
```

### Step 4: Compute fixture hashes (first 1 MiB SHA256)
```
$ DIFF_ID=$(head -c1048576 models/diffusion/flux2klein4b_tiny.safetensors | sha256sum | cut -d' ' -f1)
$ VAE_ID=$(head -c1048576 models/vae/flux2_vae_tiny.safetensors | sha256sum | cut -d' ' -f1)
$ CLIP_ID=$(head -c1048576 models/text_encoders/qwen3_tiny.safetensors | sha256sum | cut -d' ' -f1)
$ echo "DIFF_ID=$DIFF_ID"
DIFF_ID=8eecf4c7f5fba8ce0cf61da61ba4ae588ef0a3649075e8fa344b68163561eb58
$ echo "VAE_ID=$VAE_ID"
VAE_ID=2e71f7dd351bb13618a96937a32b00d69a5d4e8dfa533f5253f03ad38a1d0a2e
$ echo "CLIP_ID=$CLIP_ID"
CLIP_ID=069211c06b13a5989965f30308669e721d44be31a87a7c1236f4dc4e531fe321
```

### Step 5: Submit the Flux 2 Klein 4B generation graph
```
$ curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d '{"graph":{"nodes":[
    {"id":"model","type":"LoadModel","inputs":{"model_id":"8eecf4c7f5fba8ce0cf61da61ba4ae588ef0a3649075e8fa344b68163561eb58"}},
    {"id":"vae","type":"LoadVae","inputs":{"model_id":"2e71f7dd351bb13618a96937a32b00d69a5d4e8dfa533f5253f03ad38a1d0a2e"}},
    {"id":"encoder","type":"LoadClip","inputs":{"model_id":"069211c06b13a5989965f30308669e721d44be31a87a7c1236f4dc4e531fe321","clip_type":"qwen3"}},
    {"id":"latent","type":"EmptyLatent","inputs":{"width":64,"height":64,"model":{"node_id":"model","output_slot":"model"}}},
    {"id":"cond","type":"ClipTextEncode","inputs":{"clip":{"node_id":"encoder","output_slot":"clip"},"positive_text":"a photograph of a red fox in a snowy forest"}},
    {"id":"sampled","type":"Sampler","inputs":{"model":{"node_id":"model","output_slot":"model"},"conditioning":{"node_id":"cond","output_slot":"conditioning"},"clip":{"node_id":"encoder","output_slot":"clip"},"latent":{"node_id":"latent","output_slot":"latent"},"steps":4,"cfg":1.0,"seed":-1}},
    {"id":"decoded","type":"VaeDecode","inputs":{"vae":{"node_id":"vae","output_slot":"vae"},"latent":{"node_id":"sampled","output_slot":"latent"}}},
    {"id":"saved","type":"SaveImage","inputs":{"image":{"node_id":"decoded","output_slot":"image"},"seed":{"node_id":"sampled","output_slot":"seed"}}}
  ]},"settings":{}}'
{"job_id":"7b87140e-8966-4fde-bb96-7e1699c65fad","queue_position":1}
```

HTTP Status: 202 Accepted. Job ID: 7b87140e-8966-4fde-bb96-7e1699c65fad.

### Step 6: Poll job status
```
$ sleep 10 && curl -s "http://127.0.0.1:8488/v1/jobs/7b87140e-8966-4fde-bb96-7e1699c65fad" | python3 -m json.tool
{
    "id": "7b87140e-8966-4fde-bb96-7e1699c65fad",
    "status": "failed",
    ...
    "error": "cannot parse safetensors header: cannot infer hidden_dim from safetensors keys in ./models/diffusion/flux2klein4b_tiny.safetensors: no recognized projection keys (input_proj.weight, time_text_emb.weight, or c_crossattn_dim) found"
}
```

**Status: FAILED (not Completed).**

### Step 7: Server-side error log
```
[2026-07-23T14:16:42.855425Z ERROR anvilml_worker::managed: __main__: dispatch_loop: execute failed job_id=7b87140e-8966-4fde-bb96-7e1699c65fad error=cannot parse safetensors header: cannot infer hidden_dim from safetensors keys in ./models/diffusion/flux2klein4b_tiny.safetensors: no recognized projection keys (input_proj.weight, time_text_emb.weight, or c_crossattn_dim) found worker_id=0
```

### Step 8: Artifact retrieval
Skipped — job failed before producing an artifact.

## Test Results

Not applicable — this task is a manual Runnable Proof, not a test-writing task. No test files were created or modified.

## Format Gate

Not applicable — no source files were formatted (only anvilml.toml was modified).

## Platform Cross-Check

Not required — this task ran only on the primary platform (Linux). No secondary platform target was exercised.

## Project Gates

Not applicable — no Rust source files were modified, so cargo fmt/clippy/test gates are not triggered by this task's changes. The only change is `anvilml.toml` (config file) and two fixture copies.

## Public API Delta

No new pub items introduced. No source files were modified.

## Deviations from Plan

1. **Config change required:** The plan states "no config changes" but the `[[model_dirs]]` entries in `anvilml.toml` were commented out, preventing the model scanner from discovering any models. Without uncommenting them, the scheduler's `resolve_model_ids()` would fail with `UnknownModelId` for every model hash. Uncommenting the three `[[model_dirs]]` entries was necessary and was previously a "manual retrofit patch" (ADDENDUM_P903) that was never committed.

2. **Fixture copies to model directories:** The fixtures exist in `worker/tests/fixtures/` but the model scanner only scans the configured `model_dirs`. Copies of `flux2klein4b_tiny.safetensors` and `flux2_vae_tiny.safetensors` were placed in `models/diffusion/` and `models/vae/` respectively. The `qwen3_tiny.safetensors` symlink already existed in `models/text_encoders/` from a prior phase.

3. **Proof failed — design defect in generic node layer:** The job failed at the LoadModel node because `worker/nodes/loader.py`'s `LoadModel.execute()` hardcodes `get_module("zit")` as the dispatch key. This routes all model loading requests to the ZiT module, regardless of the checkpoint's actual architecture. The ZiT module's `_infer_hyperparams_inner()` looks for ZiT-specific keys (`input_proj.weight`, `time_text_emb.weight`, `c_crossattn_dim`) which do not exist in the Flux 2 Klein checkpoint, causing a `ValueError`.

   This is a design defect in the generic node layer. Per ANVILML_DESIGN.md §10.4, the key passed to `get_module()` should be "an arch string read from safetensors metadata or a path-derived fallback." The current implementation passes a hardcoded `"zit"` string instead. Fixing this requires modifying `loader.py` to read the architecture from the checkpoint header (or derive it from the filesystem path's directory component) and pass it to `get_module()` — which is a change to the generic node layer.

4. **Zero generic-node-layer files were NOT modified:** The plan's acceptance criterion states "Zero generic-node-layer files (loader.py, sampler.py, encoder.py, decode.py, image.py) were modified — confirmed by inspecting git diff or noting no changes were made." This is technically true for THIS task (I did not modify loader.py), but the proof fails because the generic node layer needs architecture-aware dispatch. The acceptance criterion about zero file modifications is met, but the acceptance criteria about status=completed and a valid PNG are not.

## Blockers

The Runnable Proof cannot succeed without modifying the generic node layer's architecture-aware dispatch. Specifically:

- `worker/nodes/loader.py`'s `LoadModel.execute()` hardcodes `get_module("zit")` (line 83), routing all diffusion model loading to the ZiT module regardless of the checkpoint's architecture.
- `worker/nodes/loader.py`'s `LoadVae.execute()` hardcodes `get_module("zit_vae")` (line 173), routing all VAE loading to the ZiT VAE module.
- `worker/nodes/sampler.py`'s `Sampler.execute()` uses `inputs["model"].arch` for dispatch (line 104), which IS architecture-aware — but only after the model has been loaded. The bottleneck is at `LoadModel`, not `Sampler`.

The fix requires modifying `loader.py` to determine the architecture from the checkpoint (either by reading the `arch` metadata field from the safetensors header, or by deriving it from the filesystem path's directory component — e.g., `./models/diffusion/flux2klein4b_tiny.safetensors` → "flux2klein" from the filename). This is a change to the generic node layer, which is out of scope for this task and requires its own task.

The same issue affects `LoadVae` — it hardcodes `get_module("zit_vae")` instead of reading the VAE's architecture from the checkpoint. `flux2_vae.py`'s `can_handle("flux2")` would never be reached.
