# AnvilML Backend — Functional & Technical Design

**Document:** `ANVILML_DESIGN.md`
**Revision:** 3 (expanded build-complete specification)
**Project:** SindriStudio / AnvilML
**Status:** Draft for review — supersedes Revision 2

---

## Revision History

| Rev | Summary |
| :-- | :------ |
| 1 | Initial architecture sketch. |
| 2 | Approved architecture: crate decomposition, domain types, IPC, scheduler, server, worker outline. |
| 3 | **This document.** Expands Rev 2 into a build-complete functional + technical design: per-crate module APIs, node IO contract, model cache, cancellation, logging, testing, build/toolchain, operations runbook, and implementation roadmap. Two intentional additions to the Rev 2 IPC schema (`CancelJob` message, `Cancelled` event) are introduced in §7 and flagged inline. Revision 2 remains the architectural authority; where this document adds detail it does not contradict it. A cross-platform pass (§1.5, §22.4) makes Linux and Windows co-equal first-class targets. The backend binary and database are named `anvilml` / `anvilml.db`, and SindriStudio is clarified throughout as the separate one-click launcher that starts AnvilML and BloomeryUI (Rev 2 conflated the two). |

This document is now the **single source of truth** for the AnvilML backend. Any earlier task lists or contract documents (`tasks.json`, `API_CONTRACT.md`, `IPC_PROTOCOL.md`, `ENVIRONMENT.md`, `TESTING_STRATEGY.md`) are non-authoritative and are superseded by the sections below.

---

## Table of Contents

