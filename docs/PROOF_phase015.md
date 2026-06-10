# Runnable Proof — Live Job Events on WebSocket

**Phase 015:** Live Job Events
**Task:** P15-A3
**Prerequisite:** P15-A2 (integration test for full WS lifecycle)

---

## Prerequisites

- **websocat** installed (e.g. `apt install websocat`, `brew install websocat`, or `cargo install websocat`)
- **Python 3** on PATH (the mock worker requires it; `import torch` will be skipped in mock mode)
- **`valid_zit_job.json`** present at the repository root (a valid ZiT pipeline job with 5 nodes)
- A Rust toolchain with `cargo` available

---

## Commands

Open **three separate terminal windows** in the AnvilML repository root.

### Terminal 1 — Start the mock server

```bash
ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./venv cargo run --features mock-hardware
```

Wait until you see the server bind message, e.g.:

```
anvilml_server::app: listening on 127.0.0.1:8488
```

The server is now accepting HTTP requests on port 8488 and broadcasting WebSocket frames to `/v1/events`.

### Terminal 2 — Watch the WebSocket stream

```bash
websocat ws://127.0.0.1:8488/v1/events
```

This will print one JSON frame per line as events are broadcast. You will see `system.stats` frames periodically (every ~5 seconds) and job lifecycle frames when a job is submitted.

### Terminal 3 — Submit a job

```bash
curl -s -X POST http://127.0.0.1:8488/v1/jobs \
  -H 'content-type: application/json' \
  -d @valid_zit_job.json
```

The `valid_zit_job.json` file contains a 5-node ZiT pipeline:

```json
{
  "graph": {
    "nodes": [
      { "id": "load",  "type": "ZitLoadPipeline" },
      { "id": "encode","type": "ZitTextEncode", "inputs": {
          "pipeline": { "node_id": "load",  "output_slot": "pipeline" },
          "prompt": "a red fox in a snowy forest"
      }},
      { "id": "sampler","type": "ZitSampler", "inputs": {
          "pipeline":       { "node_id": "load",  "output_slot": "pipeline" },
          "conditioning":   { "node_id": "encode","output_slot": "conditioning" },
          "steps": 8,
          "seed": 42
      }},
      { "id": "decode","type": "ZitDecode", "inputs": {
          "pipeline": { "node_id": "load",  "output_slot": "pipeline" },
          "latents":  { "node_id": "sampler","output_slot": "latents" }
      }},
      { "id": "save","type": "SaveImage", "inputs": {
          "image":    { "node_id": "decode","output_slot": "image" },
          "prompt":   "a red fox in a snowy forest",
          "seed": 42,
          "steps": 8
      }}
    ]
  },
  "settings": {
    "seed": 42,
    "steps": 8,
    "guidance_scale": 7.5,
    "width": 1024,
    "height": 1024
  }
}
```

---

## Expected Output

In **Terminal 2** (websocat), you should see frames arrive in the following order.
`system.stats` frames may appear between any job events (broadcast every ~5 seconds).

### 1. `job.queued` — the job enters the queue

```json
{"JobQueued":{"event":"job.queued","timestamp":"2026-06-10T12:00:00.000000+00:00","job_id":"550e8400-e29b-41d4-a716-446655440000"}}
```

| Field    | Type   | Description                        |
|----------|--------|------------------------------------|
| `event`  | string | Always `"job.queued"`              |
| `timestamp` | string | ISO 8601 UTC timestamp          |
| `job_id` | string | UUID of the newly queued job       |

### 2. `job.started` — a worker begins execution

```json
{"JobStarted":{"event":"job.started","timestamp":"2026-06-10T12:00:00.500000+00:00","job_id":"550e8400-e29b-41d4-a716-446655440000"}}
```

| Field       | Type   | Description                     |
|-------------|--------|---------------------------------|
| `event`     | string | Always `"job.started"`          |
| `timestamp` | string | ISO 8601 UTC timestamp          |
| `job_id`    | string | UUID of the job that started    |

### 3. `job.progress` — one or more DAG node completions

```json
{"JobProgress":{"event":"job.progress","timestamp":"2026-06-10T12:00:01.000000+00:00","job_id":"550e8400-e29b-41d4-a716-446655440000","node_index":0,"node_total":5,"node_type":"ZitLoadPipeline","step":null,"step_total":null}}
```

One frame per completed node (5 nodes = 5 frames minimum). Subsequent frames increment `node_index`:

