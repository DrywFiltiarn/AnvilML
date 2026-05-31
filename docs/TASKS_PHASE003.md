# Tasks: Phase 003 — Core Domain Types

| Field | Value |
|-------|-------|
| Phase | 003 |
| Name | Core Domain Types |
| Milestone group | Observable system state |
| Depends on phases | 1, 2 |
| Task file | `forge/tasks/tasks_phase003.json` |
| Tasks | 6 |

## Overview

Phase 3 implements the entire `anvilml-core` data model — error type, job/model/artifact/hardware/worker types, and the WebSocket event enum — and surfaces the first piece of it through a real endpoint, `GET /v1/system/env`, returning a (still stubbed) `EnvReport`. No I/O or async logic lives in core; it is pure, serializable, tested data. Surfacing `EnvReport` now proves the types serialize correctly over HTTP and gives later phases a stable contract.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P3-A1 | `crates/anvilml-core/src/error.rs` | anvilml-core: AnvilError enum and error model |
| P3-A2 | `crates/anvilml-core/src/types/job.rs` | anvilml-core: Job domain types |
| P3-A3 | `crates/anvilml-core/src/types/model.rs` | anvilml-core: Model and Artifact domain types |
| P3-A4 | `crates/anvilml-core/src/types/hardware.rs` | anvilml-core: Hardware and Worker domain types |
| P3-A5 | `crates/anvilml-core/src/types/events.rs` | anvilml-core: WebSocket event types |
| P3-A6 | `crates/anvilml-server/src/handlers/system.rs` | anvilml-server: /v1/system/env handler returning stub EnvReport |

## Task details

#### P3-A1: anvilml-core: AnvilError enum and error model

- **Prereqs:** P2-A5
- **Tags:** —

In anvilml-core add thiserror. Create src/error.rs: AnvilError enum variants ConfigLoad(String), Io(#[from] std::io::Error), Json(String), InvalidGraph(String), WorkerDead(String), JobNotFound(Uuid), ArtifactNotFound(String), DbError(String), PayloadTooLarge{size_mib:u32,limit_mib:u32}. thiserror #[error] messages each. Must be Send+Sync. Re-export from lib.rs. cargo test -p anvilml-core -- error exits 0.

#### P3-A2: anvilml-core: Job domain types

- **Prereqs:** P3-A1
- **Tags:** —

Add uuid (v4,serde), chrono (serde), utoipa to anvilml-core. Create src/types/job.rs: Job, JobStatus enum (Queued/Running/Completed/Failed/Cancelled, derive PartialEq Eq), JobSettings, SubmitJobRequest, SubmitJobResponse per ANVILML_DESIGN 4.1. Job.graph is serde_json::Value. All derive Serialize, Deserialize, Clone, Debug, utoipa::ToSchema. cargo test -p anvilml-core -- job exits 0.

#### P3-A3: anvilml-core: Model and Artifact domain types

- **Prereqs:** P3-A2
- **Tags:** —

Create src/types/model.rs: ModelMeta, ModelKind enum (Clip/Diffusion/Vae/Lora/ControlNet/Unet/Upscale), DType enum (F32/F16/BF16/Q8/Q4/Unknown). Create src/types/artifact.rs: ArtifactMeta per ANVILML_DESIGN 4.2. All derive the standard set incl utoipa::ToSchema. cargo test -p anvilml-core -- model exits 0.

#### P3-A4: anvilml-core: Hardware and Worker domain types

- **Prereqs:** P3-A3
- **Tags:** —

Create src/types/hardware.rs: HardwareInfo, GpuDevice, DeviceType enum (Cuda/Rocm/Cpu), HostInfo, InferenceCaps per 4.3. Create src/types/worker.rs: WorkerInfo, WorkerStatus enum (Initializing/Idle/Busy/Dead/Respawning). Create EnvReport struct {python_path, python_version, torch_version, preflight_ok:bool, reason:Option<String>} (used by 6.1). All derive standard set + ToSchema. cargo test -p anvilml-core -- hardware exits 0.

#### P3-A5: anvilml-core: WebSocket event types

- **Prereqs:** P3-A4
- **Tags:** reasoning

Create src/types/events.rs: WsEvent enum + variant structs per 4.5 (SystemStats, JobQueued, JobStarted, JobProgress, JobImageReady, JobCompleted, JobFailed, JobCancelled, WorkerStatusChanged). Each serializes as {event:'...', timestamp, ...fields}. Include GpuStatSnapshot{index,vram_used_mib,vram_total_mib}. JobProgress step/step_total are Option (None in MVP). cargo test -p anvilml-core -- events exits 0: assert SystemStats JSON has event='system.stats'.

#### P3-A6: anvilml-server: /v1/system/env handler returning stub EnvReport

- **Prereqs:** P3-A5
- **Tags:** —

Add anvilml-core dep to anvilml-server. Add env_report: Arc<RwLock<EnvReport>> to AppState (init with stub: python_path='', preflight_ok=false, reason='not_checked'). Create handlers/system.rs with async fn get_env(State) -> Json<EnvReport>. Wire GET /v1/system/env into build_router. Update main.rs AppState construction. Verify: curl http://127.0.0.1:8488/v1/system/env returns 200 with the stub EnvReport JSON.


## Runnable Proof

Confirm the env endpoint returns a well-formed `EnvReport`.

```bash
cargo run
curl -s http://127.0.0.1:8488/v1/system/env | python -m json.tool
```

Expected (200): a JSON object with keys `python_path`, `python_version`, `torch_version`, `preflight_ok` (false at this stage), and `reason` ("not_checked"). The values are stubs — phase 18 fills them with real preflight results — but the shape is the final contract. Phase done when the endpoint returns 200 with all five fields and `cargo test -p anvilml-core` is green.