1. [Purpose and Scope](#1-purpose-and-scope)
2. [Crate Decomposition](#2-crate-decomposition)
3. [Configuration](#3-configuration-anvilml-core)
4. [Domain Types](#4-domain-types-anvilml-core)
5. [Hardware Detection](#5-hardware-detection-anvilml-hardware)
6. [Python Environment](#6-python-environment-venv_path)
7. [IPC Protocol](#7-ipc-protocol-anvilml-ipc)
8. [Worker Management](#8-worker-management-anvilml-worker)
9. [Scheduler](#9-scheduler-anvilml-scheduler)
10. [HTTP & WebSocket Server](#10-http--websocket-server-anvilml-server)
11. [Frontend Serving](#11-frontend-serving-anvilml-server)
12. [Artifact Storage](#12-artifact-storage-anvilml-server)
13. [SQLite Schema](#13-sqlite-schema)
14. [Python Worker](#14-python-worker)
15. [Frontend Architecture Contract](#15-frontend-architecture-contract)
16. [Launcher Binary](#16-launcher-binary-backendsrcmainrs)
17. [OpenAPI Generation](#17-openapi-generation-anvilml-openapi)
18. [Error Model](#18-error-model)
19. [Logging & Observability](#19-logging--observability)
20. [Testing Strategy](#20-testing-strategy)
21. [Build, Toolchain & Release](#21-build-toolchain--release)
22. [Operations & Runbook](#22-operations--runbook)
23. [Implementation Roadmap](#23-implementation-roadmap)
24. [Conventions & Glossary](#24-conventions--glossary)
25. [Open Items & Deferred Scope](#25-open-items--deferred-scope)

---

## 1. Purpose and Scope

AnvilML is the headless backend inference engine of the **SindriStudio** project. SindriStudio itself is a separate one-click launcher executable that starts two components together — this AnvilML backend and the BloomeryUI frontend. **This document specifies AnvilML only.**

AnvilML is a standalone Rust binary (`anvilml`) that:

- Supervises one Python worker process per GPU device (one CPU worker if no GPUs are present).
- Exposes a versioned REST + WebSocket API as the sole integration surface.
- Manages job scheduling, the model registry, artifact storage, and all system state.
- Optionally serves a frontend — local directory, reverse-proxy to a remote URL, or headless.
- Never embeds frontend assets; BloomeryUI is distributed separately.

The frontend is architecturally interchangeable. BloomeryUI is the reference implementation; third-party frontends are supported by contract, not by coupling.

### 1.1 In Scope (this document)

The Rust backend (all crates), the Python inference worker, the IPC contract between them, the build/toolchain, the test strategy, and the run/operations story — i.e. everything required to go from an empty repository to a running backend that can accept a job and return a generated image.

### 1.2 Out of Scope (referenced, not specified here)

- **BloomeryUI internals** — its own component tree, state stores, and styling live in BloomeryUI's repository and design doc. Only the API/WebSocket contract it must honour (§15) is normative here.
- **SindriStudio — the one-click launcher** that starts and supervises AnvilML and BloomeryUI together, plus any installer, auto-update, or system-tray shell around them (e.g. a Tauri wrapper). AnvilML is designed to run headless and to be launched as a child process of SindriStudio, but SindriStudio is a separate deliverable in its own repository.
- **The Python ML model weights themselves.** AnvilML loads user-supplied model files; it does not ship or train them.

### 1.3 MVP Capability Target

A "functional backend" for the purposes of this document means: the server starts, detects hardware, supervises a healthy worker, accepts a text-to-image job for the **ZiT** or **SDXL** pipeline via `POST /v1/jobs`, streams progress over WebSocket, stores the resulting PNG as a content-addressed artifact, and serves it via REST — surviving worker crashes without a restart.

### 1.4 Non-Goals (MVP)

- Distributed / multi-host execution. AnvilML is single-host.
- Authentication / multi-tenant isolation (see §25; localhost bind is the security boundary).
- Sub-graph chunking, per-step progress, and latent preview (see §25).
- Intel (IPEX), Apple MPS, and AMD DirectML backends (see §5, §25). MVP targets **CUDA, ROCm, CPU**.

### 1.5 Supported Platforms

The backend is a first-class citizen on **Linux and Windows**; macOS runs CPU-only in MVP (MPS deferred). The binary, all crates, and the Python worker are cross-platform; the OS-sensitive points are itemised in §22.4 and handled inline at each subsystem.

| Capability | Linux (x86_64) | Windows (x86_64) | macOS | Notes |
| :-- | :-- | :-- | :-- | :-- |
| Core server (REST/WS/DB/scheduler) | ✓ | ✓ | ✓ | Pure cross-platform Rust. |
| NVIDIA / CUDA worker | ✓ | ✓ | ✗ | `nvidia-smi` on PATH on both OSes. |
| AMD / ROCm worker | ✓ | ✗ (→ CPU) | ✗ | ROCm tooling is Linux-only (§5); DirectML deferred (§25). |
| CPU worker | ✓ | ✓ | ✓ | Always available fallback. |
| IPC over stdio | ✓ | ✓ | ✓ | Windows requires binary-mode stdio (§7.1). |
| venv provisioning script | `.sh` | `.ps1` | `.sh` | §21.4; Windows uses `py -3.12`. |
| Graceful shutdown | `SIGINT`/`SIGTERM` | Ctrl-C / `ctrl_close` / `ctrl_shutdown` | as Unix | §16.3. |
| Orphan-worker cleanup on hard kill | `PR_SET_PDEATHSIG` | Job Object | best-effort | §22.4. |

---

## 2. Crate Decomposition

```
anvilml/
  Cargo.toml                    (workspace root)
  rust-toolchain.toml           (pinned toolchain channel)
  anvilml.toml                  (default config, checked in for reference)
  backend/
    src/main.rs                 (anvilml launcher binary)
    openapi.json                (generated; committed)
    migrations/                 (sqlx migration SQL files)
    scripts/
      install_worker_deps.sh    (Linux/macOS venv provisioning)
      install_worker_deps.ps1   (Windows venv provisioning)
      test_inference.py         (standalone debug harness, no IPC)
    tests/                      (integration tests: api_*.rs, ipc_*.rs)
  crates/
    anvilml-core/               (domain types, config, error types)
    anvilml-hardware/           (GPU + host detection)
    anvilml-registry/           (model scanner + SQLite persistence)
    anvilml-ipc/                (IPC message types + msgpack framing)
    anvilml-worker/             (WorkerPool: spawn, supervise, respawn)
    anvilml-scheduler/          (JobQueue, VramLedger, DAG, dispatch)
    anvilml-server/             (axum HTTP/WS server, all handlers)
    anvilml-openapi/            (build-time binary: generates openapi.json)
  worker/
    worker_main.py              (entry point invoked by Rust)
    ipc.py                      (stdin/stdout framing + msgpack)
    executor.py                 (graph topo-sort + node execution loop)
    pipeline_cache.py           (in-worker LRU model/pipeline cache)
    defaults.py                 (centralised, tunable per-model defaults)
    nodes/
      __init__.py               (NODE_REGISTRY + auto-import)
      base.py                   (BaseNode ABC, @register)
      common.py                 (SaveImage and shared nodes)
      zit.py                    (ZiT pipeline nodes)
      sdxl.py                   (SDXL pipeline nodes)
    requirements/
      base.txt                  (framework deps: diffusers, transformers, pillow, msgpack, etc.)
      cuda.txt                  (torch + CUDA index)
      rocm.txt                  (torch + ROCm index)
      cpu.txt                   (torch CPU-only)
    tests/                      (pytest: test_executor.py, test_nodes_*.py)
```

### 2.1 Crate Dependency Graph

```
anvilml-core
  ├── anvilml-hardware        (← core)
  ├── anvilml-registry        (← core)
  ├── anvilml-ipc             (← core)
  └── anvilml-worker          (← ipc, hardware, core)
        └── anvilml-scheduler (← worker, registry, core)
              └── anvilml-server (← all above)
                    └── backend/main.rs
```

`anvilml-openapi` depends on `anvilml-core` and `anvilml-server` for schema derivation but is a build-time dev binary only; it is never linked into the release binary.

### 2.2 Crate Responsibilities & Module Layout

| Crate | Responsibility | Key modules |
| :-- | :-- | :-- |
| `anvilml-core` | Pure data: domain types, config, errors. No I/O, no async runtime. | `config.rs`, `error.rs`, `types/{job,model,hardware,worker,events}.rs` |
| `anvilml-hardware` | Detect GPUs and host; refreshable VRAM snapshot. | `lib.rs` (`DeviceDetector` trait, `detect_all_devices`), `cuda.rs`, `rocm.rs`, `cpu.rs`, `mock.rs` |
| `anvilml-registry` | Scan model dirs, persist `ModelMeta` to SQLite, serve queries. | `scanner.rs`, `store.rs`, `lib.rs` |
| `anvilml-ipc` | IPC message enums + length-prefixed msgpack framing. | `messages.rs`, `framing.rs` |
| `anvilml-worker` | Spawn/supervise/respawn workers, IPC bridge, env injection. | `pool.rs`, `managed.rs`, `ipc_bridge.rs`, `env.rs` |
| `anvilml-scheduler` | Job queue, VRAM ledger, DAG validation, dispatch loop. | `queue.rs`, `ledger.rs`, `dag.rs`, `scheduler.rs` |
| `anvilml-server` | axum router, handlers, WS broadcaster, artifact store, frontend serving. | `lib.rs`, `state.rs`, `handlers/*.rs`, `ws/*.rs`, `artifact/store.rs`, `frontend.rs` |
| `anvilml-openapi` | Emit `openapi.json` from `utoipa` annotations. | `main.rs` |

### 2.3 Cargo Feature Flags

| Feature | Crates | Effect |
| :-- | :-- | :-- |
| `mock-hardware` | `anvilml-hardware` (re-exported by `worker`, `scheduler`, `server`) | Replaces real detection with deterministic stub devices driven by env vars. **All CI tests build with this.** |

The Python-side mock is a separate mechanism (`ANVILML_WORKER_MOCK=1`, §14.5) and is orthogonal to the Rust `mock-hardware` feature.

---

## 3. Configuration (`anvilml-core`)

Configuration loads from `anvilml.toml` at startup. Every field is overridable by an `ANVILML_*` environment variable (§3.3). The config file path is overridable via `--config <path>`. Resolution precedence, lowest to highest: **built-in defaults → `anvilml.toml` → environment variables → CLI flags**.

### 3.1 Config Types

```rust
pub struct ServerConfig {
    pub host: IpAddr,                        // default: 127.0.0.1
    pub port: u16,                           // default: 8488
    pub model_dirs: Vec<ModelDirConfig>,
    pub artifact_dir: PathBuf,               // default: ./artifacts
    pub db_path: PathBuf,                    // default: ./anvilml.db
    pub venv_path: PathBuf,                  // default: ./venv  (user-managed)
    pub rocm: RocmConfig,
    pub hardware_override: Option<HardwareOverrideConfig>,
    pub worker_log_dir: Option<PathBuf>,     // default: ./logs
    pub num_threads: usize,                  // default: 14
    pub num_interop_threads: usize,          // default: 4
    pub frontend: FrontendConfig,
    pub gpu_selection: GpuSelectionConfig,
    pub limits: LimitsConfig,
}

pub struct ModelDirConfig {
    pub path: PathBuf,
    pub kind: Option<ModelKind>,   // None = infer kind from subdirectory / filename
}

pub struct RocmConfig {
    pub use_hipblaslt: bool,                 // default: true  -> ROCBLAS_USE_HIPBLASLT=1
    pub hsa_override_gfx_version: Option<String>, // e.g. "10.3.0" for unsupported gfx
}

pub struct HardwareOverrideConfig {
    pub device_type: DeviceType,
    pub vram_total_mib: u32,
}

pub struct FrontendConfig { pub mode: FrontendMode }

pub enum FrontendMode {
    /// Serve static files from a local directory (default: ./bloomery adjacent to the binary).
    Local { path: PathBuf },
    /// Reverse-proxy non-API requests to a remote frontend dev server / host.
    Remote { url: Url },
    /// Serve no frontend; API-only.
    Headless,
}

pub struct GpuSelectionConfig {
    /// "auto" = scheduler fitness algorithm; "cpu" = force CPU worker; else a device index.
    pub default_device: String,              // default: "auto"
}

pub struct LimitsConfig {
    pub max_ipc_payload_mib: u32,            // default: 64
    pub list_default_limit: u32,             // default: 100
    pub list_max_limit: u32,                 // default: 1000
    pub ws_broadcast_capacity: usize,        // default: 256
}
```

### 3.2 `anvilml.toml` Reference

```toml
host = "127.0.0.1"
port = 8488
artifact_dir = "./artifacts"
db_path = "./anvilml.db"
venv_path = "./venv"
worker_log_dir = "./logs"
num_threads = 14
num_interop_threads = 4

[[model_dirs]]
path = "./models/diffusion"
kind = "diffusion"

[[model_dirs]]
path = "./models/vae"
kind = "vae"

[rocm]
use_hipblaslt = true
# hsa_override_gfx_version = "10.3.0"

[frontend]
mode = "local"          # local | remote | headless
# path = "./bloomery"   # for local
# url  = "http://localhost:5173"  # for remote

[gpu_selection]
default_device = "auto"  # auto | cpu | <index>

[limits]
max_ipc_payload_mib = 64
list_default_limit = 100
list_max_limit = 1000
ws_broadcast_capacity = 256
```

### 3.3 Environment Variable Reference

`ANVILML_*` variables override the matching config field. Nested fields use a double underscore. Workers additionally receive the variables in §6/§8.

| Variable | Overrides / Purpose | Default |
| :-- | :-- | :-- |
| `ANVILML_HOST` | `host` | `127.0.0.1` |
| `ANVILML_PORT` | `port` | `8488` |
| `ANVILML_DB_PATH` | `db_path` | `./anvilml.db` |
| `ANVILML_ARTIFACT_DIR` | `artifact_dir` | `./artifacts` |
| `ANVILML_VENV_PATH` | `venv_path` | `./venv` |
| `ANVILML_WORKER_LOG_DIR` | `worker_log_dir` | `./logs` |
| `ANVILML_NUM_THREADS` | `num_threads`; passed to worker | `14` |
| `ANVILML_NUM_INTEROP_THREADS` | `num_interop_threads`; passed to worker | `4` |
| `ANVILML_FRONTEND__MODE` | `frontend.mode` | `local` |
| `ANVILML_GPU_SELECTION__DEFAULT_DEVICE` | `gpu_selection.default_device` | `auto` |
| `ANVILML_LOG` / `RUST_LOG` | tracing filter (§19) | `info` |
| `ANVILML_WORKER_MOCK` | Python worker stub mode (§14.5) | unset |
| `ANVILML_WORKER_ID` | injected per worker | `worker-{index}` |
| `ANVILML_DEVICE_INDEX` | injected per worker | device index |
| `ANVILML_MOCK_DEVICE_TYPE` | `mock-hardware` device type | `cpu` |
| `ANVILML_MOCK_VRAM_MIB` | `mock-hardware` VRAM | `8192` |
| `ANVILML_MOCK_GFX_ARCH` | `mock-hardware` gfx string | `gfx1100` |

Per-worker hardware env (`CUDA_VISIBLE_DEVICES`, `HIP_VISIBLE_DEVICES`, `ROCBLAS_USE_HIPBLASLT`, `HSA_OVERRIDE_GFX_VERSION`, `OMP_NUM_THREADS`, `MKL_NUM_THREADS`, `OPENBLAS_NUM_THREADS`, `VECLIB_MAXIMUM_THREADS`) is built by `anvilml-worker::env::build_worker_env` (§8.3) and injected only into the child process environment.

---

## 4. Domain Types (`anvilml-core`)

All types derive `serde::Serialize`, `serde::Deserialize`, `Clone`, `Debug`, and `utoipa::ToSchema`. All sizes are **MiB** unless suffixed otherwise. All timestamps are `DateTime<Utc>` serialized as ISO 8601.

### 4.1 Job Types

```rust
pub struct Job {
    pub id: Uuid,
    pub status: JobStatus,
    pub graph: serde_json::Value,        // the validated graph
    pub settings: JobSettings,
    pub device_index: Option<u32>,       // assigned GPU (None until dispatched)
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub worker_id: Option<String>,
    pub artifact_count: u32,
    pub error: Option<String>,
}

pub enum JobStatus { Queued, Running, Completed, Failed, Cancelled }

pub struct JobSettings {
    pub seed: i64,                       // -1 = random, resolved by worker at exec time
    pub steps: u32,
    pub guidance_scale: f32,
    pub width: u32,
    pub height: u32,
    pub device_preference: Option<u32>,  // user-requested device; None = auto
}

pub struct SubmitJobRequest { pub graph: serde_json::Value, pub settings: JobSettings }
pub struct SubmitJobResponse { pub job_id: Uuid, pub queue_position: u32 }
```

**Authoritative-parameter rule (Decision 2).** At execution time the **graph nodes are authoritative** for all generation parameters. `JobSettings` exists to (a) drive scheduling (`device_preference`) and (b) record the parameters the frontend used to *build* the graph, for display and reproducibility. The reference frontend populates node `inputs` from `JobSettings`; the worker reads parameters from the graph nodes, never from `settings`. The single exception is the meaning of `seed = -1` (random), which the sampler node resolves at execution and reports back as the actual seed via `ArtifactMeta.seed`. The redundancy between `settings` and node inputs is intentional for MVP and flagged for simplification in §25.

### 4.2 Model & Artifact Types

```rust
pub struct ModelMeta {
    pub id: String,                 // first 16 hex chars of SHA256(canonical_path)
    pub name: String,
    pub path: PathBuf,
    pub kind: ModelKind,
    pub size_bytes: u64,
    pub dtype_hint: DType,
    pub vram_estimate_mib: u32,
    pub scanned_at: DateTime<Utc>,
}

pub enum ModelKind { Clip, Diffusion, Vae, Lora, ControlNet, Unet, Upscale }
pub enum DType { F32, F16, BF16, Q8, Q4, Unknown }

pub struct ArtifactMeta {
    pub hash: String,               // SHA256 hex of PNG bytes (content-addressed)
    pub job_id: Uuid,
    pub width: u32,
    pub height: u32,
    pub format: String,             // always "png"
    pub seed: i64,                  // actual seed used (resolved from -1)
    pub steps: u32,
    pub prompt: String,
    pub created_at: DateTime<Utc>,
}
```

`vram_estimate_mib` heuristic: `size_bytes` scaled by a dtype factor (load + working-set overhead), clamped to a sane floor. Used only as an advisory input to the pipeline cache and the VRAM ledger; never a hard gate.

### 4.3 Hardware Types

```rust
pub struct HardwareInfo { pub host: HostInfo, pub gpus: Vec<GpuDevice>, pub inference_caps: InferenceCaps }

pub struct GpuDevice {
    pub index: u32,
    pub name: String,
    pub device_type: DeviceType,
    pub vram_total_mib: u32,
    pub vram_free_mib: u32,
    pub driver_version: String,
}

pub enum DeviceType { Cuda, Rocm, Cpu }   // MVP set; see §25 for deferred backends

pub struct HostInfo { pub os: String, pub cpu_model: String, pub ram_total_mib: u64, pub ram_free_mib: u64 }
pub struct InferenceCaps { pub fp16: bool, pub bf16: bool, pub flash_attention: bool }
```

### 4.4 Worker Types

```rust
pub struct WorkerInfo {
    pub worker_id: String,          // "worker-{device_index}"
    pub device_index: u32,
    pub device_name: String,
    pub status: WorkerStatus,
    pub current_job_id: Option<Uuid>,
    pub vram_used_mib: u32,
}

pub enum WorkerStatus { Initializing, Idle, Busy, Dead, Respawning }
```

### 4.5 WebSocket Event Types

All events serialize as `{ "event": "<type>", "timestamp": "<iso8601>", ...fields }`.

```rust
pub enum WsEvent {
    SystemStats(SystemStatsEvent),              // "system.stats"
    JobQueued(JobQueuedEvent),                  // "job.queued"
    JobStarted(JobStartedEvent),                // "job.started"
    JobProgress(JobProgressEvent),              // "job.progress"
    JobImageReady(JobImageReadyEvent),          // "job.image_ready"
    JobCompleted(JobCompletedEvent),            // "job.completed"
    JobFailed(JobFailedEvent),                  // "job.failed"
    JobCancelled(JobCancelledEvent),            // "job.cancelled"
    WorkerStatusChanged(WorkerStatusChangedEvent), // "worker.status"
}

pub struct SystemStatsEvent {
    pub event: &'static str,        // "system.stats"
    pub timestamp: DateTime<Utc>,
    pub gpus: Vec<GpuStatSnapshot>, // { index, vram_used_mib, vram_total_mib }
    pub ram_used_mib: u64,
    pub ram_total_mib: u64,
}

pub struct JobProgressEvent {
    pub event: &'static str,        // "job.progress"
    pub timestamp: DateTime<Utc>,
    pub job_id: Uuid,
    pub node_index: u32,
    pub node_total: u32,
    pub node_type: String,
    pub step: Option<u32>,          // reserved for per-step progress (always None in MVP)
    pub step_total: Option<u32>,    // reserved
}

pub struct JobImageReadyEvent {
    pub event: &'static str,        // "job.image_ready"
    pub timestamp: DateTime<Utc>,
    pub job_id: Uuid,
    pub artifact_hash: String,      // fetch via GET /v1/artifacts/:hash
    pub width: u32,
    pub height: u32,
    pub seed: i64,
}
```

> Raw image bytes are **never** sent over WebSocket. Clients receive the artifact hash and fetch the PNG via REST. `JobFailed` carries `{ job_id, error, traceback? }`; `JobCancelled` carries `{ job_id }`; `WorkerStatusChanged` carries `{ worker_id, status }`.

---

## 5. Hardware Detection (`anvilml-hardware`)

Probed once at startup; `vram_free_mib` is refreshed every 5 s from worker `MemoryReport` events (§7) rather than re-probed.

```rust
#[async_trait]
pub trait DeviceDetector {
    async fn detect(&self) -> anyhow::Result<Vec<GpuDevice>>;
}

pub async fn detect_all_devices(config: &ServerConfig) -> Result<HardwareInfo>;
```

| Device | Detection method |
| :-- | :-- |
| NVIDIA / CUDA | `nvidia-smi --query-gpu=index,name,memory.total,memory.free,driver_version --format=csv,noheader,nounits` |
| AMD / ROCm | `rocm-smi --showid --showproductname --showmeminfo vram --json`; gfx arch via `rocminfo`; ReBAR hint via `lspci` |
| CPU | `sysinfo` crate — always succeeds |

**Rules.**

- Per-device detection failures are logged at `warn` and skipped; they never abort startup.
- If `hardware_override` is set, detection is bypassed entirely and a single synthetic device of the given type/VRAM is returned (used on machines where detection is unreliable).
- If no GPU is detected, exactly one CPU worker is provisioned.
- `inference_caps` is derived from device type and driver: CUDA → fp16/bf16/flash-attention probed; ROCm → fp16/bf16, flash-attention gated on gfx arch; CPU → all false.
- The `mock-hardware` feature swaps in `MockHardwareDetector`, which reads `ANVILML_MOCK_DEVICE_TYPE`, `ANVILML_MOCK_VRAM_MIB`, `ANVILML_MOCK_GFX_ARCH`. This is the only detector compiled in CI.

Intel (IPEX), Apple MPS, and AMD DirectML are **deferred** (§25). The `DeviceType` enum is deliberately limited to the three MVP variants so the rest of the system cannot reference a backend that does not yet exist.

**Platform note.** CUDA detection (`nvidia-smi`) and the CPU detector work identically on Linux and Windows. ROCm is an MVP target on **Linux only**; `rocm-smi`/`rocminfo` are absent on Windows and `lspci` (the ReBAR hint) is Linux-only, so the ROCm detector returns `Ok(vec![])` on Windows by the normal graceful-degradation path. Consequently, an AMD GPU on Windows falls back to a CPU worker in the MVP (AMD-on-Windows via DirectML is deferred, §25).

---

## 6. Python Environment (`venv_path`)

AnvilML **does not manage** the Python virtual environment; it only consumes it. This keeps the heavy, hardware-specific ML stack under explicit user control and out of the Rust build.

The `venv_path` config field (default `./venv`, relative to the config file) points to a user-managed venv. The launcher resolves the interpreter as:

- Linux/macOS: `{venv_path}/bin/python3`
- Windows: `{venv_path}\Scripts\python.exe`

Provisioning is done once by the user via the checked-in scripts (`backend/scripts/install_worker_deps.sh` / `.ps1`), which detect CUDA/ROCm/CPU and `pip install` the matching `worker/requirements/{cuda,rocm,cpu}.txt` on top of `base.txt`. See §21.4.

### 6.1 Preflight Check

At startup, before spawning workers:

1. Verify the resolved interpreter exists and is executable. If not → all workers `Dead` with reason `python_missing`; server still starts; `POST /v1/jobs` returns `503` (`workers_unavailable`).
2. Run `python --version`; warn (do not abort) if not `Python 3.12.x`.
3. Run `python -c "import torch; print(torch.__version__)"` with `ANVILML_WORKER_MOCK` unset. On failure → workers `Dead` with reason `torch_unavailable`; server starts; job submission returns `503` until repaired.

`GET /v1/system/env` reports `EnvReport { python_path, python_version, torch_version, preflight_ok, reason }` so the frontend can surface environment health.

### 6.2 Repair Flow

The user edits the venv independently (install nightly torch, swap ROCm version, etc.). AnvilML re-runs preflight and re-detects on next startup, or on demand via `POST /v1/workers/:id/restart` (§8.5), which respawns the worker and re-runs `InitializeHardware`.

---

## 7. IPC Protocol (`anvilml-ipc`)

Communication uses the worker's **stdin/stdout** pipes. stderr is captured to `{worker_log_dir}/worker-{device_index}.log` (rotated at 10 MiB, 3 retained). Using the standard pipes (rather than TCP/UDS) avoids per-platform socket handling and port allocation; the worker is a pure child process.

### 7.1 Framing

```
[ 4 bytes big-endian u32: payload length N ] [ N bytes: msgpack payload ]
```

Maximum payload: `limits.max_ipc_payload_mib` (default 64 MiB). A frame exceeding the limit, or a deserialization failure, causes an immediate worker kill + respawn (it indicates a desynced stream). The reader enforces the cap **before** allocating the buffer.

**Windows binary-stdio requirement (critical).** Because frames are raw binary msgpack carried over stdout/stdin, the Python worker **must** put its standard streams into binary mode on Windows before the first frame, or the C runtime will translate every `0x0A` byte to `0x0D 0x0A` and corrupt the stream:

```python
# worker/ipc.py — at module import, before any read/write
import sys
if sys.platform == "win32":
    import msvcrt, os
    msvcrt.setmode(sys.stdin.fileno(),  os.O_BINARY)
    msvcrt.setmode(sys.stdout.fileno(), os.O_BINARY)
```

All worker I/O uses `sys.stdin.buffer` / `sys.stdout.buffer` (never the text wrappers), and `sys.stdout.buffer.flush()` is called after every frame. The Rust side reads/writes the child's pipe handles as raw bytes and performs no translation, so no Rust-side change is needed. On both platforms the framing reader must **read-fully** (loop until the 4-byte header and then the full N-byte payload are received), because pipe reads — especially on Windows — may return fewer bytes than requested.

### 7.2 Messages (Rust → Python)

```
Ping        { seq: u64 }
Shutdown    { }
InitializeHardware { device_str: String }     // "cuda", "cuda:0" semantics via env; "cpu"
Execute     { job_id: Uuid, graph: Value, settings: JobSettings, device_index: u32 }
CancelJob   { job_id: Uuid }                   // NEW in Rev 3 — cooperative cancel (§9.4)
MemoryQuery { }
```

> `CancelJob` is an addition over Rev 2. It instructs the worker to abort the named in-flight job at the next cooperative checkpoint (between nodes, and on the sampler per-step callback). It is best-effort and may take effect up to one node / one diffusion step later.

### 7.3 Events (Python → Rust)

```
Ready        { worker_id: String, device_index: u32, vram_total_mib: u32 }
Pong         { seq: u64 }
Dying        { reason: String }
MemoryReport { vram_used_mib: u32, ram_used_mib: u64 }
Progress     { job_id: Uuid, node_index: u32, node_total: u32, node_type: String,
               step: Option<u32>, step_total: Option<u32> }   // step fields None in MVP
ImageReady   { job_id: Uuid, image_b64: String, width: u32, height: u32,
               format: String, seed: i64, steps: u32, prompt: String }
Completed    { job_id: Uuid, elapsed_ms: u64 }
Failed       { job_id: Uuid, error: String, traceback: String }
Cancelled    { job_id: Uuid }                  // NEW in Rev 3 — clean ack of CancelJob
```

> `Cancelled` is an addition over Rev 2 so the scheduler can distinguish a user-cancelled job (terminal status `Cancelled`) from a genuine `Failed`. If a worker dies mid-cancel, the watchdog path (§7.5) still resolves the job as `Failed(server_restart)`.

`ImageReady` carries the PNG as base64 over the pipe (within the 64 MiB cap; a 1024×1024 PNG is well under it). Rust decodes, hashes, and persists it (§12); the b64 never reaches a client.

### 7.4 Happy Path (one image)

```
1. Rust → InitializeHardware { device_str }          (once, at worker start)
2. Python → Ready { vram_total_mib }                 → worker becomes Idle
3. Rust → Execute { job_id, graph, settings, dev }   → worker Busy, job Running
4. Python → Progress (per node) … → ImageReady → Completed
5. Rust persists artifact on ImageReady, sets job Completed on Completed, worker → Idle
```

### 7.5 Watchdog Path (fatal crash)

A segfault or native abort terminates the process, bypassing Python exception handling:

```
1. The stdout pipe closes → Rust reader observes EOF / broken pipe.
2. Worker → Dead; emit WsEvent::WorkerStatusChanged.
3. Any job in Running on that worker → Failed { error: "worker_crashed" }, DB updated, WS broadcast.
4. After 2 s → Respawning → spawn fresh process → InitializeHardware re-sent on next Ready.
5. The pipeline cache lived in the dead process; it is gone. No Rust-side cache references survive
   (Rust never holds tensor handles — see §14.2), so nothing needs invalidation.
6. Server stays online and accepts new jobs throughout.
```

---

## 8. Worker Management (`anvilml-worker`)

### 8.1 Lifecycle

```
Initializing → Idle → Busy → Idle
                 ↓           ↓
               Dead  ←───────┘   (pipe close | process exit | ping timeout)
                 ↓
          Respawning (2 s delay) → Initializing
```

`WorkerPool` holds one `ManagedWorker` per detected GPU device, plus one CPU worker if no GPUs exist. **Workers are never shared between concurrent jobs**; a worker runs exactly one job at a time. Therefore maximum job concurrency equals the number of workers (one per device).

### 8.2 Public API

```rust
impl WorkerPool {
    pub async fn spawn_all(hw: &HardwareInfo, cfg: &ServerConfig) -> Result<Self>;
    pub fn list(&self) -> Vec<WorkerInfo>;
    pub fn acquire_idle(&self, device_index: Option<u32>) -> Option<WorkerRef>;
    pub fn set_busy(&self, worker_id: &str, job_id: Uuid);
    pub fn set_idle(&self, worker_id: &str);
    pub fn subscribe_events(&self) -> broadcast::Receiver<(String, WorkerEvent)>;
    pub async fn send(&self, worker_id: &str, msg: WorkerMessage) -> Result<()>;
    pub async fn restart(&self, worker_id: &str) -> Result<()>;
    pub async fn shutdown_all(&self);
}
```

### 8.3 Spawn & Environment Injection

```
{venv_python} worker/worker_main.py --worker-id worker-{n} --device-index {n}
```

`env::build_worker_env(device, config) -> HashMap<String, String>`:

| DeviceType | Variables injected |
| :-- | :-- |
| CUDA | `CUDA_VISIBLE_DEVICES={n}` |
| ROCm | `HIP_VISIBLE_DEVICES={n}`, `ROCBLAS_USE_HIPBLASLT={0\|1}`, `HSA_OVERRIDE_GFX_VERSION` (if configured) |
| CPU | (no device-visibility var) |
| All | `OMP_NUM_THREADS`, `MKL_NUM_THREADS`, `OPENBLAS_NUM_THREADS`, `VECLIB_MAXIMUM_THREADS`, `ANVILML_NUM_THREADS`, `ANVILML_NUM_INTEROP_THREADS`, `ANVILML_WORKER_ID`, `ANVILML_DEVICE_INDEX`, and `ANVILML_WORKER_MOCK` if set on the server |

Thread vars are set in the child environment *and* re-applied inside `worker_main.py` before `import torch`, because torch resets some of them on import.

### 8.4 IPC Bridge (`ipc_bridge.rs`)

Two async tasks per worker, owned by `ManagedWorker`:

- **stdin writer**: `mpsc::Receiver<WorkerMessage>` → serialize (framed msgpack) → child stdin.
- **stdout reader**: framed bytes from child stdout → deserialize → `broadcast::Sender<(worker_id, WorkerEvent)>`.

On pipe close: transition `Dead`, emit `WorkerStatusChanged`, schedule respawn after 2 s.

### 8.5 Keepalive & Manual Restart

- `Ping { seq }` every 30 s. If the matching `Pong { seq }` is not received within 10 s → force-kill the child via `tokio::process::Child::kill()` (`SIGKILL` on Unix, `TerminateProcess` on Windows); the `Dead → Respawning` path handles recovery.
- `POST /v1/workers/:id/restart` → send `Shutdown`, wait up to 5 s for `Dying`, then force-kill and respawn. Returns `202`. Used after the user repairs the venv.

---

## 9. Scheduler (`anvilml-scheduler`)

### 9.1 JobQueue

In-memory `VecDeque<Job>` behind a `tokio::sync::Mutex`. Jobs are appended on submission, popped from the front on dispatch. Cancellation of a queued job marks it `Cancelled` in place (it is skipped, then removed, on the next dispatch pass). The queue is **not persisted**; on restart the startup sequence (§16.2) marks any DB job in `Running`/`Queued` as `Failed("server_restart")`.

**Job history lives in SQLite.** The in-memory queue holds only `Queued`/`Running` jobs. Every state transition writes through to the DB synchronously **before** broadcasting the WebSocket event, so a client that re-fetches via REST after a missed event always sees consistent state.

**Retention.** Job records are kept indefinitely while the app runs. Removal is user-driven: `DELETE /v1/jobs/:id` (single, terminal jobs only) and `DELETE /v1/jobs?status=…` (bulk). Running/queued jobs must be cancelled before deletion.

### 9.2 VramLedger

```rust
pub struct VramLedger { devices: HashMap<u32, (u32 /*total*/, u32 /*used*/)> }
impl VramLedger {
    pub fn update(&mut self, device_index: u32, used_mib: u32, total_mib: u32);
    pub fn free_mib(&self, device_index: u32) -> u32;
    pub fn would_fit(&self, device_index: u32, required_mib: u32) -> bool;
}
```

Updated on every `MemoryReport` (authoritative source of truth for free VRAM). Used for **dispatch admission ranking only** — never as a hard gate, because real OOM is handled by the worker (§14.4), not predicted by Rust.

### 9.3 GPU Selection — `select_worker(job, workers, ledger) -> Option<WorkerRef>`

1. **User-specified** (`settings.device_preference = Some(n)`): route to worker `n` if `Idle`; if `Busy`, hold in queue (do not re-route); if device `n` does not exist, the job was already rejected at submission with `422`.
2. **Auto** (`None`, or `gpu_selection.default_device = "auto"`): candidate set = all `Idle` workers; rank by `vram_free_mib` descending; ties broken by device index ascending; pick the top candidate.
3. **Force-CPU** (`gpu_selection.default_device = "cpu"`): only the CPU worker is eligible.

### 9.4 Cancellation (Decision 1)

`POST /v1/jobs/:id/cancel`:

- Job is **terminal** (`Completed`/`Failed`/`Cancelled`) → `409 job_not_cancellable`.
- Job is **Queued** → mark `Cancelled` in queue + DB, broadcast `JobCancelled`, return `202`. Never dispatched.
- Job is **Running** → send `CancelJob { job_id }` IPC to the owning worker, return `202`. The worker aborts cooperatively (between nodes, and via the sampler per-step callback) and replies `Cancelled { job_id }`, which the scheduler maps to status `Cancelled`. Cancellation latency is bounded by one node or one diffusion step.

A running CUDA kernel cannot be force-preempted from Python; cooperative checkpoints are the only safe mechanism. If the worker dies before acknowledging, the watchdog path resolves the job as `Failed`.

### 9.5 DAG Engine (`dag.rs`)

Validates the submitted graph before enqueue. Graph JSON:

```json
{
  "nodes": [
    { "id": "n0", "type": "ZitLoadPipeline", "inputs": { "model_id": "abc123def456" } },
    { "id": "n1", "type": "ZitTextEncode",
      "inputs": { "pipeline": { "node_id": "n0", "output_slot": "pipeline" }, "prompt": "a fox" } }
  ]
}
```

An input value is either a **literal** (string / number / bool / array) or an **edge reference** `{ "node_id": "...", "output_slot": "..." }`. Validation (errors collected and returned together, not fail-fast):

1. No duplicate node `id`.
2. Every edge `node_id` references an existing node, and that node declares the named `output_slot`.
3. Graph is acyclic (Kahn's algorithm); on a cycle, report `CycleDetected` naming the involved node IDs.
4. **Every `type` exists in the server's authoritative `KNOWN_NODE_TYPES` set (Decision 5).** Unknown type → `422 invalid_graph` listing the offending types.

A clean validation yields a `ValidatedGraph` newtype wrapping the original `Value`; only a `ValidatedGraph` can be enqueued.

`KNOWN_NODE_TYPES` (MVP): `ZitLoadPipeline`, `ZitTextEncode`, `ZitSampler`, `ZitDecode`, `SdxlLoadPipeline`, `SdxlTextEncode`, `SdxlSampler`, `SdxlDecode`, `SaveImage`. This set is a single constant shared by validation and is the same set the Python `NODE_REGISTRY` self-registers (§14.3); a mismatch is a build-time/test-time error (§20).

### 9.6 Dispatch Loop

Background `tokio` task started once via `start_dispatch_loop() -> JoinHandle`. Wakes on either a new job (`Notify`) or any worker transitioning to `Idle`. Per wake: iterate the queue front-to-back, call `select_worker` for each `Queued` job, dispatch the first match, and repeat until no further match exists in this cycle. The dispatch lock is `tokio::sync::Mutex` because it is held across `.await` points (sending to a worker).

---

## 10. HTTP & WebSocket Server (`anvilml-server`)

Framework: `axum` on `tokio`. **No TLS** — front with a reverse proxy for any remote exposure (MVP binds localhost).

### 10.1 AppState

```rust
pub struct AppState {
    pub config: Arc<ServerConfig>,
    pub scheduler: Arc<JobScheduler>,
    pub workers: Arc<WorkerPool>,
    pub registry: Arc<ModelRegistry>,
    pub hardware: Arc<RwLock<HardwareInfo>>,
    pub db: SqlitePool,
    pub broadcaster: Arc<EventBroadcaster>,
    pub artifact_store: Arc<ArtifactStore>,
    pub env_report: Arc<RwLock<EnvReport>>,
}
```

### 10.2 Middleware Stack (outermost first)

1. `TraceLayer` — structured request/response logging.
2. `SetRequestIdLayer` — injects `X-Request-Id`.
3. `CompressionLayer` — gzip/br for text responses.
4. `CorsLayer` — `Access-Control-Allow-Origin: *` (local-only default; configurable).

### 10.3 REST Routes

| Method | Path | Description | Success |
| :-- | :-- | :-- | :-- |
| GET | `/health` | Liveness probe | 200 `{ status, version, uptime_s }` |
| GET | `/v1/system` | Full hardware info | 200 `HardwareInfo` |
| GET | `/v1/system/env` | Python environment health | 200 `EnvReport` |
| POST | `/v1/jobs` | Submit job (validates graph) | 202 `SubmitJobResponse` |
| GET | `/v1/jobs` | List jobs (`?status=`, `?limit=`, `?before=`) | 200 `Vec<Job>` |
| GET | `/v1/jobs/:id` | Get one job | 200 `Job` |
| POST | `/v1/jobs/:id/cancel` | Cancel queued or running job (§9.4) | 202 |
| DELETE | `/v1/jobs/:id` | Remove a terminal job + its artifacts | 204 |
| DELETE | `/v1/jobs` | Bulk clear (`?status=completed\|failed\|cancelled\|all`) | 200 `{ removed: u32 }` |
| GET | `/v1/models` | List models (`?kind=`) | 200 `Vec<ModelMeta>` |
| GET | `/v1/models/:id` | Get one model | 200 `ModelMeta` |
| POST | `/v1/models/rescan` | Rescan model dirs | 202 |
| GET | `/v1/workers` | List workers + status | 200 `Vec<WorkerInfo>` |
| POST | `/v1/workers/:id/restart` | Restart a worker | 202 |
| GET | `/v1/artifacts` | List artifacts (`?job_id=`) | 200 `Vec<ArtifactMeta>` |
| GET | `/v1/artifacts/:hash` | Serve the PNG | 200 `image/png` |

All list endpoints honour `?limit=` (default 100, max 1000) and `?before=<iso8601>` cursor pagination.

### 10.4 WebSocket Route

| Path | Notes |
| :-- | :-- |
| `/v1/events` | WS upgrade; JSON text frames; ping/pong 30 s; per-subscriber broadcast capacity 256 |

Deltas only — **no history replay**. A subscriber that falls behind the 256-event buffer is disconnected with close code `1008`; the client reconnects and re-fetches state via REST (§15). 

```rust
pub struct EventBroadcaster { sender: broadcast::Sender<Arc<WsEvent>> } // Arc avoids per-subscriber clone
```

All state-change codepaths call `broadcaster.send(...)`; send errors (no subscribers) are ignored.

### 10.5 `system.stats` Tick

Background `tokio` task, interval 5 s: reads the latest `MemoryReport` per worker and host RAM via `sysinfo`, builds `SystemStatsEvent`, broadcasts it.

---

## 11. Frontend Serving (`anvilml-server`)

Determined at startup from `frontend.mode`. A single catch-all axum route is registered last (lowest priority, after all `/v1/*` and `/health`).

- **`Local { path }`** — `ServeDir` mounted at `/`; SPA fallback serves `{path}/index.html` (200) for unmatched paths. Default `path` is `./bloomery` adjacent to the binary. If the directory is missing, log a warning and serve a minimal inline HTML page explaining the situation; the API stays fully functional.
- **`Remote { url }`** — reverse-proxy all non-API, non-`/health` requests to `url` via a `hyper` client in a catch-all handler. Forward request headers; rewrite `Host`; stream the response back. For running BloomeryUI under a dev server (e.g. Vite) while AnvilML is the backend.
- **`Headless`** — no frontend route; pure API server; the launcher's browser-open step is skipped.

---

## 12. Artifact Storage (`anvilml-server`)

On `WorkerEvent::ImageReady`:

1. Decode `image_b64` → raw PNG bytes.
2. `hash = hex(SHA256(png_bytes))`.
3. Write `{artifact_dir}/{hash[0..2]}/{hash}.png` (two-char prefix sharding).
4. Insert `ArtifactMeta` into `artifacts`.
5. Increment `jobs.artifact_count`.
6. Broadcast `JobImageReady { artifact_hash, … }` — **no image data in the event**.

`GET /v1/artifacts/:hash` serves the file with `Content-Type: image/png`, `Cache-Control: public, immutable, max-age=31536000` (content-addressed → cache forever), and `ETag: "{hash}"`.

`DELETE /v1/jobs/:id` removes the job record and, for each artifact with that `job_id`, deletes the on-disk file then the DB row. Because artifacts are content-addressed, a future enhancement may refcount shared hashes; for MVP, hashes are not shared across jobs in practice (random seeds differ), so direct deletion is safe.

### 12.1 `ArtifactStore` API

```rust
impl ArtifactStore {
    pub async fn save(&self, job_id: Uuid, image_b64: &str, meta: ArtifactMetaInput) -> Result<ArtifactMeta>;
    pub async fn get_path(&self, hash: &str) -> Result<PathBuf>;
    pub async fn list(&self, job_id: Option<Uuid>, limit: u32, before: Option<DateTime<Utc>>) -> Result<Vec<ArtifactMeta>>;
    pub async fn delete_for_job(&self, job_id: Uuid) -> Result<u32>;
}
```

---

## 13. SQLite Schema

`sqlx` with the bundled SQLite driver. WAL mode enabled at pool init (`PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;`). Migrations run via `sqlx::migrate!` at startup, before any worker is spawned.

```sql
-- migrations/001_jobs.sql
CREATE TABLE IF NOT EXISTS jobs (
    id              TEXT PRIMARY KEY,
    status          TEXT NOT NULL,
    graph           TEXT NOT NULL,
    settings        TEXT NOT NULL,
    device_index    INTEGER,
    created_at      TEXT NOT NULL,
    started_at      TEXT,
    completed_at    TEXT,
    worker_id       TEXT,
    artifact_count  INTEGER NOT NULL DEFAULT 0,
    error           TEXT
);
CREATE INDEX IF NOT EXISTS idx_jobs_status     ON jobs(status);
CREATE INDEX IF NOT EXISTS idx_jobs_created_at ON jobs(created_at);

-- migrations/002_models.sql
CREATE TABLE IF NOT EXISTS models (
    id                TEXT PRIMARY KEY,
    name              TEXT NOT NULL,
    path              TEXT NOT NULL UNIQUE,
    kind              TEXT NOT NULL,
    size_bytes        INTEGER NOT NULL,
    dtype_hint        TEXT NOT NULL,
    vram_estimate_mib INTEGER NOT NULL,
    scanned_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_models_kind ON models(kind);

-- migrations/003_artifacts.sql
CREATE TABLE IF NOT EXISTS artifacts (
    hash        TEXT PRIMARY KEY,
    job_id      TEXT NOT NULL,
    width       INTEGER NOT NULL,
    height      INTEGER NOT NULL,
    format      TEXT NOT NULL DEFAULT 'png',
    seed        INTEGER NOT NULL,
    steps       INTEGER NOT NULL,
    prompt      TEXT NOT NULL,
    created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_artifacts_job_id ON artifacts(job_id);
```

All timestamps are ISO 8601 UTC strings; all JSON blobs (`graph`, `settings`) are TEXT. On startup, any job in `Running`/`Queued` is updated to `Failed` with `error = "server_restart"` before the scheduler initializes, preventing ghost jobs.

---

## 14. Python Worker

A long-lived child process; one per device. Single-threaded message loop on stdin; torch work runs on the main thread. The worker holds **all tensor state** (Decision: pass-by-value execution; Rust never holds tensor handles).

### 14.1 `worker_main.py` Startup

```
1.  Parse --worker-id, --device-index.
2.  Read ANVILML_NUM_THREADS, ANVILML_NUM_INTEROP_THREADS.
3.  Set OMP_NUM_THREADS, MKL_NUM_THREADS, OPENBLAS_NUM_THREADS, VECLIB_MAXIMUM_THREADS
    *** before any import that could pull in torch ***.
4.  If ANVILML_WORKER_MOCK=1: skip torch import entirely (§14.5).
5.  import torch
    torch.set_num_threads(N); torch.set_num_interop_threads(M)
    torch.backends.cuda.matmul.allow_tf32 = False
    torch.backends.cudnn.allow_tf32 = False
6.  Wait for InitializeHardware → resolve device string → send Ready { vram_total_mib }.
7.  Start a background MemoryReport thread (every 10 s).
8.  Enter the message loop (blocking read of framed msgpack on stdin).
```

### 14.2 `executor.py` — `run_graph(graph, settings, device_str, cancel_flag) -> None`

1. Kahn topological sort (local safety check; the server already validated, but the worker re-checks defensively).
2. For each node in order:
   - If `cancel_flag` is set → emit `Cancelled` and return.
   - Resolve inputs: literals pass through; edge refs resolve to `node_outputs[ref.node_id][ref.output_slot]`.
   - Look up `NODE_REGISTRY[node.type]`, instantiate, call `node.execute(**resolved_inputs) -> dict[str, Any]`.
   - Store the returned dict under `node_outputs[node.id]`.
   - Emit `Progress { node_index, node_total, node_type }`.
3. On completion → `Completed { elapsed_ms }`. On any exception → `Failed { error, traceback }` (full traceback). On cancel-flag during a sampler step → the step callback raises `CancelledError`, caught here → `Cancelled`.

The executor passes `cancel_flag` (and the IPC `emit` callback) into sampler nodes so they can check it inside the diffusion loop via `callback_on_step_end`.

### 14.3 `nodes/base.py` — Node Contract

```python
class BaseNode(ABC):
    NODE_TYPE: ClassVar[str]
    # Declared slots used by server-side validation parity tests (§20):
    INPUT_SLOTS: ClassVar[list[str]]
    OUTPUT_SLOTS: ClassVar[list[str]]

    def __init__(self, ctx: NodeContext): ...   # ctx: pipeline_cache, device, emit, cancel_flag
    @abstractmethod
    def execute(self, **inputs) -> dict[str, Any]: ...

NODE_REGISTRY: dict[str, type[BaseNode]] = {}
def register(cls): NODE_REGISTRY[cls.NODE_TYPE] = cls; return cls
```

Node modules self-register via the `@register` decorator on import; `nodes/__init__.py` imports every node module so the registry is populated at startup.

### 14.4 Pipeline Cache & OOM Handling (`pipeline_cache.py`)

**Cache (Decision 3).** An `OrderedDict` keyed by `(model_id, dtype)` → `{ pipeline, est_vram_mib }`, MRU at the end. `LoadPipeline` nodes call `cache.get_or_load(model_id, dtype, loader)`:

- Hit → move to MRU, return.
- Miss → while `free_vram < est_vram` and the cache is non-empty, evict the LRU entry (drop the pipeline, then `torch.cuda.empty_cache()` **once per eviction**), then load.

`empty_cache()` is called only on eviction and OOM recovery — never per node — so PyTorch's caching allocator keeps serving the active job efficiently.

**OOM trap.** Node execution is wrapped so a `torch.cuda.OutOfMemoryError` is caught, the partial state for the current node is dropped, `empty_cache()` is run once, and a `Failed { error: "cuda_oom", … }` event is emitted. The worker stays alive and returns to `Idle`. This handles GPU-allocator OOM only; host-RAM exhaustion is an OS-level condition outside Python's control and is not claimed to be recoverable.

### 14.5 Mock Mode

`ANVILML_WORKER_MOCK=1` makes every `execute()` return stub data: `LoadPipeline` returns a sentinel handle, samplers return zero latents, `Decode`/`SaveImage` return a black 1024×1024 PNG. No GPU, torch, or diffusers required. **This is the only mode used in CI.**

### 14.6 Node Set & Tunable Defaults (`defaults.py`)

Per-model defaults live in **one place** so they are trivial to adjust later:

```python
# worker/defaults.py  — single source of tunable generation defaults
ZIT_DEFAULTS  = ModelDefaults(steps=8,  guidance_scale=0.0, width=1024, height=1024, dtype="bf16")
SDXL_DEFAULTS = ModelDefaults(steps=20, guidance_scale=7.5, width=1024, height=1024, dtype="fp16",
                              supports_negative_prompt=True)
```

| Node | Inputs (slots) | Outputs (slots) | Notes |
| :-- | :-- | :-- | :-- |
| `ZitLoadPipeline` | `model_id` | `pipeline` | Distilled/turbo pipeline; CFG-free (`guidance_scale=0.0`). |
| `ZitTextEncode` | `pipeline`, `prompt` | `conditioning` | Single encoder. |
| `ZitSampler` | `pipeline`, `conditioning`, `steps`, `seed` | `latents`, `seed` | Resolves `seed=-1`→random; emits resolved `seed`. Default `steps=8`. |
| `ZitDecode` | `pipeline`, `latents` | `image` | VAE decode → PIL image. |
| `SdxlLoadPipeline` | `model_id` | `pipeline` | Dual text encoders. |
| `SdxlTextEncode` | `pipeline`, `prompt`, `negative_prompt?` | `conditioning` | Negative prompt optional. |
| `SdxlSampler` | `pipeline`, `conditioning`, `steps`, `guidance_scale`, `seed` | `latents`, `seed` | Default `steps=20`, `guidance_scale=7.5`. |
| `SdxlDecode` | `pipeline`, `latents` | `image` | VAE decode. |
| `SaveImage` (shared) | `image`, `prompt`, `seed`, `steps` | — | Encodes PNG, emits `ImageReady`. |

The Rust `KNOWN_NODE_TYPES` (§9.5) must equal the set of `NODE_TYPE` values registered here; a parity test (§20) fails the build on divergence.

---

## 15. Frontend Architecture Contract

Any frontend (BloomeryUI, third-party, custom) must conform to be AnvilML-compatible:

1. **API-only integration.** Communicate exclusively via `GET/POST/DELETE /v1/*` REST and `WebSocket /v1/events`. No filesystem, process-memory, or out-of-band assumptions.
2. **No assumed embedding.** Must work served from any origin, including a different host/port; CORS is handled by AnvilML.
3. **OpenAPI compliance.** Request/response shapes must conform to `openapi.json` (§17); do not depend on undocumented fields.
4. **WebSocket reconnect.** Implement exponential-backoff reconnection for `/v1/events`. On reconnect, re-fetch current state via REST before re-subscribing (the socket delivers deltas only; it does not replay history).
5. **Static build output.** A frontend served in `Local` mode ships as a compiled static directory (e.g. `dist/`) with an `index.html` entry point for SPA fallback.

The detailed BloomeryUI component/state design is owned by its own repository and is **out of scope** here.

---

## 16. Launcher Binary (`backend/src/main.rs`)

Thin binary `anvilml` wrapping `anvilml_server::start(config)`.

### 16.1 CLI

```
anvilml [--config <path>] [--host <ip>] [--port <u16>] [--no-browser]
```

Flags override config (highest precedence). `--help` prints usage.

### 16.2 Startup Sequence

```
1.  Parse CLI; load + merge config (defaults → toml → env → flags).
2.  Init tracing subscriber (§19).
3.  Open SqlitePool; set PRAGMAs; run sqlx migrations.
4.  Reset ghost jobs: UPDATE jobs SET status='Failed', error='server_restart'
    WHERE status IN ('Running','Queued').
5.  Detect hardware (§5) → HardwareInfo.
6.  Python preflight (§6.1) → EnvReport.
7.  Build registry; perform initial model scan (async; non-blocking for server bind).
8.  Spawn WorkerPool (one per device, or one CPU worker). Send InitializeHardware to each.
9.  Build AppState; start scheduler dispatch loop; start system.stats tick.
10. Bind axum server on host:port. (If preflight failed, server still binds; jobs 503.)
11. Unless --no-browser or Headless: open the default browser at http://host:port.
12. Await shutdown signal.
```

### 16.3 Graceful Shutdown

Triggered by a cross-platform shutdown future built from `tokio::signal::ctrl_c()` (handles Ctrl-C / `SIGINT` on both OSes) joined with the platform-specific sources: on Unix, `tokio::signal::unix` `SIGTERM`; on Windows, `tokio::signal::windows` `ctrl_close()` and `ctrl_shutdown()`. Whichever fires first initiates:

```
1. Stop accepting new submissions (POST /v1/jobs → 503).
2. Send Shutdown (IPC) to all workers.
3. Wait up to 10 s for each worker's Dying.
4. Force-kill any worker that has not exited via Child::kill()
   (SIGKILL on Unix, TerminateProcess on Windows).
5. Flush SQLite WAL (close the sqlx pool).
6. Exit 0.
```

---

## 17. OpenAPI Generation (`anvilml-openapi`)

A build-time dev binary (never linked into the release). Run in CI after the workspace builds clean. Produces `backend/openapi.json` (committed), which is:

- The normative spec for third-party frontends.
- Consumed by BloomeryUI's type generation to produce its TypeScript client types.

The spec includes all REST routes, request/response schemas, WebSocket event schemas (as component schemas — OpenAPI 3.1 does not natively describe WS frames), and the error response shape (§18). All `anvilml-core` types derive `utoipa::ToSchema`; all handlers carry `#[utoipa::path]` annotations. A CI check fails if the committed `openapi.json` differs from a freshly generated one.

---

## 18. Error Model

Uniform error body:

```json
{ "error": "snake_case_code", "message": "Human-readable description.", "request_id": "01J..." }
```

| Situation | HTTP | Error code |
| :-- | :-- | :-- |
| Unknown job/model/artifact id | 404 | `not_found` |
| Invalid graph (cycle, bad ref, unknown type, duplicate id) | 422 | `invalid_graph` |
| Cancel a terminal job | 409 | `job_not_cancellable` |
| Delete a running/queued job | 409 | `job_active` |
| Worker environment unhealthy / shutting down | 503 | `workers_unavailable` |
| Malformed request body / query | 400 | `bad_request` |
| Internal error | 500 | `internal_error` |

No HTML error pages; every 4xx/5xx is `application/json`. `request_id` echoes `X-Request-Id`.

---

## 19. Logging & Observability

### 19.1 Rust

- `tracing` + `tracing-subscriber` with an `EnvFilter` from `ANVILML_LOG` (falling back to `RUST_LOG`, default `info`).
- Human-readable formatter by default; a `--log-format json` flag (and `ANVILML_LOG_FORMAT=json`) selects structured JSON for log shipping.
- `TraceLayer` logs each request with method, path, status, latency, and `X-Request-Id`.
- Span fields of interest: `job_id`, `worker_id`, `device_index`.

### 19.2 Python Worker

- stderr of each worker is captured by Rust to `{worker_log_dir}/worker-{device_index}.log`, rotated at 10 MiB with 3 files retained.
- The worker logs node start/finish, cache hits/evictions, and exceptions (the same traceback sent in `Failed`).

### 19.3 Metrics & Health

- `GET /health` → `{ status, version, uptime_s }`.
- `system.stats` WS event every 5 s (VRAM/RAM/queue snapshot).
- **No log-retrieval API in MVP** (the discarded earlier specs proposed one; it is deferred — see §25). Logs are read from disk.

---

## 20. Testing Strategy

Three CI jobs, all hermetic (no real GPU, no model downloads):

### 20.1 Rust (`backend`)

```
cargo fmt --all --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test  --workspace --features mock-hardware
cargo run -p anvilml-openapi   # regenerate + diff openapi.json
```

- **Unit tests** per crate: framing round-trips + size-limit enforcement (`anvilml-ipc`); detector fixtures (`anvilml-hardware`); queue/ledger/DAG logic incl. cycle detection (`anvilml-scheduler`); scanner with tempdir fixtures (`anvilml-registry`).
- **Worker tests** spawn the *real* Python worker with `ANVILML_WORKER_MOCK=1` and assert `Ping→Pong`, `Execute→Progress→ImageReady→Completed`, and `CancelJob→Cancelled`.
- **Integration tests** (`backend/tests/api_*.rs`) drive the live axum app with `mock-hardware`: health, system, jobs CRUD + cancel, models, workers, artifacts, and a WS test (`tokio-tungstenite`) asserting `system.stats` arrives within 6 s and that an `ImageReady` IPC event yields a `job.image_ready` WS event.
- **Parity test**: `KNOWN_NODE_TYPES` (Rust) == `set(NODE_REGISTRY)` (Python), asserted from a small fixture the worker can dump.

### 20.2 Python Worker (`python-worker`)

```
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests
```

`test_executor.py` (topo-sort, input resolution, cycle detection, exception → Failed, cancel-flag → Cancelled) and `test_nodes_zit.py` / `test_nodes_sdxl.py` (each node returns the declared output slots; SaveImage emits ImageReady).

### 20.3 Frontend (`frontend`)

Owned by BloomeryUI; gate is type-check + lint + unit + build against the committed `openapi.json`. Referenced here only as the third CI job.

### 20.4 Manual Smoke Tests (pre-release)

- **Phase-1 smoke**: build release, start `anvilml`, browser opens, frontend connects, `system.stats` ticks, submit an empty/trivial job, Ctrl-C shuts down cleanly.
- **ZiT end-to-end**: place a ZiT model in `models/diffusion/`, submit via the reference form, see progress then image in the gallery; submit again and confirm the second run is faster (pipeline cache hit).
- **SDXL end-to-end**: same sequence with an SDXL model; confirm both pipelines coexist within VRAM or evict cleanly.
- **Crash recovery**: kill a worker process manually; confirm the running job fails, the worker respawns within ~2 s, and a new job succeeds.

### 20.5 Debug Harness

`backend/scripts/test_inference.py --model-type zit|sdxl --model-path … --prompt … --output …` runs a pipeline directly (no IPC, no server) and prints timing + VRAM, for isolating worker issues from orchestration issues.

---

## 21. Build, Toolchain & Release

### 21.1 Rust Toolchain

- Pinned via `rust-toolchain.toml` (stable channel; exact version pinned in-repo). Edition 2021.
- Cargo **workspace** with a shared `[workspace.dependencies]` table; crates reference versions via `workspace = true` to keep them aligned.

Representative dependency set (exact versions pinned in `Cargo.toml`):

| Concern | Crate(s) |
| :-- | :-- |
| Async runtime | `tokio` (full), `futures` |
| HTTP/WS | `axum`, `tower`, `tower-http`, `hyper` |
| Serialization | `serde`, `serde_json`, `rmp-serde` (msgpack) |
| DB | `sqlx` (sqlite, runtime-tokio, macros, migrate) |
| OpenAPI | `utoipa`, `utoipa-swagger` (dev) |
| IDs / time | `uuid` (v4, serde), `chrono` (serde) |
| Hardware/host | `sysinfo` |
| Config / CLI | `config`, `clap` (derive), `figment`-style env layering |
| Logging | `tracing`, `tracing-subscriber` (env-filter, json) |
| Errors | `thiserror`, `anyhow` |
| Misc | `url`, `open` (browser launch), `bytes` |

### 21.2 Build Commands

```
cargo build --release                 # produces target/release/anvilml
cargo run -p anvilml-openapi          # regenerate backend/openapi.json
```

### 21.3 Python Worker Toolchain

- Python **3.12.x**, managed by the user. `uv` recommended for fast, reproducible installs.
- Split requirements: `base.txt` (diffusers, transformers, pillow, msgpack, numpy, safetensors), plus exactly one of `cuda.txt` / `rocm.txt` / `cpu.txt` selecting the matching torch wheel index.

### 21.4 venv Provisioning Scripts

`backend/scripts/install_worker_deps.sh` (Linux/macOS) and `.ps1` (Windows):

```
1. Detect backend: nvidia-smi present → cuda; rocminfo present → rocm; else cpu.
2. Create the venv: Linux/macOS `python3.12 -m venv {venv_path}`; Windows `py -3.12 -m venv {venv_path}` (the `python3.12` command name does not exist on Windows). `uv venv --python 3.12` works identically on both.
3. Activate; pip install -r worker/requirements/base.txt.
4. pip install -r worker/requirements/{cuda|rocm|cpu}.txt.
5. Print resolved torch version + detected device for verification.
```

These are run once by the user; AnvilML never invokes them automatically.

### 21.5 Runtime Directory Layout (working directory)

```
./anvilml            (binary)
./anvilml.toml            (config)
./venv/                   (user-managed Python env)
./bloomery/               (frontend dist, if Local mode)
./models/                 (diffusion/, vae/, lora/, … per model_dirs)
./artifacts/{ab}/{hash}.png
./anvilml.db (+ -wal, -shm)
./logs/worker-{n}.log
```

### 21.6 Release Artifact

A single self-contained Rust binary per OS/arch (`x86_64`/`aarch64` × Linux/Windows/macOS). The binary plus `anvilml.toml`, the `worker/` directory, and the provisioning scripts constitute a release. The venv and models are provisioned by the user post-install.

---

## 22. Operations & Runbook

### 22.1 First Run

1. Place the binary, `worker/`, and scripts in a working directory; create `anvilml.toml` (or rely on defaults).
2. Run the provisioning script to build `./venv` with the correct torch backend.
3. Drop model files into the configured `model_dirs`.
4. Start `./anvilml`. Verify `GET /health` and `GET /v1/system/env` (`preflight_ok = true`).

### 22.2 Common Failure Modes

| Symptom | Likely cause | Resolution |
| :-- | :-- | :-- |
| Jobs return `503 workers_unavailable` | Preflight failed (`python_missing` / `torch_unavailable`) | Re-run provisioning; check `GET /v1/system/env`; `POST /v1/workers/:id/restart`. |
| Worker repeatedly `Respawning` | Native crash on load (bad wheel, driver mismatch) | Inspect `logs/worker-{n}.log`; run `test_inference.py` to reproduce in isolation. |
| Job `Failed: cuda_oom` | Model + working set exceeds VRAM | Lower resolution/steps; rely on pipeline-cache eviction; use a smaller dtype. |
| No GPU detected | `nvidia-smi`/`rocm-smi` not on PATH | Fix PATH, or set `hardware_override` for forced operation. |
| Second job not faster | Different `(model_id, dtype)` each run, or eviction churn | Confirm cache key stability; raise VRAM headroom. |

### 22.3 Maintenance

- **Backup**: copy `anvilml.db` (after WAL checkpoint / clean stop) and the `artifacts/` tree.
- **Disk reclaim**: `DELETE /v1/jobs?status=all` removes job rows + artifacts; orphan-scan of `artifacts/` against the DB can be added later.
- **Upgrades**: replace the binary and `worker/`; re-run provisioning only if torch requirements changed; migrations apply automatically on next start.

### 22.4 Cross-Platform Implementation Notes (Linux & Windows)

The single normative reference for every OS-divergent detail. Each item is also enforced at its subsystem.

| Concern | Linux | Windows | Where |
| :-- | :-- | :-- | :-- |
| venv interpreter | `{venv}/bin/python3` | `{venv}\Scripts\python.exe` | §6 |
| venv creation | `python3.12 -m venv` | `py -3.12 -m venv` | §21.4 |
| IPC stdio mode | binary by default | **must** set `O_BINARY` on stdin/stdout | §7.1 |
| Pipe partial reads | read-fully loop | read-fully loop (more frequent) | §7.1 |
| Shutdown signal | `ctrl_c` + `SIGTERM` | `ctrl_c` + `ctrl_close`/`ctrl_shutdown` | §16.3 |
| Force-kill child | `Child::kill()` (`SIGKILL`) | `Child::kill()` (`TerminateProcess`) | §8.5, §16.3 |
| Device visibility env | `CUDA_/HIP_VISIBLE_DEVICES` | identical | §8.3 |
| Browser launch | `xdg-open` (via `open` crate) | `cmd /c start` (via `open` crate) | §16.2 |
| Static file serving | `ServeDir` | `ServeDir` (path separators normalised by `PathBuf`) | §11 |

**Orphan-worker cleanup.** If the server is hard-killed (not graceful), Python workers can be orphaned. Mitigation, applied at spawn:

- **Linux:** set `PR_SET_PDEATHSIG` (SIGKILL) on the child via a `pre_exec` hook so the worker dies if the parent dies.
- **Windows:** create a **Job Object** with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` and assign each spawned worker to it; the workers are terminated when the server's process handle closes.

Both are best-effort hardening; the normal `Shutdown` IPC path (§16.3) remains the primary mechanism.

**PowerShell execution policy.** `install_worker_deps.ps1` documents running under `powershell -ExecutionPolicy Bypass -File …` for first-time setup on locked-down hosts.

**Line endings.** A repo `.gitattributes` pins `*.sh text eol=lf` and `*.ps1 text eol=crlf` so provisioning scripts are not corrupted by `core.autocrlf` on Windows checkouts. Python worker files are `eol=lf`.

---

## 23. Implementation Roadmap

Milestones follow the crate dependency order so each builds on a compiling, tested base. (This supersedes the earlier non-authoritative task list.)

| Milestone | Deliverable | Exit criterion |
| :-- | :-- | :-- |
| **M0 — Scaffold** | Workspace, 8 crate skeletons, launcher stub, CI skeleton with `mock-hardware`. | `cargo build/test --workspace --features mock-hardware` exits 0. |
| **M1 — Core & Contracts** | `anvilml-core` types/config/error; `anvilml-ipc` messages + framing; `anvilml-hardware` detectors + mock. | Round-trip + detector fixture tests green; `openapi.json` generates. |
| **M2 — Persistence & Workers** | `anvilml-registry` scanner + store + migrations; `anvilml-worker` pool/bridge/env. | Real mock Python worker does `Ping→Pong`; models scan into DB. |
| **M3 — Scheduling** | `anvilml-scheduler` queue/ledger/DAG/dispatch incl. cancel + node-type validation. | Cycle/unknown-type rejection; dispatch assigns to idle worker; cancel of queued job works. |
| **M4 — Server & API** | `anvilml-server` AppState, all REST handlers, `/v1/events`, `system.stats`, artifact store, frontend serving; launcher full startup/shutdown. | All `api_*.rs` integration tests green; release binary starts, browser opens, graceful shutdown. |
| **M5 — Python Worker (ZiT)** | `worker_main`, `executor`, `base`/registry, `pipeline_cache`, ZiT nodes + `SaveImage`, mock mode. | `Execute→Progress→ImageReady→Completed` in mock; ZiT end-to-end smoke on real hardware. |
| **M6 — SDXL & Hardening** | SDXL nodes; cancel cooperative path end-to-end; OOM trap; crash-recovery validation; OpenAPI diff gate in CI. | Both pipelines run; cancel + crash-recovery smoke pass; CI fully green. |

Frontend (BloomeryUI) work proceeds in parallel against the committed `openapi.json` and is gated by its own repo.

---

## 24. Conventions & Glossary

### 24.1 Conventions

- **Units**: VRAM/RAM in **MiB** throughout (`*_mib`); file sizes in bytes (`size_bytes`).
- **Time**: `DateTime<Utc>`, ISO 8601; SQLite stores ISO 8601 strings.
- **IDs**: jobs = `Uuid` v4; models = first 16 hex chars of `SHA256(canonical_path)`; artifacts = full `SHA256` hex of PNG bytes (content-addressed); workers = `worker-{device_index}`.
- **Errors**: `snake_case` codes (§18); Rust internal errors via `thiserror`, boundaries via `anyhow`.
- **API versioning**: path-prefixed `/v1`. Additive changes only within a version; breaking changes bump the prefix.
- **Commits / scopes** (if conventional commits are used): scope = crate name without the `anvilml-` prefix, plus `py-worker`, `bloomeryui`, `root`.

### 24.2 Glossary

| Term | Meaning |
| :-- | :-- |
| **Artifact** | A content-addressed output file (PNG) produced by a job. |
| **Edge reference** | A node input pointing to another node's output: `{ node_id, output_slot }`. |
| **ManagedWorker** | Rust supervisor wrapper around one Python child process. |
| **Pipeline cache** | In-worker LRU of loaded diffusion pipelines keyed by `(model_id, dtype)`. |
| **ValidatedGraph** | Newtype proving a graph passed DAG validation; the only enqueueable graph form. |
| **VramLedger** | Per-device free-VRAM tracker used for dispatch ranking (advisory, not a gate). |
| **Worker** | A Python inference process; one per device. |
| **ZiT / SDXL node set** | The two MVP text-to-image pipelines and their nodes. |

---

## 25. Open Items & Deferred Scope

Tracked, intentionally out of MVP:

1. **Per-step progress & latent preview.** `Progress.step/step_total` fields and `ImageReady`-style preview frames are reserved but unused; wiring the diffusers step callback to emit them is a fast-follow.
2. **Additional backends.** Intel IPEX, Apple MPS, AMD DirectML — each adds a `DeviceType` variant, a detector, and worker env/device-string handling. Deferred to keep the MVP matrix at CUDA/ROCm/CPU.
3. **Authentication.** Pluggable API-key / JWT for non-localhost deployment. MVP relies on `127.0.0.1` binding.
4. **Sub-graph chunking.** Batching contiguous fast nodes into one `Execute` to cut IPC overhead. The executor already produces an ordered step list, so this is additive.
5. **JobSettings / graph parameter redundancy.** MVP keeps both; a later revision can make the graph the sole carrier and reduce `JobSettings` to `device_preference` + a `seed` policy.
6. **Log-retrieval API / debug bundle.** A `GET /v1/system/logs?job_id=` and export feature. MVP reads logs from disk.
7. **Artifact refcounting.** If identical hashes ever span jobs, deletion must refcount; MVP deletes directly.
8. **Multi-job-per-GPU.** Currently one job per worker; concurrent jobs on a single large GPU would need per-job VRAM partitioning and a richer ledger.
9. **Node parameter exposure.** A `GET /v1/nodes` returning each node's slots + tunable defaults (from `worker/defaults.py`) so frontends render forms without hardcoding numbers.

---

*End of document.*