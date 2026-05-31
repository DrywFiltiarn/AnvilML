# Tasks: Phase 006 — Model Registry

| Field | Value |
|-------|-------|
| Phase | 006 |
| Name | Model Registry |
| Milestone group | Observable system state |
| Depends on phases | 1-5 |
| Task file | `forge/tasks/tasks_phase006.json` |
| Tasks | 7 |

## Overview

Phase 6 implements the model scanner and `ModelRegistry` store, scans the configured model directories at startup, and exposes `GET /v1/models`, `GET /v1/models/:id`, and `POST /v1/models/rescan`. After this phase you can drop a model file on disk and see it appear over the API.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|---------------|---------|
| P6-A1 | `src/scanner.rs` | anvilml-registry: model directory scanner |
| P6-A2 | `src/store.rs` | anvilml-registry: ModelRegistry store (upsert, get) |
| P6-A3 | anvilml-registry | anvilml-registry: ModelRegistry list (with kind filter) |
| P6-A4 | anvilml-registry | anvilml-registry: ModelRegistry rescan (scan + bulk upsert) |
| P6-A5 | anvilml | anvilml: initial model scan at startup + registry in AppState |
| P6-A6 | `GET /v1/models` | anvilml-server: GET /v1/models handler (list with kind filter) |
| P6-A7 | `GET /v1/models/:id` | anvilml-server: GET /v1/models/:id and POST /v1/models/rescan |

## Task details

#### P6-A1: anvilml-registry: model directory scanner

- **Prereqs:** P5-A4
- **Tags:** —

Add walkdir, sha2, hex to anvilml-registry. Create src/scanner.rs: async fn scan_dirs(dirs:&[ModelDirConfig])->Vec<ModelMeta>. Walk each dir (follow_links false), match .safetensors/.ckpt/.pt/.bin. id=first16hex of SHA256(canonical path string). name=file stem. kind from ModelDirConfig.kind or infer from parent dir name. dtype from filename suffix else Unknown. vram_estimate_mib=size_mib*factor (f32 2.0,f16/bf16 1.0,q8 0.5,q4 0.25,unknown 1.0,min 1). cargo test -p anvilml-registry -- scanner exits 0 with tempdir fixture (2 files).

#### P6-A2: anvilml-registry: ModelRegistry store (upsert, get)

- **Prereqs:** P6-A1
- **Tags:** —

Create src/store.rs: ModelRegistry{pool}. ModelRegistry::new(pool). async fn upsert(&self,&ModelMeta)->Result (INSERT OR REPLACE INTO models). async fn get(&self,id:&str)->Result<Option<ModelMeta>>. Map all columns. Re-export ModelRegistry from lib.rs. cargo test -p anvilml-registry -- store_get exits 0: upsert then get returns equal meta; get missing returns None.

#### P6-A3: anvilml-registry: ModelRegistry list (with kind filter)

- **Prereqs:** P6-A2
- **Tags:** —

Add to store.rs: async fn list(&self, kind:Option<ModelKind>)->Result<Vec<ModelMeta>> -> SELECT * FROM models, optional WHERE kind=?, ORDER BY name ASC. cargo test -p anvilml-registry -- store_list exits 0: empty returns []; after 3 upserts list returns 3 ordered; kind filter returns only matching.

#### P6-A4: anvilml-registry: ModelRegistry rescan (scan + bulk upsert)

- **Prereqs:** P6-A3
- **Tags:** —

Add to store.rs: async fn rescan(&self, dirs:&[ModelDirConfig])->Result<u32> calling scan_dirs then upsert each, returning count upserted. Never auto-removes stale rows (manual only). cargo test -p anvilml-registry -- rescan exits 0: rescan tempdir adds N, second rescan keeps N (idempotent).

#### P6-A5: anvilml: initial model scan at startup + registry in AppState

- **Prereqs:** P6-A4
- **Tags:** —

Add registry: Arc<ModelRegistry> to AppState. In main.rs after DB open: build ModelRegistry::new(db.clone()), spawn a non-blocking tokio task calling registry.rescan(&cfg.model_dirs) (log count). Store registry Arc in AppState. Do not block server bind on the scan. Verify in next task via REST.

#### P6-A6: anvilml-server: GET /v1/models handler (list with kind filter)

- **Prereqs:** P6-A5
- **Tags:** —

Create handlers/models.rs: async fn list_models(State, Query{kind:Option<ModelKind>})->Json<Vec<ModelMeta>> calling registry.list(kind). Wire GET /v1/models. Verify: create ./models/diffusion/, drop a fake file model-fp16.safetensors, set anvilml.toml model_dirs to it, cargo run --features mock-hardware, curl 'http://127.0.0.1:8488/v1/models' lists the model with kind diffusion + dtype f16.

#### P6-A7: anvilml-server: GET /v1/models/:id and POST /v1/models/rescan

- **Prereqs:** P6-A6
- **Tags:** —

Add to handlers/models.rs: async fn get_model(State, Path<String>)->Result returning 200 ModelMeta or 404 not_found JSON body. async fn rescan_models(State)->202 spawning registry.rescan(&cfg.model_dirs) without waiting. Wire GET /v1/models/:id and POST /v1/models/rescan. Verify: curl /v1/models/<id> returns the model; curl -X POST /v1/models/rescan returns 202; add a new file then rescan then list shows it.


## Runnable Proof

Create a model directory with a fake model file and confirm it appears via the API.

```bash
mkdir -p models/diffusion
touch models/diffusion/mymodel-fp16.safetensors
# ensure anvilml.toml has a [[model_dirs]] path = "./models/diffusion" kind = "diffusion"
cargo run --features mock-hardware
curl -s http://127.0.0.1:8488/v1/models | python -m json.tool
```

Expected (200): an array with one `ModelMeta` whose `name` is `mymodel-fp16`, `kind` is `diffusion`, `dtype_hint` is `f16`, and a 16-hex-char `id`. `curl /v1/models/<id>` returns that one model; adding a second file then `curl -X POST /v1/models/rescan` (202) then re-listing shows both. Phase done when models scanned from disk are listed via REST.
