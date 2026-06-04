# Tasks: Phase 006 — Model Registry

| Field | Value |
|-------|-------|
| Phase | 006 |
| Name | Model Registry |
| Milestone group | Observable system state |
| Depends on phases | 1-5 |
| Task file | `forge/tasks/tasks_phase006.json` |
| Tasks | 10 |

## Overview

Phase 6 implements the model scanner and `ModelRegistry` store, scans the configured model directories at startup, and exposes `GET /v1/models`, `GET /v1/models/:id`, and `POST /v1/models/rescan`. After this phase you can drop a model file on disk and see it appear over the API.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P6-A1 | `crates/anvilml-registry/src/scanner.rs` | anvilml-registry: model directory scanner |
| P6-A2 | `crates/anvilml-registry/src/store.rs` | anvilml-registry: ModelRegistry store (upsert, get) |
| P6-A3 | `crates/anvilml-registry/src/store.rs` | anvilml-registry: ModelRegistry list (with kind filter) |
| P6-A4 | `crates/anvilml-registry/src/store.rs` | anvilml-registry: ModelRegistry rescan (scan + bulk upsert) |
| P6-A5 | `backend/src/main.rs` | anvilml: initial model scan at startup + registry in AppState |
| P6-A6 | `crates/anvilml-server/src/handlers/models.rs` | anvilml-server: GET /v1/models handler (list with kind filter) |
| P6-A7 | `crates/anvilml-server/src/handlers/models.rs` | anvilml-server: GET /v1/models/:id and POST /v1/models/rescan |
| P6-B1 | `crates/anvilml-hardware/src/lib.rs` | anvilml-hardware: fix real-hardware build errors (no-feature compile check) |
| P6-B2 | `.github/workflows/ci.yml` | anvilml: add real-hardware compile check to rust-linux and rust-windows CI jobs |
| P6-C1 | `crates/anvilml-core/src/config.rs`, `crates/anvilml-core/src/types/hardware.rs` | anvilml-core: add serde snake_case to FrontendMode and DeviceType config enums |

## Task details

### Group A — Model Registry

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

---

### Group B — CI Hardening

#### P6-B1: anvilml-hardware: fix real-hardware build errors surfaced by no-feature compile check

- **Prereqs:** P6-A7
- **Tags:** —

The `mock-hardware` feature flag completely replaces the real-hardware detection branch at compile time. All prior CI runs and every ACT-session gate have used `--features mock-hardware` exclusively, meaning the `#[cfg(windows)]` and `#[cfg(unix)]` code paths in `anvilml-hardware` have never been compiled as part of the automated flow. Errors in those paths are invisible until a user attempts a real run.

This task runs both compile checks without the feature flag and fixes every error that surfaces:

```
cargo check --bin anvilml                                    # native Linux — exercises #[cfg(unix)] paths
cargo check --bin anvilml --target x86_64-pc-windows-gnu    # Windows-gnu cross — exercises #[cfg(windows)] paths
```

The known entry point for errors is `enumerate_gpus()` in `crates/anvilml-hardware/src/lib.rs`, where `DxgiDetector`, `SysfsDetector`, and `NvmlDetector` are called with dot-syntax on the type path (e.g. `dxgi::DxgiDetector.detect()`) instead of being constructed first via `::default()` (e.g. `dxgi::DxgiDetector::default().detect()`). All three structs derive `Default`. Do not assume this is the only error — run both checks and fix everything reported.

**Files to create or modify:**
- `crates/anvilml-hardware/src/lib.rs` — fix all constructor call errors in `enumerate_gpus()` and any other errors surfaced by the no-feature checks

**Key implementation notes:**
- Do not add, remove, or change any `#[cfg(...)]` feature gates. Scope is compile-error fixes only.
- Do not modify any test. The existing test suite under `--features mock-hardware` must still pass after the fixes.
- Both no-feature `cargo check` invocations must produce zero errors before writing the implementation report. Record their verbatim output in `## Platform Cross-Check`.

**Acceptance criterion:** `cargo check --bin anvilml` exits 0 AND `cargo check --bin anvilml --target x86_64-pc-windows-gnu` exits 0.

---

#### P6-B2: anvilml: add real-hardware compile check steps to rust-linux and rust-windows CI jobs

- **Prereqs:** P6-B1
- **Tags:** —

With the real-hardware paths compiling cleanly after P6-B1, this task locks that guarantee into CI so it cannot regress. Both jobs in `.github/workflows/ci.yml` receive a new step placed immediately after their existing `Run tests` step:

```yaml
- name: Real-hardware compile check
  run: cargo check --bin anvilml
```

No `--features` flag on either. On `rust-linux` (`ubuntu-latest`) this exercises the `#[cfg(unix)]` paths natively. On `rust-windows` (`windows-latest`, native MSVC toolchain) this exercises the `#[cfg(windows)]` paths — the same environment a real user runs. All existing jobs and steps are preserved unchanged; this task inserts only, it does not reorder or alter any existing step.

**Files to create or modify:**
- `.github/workflows/ci.yml` — add `Real-hardware compile check` step to both `rust-linux` and `rust-windows` jobs, each placed immediately after their existing `Run tests` step

**Key implementation notes:**
- Per `FORGE_AGENT_RULES §3.7`, CI workflow files may only be modified when explicitly listed in the task's Files Affected table — which this task does.
- Do not alter any existing step name, command, or position.
- Do not add the step to any job other than `rust-linux` and `rust-windows`.

**Acceptance criterion:** `grep -c 'Real-hardware compile check' .github/workflows/ci.yml` prints `2`.


---

### Group C — Config Correctness