```json
{"JobProgress":{"event":"job.progress","timestamp":"...","job_id":"...","node_index":1,"node_total":5,"node_type":"ZitTextEncode","step":null,"step_total":null}}
{"JobProgress":{"event":"job.progress","timestamp":"...","job_id":"...","node_index":2,"node_total":5,"node_type":"ZitSampler","step":null,"step_total":null}}
{"JobProgress":{"event":"job.progress","timestamp":"...","job_id":"...","node_index":3,"node_total":5,"node_type":"ZitDecode","step":null,"step_total":null}}
{"JobProgress":{"event":"job.progress","timestamp":"...","job_id":"...","node_index":4,"node_total":5,"node_type":"SaveImage","step":null,"step_total":null}}
```

| Field        | Type    | Description                                  |
|--------------|---------|----------------------------------------------|
| `event`      | string  | Always `"job.progress"`                      |
| `timestamp`  | string  | ISO 8601 UTC timestamp                       |
| `job_id`     | string  | UUID of the job                              |
| `node_index` | u32     | 0-based index of the completed node          |
| `node_total` | u32     | Total number of nodes in the DAG             |
| `node_type`  | string  | Class name of the completed node             |
| `step`       | u32\|null | Per-step progress within a node (always `null` in MVP) |
| `step_total` | u32\|null | Total steps within a node (always `null` in MVP) |

### 4. `job.image_ready` — an output image is available

```json
{"JobImageReady":{"event":"job.image_ready","timestamp":"2026-06-10T12:00:05.000000+00:00","job_id":"550e8400-e29b-41d4-a716-446655440000","artifact_hash":"sha256:abcdef123456","width":1024,"height":1024,"seed":42}}
```

| Field           | Type   | Description                                                    |
|-----------------|--------|----------------------------------------------------------------|
| `event`         | string | Always `"job.image_ready"`                                     |
| `timestamp`     | string | ISO 8601 UTC timestamp                                         |
| `job_id`        | string | UUID of the job                                                |
| `artifact_hash` | string | SHA-256 hash for fetching via `GET /v1/artifacts/:hash`        |
| `width`         | u32    | Image width in pixels                                          |
| `height`        | u32    | Image height in pixels                                         |
| `seed`          | i64    | Random seed used to generate the image                         |

### 5. `job.completed` — the job finished successfully

```json
{"JobCompleted":{"event":"job.completed","timestamp":"2026-06-10T12:00:05.500000+00:00","job_id":"550e8400-e29b-41d4-a716-446655440000"}}
```

| Field       | Type   | Description                    |
|-------------|--------|--------------------------------|
| `event`     | string | Always `"job.completed"`       |
| `timestamp` | string | ISO 8601 UTC timestamp         |
| `job_id`    | string | UUID of the completed job      |

---

## Interleaved `system.stats`

Between any of the job events above, you may see periodic `system.stats` frames:

```json
{"SystemStats":{"event":"system.stats","timestamp":"2026-06-10T12:00:02.000000+00:00","gpus":[{"index":0,"vram_used_mib":45000,"vram_total_mib":81920}],"ram_used_mib":32768,"ram_total_mib":65536}}
```

These are broadcast approximately every 5 seconds regardless of job activity and carry GPU VRAM and host RAM snapshots.

---

## Troubleshooting

| Problem | Cause | Fix |
|---------|-------|-----|
| **Port 8488 already in use** | Another process binds the default port | Stop the other process, or override with `ANVILML_PORT=8489` and adjust the websocat/curl URLs accordingly |
| **`python3: command not found`** | Python 3 is not on PATH | Install Python 3 and ensure `python3` is available. In mock mode the worker skips `import torch`, so only the interpreter itself is needed |
| **No frames appear in websocat** | Server did not start, or curl was sent before the server was ready | Wait for the `listening on` message in Terminal 1 before sending the curl request |
| **`cargo: command not found`** | Rust toolchain not installed | Install via `rustup`: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| **`websocat: command not found`** | websocat not installed | Install via package manager (`apt`, `brew`, `choco`) or `cargo install websocat` |
| **`valid_zit_job.json: No such file`** | Working directory is not the repo root | Run curl from the repository root, or provide the full path to the JSON file |
| **`mock env vars missing`** | `ANVILML_WORKER_MOCK` not set | Ensure `ANVILML_WORKER_MOCK=1` is set in Terminal 1; without it the server attempts real hardware detection which may fail |

---

## Verification

The proof is complete when Terminal 2 shows all five event types in order:

1. `job.queued`
2. `job.started`
3. one or more `job.progress` frames
4. `job.image_ready`
5. `job.completed`

`system.stats` frames may appear between any of these but should not disrupt the ordering of the job lifecycle events.
