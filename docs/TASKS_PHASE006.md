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
| C | anvilml-core, anvilml-hardware | P6-C1 … P6-C2 | GpuDevice db_name field + SQLite capability enrichment |

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

**Goal:** Implement `DeviceCapabilityStore` in `device_store.rs` with `get(vendor_id: u16, device_id: u16) -> Option<DeviceRow>`. `DeviceRow` carries the full capability set matching `InferenceCaps`: `vendor_id: u16`, `device_id: u16`, `name: String`, `arch: String`, `fp32: bool`, `fp16: bool`, `bf16: bool`, `fp8: bool`, `fp4: bool`, `flash_attention: bool`. Booleans are stored as `INTEGER 0/1` in SQLite; map at the store boundary via `value != 0`. Backed by the `device_capabilities` table populated by `SeedLoader`.

**Acceptance criterion:** `cargo test -p anvilml-registry -- device_store` exits 0.

### Group B — anvilml-server

#### P6-B1: anvilml-server: GET /v1/models + GET /v1/models/:id

**Goal:** Implement `handlers/models.rs` with `list_models` and `get_model` handlers reading from `AppState.registry`. Add `registry: Arc<ModelRegistry>` to `AppState` where `ModelRegistry` wraps `ModelStore`. Mount routes in `build_router`.

**Acceptance criterion:** `curl /v1/models` → 200 JSON array; `curl /v1/models/:id` → 200 or 404.

#### P6-B2: anvilml-server: POST /v1/models/rescan

**Goal:** Implement `rescan_models` handler that triggers a background scan and upserts results into the store. Wire initial scan at server startup. Mount `POST /v1/models/rescan` in `build_router`.

**Acceptance criterion:** Place a `.safetensors` file in `./models/diffusion/`; `curl /v1/models` lists it with `kind: "diffusion"`.

### Group C — anvilml-core / anvilml-hardware

#### P6-C1: anvilml-core: add db_name field to GpuDevice

**Goal:** Add `pub db_name: Option<String>` to `GpuDevice` in `crates/anvilml-core/src/types/hardware.rs` to carry the device group name retrieved from the `device_capabilities` table alongside the enumerator-reported `name`. This separation is necessary because AMD (and potentially other vendors) assign one `device_id` to multiple marketed SKUs — the DB `name` field records the group (e.g. `"AMD Radeon RX 9070/RX 9070 XT/RX 9070 GRE"`) while the enumerator reports the specific installed SKU (e.g. `"AMD Radeon RX 9070"`). Both are useful: `name` for display, `db_name` for confirming which DB entry was matched.

**Files to create or modify:**
- `crates/anvilml-core/src/types/hardware.rs` — add `pub db_name: Option<String>` field to `GpuDevice` after `name`, with `///` doc comment
- `crates/anvilml-core/tests/hardware_tests.rs` — update `test_hardware_info_json_roundtrip` to set `db_name: None` on all constructed `GpuDevice` instances; add assertion that `db_name` roundtrips correctly
- `crates/anvilml-hardware/src/detect.rs` — set `db_name: None` on all `GpuDevice` constructions in the override and mock paths
- `crates/anvilml-hardware/src/mock.rs` — set `db_name: None` on the synthesised `GpuDevice`
- All other crates constructing `GpuDevice` directly (check `anvilml-hardware` detector impls and test files) — add `db_name: None` to each struct literal

**Key implementation notes:**
- `db_name` is `Option<String>` and defaults to `None` at construction time everywhere. It is only populated by the SQLite enrichment step in P6-C2.
- The field must be positioned after `name` in the struct definition to maintain logical grouping in the JSON output: `name` (enumerator), `db_name` (database), then the rest.
- `anvilml-core` version bump: patch `0.x.y → 0.x.(y+1)`.
- `anvilml-hardware` version bump: patch, to reflect the struct literal updates.