#### P6-C1: anvilml-core: add serde snake_case to FrontendMode and DeviceType config enums

- **Prereqs:** P6-B2
- **Tags:** —

`FrontendMode` and `DeviceType` in `crates/anvilml-core/src/config.rs` have no `#[serde(...)]` attribute, so serde uses variant names verbatim: `Headless`, `Cuda`, `Rocm`, `Cpu`. The committed `anvilml.toml` uses lowercase values (`mode = "headless"`, `device_type = "cpu"`) because that is the documented and expected user-facing format — but the deserialiser rejects them, causing a panic at startup as seen in production. `ModelKind` and `DType` already use `#[serde(rename_all = "snake_case")]` consistently; this task brings `FrontendMode` and `DeviceType` into alignment.

Add `#[serde(rename_all = "snake_case")]` to both enums. Two existing tests in `crates/anvilml-core/src/types/hardware.rs` hardcode PascalCase JSON strings for `DeviceType` and must be updated:

- `device_type_json_strings`: change assertions to `"cuda"`, `"rocm"`, `"cpu"`.
- `gpu_device_backward_compat`: change the hardcoded JSON literal `"device_type": "Cuda"` to `"device_type": "cuda"`.

No other test changes are expected — all other `DeviceType` and `FrontendMode` usage goes through round-trip serialisation and will naturally produce and consume the new lowercase strings. Verify `anvilml.toml` already uses lowercase values throughout (it does — no TOML changes required). After the fix, `cargo run --bin anvilml -- --print-hardware` must complete without a config-load panic.

**Files to create or modify:**
- `crates/anvilml-core/src/config.rs` — add `#[serde(rename_all = "snake_case")]` to `FrontendMode` and `DeviceType`
- `crates/anvilml-core/src/types/hardware.rs` — update `device_type_json_strings` and `gpu_device_backward_compat` test assertions to lowercase

**Key implementation notes:**
- `FrontendMode` has struct variants (`Local { path }`, `Remote { url }`). `rename_all = "snake_case"` renames the tag strings only (`"Local"` → `"local"`, etc.); the field names within the variants are unaffected.
- `DeviceType` is also used in JSON API responses via `GpuDevice` in `anvilml-core/src/types/hardware.rs`. Changing its serialisation to lowercase is a **breaking API change** for any client consuming `GET /v1/hardware`. Since no external clients exist yet (pre-release), this is acceptable. Document this in the implementation report under `## Deviations from Plan`.
- The `config_reference` drift-guard test in `backend/tests/config_reference.rs` compares TOML key-sets only, not values — it will continue to pass without modification.

**Acceptance criterion:** `cargo test --workspace --features mock-hardware` exits 0 AND `cargo run --bin anvilml -- --print-hardware` exits 0 without a config-load panic.



## Runnable Proof

Create a model directory with a fake model file and confirm it appears via the API.

```bash
mkdir -p models/diffusion
dd if=/dev/zero bs=1M count=1 > models/diffusion/model-fp16.safetensors
cargo run --bin anvilml --features mock-hardware &
sleep 2
curl -s http://127.0.0.1:8488/v1/models | python3 -m json.tool
curl -s http://127.0.0.1:8488/v1/models?kind=diffusion | python3 -m json.tool
ID=$(curl -s http://127.0.0.1:8488/v1/models | python3 -c "import sys,json; print(json.load(sys.stdin)[0]['id'])")
curl -s http://127.0.0.1:8488/v1/models/$ID | python3 -m json.tool
curl -s -X POST http://127.0.0.1:8488/v1/models/rescan
# expected: 202
kill %1
# Additional: verify no-feature compile
cargo check --bin anvilml
cargo check --bin anvilml --target x86_64-pc-windows-gnu
```

Expected: `GET /v1/models` returns a JSON array containing the model with `kind: "diffusion"` and `dtype_hint: "F16"`. `GET /v1/models/:id` returns the same model. `POST /v1/models/rescan` returns 202. Both no-feature `cargo check` invocations exit 0. After P6-C1: `cargo run --bin anvilml -- --print-hardware` exits 0.

## Known Constraints and Gotchas

- The `mock-hardware` feature flag completely replaces the real-hardware detection branch at compile time. All `cargo` invocations in the automated flow use `--features mock-hardware`; the real-hardware `#[cfg(windows)]` and `#[cfg(unix)]` paths are only exercised by P6-B1 and the CI steps added in P6-B2.
- P6-B1 must be run on the Linux build machine where the `x86_64-pc-windows-gnu` target and `gcc-mingw-w64` linker are installed. The Windows-gnu cross-check is a local ACT gate; CI uses the native `windows-latest` runner for the same coverage.
- P6-B1 scope is compile-error fixes only — no feature gate changes, no new tests, no behavioural changes. If the no-feature checks surface errors outside `anvilml-hardware` (e.g. in `backend/src/main.rs`), fix them in the same task.
- P6-B2 modifies `.github/workflows/ci.yml`. Per `FORGE_AGENT_RULES §3.7` this is only permitted because the file is explicitly listed in that task's Files Affected table.
- P6-B1 and P6-B2 must run after P6-A7 to avoid disrupting the in-progress model registry implementation chain.
- P6-C1 changes `DeviceType` JSON serialisation from PascalCase (`"Cuda"`) to snake_case (`"cuda"`). This is a breaking change to the `GET /v1/hardware` response shape. It is acceptable pre-release but must be noted in the implementation report.
- P6-C1 must not change any `#[serde]` attributes on `EnumerationSource` or `CapabilitySource` — those are internal runtime types used only in JSON API responses and must retain their current PascalCase serialisation.