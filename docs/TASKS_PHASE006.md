# Tasks: Phase 006 — Model Registry

| Field | Value |
|-------|-------|
| Phase | 006 |
| Name | Model Registry |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 5 |

## Overview

Phase 006 implements the model scanner, SQLite persistence for model metadata, and the three model REST endpoints. After this phase, a user can place `.safetensors` files in the configured model directories and immediately query them via the API.

The scanner infers `ModelKind` from the directory path component relative to the model root (e.g. `diffusion/` → `Diffusion`, `text_encoders/` or `clip/` → `TextEncoder`, `vae/` → `Vae`) and `ModelDtype` from filename substrings (`fp8`, `fp16`, `bf16`). The model ID is the lower-case hex SHA256 of the first 1 MiB of the file. The scanner is non-recursive by default; `ModelDirConfig.recursive = true` enables depth-limited traversal.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-registry | P6-A1 … P6-A3 | scanner.rs, store.rs, DeviceCapabilityStore |
| B | anvilml-server | P6-B1 … P6-B2 | Model endpoints + AppState wiring |

## Prerequisites

Phase 005 complete: `SqlitePool` available via `open()` and `open_in_memory()`.

## Task Descriptions

### Group A — anvilml-registry

#### P6-A1: anvilml-registry: ModelScanner (directory walk + metadata derivation)

**Goal:** Implement `ModelScanner` in `scanner.rs` with `pub async fn scan(dirs: &[ModelDirConfig]) -> Vec<ModelMeta>`. Infer kind from directory, dtype from filename, id from SHA256 first 1 MiB. Log each file examined at DEBUG with `path=` and `reason=` if skipped. Log scan completed at INFO with `count=`, `dir=`.

**Acceptance criterion:** `cargo test -p anvilml-registry -- scanner` exits 0 with ≥ 6 tests (kind inference, dtype inference, id derivation, non-existent dir skipped).

#### P6-A2: anvilml-registry: ModelStore SQLite CRUD

**Goal:** Implement `ModelStore` in `store.rs` with `upsert`, `get`, `list`, `list_by_kind`, `delete`. All queries use `sqlx` typed macros. Use `open_in_memory()` in tests.

**Acceptance criterion:** `cargo test -p anvilml-registry -- store` exits 0 with ≥ 6 tests.

#### P6-A3: anvilml-registry: DeviceCapabilityStore

**Goal:** Implement `DeviceCapabilityStore` in `device_store.rs` with `get(vendor_id, device_id) -> Option<DeviceRow>`. Backed by the `device_capabilities` table populated by `SeedLoader`.

**Acceptance criterion:** `cargo test -p anvilml-registry -- device_store` exits 0.

### Group B — anvilml-server

#### P6-B1: anvilml-server: GET /v1/models + GET /v1/models/:id

**Goal:** Implement `handlers/models.rs` with `list_models` and `get_model` handlers reading from `AppState.registry`. Add `registry: Arc<ModelRegistry>` to `AppState` where `ModelRegistry` wraps `ModelStore`. Mount routes in `build_router`.

**Acceptance criterion:** `curl /v1/models` → 200 JSON array; `curl /v1/models/:id` → 200 or 404.

#### P6-B2: anvilml-server: POST /v1/models/rescan

**Goal:** Implement `rescan_models` handler that triggers a background scan and upserts results into the store. Wire initial scan at server startup. Mount `POST /v1/models/rescan` in `build_router`.

**Acceptance criterion:** Place a `.safetensors` file in `./models/diffusion/`; `curl /v1/models` lists it with `kind: "diffusion"`.

## Phase Acceptance Criteria

```bash
mkdir -p models/diffusion models/text_encoders models/vae
dd if=/dev/urandom of=models/diffusion/test_model_fp8.safetensors bs=1M count=2
cargo run --features mock-hardware &
sleep 2
curl -s http://127.0.0.1:8488/v1/models | python3 -c "import sys,json; items=json.load(sys.stdin); assert len(items)>=1"
kill %1
```

## Known Constraints and Gotchas

- SHA256 computation reads only the first 1 MiB to keep scanning fast for large model files.
- `ModelDtype` inference checks filename substrings case-insensitively: `fp8_e4m3fn` and `fp8_e5m2` both map to `Fp8`. The check order matters — check `fp8` before `fp16` to avoid false matches on filenames like `fp16_fp8_quantized`.
- The `POST /v1/models/rescan` handler should respond 202 immediately and run the scan in a `tokio::spawn` background task, not block the HTTP thread.
