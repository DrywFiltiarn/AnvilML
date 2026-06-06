# AnvilML Backend — Functional & Technical Design

**Document:** `ANVILML_DESIGN.md`
**Revision:** 7 (CI Python prerequisites for Rust worker tests; python-worker job on Linux + Windows from Phase 9)
**Project:** SindriStudio / AnvilML
**Status:** Active — supersedes Revision 6

---

## Revision History

| Rev | Summary |
| :-- | :------ |
| 1 | Initial architecture sketch. |
| 2 | Approved architecture: crate decomposition, domain types, IPC, scheduler, server, worker outline. |
| 3 | **This document.** Expands Rev 2 into a build-complete functional + technical design: per-crate module APIs, node IO contract, model cache, cancellation, logging, testing, build/toolchain, operations runbook, and implementation roadmap. Two intentional additions to the Rev 2 IPC schema (`CancelJob` message, `Cancelled` event) are introduced in §7 and flagged inline. Revision 2 remains the architectural authority; where this document adds detail it does not contradict it. A cross-platform pass (§1.5, §22.4) makes Linux and Windows co-equal first-class targets. The backend binary and database are named `anvilml` / `anvilml.db`, and SindriStudio is clarified throughout as the separate one-click launcher that starts AnvilML and BloomeryUI (Rev 2 conflated the two). |
| 4 | Roadmap correction (§23 only). The implementation roadmap is reframed around **vertical-slice phases** (000–025, authoritative in `docs/PHASES.md`) rather than crate-dependency-ordered layers; each phase delivers a runnable, independently verifiable binary. The M0–M6 milestones are retained as a higher-level capability summary mapped to phase ranges, no longer as the unit of execution. No architectural, type, API, or IPC content changed. |
| 5 | Re-applies two decisions made after this document branched from Rev 3 (absent from the Rev 4 base): **(a) ROCm on Windows promoted to a mandatory MVP backend** (Linux + Windows, via AMD's *PyTorch on Windows* package, ROCm ≥ 7.2, on supported Radeon RX 7000/9000-series / Ryzen AI hardware); and **(b) SDK-free GPU detection** — Vulkan primary (driver-bundled), DXGI (Windows) / PCI-sysfs + NVML (Linux) fallback, a hardcoded PCI-ID capability table (`device_db.rs`), and authoritative capabilities reported by the worker's PyTorch at `Ready`. VRAM is read dynamically in every path. Updates §1.5, §2.2, §4.3, §5, §7.3, §8.3, §16, §21, §22; adds a `--print-hardware` CLI subcommand. The headless-by-default frontend model and the vertical-slice roadmap (Rev 4) are unchanged. |
| 6 | Distribution phases 023–025: auto-provisioning, version introspection, release automation, documentation site. §6 updated: AnvilML now auto-provisions the Python venv on first run via background execution of the provisioning scripts; `ProvisioningState` enum and `provisioning` field added to `EnvReport` (§4.4). `ComponentVersions` type added (§4.5). `WsEvent::ProvisioningProgress` added (§4.5). `GET /v1/system/versions` endpoint added (§10.3). `/health` `version` field now reports workspace release version (§16.2). Startup sequence updated for immediate-bind with deferred WorkerPool when provisioning needed (§16.2). `provisioning` 503 error code added (§18). `seeds_path` field added to `ServerConfig` (§3.1) and `anvilml.toml` (§3.2); `SeedLoader` documented (§5.5, §2). §21.4 updated: provisioning scripts are invoked automatically on first run. §21.5 runtime layout updated. §21.6 updated with full automated release pipeline (signed cross-platform GitHub Release zips, SHA256SUMS + GPG). §22.1 first-run runbook updated. §23 roadmap extended to phases 000–025 with M7. §25 gains items 10–11. |
| 7 | CI testing clarification for §20. (a) The Rust worker tests (`managed`, `pool`) spawn a real Python subprocess and require Python + `msgpack` + `pillow` in a CI venv (`ANVILML_VENV_PATH`) on both Linux and Windows runners; stated explicitly in §20.1. (b) The `python-worker` CI job runs on both Linux and Windows (not Linux only); corrected in §20.2 and in `ARCHITECTURE.md §9`. The job is first activated in Phase 9 when `worker/tests/test_ipc.py` is created. The Forge agent verifies the Linux runner locally; the Windows runner is verified by CI only. No architectural, type, API, or IPC content changed. |

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
| NVIDIA / CUDA worker | ✓ | ✓ | ✗ | Enumerated via Vulkan (driver-bundled); no CUDA SDK / `nvidia-smi` needed (§5). |
| AMD / ROCm worker | ✓ | ✓ | ✗ | **MVP-mandatory on both OSes** (§5). Windows uses AMD's *PyTorch on Windows* package (ROCm ≥ 7.2) on supported Radeon RX 7000/9000-series + select Ryzen AI parts. DirectML still deferred (§25). |
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
    seeds/
      devices.sql               (SHA256-gated device capability seed data)
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
      rocm-linux.txt            (torch + ROCm index, Linux: stable or nightly)
      rocm-windows.txt          (AMD PyTorch-on-Windows package, ROCm >= 7.2)
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
| `anvilml-hardware` | Detect GPUs and host **SDK-free**; refreshable VRAM snapshot. | `lib.rs` (`detect_all_devices`), `vulkan.rs`, `dxgi.rs` (Windows), `sysfs.rs` + `nvml.rs` (Linux fallback), `device_db.rs` (capability table), `cpu.rs`, `mock.rs` |
| `anvilml-registry` | Scan model dirs, persist `ModelMeta` to SQLite, serve queries; `SeedLoader` (SHA256-gated SQL seed runner for `backend/seeds/`). | `scanner.rs`, `store.rs`, `device_store.rs`, `seed_loader.rs`, `lib.rs` |
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
    pub venv_path: PathBuf,                  // default: ./venv  (auto-provisioned on first run)
    pub rocm: RocmConfig,
    pub hardware_override: Option<HardwareOverrideConfig>,
    pub worker_log_dir: Option<PathBuf>,     // default: ./logs
    pub seeds_path: PathBuf,                 // default: <exe_dir>/seeds (debug: backend/seeds fallback)
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
    /// Serve static files from a local directory (for a custom/third-party frontend).
    /// Not used for BloomeryUI, which SindriStudio runs as a separate server.
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
# seeds_path = "./seeds"  # default: <exe_dir>/seeds; falls back to backend/seeds/ in debug builds
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
mode = "headless"       # headless (default) | local | remote
# path = "./frontend"   # custom frontend dir, for mode = "local" (NOT BloomeryUI)
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
| `ANVILML_SEEDS_PATH` | `seeds_path` | `<exe_dir>/seeds` |
| `ANVILML_NUM_THREADS` | `num_threads`; passed to worker | `14` |
| `ANVILML_NUM_INTEROP_THREADS` | `num_interop_threads`; passed to worker | `4` |
| `ANVILML_FRONTEND__MODE` | `frontend.mode` | `headless` |
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
pub struct HardwareInfo { pub host: HostInfo, pub gpus: Vec<GpuDevice> }

pub struct GpuDevice {
    pub index: u32,
    pub name: String,                 // canonical (device_db) or driver-reported
    pub device_type: DeviceType,      // vendor-mapped; confirmed by the worker's torch build
    pub pci_vendor_id: u16,           // 0x10DE NVIDIA, 0x1002 AMD, 0x8086 Intel
    pub pci_device_id: u16,
    pub vram_total_mib: u32,          // DYNAMIC: Vulkan heap / DXGI / NVML / torch — never the table
    pub vram_free_mib: u32,           // DYNAMIC: refreshed from worker MemoryReport
    pub driver_version: String,       // Vulkan driverInfo / DXGI / NVML
    pub arch: Option<String>,         // CUDA SM ("8.9") or ROCm gfx ("gfx1100"); None until known
    pub caps: InferenceCaps,
    pub enumeration_source: EnumerationSource,
    pub capabilities_source: CapabilitySource,
}

pub enum DeviceType { Cuda, Rocm, Cpu }   // MVP set; see §25 for deferred backends
pub enum EnumerationSource { Vulkan, Dxgi, Sysfs, Nvml, Override, Mock }
pub enum CapabilitySource { Worker, DeviceTable, Fallback }

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

pub struct EnvReport {
    pub python_path: String,
    pub python_version: String,
    pub torch_version: String,
    pub preflight_ok: bool,
    pub reason: String,             // empty on success; "python_missing" | "torch_unavailable" | …
    pub provisioning: ProvisioningState,
}

pub enum ProvisioningState {
    NotStarted,
    InProgress { percent: Option<u8>, message: String },
    Ready,
    Failed { reason: String },
}
```

### 4.5 Version Types

```rust
pub struct ComponentVersions {
    pub anvilml: String,            // workspace release version ([workspace.package] version)
    pub backend: String,
    pub core: String,
    pub hardware: String,
    pub registry: String,
    pub ipc: String,
    pub worker: String,
    pub scheduler: String,
    pub server: String,
    pub openapi: String,
    pub python_worker: Option<String>, // from worker/__init__.py __version__; None if absent
}
```

Each workspace crate exposes `pub const VERSION: &str = env!("CARGO_PKG_VERSION")`. The workspace root additionally declares `[workspace.package] version` as the **product release version**, independent of per-crate versions; this is what `GET /health` and `GET /v1/system/versions` report as `anvilml`.

### 4.6 WebSocket Event Types

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
    ProvisioningProgress(ProvisioningProgressEvent), // "provisioning.progress"
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

> Raw image bytes are **never** sent over WebSocket. Clients receive the artifact hash and fetch the PNG via REST. `JobFailed` carries `{ job_id, error, traceback? }`; `JobCancelled` carries `{ job_id }`; `WorkerStatusChanged` carries `{ worker_id, status }`; `ProvisioningProgress` carries `{ state, percent?, message, timestamp }` and is emitted on every provisioning state transition.

---

## 5. Hardware Detection (`anvilml-hardware`)

Detection works **without any vendor SDK or CLI tool** (no `nvidia-smi`, `rocm-smi`, `rocminfo`, `lspci`, or CUDA/ROCm toolkits). The only runtime dependency is the **Vulkan loader**, which ships with every modern GPU driver on Linux and Windows — not with an SDK. Enumeration runs once at startup (pre-spawn); ML capabilities and live VRAM are then refined by the worker's PyTorch, which is already installed and therefore adds no new dependency.

### 5.1 Three-layer model

1. **Enumeration + VRAM — Rust, pre-spawn, vendor-neutral, dynamic.** List physical GPUs and read total/available VRAM straight from the driver. No SDK.
2. **Capabilities — PyTorch worker, at `Ready`, authoritative.** Once a worker is live on a device, torch reports the ground-truth ML capabilities and re-confirms VRAM; Rust merges these over the pre-spawn record.
3. **Device capability table (`device_db.rs`) — hint + fallback.** A curated, hardcoded map of PCI `(vendor_id, device_id)` → canonical model name, architecture, and capability hints; used before/without a worker and when torch cannot report. **VRAM is never taken from this table — it is always read dynamically.**

```rust
pub async fn detect_all_devices(config: &ServerConfig) -> Result<HardwareInfo>;
```

### 5.2 Enumeration backends (priority order)

| Backend | Platforms | Yields | Dependency |
| :-- | :-- | :-- | :-- |
| **Vulkan** (primary) | Linux + Windows | name, PCI vendor/device IDs, device type, driver id/version, **total + available VRAM** | Vulkan loader (driver-bundled) |
| DXGI (fallback) | Windows | name, vendor/device IDs, dedicated VRAM | built into Windows |
| PCI sysfs + NVML (fallback) | Linux | vendor/device IDs (`/sys/bus/pci/devices/*`); VRAM via amdgpu sysfs or NVML (`libnvidia-ml`, driver-bundled) | none / driver-bundled |
| CPU | all | host info | `sysinfo` (always succeeds) |

Vulkan path: create a headless `VkInstance` (no surface/window) → `vkEnumeratePhysicalDevices` → `vkGetPhysicalDeviceProperties2` (+ `VK_KHR_driver_properties`) → `vkGetPhysicalDeviceMemoryProperties2` (+ `VK_EXT_memory_budget`). **Total VRAM** = the largest `DEVICE_LOCAL` heap's `heapSize` (an extra small device-local + host-visible Resizable-BAR heap is ignored). **Available VRAM** = `heapBudget − heapUsage` for that heap when `VK_EXT_memory_budget` is present, else `heapSize`. Rust bindings via `ash`, which loads the loader at runtime — its absence triggers the fallback path, not a crash.

### 5.3 Vendor → backend mapping

The PCI vendor ID maps to a candidate ML backend; the worker's torch build confirms it at `Ready` (`torch.version.cuda` vs `torch.version.hip`).

| Vendor ID | Vendor | MVP backend |
| :-- | :-- | :-- |
| `0x10DE` | NVIDIA | CUDA |
| `0x1002` | AMD | ROCm (Linux **and** Windows) |
| `0x8086` | Intel | — (IPEX deferred §25; enumerated but not used for inference → CPU) |

**ROCm is a mandatory MVP backend on both Linux and Windows.** Enumeration is identical on both OSes (Vulkan/DXGI), so it does not depend on Linux-only ROCm CLIs. The Windows ROCm *execution* path requires AMD's *PyTorch on Windows* package (ROCm ≥ 7.2) on a supported Radeon RX 7000/9000-series or Ryzen AI part (§6, §21); an AMD GPU outside that support list is enumerated but falls back to CPU for inference.

### 5.4 Capability resolution

For each enumerated GPU:

- **Pre-spawn:** look up `(vendor_id, device_id)` in `device_db`. Hit → fill `arch`, `caps` (fp16/bf16/flash-attention), canonical `name`; `capabilities_source = DeviceTable`. Miss → conservative defaults (fp16 from the Vulkan `shaderFloat16` feature when present, bf16 = false, flash-attention = false); `capabilities_source = Fallback`; emit a `warn!` naming the unknown PCI ID so the table can be extended.
- **At worker `Ready` (authoritative):** torch reports `fp16`/`bf16`/flash-attention, `arch` (CUDA SM or ROCm gfx), and `mem_get_info()`. Rust overwrites the device record and sets `capabilities_source = Worker`. This is what the scheduler and API serve once a worker is live.

Querying ML-relevant capabilities *directly from the driver* is intentionally **not** attempted: Vulkan/driver feature bits do not reliably express "PyTorch supports bf16 / flash-attention on this architecture." The already-required PyTorch runtime is the correct authority; the table is only the pre-spawn hint and the no-worker fallback.

### 5.5 Device capability table (`device_db.rs`)

A compile-time table (a `const` slice, or an embedded RON file validated by a unit test) mapping `(u16 vendor_id, u16 device_id)` → `DeviceCapabilityEntry { model_name, arch, fp16, bf16, flash_attention }`. It is deliberately hardcoded and must be updated as new GPUs ship; the update procedure is documented alongside it. Because torch is authoritative at runtime (§5.4), a missing or stale entry degrades only pre-spawn *display* for that card — never the correctness of a running job. **No VRAM values are stored in the table.**

### 5.6 SeedLoader and `backend/seeds/`

The `device_capabilities` SQLite table is populated from SQL seed files in `backend/seeds/` rather than from a compiled-in Rust const, so the data can be updated without a recompile. `anvilml_registry::seed_loader::run(pool, seeds_dir)` runs at startup after migrations:

1. Bootstraps a `seed_history` table (`CREATE TABLE IF NOT EXISTS`) — self-managed, not in migrations.
2. For each `.sql` file in `seeds_dir`: parses two required header directives (`-- anvil:seed_table <name>` and `-- anvil:seed_strategy <replace_all|merge>`); computes SHA256 of the file content; skips if the hash matches `seed_history`; otherwise executes in a single transaction.
   - `replace_all`: `DELETE FROM <table>` then all INSERTs then `seed_history` upsert.
   - `merge`: `INSERT OR REPLACE` statements only then `seed_history` upsert.
3. A missing `anvil:seed_table` directive is a fatal startup error.

`backend/seeds/devices.sql` is the first seed file; it uses `replace_all` and contains `INSERT OR REPLACE` rows for all known NVIDIA and AMD SKUs. It ships co-located with the binary in release packages. In debug builds, if `<exe_dir>/seeds` does not exist the loader falls back to the workspace-relative `backend/seeds/` path.

The `seeds_path` config field (default `<exe_dir>/seeds`; env `ANVILML_SEEDS_PATH`; CLI `--seeds-path`) overrides the resolved directory.

### 5.6 Rules

- Per-device enumeration/probe failures are logged at `warn` and skipped; they never abort startup.
- `hardware_override` (config) bypasses enumeration entirely and returns one synthetic device of the given type/VRAM (`enumeration_source = Override`).
- No GPU enumerated → exactly one CPU worker.
- `vram_free_mib` is refreshed during operation from worker `MemoryReport` (torch `mem_get_info`), the runtime authority; the Vulkan/DXGI budget read is used at startup and whenever no worker is attached to a device.
- The `mock-hardware` feature swaps in `MockHardwareDetector` (driven by `ANVILML_MOCK_DEVICE_TYPE`/`_VRAM_MIB`/`_GFX_ARCH`), bypassing Vulkan/DXGI. It is the only detector compiled in CI.

Intel (IPEX), Apple MPS, and AMD DirectML are **deferred** (§25); `DeviceType` stays limited to the three MVP variants so nothing can reference a backend that does not yet exist.

---

## 6. Python Environment (`venv_path`)

The `venv_path` config field (default `./venv`, relative to the config file) points to the Python virtual environment. The launcher resolves the interpreter as:

- Linux/macOS: `{venv_path}/bin/python3`
- Windows: `{venv_path}\Scripts\python.exe`

### 6.1 Auto-Provisioning on First Run

On startup, if the venv is absent or `import torch` fails (and `ANVILML_WORKER_MOCK` is unset), AnvilML automatically provisions the venv by spawning the checked-in provisioning script as a background child process (`anvilml-worker::provisioner::provision`). The binary **binds the HTTP server immediately** before provisioning completes — the API is responsive at `:8488` throughout.

Provisioning state is surfaced via:
- `GET /v1/system/env` → `EnvReport.provisioning` field (`NotStarted → InProgress → Ready / Failed`)
- `WS /v1/events` → `provisioning.progress` frames

`POST /v1/jobs` returns `503` (`provisioning`) while `ProvisioningState` is `NotStarted` or `InProgress`, and `503` (`workers_unavailable`) if it reaches `Failed`. Jobs are accepted normally once `Ready`. The `WorkerPool` is spawned automatically when provisioning reaches `Ready`.

### 6.2 Provisioning Scripts

`backend/scripts/install_worker_deps.sh` (Linux/macOS) and `.ps1` (Windows) are the provisioning scripts. AnvilML invokes them automatically on first run; they may also be run manually for venv repair or to swap torch versions without restarting the server. See §21.4 for the full script logic.

### 6.3 Preflight Check

At startup (before or during the provisioning decision):

1. Verify the resolved interpreter exists and is executable. If not → `reason = "python_missing"`.
2. Run `python --version`; warn (do not abort) if not `Python 3.12.x`.
3. Run `python -c "import torch; print(torch.__version__)"` with `ANVILML_WORKER_MOCK` unset. On failure → `reason = "torch_unavailable"`.

`GET /v1/system/env` reports the full `EnvReport` including `preflight_ok`, `reason`, and `provisioning` state.

### 6.4 Repair Flow

To repair a broken venv without restarting the server: delete and recreate the venv manually (or re-run the provisioning script), then call `POST /v1/workers/:id/restart`. The restart re-runs preflight and re-sends `InitializeHardware` to the worker.

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
Ready        { worker_id: String, device_index: u32, vram_total_mib: u32, vram_free_mib: u32, arch: String, fp16: bool, bf16: bool, flash_attention: bool }  // caps authoritative; Rust merges into GpuDevice (§5.4)
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
2. Python → Ready { vram_total_mib, vram_free_mib, arch, caps } → worker Idle; Rust merges authoritative caps (§5.4)
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
| ROCm | `HIP_VISIBLE_DEVICES={n}` (Linux **and** Windows); `ROCBLAS_USE_HIPBLASLT={0\|1}`; `HSA_OVERRIDE_GFX_VERSION` (Linux ROCm runtime only — not applicable on Windows, where supported GPUs are fixed by AMD's package) |
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
| GET | `/v1/system/env` | Python environment health + provisioning state | 200 `EnvReport` |
| GET | `/v1/system/versions` | Per-component version report | 200 `ComponentVersions` |
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

**`/health` version field.** Reports the workspace release version (`[workspace.package] version` in `Cargo.toml`), not the backend crate version. This is the canonical way to determine which AnvilML release is running.

**`GET /v1/system/versions` response.** Returns `ComponentVersions` (§4.5) with each crate's `pub const VERSION`, the workspace release version as `anvilml`, and the Python worker's `__version__` as `python_worker` (read from `worker/__init__.py` at startup; `null` if absent).

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

Determined at startup from `frontend.mode`. **The default is `Headless`** — AnvilML serves only the API; BloomeryUI is run as a separate server by SindriStudio. `Local` and `Remote` exist purely for users running a custom frontend through AnvilML standalone. A single catch-all axum route is registered last (lowest priority, after all `/v1/*` and `/health`).

- **`Local { path }`** — `ServeDir` mounted at `/`; SPA fallback serves `{path}/index.html` (200) for unmatched paths. For serving a **custom/third-party** frontend from disk (e.g. `./frontend`); it is **not** used for BloomeryUI. If the directory is missing, log a warning and serve a minimal inline HTML page explaining the situation; the API stays fully functional.
- **`Remote { url }`** — reverse-proxy all non-API, non-`/health` requests to `url` via a `hyper` client in a catch-all handler. Forward request headers; rewrite `Host`; stream the response back. For proxying a **custom** frontend dev server (e.g. Vite) while AnvilML is the backend; BloomeryUI is run separately by SindriStudio, not proxied here.
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
6.  Wait for InitializeHardware → resolve device string → probe torch caps → send Ready { vram_total_mib, vram_free_mib, arch, fp16, bf16, flash_attention }.
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
anvilml [--config <path>] [--host <ip>] [--port <u16>] [--no-browser] [--print-hardware]
```

Flags override config (highest precedence). `--help` prints usage. `--print-hardware` runs SDK-free GPU enumeration (§5), prints the detected devices as JSON, and exits — used by the provisioning script to pick the torch wheel without vendor CLIs.

### 16.2 Startup Sequence

```
1.  Parse CLI; load + merge config (defaults → toml → env → flags).
2.  Init tracing subscriber (§19).
3.  Open SqlitePool; set PRAGMAs; run sqlx migrations; run SeedLoader (§5.6).
4.  Reset ghost jobs: UPDATE jobs SET status='Failed', error='server_restart'
    WHERE status IN ('Running','Queued').
5.  Detect hardware (§5) → HardwareInfo.
6.  Python preflight (§6.3) → EnvReport (initial).
7.  Build registry; perform initial model scan (async; non-blocking for server bind).
8.  If venv absent or torch unavailable AND ANVILML_WORKER_MOCK unset:
      Set EnvReport.provisioning = InProgress; tokio::spawn provisioner::provision.
      (WorkerPool deferred until provisioning reaches Ready.)
    Else:
      Spawn WorkerPool (one per device, or one CPU worker). Send InitializeHardware to each.
9.  Build AppState (includes ComponentVersions); start scheduler dispatch loop; start system.stats tick.
10. Bind axum server on host:port. (API immediately responsive; jobs 503 until provisioning Ready.)
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
| Job submitted while provisioning is NotStarted or InProgress | 503 | `provisioning` |
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
- **Worker tests** spawn the *real* Python worker with `ANVILML_WORKER_MOCK=1` and assert `Ping→Pong`, `Execute→Progress→ImageReady→Completed`, and `CancelJob→Cancelled`. **CI prerequisite (both Linux and Windows runners):** a venv containing `msgpack` and `pillow` must be created before `cargo test` runs, and `ANVILML_VENV_PATH` must point to it. No torch is required; mock mode skips the torch import. The venv is created inline in the CI step and is not committed to the repository.
- **Integration tests** (`backend/tests/api_*.rs`) drive the live axum app with `mock-hardware`: health, system, jobs CRUD + cancel, models, workers, artifacts, and a WS test (`tokio-tungstenite`) asserting `system.stats` arrives within 6 s and that an `ImageReady` IPC event yields a `job.image_ready` WS event.
- **Parity test**: `KNOWN_NODE_TYPES` (Rust) == `set(NODE_REGISTRY)` (Python), asserted from a small fixture the worker can dump.

### 20.2 Python Worker (`python-worker`)

Runs on Linux + Windows CI. First activated in Phase 9 when `worker/tests/test_ipc.py` is
created; coverage grows with each subsequent phase. No torch installation required —
`ANVILML_WORKER_MOCK=1` skips the torch import throughout. The Forge agent verifies the
Linux runner locally; the Windows runner is verified by CI only.

```
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v
```

- `test_ipc.py` — IPC framing round-trips: `read_frame`/`write_frame` correctness, length-prefix encoding, Windows binary-mode guard (Phase 9).
- `test_executor.py` — topo-sort, input resolution, cycle detection, exception → Failed, cancel-flag → Cancelled (Phase 21).
- `test_nodes_zit.py` / `test_nodes_sdxl.py` — each node returns the declared output slots; SaveImage emits ImageReady (Phase 21/22).

### 20.3 Frontend (`frontend`)

Owned by BloomeryUI; gate is type-check + lint + unit + build against the committed `openapi.json`. Referenced here only as the third CI job.

### 20.4 Manual Smoke Tests (pre-release)

- **Phase-1 smoke**: build release, start `anvilml` (headless by default — no browser), confirm `/health` and `system.stats` tick, submit an empty/trivial job, Ctrl-C shuts down cleanly. (With a custom frontend configured via `frontend.mode=local`, the browser-open step also fires.)
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
| GPU enumeration | `ash` (Vulkan), `windows` (DXGI; Windows target), `nvml-wrapper` (Linux NVML fallback) |
| Host info | `sysinfo` |
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
- Split requirements: `base.txt` (diffusers, transformers, pillow, msgpack, numpy, safetensors), plus a torch selector chosen by OS + backend: `cuda.txt`; `rocm-linux.txt` (Linux ROCm pip index, stable or nightly); `rocm-windows.txt` (AMD *PyTorch on Windows*, ROCm ≥ 7.2 / nightly); or `cpu.txt`.

### 21.4 venv Provisioning Scripts

`backend/scripts/install_worker_deps.sh` (Linux/macOS) and `.ps1` (Windows):

```
1. Detect backend and OS **without vendor SDKs**: run `anvilml --print-hardware` (Vulkan/DXGI enumeration, §5) and read each GPU's PCI vendor ID — `0x10DE` → cuda, `0x1002` → rocm, else cpu. (Equivalent SDK-free fallbacks if the binary isn't built yet: PCI sysfs on Linux, `Get-CimInstance Win32_VideoController` on Windows.)
2. Create the venv: Linux/macOS `python3.12 -m venv {venv_path}`; Windows `py -3.12 -m venv {venv_path}` (the `python3.12` command name does not exist on Windows). `uv venv --python 3.12` works identically on both.
3. Activate; pip install -r worker/requirements/base.txt.
4. Install torch: Linux runs `pip install -r worker/requirements/{cuda|rocm|cpu}.txt`; **Windows + ROCm** installs AMD's *PyTorch on Windows* build per `rocm-windows.txt` (AMD-hosted wheels + driver package, ROCm ≥ 7.2), not the Linux ROCm index.
5. Print resolved torch version + detected device for verification.
```

These scripts are invoked automatically by the provisioner on first run (§6.1) and may also be run manually for repair or to swap torch versions without restarting the server.

### 21.5 Runtime Directory Layout (working directory)

```
./anvilml            (binary)
./anvilml.toml            (config)
./seeds/                  (SQL seed files — devices.sql)
./venv/                   (Python env — auto-provisioned on first run)
./frontend/               (custom frontend dist, only if Local mode; not BloomeryUI)
./models/                 (diffusion/, vae/, lora/, … per model_dirs)
./artifacts/{ab}/{hash}.png
./anvilml.db (+ -wal, -shm)
./logs/worker-{n}.log
```

### 21.6 Release Artifact & Automation

**Product release version:** `[workspace.package] version` in the workspace root `Cargo.toml`. This value is independent of per-crate versions. Bumping it on `main` is the sole trigger for a release.

**Release pipeline:**

1. `release-tag.yml` (triggered on push to `main`) reads `[workspace.package] version` at `HEAD` vs `HEAD~1`. If changed, creates and pushes annotated tag `v<new>`. Unchanged pushes are no-ops.
2. `release.yml` (triggered on `v*` tag push):
   - **build-linux** (ubuntu-latest): `cargo build --release --target x86_64-unknown-linux-gnu -p anvilml`. Packages into `anvilml-<version>-linux-x64.zip`.
   - **build-windows** (windows-latest): `cargo build --release --target x86_64-pc-windows-msvc -p anvilml`. Packages into `anvilml-<version>-windows-x64.zip`.
   - **sign** (ubuntu): generates `SHA256SUMS` covering both zips plus detached GPG signatures (`.asc` per zip + `SHA256SUMS.asc`) using `ANVILML_GPG_KEY` / `ANVILML_GPG_PASSPHRASE` from repo secrets. If secrets absent, produces `SHA256SUMS` and warns; does not fail the release.
   - **publish**: creates GitHub Release titled `AnvilML <version>`; attaches both zips, `SHA256SUMS`, and all `.asc` signatures; auto-generates release notes (commits since the previous tag); marks pre-release if version contains a hyphen suffix (e.g. `0.2.0-rc1`).

**Release zip contents** (both platforms):
```
anvilml[.exe]
anvilml.toml                (frontend.mode = "headless")
seeds/devices.sql
worker/                     (full Python source + requirements/*.txt baseline)
scripts/install_worker_deps.{sh,ps1}
backend/openapi.json
dist/QUICKSTART.md
LICENSE
models/diffusion/ models/lora/ models/vae/ models/controlnet/
models/clip/ models/unet/ models/upscale/  (each with README.txt)
logs/  artifacts/           (empty, with .gitkeep)
```

Release documentation: `docs/RELEASE.md`.

---

## 22. Operations & Runbook

### 22.1 First Run

1. Extract the release zip to a working directory. Ensure Python 3.12 is installed (`python3.12` on Linux, `py -3.12` on Windows).
2. Start `./anvilml` (or `anvilml.exe`). The binary binds `http://127.0.0.1:8488` immediately. On first run with no venv, AnvilML auto-provisions the Python venv in the background. Progress is visible via `GET /v1/system/env` (`.provisioning` field) and `WS /v1/events` (`provisioning.progress` frames). Jobs return `503 provisioning` until `Ready`.
3. Drop model files into the configured `model_dirs`.
4. Once `GET /v1/system/env` reports `provisioning = "Ready"`, submit jobs normally.

### 22.2 Common Failure Modes

| Symptom | Likely cause | Resolution |
| :-- | :-- | :-- |
| Jobs return `503 provisioning` | Provisioning in progress | Wait; monitor `GET /v1/system/env` `.provisioning` |
| Jobs return `503 workers_unavailable` | Preflight failed or provisioning `Failed` | Check `GET /v1/system/env`; repair venv; `POST /v1/workers/:id/restart`. |
| Worker repeatedly `Respawning` | Native crash on load (bad wheel, driver mismatch) | Inspect `logs/worker-{n}.log`; run `test_inference.py` to reproduce in isolation. |
| Job `Failed: cuda_oom` | Model + working set exceeds VRAM | Lower resolution/steps; rely on pipeline-cache eviction; use a smaller dtype. |
| No GPU detected | GPU driver / Vulkan runtime not installed (Vulkan loader missing), or enumeration failed | Install the GPU driver (it bundles the Vulkan runtime); confirm with `anvilml --print-hardware`; or set `hardware_override` in `anvilml.toml`. |
| ROCm worker dead on Windows | AMD *PyTorch on Windows* package missing, or GPU unsupported | Install **AMD Software: PyTorch on Windows** (ROCm ≥ 7.2); confirm the GPU is a supported Radeon RX 7000/9000-series or Ryzen AI part. |
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
| GPU enumeration | Vulkan → PCI sysfs/NVML fallback | Vulkan → DXGI fallback | §5 |
| ROCm torch install | Linux pip ROCm index | AMD *PyTorch on Windows* package (ROCm ≥ 7.2) | §21.3 |
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

Implementation is executed as a sequence of **vertical slices**, not as a build-up of architectural layers. Each phase delivers a runnable binary with one new observable capability and ends with an explicit, command-based "Runnable Proof" — the work begins with a walking skeleton (a server that binds and answers `/health`) and thickens it slice by slice up to real ZiT and SDXL inference. The authoritative execution sequence — phase numbers, names, and per-phase proofs — lives in [`docs/PHASES.md`](./docs/PHASES.md), with the atomic task breakdown in `docs/TASKS_PHASE*.md` and `forge/tasks/tasks_phase*.json`.

The milestone groupings below are a higher-level capability summary that the phases roll up into; they are deliverable-and-exit-criterion checkpoints, **not** the unit of execution. (Earlier revisions sequenced work by crate dependency order; that horizontal-layer approach was replaced by the vertical-slice phases because it produced no runnable, verifiable artifact until late in the build.)

| Milestone | Phases | Deliverable | Exit criterion |
| :-- | :-- | :-- | :-- |
| **M0 — Pre-flight & Scaffold** | 000–001 | Repository hygiene (`.gitignore`, `.gitattributes`, pinned `rust-toolchain.toml`); workspace, 8 crate skeletons, launcher that binds and serves `/health`; CI with `mock-hardware`. | `curl /health` → 200; `cargo build/test --workspace --features mock-hardware` exits 0. |
| **M1 — Core & Contracts** | 002–004 | Config + graceful shutdown; `anvilml-core` types/config/error; `anvilml-hardware` detectors + mock, surfaced via `/v1/system`. | Configurable start + clean shutdown; `curl /v1/system` shows detected (or mock) hardware; detector fixtures green. |
| **M2 — Persistence & Workers** | 005–010 | SQLite open/migrate + ghost reset; `anvilml-registry` scanner/store; WS event stream; `anvilml-ipc` framing; `anvilml-worker` pool/bridge/env; crash recovery. | Models scan into DB and list via REST; real mock Python worker does `Ping→Pong`; killed worker respawns to Idle. |
| **M3 — Scheduling** | 011–013 | `anvilml-scheduler` node-type validation + DAG; job queue + submission/persistence; VRAM ledger + dispatch. | Cycle/unknown-type rejection (422); submitted job persists Queued; dispatch drives a mock job to Completed. |
| **M4 — End-to-end & Server Surface** | 014–020 | Artifact storage; full job lifecycle over WS; cancellation; job/artifact management; worker restart API + preflight; frontend serving; OpenAPI + launcher polish. | Completed job's PNG downloads via REST; cancel + delete work; `openapi.json` diff gate green; binary opens browser when a custom frontend is configured (headless default opens none). |
| **M5 — Python Worker (ZiT)** | 021 | `worker_main`, `executor`, `base`/registry, `pipeline_cache`, ZiT nodes + `SaveImage`, mock mode + parity test. | `Execute→Progress→ImageReady→Completed` in mock; ZiT end-to-end smoke on real hardware. |
| **M6 — SDXL & Hardening** | 022 | SDXL nodes; cancel cooperative path end-to-end; OOM trap; crash-recovery + full REST integration tests; provisioning scripts; debug harness. | Both pipelines run; cancel + crash-recovery smoke pass; CI fully green on Linux and Windows. |
| **M7 — Distribution** | 023–025 | Auto-provisioning with background venv install + live state; workspace release version + `GET /v1/system/versions`; signed cross-platform GitHub Release zips (SHA256SUMS + GPG); mdBook documentation site on GitHub Pages. | Clean run: API up immediately; `provisioning.progress` → Ready; version bump → published zips; docs site live. |

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
| **ComponentVersions** | Aggregate type reporting the version of every crate, the workspace release version, and the Python worker version. Returned by `GET /v1/system/versions`. |
| **Edge reference** | A node input pointing to another node's output: `{ node_id, output_slot }`. |
| **ManagedWorker** | Rust supervisor wrapper around one Python child process. |
| **Pipeline cache** | In-worker LRU of loaded diffusion pipelines keyed by `(model_id, dtype)`. |
| **ProvisioningState** | `NotStarted \| InProgress \| Ready \| Failed` — lifecycle of the background venv provisioning step. |
| **Release version** | `[workspace.package] version` in `Cargo.toml`; the single value bumped to trigger a release (phases 024). |
| **ValidatedGraph** | Newtype proving a graph passed DAG validation; the only enqueueable graph form. |
| **VramLedger** | Per-device free-VRAM tracker used for dispatch ranking (advisory, not a gate). |
| **Worker** | A Python inference process; one per device. |
| **ZiT / SDXL node set** | The two MVP text-to-image pipelines and their nodes. |

---

## 25. Open Items & Deferred Scope

Tracked, intentionally out of MVP:

1. **Per-step progress & latent preview.** `Progress.step/step_total` fields and `ImageReady`-style preview frames are reserved but unused; wiring the diffusers step callback to emit them is a fast-follow.
2. **Additional backends.** Intel IPEX, Apple MPS, and AMD DirectML — each adds a `DeviceType` variant, a detector, and worker env/device-string handling. Deferred; the MVP matrix is CUDA + ROCm (Linux **and** Windows) + CPU. DirectML remains deferred as a future fallback for AMD GPUs not covered by ROCm-on-Windows.
3. **Authentication.** Pluggable API-key / JWT for non-localhost deployment. MVP relies on `127.0.0.1` binding.
4. **Sub-graph chunking.** Batching contiguous fast nodes into one `Execute` to cut IPC overhead. The executor already produces an ordered step list, so this is additive.
5. **JobSettings / graph parameter redundancy.** MVP keeps both; a later revision can make the graph the sole carrier and reduce `JobSettings` to `device_preference` + a `seed` policy.
6. **Log-retrieval API / debug bundle.** A `GET /v1/system/logs?job_id=` and export feature. MVP reads logs from disk.
7. **Artifact refcounting.** If identical hashes ever span jobs, deletion must refcount; MVP deletes directly.
8. **Multi-job-per-GPU.** Currently one job per worker; concurrent jobs on a single large GPU would need per-job VRAM partitioning and a richer ledger.
9. **Node parameter exposure.** A `GET /v1/nodes` returning each node's slots + tunable defaults (from `worker/defaults.py`) so frontends render forms without hardcoding numbers.
10. **Authenticode / cosign signing.** Real code-signing for Windows SmartScreen trust and verifiable Linux binaries. Current release uses GPG detached signatures only.
11. **Worker-dependency update facility.** A built-in mechanism for updating `worker/requirements/*.txt` and re-provisioning in place after a release, without requiring a full binary upgrade.

---

*End of document.*