**Acceptance criterion:** `cargo test --workspace --features mock-hardware` exits 0 with no struct initialisation errors; `GET /v1/system` response includes `"db_name": null` on all GPU devices.

#### P6-C2: anvilml-hardware: SQLite capability enrichment in detect_all_devices

**Goal:** Replace the deferred step h stub in `detect_all_devices` with a real SQLite lookup that enriches each detected non-CPU `GpuDevice` with the full capability row from the seeded `device_capabilities` table. After this task, any device whose PCI vendor/device ID pair is present in the seed data will have `arch`, all six `InferenceCaps` fields (`fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention`), `db_name`, and `capabilities_source = DeviceTable` correctly populated before `AppState` is constructed. The enumerator-reported `name` is never overwritten.

**Files to create or modify:**
- `crates/anvilml-hardware/Cargo.toml` — add `anvilml-registry = { path = "../anvilml-registry" }` dependency; bump version patch
- `crates/anvilml-hardware/src/detect.rs` — rename `_pool` to `pool`; update `#[instrument]` skip list; add step e2 SQLite enrichment loop after step e; remove step h deferred stub; update function doc comment

**Key implementation notes:**
- Step e2 constructs `DeviceCapabilityStore::new(pool.clone()).await` once, then iterates `devices.iter_mut()`, skipping CPU devices. For each GPU device, calls `store.get(pci_vendor_id, pci_device_id).await`.
- On `Ok(Some(row))`: overwrite `dev.arch`, all six `InferenceCaps` fields, and `dev.capabilities_source = CapabilitySource::DeviceTable`. Set `dev.db_name = Some(row.name.clone())`. Do not touch `dev.name`. Log at `tracing::debug!` with `vendor_id`, `device_id`, `arch`, `source = "sqlite"`.
- On `Ok(None)`: log at `tracing::warn!` with `vendor_id`, `device_id`, `name`. Caps and `db_name` remain at their step-e values (caps may be partially populated by `DEVICE_DB`; `db_name` remains `None`).
- On `Err(e)`: log at `tracing::error!` and continue — a DB query failure must not abort hardware detection.
- `anvilml-registry` has no `mock-hardware` feature and does not depend on `anvilml-hardware`, so no dependency cycle and no feature forwarding change is required.
- Existing mock tests pass an in-memory pool with an empty `device_capabilities` table; step e2 produces `Ok(None)` for all mock devices (PCI IDs = 0), which is the correct no-op path. No test changes required.

**Acceptance criterion:** `cargo test --workspace --features mock-hardware` exits 0; `GET /v1/system` for a device present in `database/seeds/devices.sql` returns `capabilities_source: "device_table"`, correct non-false capability flags, and `db_name` set to the group name from the seed (e.g. `"AMD Radeon RX 9070/RX 9070 XT/RX 9070 GRE"` alongside `name: "AMD Radeon RX 9070"`).

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
- `DeviceRow` carries all six `InferenceCaps`-aligned capability fields (`fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention`). These are pre-spawn scheduling hints loaded from the seed table; the Python worker overwrites them with authoritative values at the `Ready` event. `fp4 = true` means native 4-bit matrix compute is available regardless of vendor format (AMD MXFP4 or NVIDIA NVFP4); the worker resolves the vendor-specific execution path.
- P6-C1 touches `GpuDevice` struct literals across multiple crates. The agent must search for all construction sites — not just `detect.rs` — before staging. `cargo build --workspace` is the reliable check that all sites were found.
- P6-C2 adds `anvilml-registry` as a direct dependency of `anvilml-hardware`. This is the only direction the dependency may run — `anvilml-registry` must never take a dependency on `anvilml-hardware` (cycle). The enrichment is placed after all detection and enumeration steps so that mock and override paths are unaffected.
- Delete `anvilml.db` before first run after P6-C2 lands if the database was created before the seed patch was applied, to ensure `device_capabilities` is populated from a clean migration and seed run.