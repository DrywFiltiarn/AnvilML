# Tasks: Phase 014 — Artifact Storage

| Field | Value |
|-------|-------|
| Phase | 014 |
| Name | Artifact Storage |
| Milestone group | End-to-end generation (mock) |
| Depends on phases | 1-13 |
| Task file | `forge/tasks/tasks_phase014.json` |
| Tasks | 5 |

## Overview

Phase 14 makes jobs produce a retrievable image: the mock `SaveImage` emits an `ImageReady` (black PNG), the `ArtifactStore` decodes/hashes/writes the file and inserts metadata, the scheduler persists it on `ImageReady`, and `GET /v1/artifacts/:hash` + `GET /v1/artifacts` serve it. After this phase a completed job yields a downloadable PNG.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|---------------|---------|
| P14-A1 | worker | worker: mock SaveImage emits ImageReady with black PNG |
| P14-A2 | `src/artifact/store.rs` | anvilml-server: ArtifactStore.save (decode, hash, write, db insert) |
| P14-A3 | anvilml-scheduler | anvilml-scheduler: handle ImageReady -> ArtifactStore.save + JobImageReady |
| P14-A4 | `GET /v1/artifacts/:hash` | anvilml-server: GET /v1/artifacts/:hash serves PNG |
| P14-A5 | `GET /v1/artifacts` | anvilml-server: GET /v1/artifacts list (by job_id) |

## Task details

#### P14-A1: worker: mock SaveImage emits ImageReady with black PNG

- **Prereqs:** P13-A6
- **Tags:** —

In worker_main.py mock Execute: when a node of type SaveImage is reached, generate a black PNG (PIL Image.new RGB 64x64) -> PNG bytes -> base64, and emit ImageReady{job_id, image_b64, width:64, height:64, format:'png', seed:resolved (seed -1 -> random int), steps, prompt} before Completed. Pull prompt/seed/steps from graph node inputs if present else defaults. Verify via P14-A4.

#### P14-A2: anvilml-server: ArtifactStore.save (decode, hash, write, db insert)

- **Prereqs:** P14-A1
- **Tags:** reasoning

Add sha2, hex, base64, tokio fs to anvilml-server. Create src/artifact/store.rs: ArtifactStore{artifact_dir,db}. async fn save(job_id, image_b64,&meta_input)->Result<ArtifactMeta>: base64 decode, hash=hex(SHA256(bytes)), write {artifact_dir}/{hash[0..2]}/{hash}.png (create_dir_all), INSERT ArtifactMeta, UPDATE jobs.artifact_count+1. cargo test -p anvilml-server --features mock-hardware -- artifact_save exits 0 (tempdir + memory DB).

#### P14-A3: anvilml-scheduler: handle ImageReady -> ArtifactStore.save + JobImageReady

- **Prereqs:** P14-A2
- **Tags:** reasoning

Add artifact_store: Arc<ArtifactStore> to AppState and pass to JobScheduler. In dispatch loop event handling: on WorkerEvent::ImageReady call artifact_store.save(...), then broadcaster.send(WsEvent::JobImageReady{job_id, artifact_hash, width,height,seed}). No image bytes in the event. cargo test --workspace --features mock-hardware exits 0.

#### P14-A4: anvilml-server: GET /v1/artifacts/:hash serves PNG

- **Prereqs:** P14-A3
- **Tags:** —

Add to artifact/store.rs get_path(hash)->Result<PathBuf>. Create handlers/artifacts.rs serve_artifact(State,Path<String>): 404 artifact_not_found if missing, else stream file with Content-Type image/png, Cache-Control 'public, immutable, max-age=31536000', ETag '"{hash}"'. Wire GET /v1/artifacts/:hash. Verify end-to-end: run server+mock worker, submit ZiT job, get hash from /v1/jobs/<id> artifacts or /v1/events JobImageReady, curl -o out.png /v1/artifacts/<hash>, confirm a valid 64x64 PNG downloads.

#### P14-A5: anvilml-server: GET /v1/artifacts list (by job_id)

- **Prereqs:** P14-A4
- **Tags:** —

Add to store.rs list(job_id:Option<Uuid>,limit,before)->Vec<ArtifactMeta>. Add handlers/artifacts.rs list_artifacts(State,Query{job_id})->Json<Vec<ArtifactMeta>>. Wire GET /v1/artifacts. Verify: after a completed job, curl '/v1/artifacts?job_id=<id>' returns the artifact metadata (hash,width,height,seed,prompt).


## Runnable Proof

Run a job, then download its artifact.

```bash
ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./venv cargo run --features mock-hardware
JOB=$(curl -s -X POST .../v1/jobs -d @valid_zit_job.json -H 'content-type: application/json' | jq -r .job_id)
sleep 3
HASH=$(curl -s "http://127.0.0.1:8488/v1/artifacts?job_id=$JOB" | jq -r '.[0].hash')
curl -s -o out.png "http://127.0.0.1:8488/v1/artifacts/$HASH"
file out.png        # PNG image data, 64 x 64
```

Expected: the artifact list returns one `ArtifactMeta`; downloading by hash returns a valid 64x64 PNG with `Content-Type: image/png` and a long-lived `Cache-Control`. Phase done when a completed job's PNG can be downloaded via REST.
