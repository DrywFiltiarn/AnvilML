# AnvilML Backend — Functional & Technical Design

**Document:** `ANVILML_DESIGN.md`
**Revision:** 1 (v4 ground-up rewrite)
**Project:** SindriStudio / AnvilML
**Status:** Active — supersedes all v3 design documents
**Authoring constraint:** This document is read by a small local LLM agent (Qwen3.6
35B A3B, ~3B active params), not a frontier model. Every section states facts and
contracts directly. Tables over prose. No section requires cross-referencing more
than one other section to apply correctly. If a rule needs an example to be
unambiguous, the example is inline, not implied.

---

## Revision History

| Rev | Date | Summary |
|:----|:-----|:--------|
| 1 | 2026-06-25 | v4 ground-up rewrite. Built from a fresh ComfyUI-rebuild reference plan plus the full v3 retrofit history (P900–P904). Real-path implementation is mandatory everywhere — there is no mock-only node. Adds runtime hardware-capability self-detection, a mechanically-enforced mock/real parity marker, ComfyUI-style raw-construction model loading (no `diffusers`/`transformers` model classes in the load path), and an explicit IPC ownership model that removes the design ambiguity that caused three IPC rewrites in v2/v3. |

---

## 0. How This Document Is Organized

| Section | Answers |
|:--------|:--------|
| §1 | What is AnvilML and what is explicitly NOT AnvilML |
| §2 | Which OS/GPU/Python targets are real and which are out of scope |
| §3 | Crate list, what each crate owns, dependency graph (acyclic, checked) |
| §4 | Code quality rules: file size, error handling, async, doc comments |
| §5 | Rust domain types (`anvilml-core`) |
| §6 | Hardware detection + runtime capability self-detection |
| §7 | Model registry (SQLite, scanning, hashing) |
| §8 | IPC protocol + the IPC ownership model (read this before touching `anvilml-ipc` or `anvilml-worker`) |
| §9 | Worker process lifecycle (Rust side) |
| §10 | Generic node system + the mock/real parity rule |
| §11 | Model loading contract (ComfyUI-style raw construction) |
| §12 | Job scheduler |
| §13 | HTTP/WebSocket server |
| §14 | Python worker process internals |
| §15 | Configuration |
| §16 | Logging |
| §17 | Testing strategy (mock-mode AND real-mode, both mandatory) |
| §18 | Build, toolchain, CI (GitHub Actions matrix) |
| §19 | Operations runbook |
| §20 | Implementation roadmap (phase groups) |
| Appendix A | v3 → v4 change table |
| Appendix B | MVP model matrix and example graph |

---

## 1. Purpose & Boundaries

AnvilML is the **Rust backend binary** (`anvilml`) of the SindriStudio image-generation
platform.

AnvilML DOES:
- Spawn and supervise one Python worker process per GPU, or one CPU worker if no GPU
  is present.
- Expose a versioned REST + WebSocket API. This API is the only way any client talks
  to AnvilML.
- Own job scheduling, the model registry, artifact storage, and all persistent state
  (SQLite).
- Run fully offline. AnvilML never makes a network call to any external service
  (no Hugging Face Hub, no telemetry, no update check) in the code paths that ship
  in the first public release.

AnvilML DOES NOT:
- Serve a web UI, host static files, or act as a reverse proxy. It is headless.
- Build, launch, or reference **BloomeryUI** (the separate frontend repo) in any way.
  No code path in AnvilML may import, spawn, or special-case BloomeryUI.
- Build, launch, or reference **SindriStudio** (the separate top-level launcher that
  spawns AnvilML and BloomeryUI as sibling OS processes).
- Provide a plugin/extension marketplace, a node-graph canvas, or any frontend
  concern. These belong to BloomeryUI or are out of scope entirely.
- Target Apple Silicon (MPS) or Intel (IPEX/OpenVINO) in this design. See §2.

**Why this boundary matters for the agent:** if a task description seems to require
touching frontend rendering, packaging a desktop app, or contacting a model hub, that
task description is wrong. Stop and write a blocker. Do not implement it "to be safe."

---

## 2. Platform & Runtime Targets

### 2.1 What is actually in scope

| Concern | Linux | Windows | Out of scope |
|:--------|:------|:--------|:--------------|
| Compilation | `x86_64-unknown-linux-gnu` | `x86_64-pc-windows-msvc` | macOS, ARM |
| GPU backend | CUDA (NVIDIA), ROCm (AMD) | CUDA (NVIDIA), ROCm (AMD, via PyTorch-on-Windows ROCm ≥ 7.2) | MPS, IPEX/OpenVINO, DirectML |
| CPU backend | torch CPU wheel | torch CPU wheel | — |
| IPC transport | ZeroMQ TCP loopback | ZeroMQ TCP loopback (identical code path) | — |
| Orphan cleanup | `PR_SET_PDEATHSIG` | Windows Job Object (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) | — |
| Python | 3.12.x, user-managed venv | 3.12.x, user-managed venv | — |

CPU is a first-class, fully tested device, not a degraded fallback. Every node must
work correctly on CPU even if slowly.

### 2.2 Real hardware availability during development (read this before writing a test)

| Environment | Has real GPU? | What runs here |
|:------------|:--------------|:----------------|
| Forge agent (WSL2 VM, 10 GB RAM) | No | Mock-mode tests (no torch). Real-mode tests using **tiny synthetic fixture checkpoints** on torch CPU. Never a real production-size checkpoint. |
| GitHub CI (`ubuntu-latest`, `windows-latest`) | No | Same as the Forge agent: mock-mode + real-mode-with-fixtures, torch CPU wheel only. |
| Project owner's Windows box | Yes — AMD RX 9070 (ROCm ≥ 7.2) | Manual verification only. Not part of any automated task. A task is never "blocked" on this — if real-GPU verification is needed, the task's report says so explicitly under `## Blockers` or a dedicated note, and the project owner runs it manually. |

**Hard rule:** no task may assume real GPU hardware is available to run its own
tests. Every automated test — mock or real — must pass on torch CPU. "Real-mode" does
not mean "GPU-mode"; it means "the actual code path that touches `torch`/`safetensors`/
`diffusers`/`transformers`, with no mock shortcut," and on CPU that real code path uses
a tiny fixture checkpoint, never the production-size model.

### 2.3 Model matrix (MVP)

These are the only model architectures in scope for the initial release. See
Appendix B for the full per-model file/component breakdown.

| Diffusion model | Text encoder | Notes |
|:----------------|:-------------|:------|
| Z-Image Turbo (ZiT), FP8 | Qwen3 4B | |
| Flux 2 Klein 4B, FP8 | Qwen3 4B | |
| Flux 2 Klein 9B, FP8 | Qwen3 8B (FP8-mixed) | |

All three share the same VAE family per their own architecture (ZiT-compatible VAE;
Flux 2-compatible VAE — two VAE arch modules, not three).

---

## 3. Crate Decomposition

### 3.1 Workspace Layout

```
AnvilML/
├── Cargo.toml                        # Workspace root
├── Cargo.lock                        # Committed; updated by cargo
├── rust-toolchain.toml               # Pinned to 1.96.0, edition 2024 — see §18.1
├── anvilml.toml                      # Checked-in default configuration
├── .gitattributes                    # *.sh / *.py / *.rs = LF; *.ps1 = CRLF
│
├── api/
│   └── openapi.json                  # Generated by anvilml-openapi; committed
│
├── backend/
│   ├── src/
│   │   ├── main.rs                   # Entry point: parse CLI, load config, start server
│   │   ├── cli.rs                    # clap argument parsing
│   │   └── shutdown.rs               # Cross-platform graceful shutdown signal handler
│   └── tests/                        # Integration tests (Rust): api_*.rs
│
├── crates/
│   ├── anvilml-core/                 # Domain types, config, errors — zero I/O, zero async
│   ├── anvilml-hardware/             # GPU + host detection; refreshable VRAM snapshot
│   ├── anvilml-registry/             # Model scanner + SQLite persistence
│   ├── anvilml-artifacts/            # Content-addressed PNG artifact storage
│   ├── anvilml-ipc/                  # IPC message types + ZeroMQ transport (no process mgmt)
│   ├── anvilml-worker/               # Worker pool: spawn, supervise, respawn
│   ├── anvilml-scheduler/            # Job queue, VRAM ledger, graph validation, dispatch
│   ├── anvilml-server/               # axum HTTP/WS server, all handlers
│   └── anvilml-openapi/              # Build-time binary: emits openapi.json
│
├── database/
│   ├── migrations/                   # sqlx SQL migration files (numbered, sequential)
│   └── seeds/
│       └── devices.sql               # One-time hand-converted from docs/SUPPORTED_DEVICES_DB.md
│                                       # (source file deleted after conversion) — see §7.5.
│                                       # Maintained externally to AnvilML from this point on.
│
├── scripts/
│   ├── install_worker_deps.sh        # Linux venv provisioning
│   └── install_worker_deps.ps1       # Windows venv provisioning
│
├── worker/
│   ├── worker_main.py                # Entry point spawned by Rust. NO mock-only gate — see §14.1
│   ├── ipc.py                        # ZeroMQ DEALER transport + msgpack framing
│   ├── executor.py                   # Graph topological sort + node execution loop
│   ├── pipeline_cache.py             # In-worker LRU model/pipeline cache
│   ├── capability.py                 # Runtime torch capability self-detection — see §6.6
│   ├── nodes/
│   │   ├── __init__.py               # NODE_REGISTRY + auto-import
│   │   ├── base.py                   # BaseNode ABC, @register decorator, SlotType enum
│   │   ├── loader.py                 # Generic LoadModel, LoadClip, LoadVae nodes
│   │   ├── encoder.py                # Generic ClipTextEncode node
│   │   ├── sampler.py                # Generic Sampler node (dispatches by arch)
│   │   ├── decode.py                 # Generic Decode (VAE decode) node
│   │   ├── image.py                  # SaveImage node
│   │   └── arch/
│   │       ├── diffusion/
│   │       │   ├── __init__.py       # Diffusion architecture registry + dispatch
│   │       │   ├── zit.py            # Z-Image Turbo: shape inference + raw construction
│   │       │   └── flux2klein.py     # Flux 2 Klein (4B and 9B): shape inference + raw construction
│   │       ├── clip/
│   │       │   ├── __init__.py       # Text-encoder architecture registry + dispatch
│   │       │   └── qwen3.py          # Qwen3 4B / 8B: shape inference + raw construction
│   │       └── vae/
│   │           ├── __init__.py       # VAE architecture registry + dispatch
│   │           ├── zit_vae.py        # ZiT-compatible VAE
│   │           └── flux2_vae.py      # Flux 2-compatible VAE
│   ├── assets/
│   │   └── qwen3_tokenizer/          # Vendored Qwen3 tokenizer (vocab/merges/config)
│   ├── tools/
│   │   ├── seed_tokenizers.sh        # Re-seeds worker/assets/ from upstream sources
│   │   └── seed_tokenizers.ps1       # Windows equivalent
│   ├── requirements/
│   │   ├── base.txt                  # Core deps: diffusers, transformers, safetensors,
│   │   │                             #   pillow, msgpack, pyzmq, pytest. NEVER torch — see §18.6
│   │   ├── cuda.txt                  # torch + CUDA index
│   │   ├── rocm-linux.txt            # torch + ROCm index (Linux)
│   │   ├── rocm-windows.txt          # AMD PyTorch-on-Windows (ROCm ≥ 7.2)
│   │   ├── cpu-linux-agent.txt       # torch CPU wheel — used by the Forge agent, unchanged
│   │   └── cpu-runner-reqs.txt       # torch CPU wheel — used by GitHub CI runners
│   └── tests/
│       ├── fixtures/                 # Tiny synthetic .safetensors checkpoints — see §17.5
│       └── ...                       # pytest: one file per module under test
│
└── docs/
    ├── ANVILML_DESIGN.md             # This document — single source of truth
    ├── ARCHITECTURE.md                # Navigational summary; reads from this file
    ├── ENVIRONMENT.md                 # Config fields, env vars, log fields, build commands
    ├── PHASES.md                      # Phase registry (vertical slices)
    ├── TESTS.md                       # Test catalogue: every test, context, inputs, outputs
    ├── SUPPORTED_DEVICES_DB.md         # Human reference only — see §7.5. NEVER deleted,
    │                                   # NEVER auto-processed again after the one-time
    │                                   # conversion task. No task may delete this file.
    └── TASKS_PHASE*.md                # Per-phase narrative + Runnable Proof
```

**Note on `requirements/base.txt`:** `torch` must never appear in this file. `torch`
is GPU-architecture-dependent and is installed by the matching `cuda.txt` /
`rocm-linux.txt` / `rocm-windows.txt` / `cpu-linux-agent.txt` / `cpu-runner-reqs.txt`
file for the target environment. A task that adds `torch` to `base.txt` has broken
the provisioning system for every other target. `cpu-linux-agent.txt` and
`cpu-runner-reqs.txt` are separate files (not one shared file) so a CI-only pin
change never touches the Forge agent's own environment file, and vice versa.

### 3.2 Crate Dependency Graph

```
anvilml-core  (no deps — pure data)
  ├── anvilml-hardware      (← core)
  ├── anvilml-registry      (← core)
  ├── anvilml-artifacts     (← core)
  └── anvilml-ipc           (← core)
        └── anvilml-worker  (← ipc, hardware, core)
              └── anvilml-scheduler  (← worker, registry, artifacts, core)
                    └── anvilml-server  (← all above)
                          └── backend/src/main.rs

anvilml-openapi  (← core, server — build-time only)
```

**Invariant, checked every task:** no crate may depend on a crate above it in this
graph. Before adding any `path = "../anvilml-X"` dependency to a `Cargo.toml`, find
`anvilml-X` in the graph above and confirm the crate being edited is below it, not
above it. A new edge that would create a cycle is a design error, not a "we'll fix
the dependency direction in the implementation" situation — write a blocker and STOP.
**This exact category of error (an agent inventing a cross-crate dependency outside
this graph, justified by a claimed circular-dependency problem that did not actually
exist) is the single most serious failure recorded in this project's history.** See
§8.0 for the specific IPC-layer rule that exists because of it. A claim of the form
"X must depend on Y because otherwise there is a cycle" is **always** to be verified
against this exact diagram and the real `Cargo.toml` dependency lists before being
acted on — never accepted as a reason to relocate a module.

### 3.3 Crate Responsibilities

| Crate | Responsibility | Hard constraints |
|:------|:---------------|:----------------|
| `anvilml-core` | Pure data: all domain types, config schema, error enum. | Zero I/O. Zero async. No `tokio`, no `sqlx`, no network. |
| `anvilml-hardware` | GPU/CPU detection; refreshable VRAM snapshot; pre-spawn capability hints. | Never panics on missing driver. Always returns at least one CPU device. **Does not claim to know real torch-level capability** — see §6. |
| `anvilml-registry` | Scan model directories; persist `ModelMeta` to SQLite; `SeedLoader`. | Scanner is non-recursive by default; configurable depth. |
| `anvilml-artifacts` | Content-addressed PNG artifact storage; persist `ArtifactMeta` to SQLite. | Owned independently; both `anvilml-scheduler` and `anvilml-server` depend on it directly — neither owns the other's copy. |
| `anvilml-ipc` | ZeroMQ ROUTER transport wrapper (Rust side); `WorkerMessage`/`WorkerEvent` enums; msgpack serialisation. | No business logic. No process management. No knowledge of `ManagedWorker` or any worker lifecycle state. |
| `anvilml-worker` | Spawn Python worker subprocesses; manage lifecycle; respawn on crash; keepalive. | One `ManagedWorker` owns exactly one subprocess. See §8/§9 for the exact ownership model — do not improvise it. |
| `anvilml-scheduler` | Accept submitted job graphs; validate DAG; maintain job queue; track VRAM; dispatch. | Node type registry is dynamic — populated from worker's `Ready` event, never hardcoded. |
| `anvilml-server` | axum router; all HTTP handlers; WebSocket broadcaster; OpenAPI annotations. | No business logic in handler functions — handlers call into scheduler/worker/registry/artifacts only. |

---

## 4. Code Quality & Conventions

These conventions are enforced by CI and by code review. A task that violates them is
not complete.

### 4.1 File Size Guidelines

Thresholds are a **review trigger**, not an automatic split requirement. When a file
crosses its threshold, ask: does this file own one coherent concern, or did unrelated
logic get mixed in?

| Language | Review threshold | Split signal |
|:---------|:----------------|:--------------------|
| Rust source (`.rs`) | 400 lines | File mixes data types, business logic, and utilities |
| Python source (`.py`) | 350 lines | File mixes I/O, computation, and configuration |
| Test files (any language) | 500 lines | Tests cover more than one logical unit |
| `lib.rs` (any crate) | **80 lines, hard cap** | N/A — `lib.rs` never contains implementation code |

`lib.rs` is the one absolute rule in this table: `pub mod` / `pub use` / crate-level
`//!` doc comment only. Never implementation code. The 80-line cap exists because
legitimate `lib.rs` content never approaches it — if a `lib.rs` is near 80 lines,
something does not belong there.

If a file legitimately needs to exceed its threshold (a complex state machine, a
comprehensive protocol codec), keep it whole and write the justification in
`## Deviations from Plan`. Splitting purely to hit a number produces worse
architecture than a coherent large file.

### 4.2 Module Splitting Rules

1. **Extract by concern.** Data types → `types.rs`. Dispatch logic → `dispatch.rs`.
   Utilities → `util.rs`.
2. **Extract tests.** All `#[cfg(test)]` blocks move to `crates/{name}/tests/` except
   trivial single-function unit tests (≤ 20 lines, no helpers, no I/O).
3. **Extract subtypes.** A struct/enum that has grown large moves to its own
   `{name}.rs`.

### 4.3 Rust Module Structure per Crate

```
crates/anvilml-{name}/
├── Cargo.toml
├── src/
│   ├── lib.rs          # re-exports only; ≤ 80 lines
│   ├── {concern_a}.rs
│   └── {concern_b}.rs
└── tests/
    ├── {concern_a}_tests.rs
    └── {concern_b}_tests.rs
```

### 4.4 Python Module Structure

```
worker/
├── {module}.py
└── tests/
    ├── test_{module}.py
    └── conftest.py     # shared fixtures only — no test functions here
```

Test files import only the public interface of the module under test.

### 4.5 Documentation Requirements

Every `pub fn` / `pub struct` / `pub enum` / `pub trait` / `pub const` has a `///` doc
comment describing what it *does* (not what it *is*), preconditions/postconditions,
and argument/return meaning.

Every Python class and function with a non-obvious contract has a Google-style
docstring (one-sentence summary, then `Args`/`Returns`/`Raises`).

Inline comments (`//`, `#`) are mandatory at every decision point: a branch taken, a
value selected, a type converted, a fallback used. Silence in code is a defect.

### 4.6 Error Handling

- **Rust:** never `.unwrap()`/`.expect()` in non-test code. Use `?`. A failure that
  must not propagate (non-critical background task) is handled with an explicit
  `Err` branch and a WARN/ERROR log call.
- **Python:** never bare `except:`. Catch specific exception types. Every `except`
  re-raises, logs, or documents why swallowing is correct.
- **Rust tests:** `.unwrap()`/`.expect("message")` acceptable — a panic is the
  correct failure signal in a test.

### 4.7 Async Discipline

- Never `.await` inside a `std::sync::Mutex` guard. Use `tokio::sync::Mutex` for
  guards held across `.await`.
- Never `tokio::task::spawn_blocking` unless there is no async alternative; document
  why with an inline comment.
- Every `JoinHandle` is stored and awaited on shutdown. No detached `tokio::spawn`
  with a discarded result.

### 4.8 Cargo Version Bumping

Every task that modifies a source file inside a crate increments that crate's patch
version (`Z` in `X.Y.Z`) before staging. Exact procedure in `docs/ENVIRONMENT.md §12`.

### 4.9 Mandatory Build Cache Cleanup (every ACT session, no exceptions)

**Every task's ACT session runs a cache cleanup as its last step, after all
build/test commands have completed and before the session ends — regardless of task
size, crate touched, or whether the task built anything new.** This is a direct,
quantified fix for a real, recorded incident: across the dozens of phases and tasks
executed in v2/v3, accumulated build caches reached over 200GB of disk before being
cleared, entirely from uncleaned `cargo` artifacts. On the 10GB-RAM WSL2 agent VM
(§2.2) this is not just disk hygiene — an unbounded cache is a standing risk to the
agent's own ability to keep running.

**Required commands, every ACT session, every task, run in this order:**

```bash
# Rust — run from the workspace root, regardless of which crate(s) the task touched
cargo clean

# Python — run from worker/, regardless of whether the task touched worker/ at all
find . -type d -name "__pycache__" -exec rm -rf {} +
find . -type d -name ".pytest_cache" -exec rm -rf {} +
rm -rf .mypy_cache .ruff_cache
```

**Rules, stated explicitly so this is never skipped or narrowed under task pressure:**

1. This runs **unconditionally** at the end of every ACT session that ran any
   `cargo build`/`cargo test`/`cargo check`/`pytest` command during that session —
   which in practice means every ACT session, since verifying the task's own change
   always requires at least one of these. A task that built nothing new (e.g. a
   pure-documentation task) is the only category exempt, and only because no cache
   was created to clean.
2. `cargo clean` is run **workspace-wide** (no `-p <crate>` scoping) even if the
   task touched only one crate — partial cleans leave the bulk of the 200GB-class
   accumulation in place, since most of that volume comes from dependency
   compilation artifacts shared across crates, not from the one crate the task
   happened to touch.
3. This is a step in the task's own ACT session, not a separately scheduled
   maintenance task, and not something deferred to "the next session" — a `defers_to`
   entry pointing this obligation at a future task is non-compliant with
   `FORGE_AGENT_RULES.md §9.7a` in exactly the way deferring any other mandatory step
   would be.
4. Running cleanup means the next task's ACT session starts from a cold build cache.
   This is an accepted, deliberate cost (slower first build of the next session) in
   exchange for bounded disk usage across the life of the project — it is not a
   regression to "optimize away" by skipping the clean.
5. This rule applies identically in CI (§18.3) is **not** required — CI runners are
   ephemeral and already discard their entire filesystem at job end. This rule is
   scoped to the Forge agent's persistent WSL2 VM and to the project owner's own
   local development environment, both of which are long-lived and accumulate cache
   across sessions.

---

## 5. Domain Types (`anvilml-core`)

`anvilml-core` is the type authority for the entire system. Zero runtime dependencies
beyond `serde`, `uuid`, `chrono`. No I/O. No async. No tokio.

### 5.1 Module Layout

```
anvilml-core/src/
├── lib.rs           # re-exports; declares submodules; ≤ 80 lines
├── config.rs        # ServerConfig and nested config structs
├── config_load.rs   # load(): layered config precedence (defaults → toml → env → CLI)
├── error.rs         # AnvilError enum + IntoResponse impl
├── node_registry.rs # NodeTypeRegistry: dynamic node type map, populated from worker Ready
└── types/
    ├── mod.rs
    ├── job.rs        # Job, JobStatus, JobSettings, SubmitJobRequest/Response
    ├── model.rs      # ModelMeta, ModelKind, ModelDtype, ModelFormat
    ├── artifact.rs   # ArtifactMeta
    ├── hardware.rs   # HardwareInfo, GpuDevice, DeviceType, InferenceCaps, CapabilitySource
    ├── worker.rs      # WorkerInfo, WorkerStatus, EnvReport, ProvisioningState
    ├── node.rs        # NodeTypeDescriptor, SlotDescriptor, SlotType
    └── events.rs      # WsEvent and all sub-event structs
```

### 5.2 `AnvilError`

```rust
#[derive(Debug, thiserror::Error)]
pub enum AnvilError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(String),
    #[error("IPC error: {0}")]
    Ipc(String),
    #[error("payload too large: {0}")]
    PayloadTooLarge(String),
    #[error("worker not found: {0}")]
    WorkerNotFound(String),
    #[error("job not found: {0}")]
    JobNotFound(String),
    #[error("invalid graph: {0:?}")]
    InvalidGraph(Vec<String>),
    #[error("graph cycle detected: {0:?}")]
    CycleDetected(Vec<String>),
    #[error("model not found: {0}")]
    ModelNotFound(String),
    #[error("workers unavailable: {0}")]
    WorkersUnavailable(String),
    #[error("internal error: {0}")]
    Internal(String),
}
```

`AnvilError` implements `IntoResponse` for axum: each variant maps to an HTTP status
and a structured JSON body `{ "error": "<kind>", "message": "<text>", "request_id": "<uuid>" }`.

### 5.3 Job Types

```rust
/// A submitted generation job.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Job {
    pub id: Uuid,
    pub status: JobStatus,
    pub graph: serde_json::Value,   // submitted graph JSON; opaque to Rust
    pub settings: JobSettings,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub worker_id: Option<String>,
    pub error: Option<String>,
    pub queue_position: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Settings that accompany every job submission.
/// Node-level parameters are in the graph itself; these are execution-level settings.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct JobSettings {
    /// Requested device. None = auto-select by VRAM.
    pub device_preference: Option<String>,
}
```

### 5.4 Model Types

```rust
/// Metadata about a discovered model file.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ModelMeta {
    /// Stable identifier: SHA256 hex of the first 1 MiB of the file.
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub kind: ModelKind,
    pub dtype: ModelDtype,
    pub format: ModelFormat,
    pub size_bytes: u64,
    pub scanned_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    Diffusion,
    TextEncoder,
    Vae,
    Lora,
    ControlNet,
    Upscale,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelDtype {
    Fp32, Fp16, Bf16, Fp8, Fp4, Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelFormat {
    Safetensors,
    Ckpt,
    Pt,
    Bin,
    Unknown,
}
```

### 5.5 Hardware Types

```rust
/// Full hardware snapshot for the host.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HardwareInfo {
    pub host: HostInfo,
    pub gpus: Vec<GpuDevice>,
    /// Union of all per-device inference capabilities.
    pub inference_caps: InferenceCaps,
}

/// A single detected compute device.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GpuDevice {
    pub index: u32,
    pub name: String,
    pub device_type: DeviceType,
    pub vram_total_mib: u32,
    pub vram_free_mib: u32,
    pub driver_version: String,
    pub pci_vendor_id: u16,
    pub pci_device_id: u16,
    pub arch: Option<String>,
    pub caps: InferenceCaps,
    pub enumeration_source: EnumerationSource,
    pub capabilities_source: CapabilitySource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum DeviceType { Cuda, Rocm, Cpu }

/// Inference precision capabilities.
///
/// Pre-spawn values (`capabilities_source = DeviceTable` or `Fallback`) are HINTS
/// only — they exist so the scheduler can make a provisional VRAM/dtype guess before
/// any worker has started. They are never trusted as ground truth for an actual
/// inference decision. The authoritative values come from the Python worker's own
/// runtime probe at `Ready` (`capabilities_source = PyTorch`) — see §6.6.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct InferenceCaps {
    pub fp32: bool,
    pub fp16: bool,
    pub bf16: bool,
    pub fp8: bool,       // Compute capability, not storage capability — see §6.6
    pub fp4: bool,
    pub flash_attention: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum EnumerationSource { Vulkan, Dxgi, Sysfs, Nvml, Mock, Override }

/// Where an `InferenceCaps` value came from. `PyTorch` is the only source an arch
/// module's loader is permitted to make a compute-dtype decision from at runtime.
/// `DeviceTable` and `Fallback` are pre-spawn hints for scheduling estimates only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum CapabilitySource { PyTorch, DeviceTable, Fallback }
```

### 5.6 Node Types (Generic Contract)

```rust
/// Description of a node type as reported by the Python worker at Ready.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NodeTypeDescriptor {
    /// Unique name (e.g. "LoadModel", "ClipTextEncode", "Sampler").
    pub type_name: String,
    pub display_name: String,
    pub category: String,
    pub description: String,
    pub inputs: Vec<SlotDescriptor>,
    pub outputs: Vec<SlotDescriptor>,
}

/// Describes one input or output slot on a node type.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SlotDescriptor {
    pub name: String,
    pub slot_type: SlotType,
    /// True if this input can be omitted (uses a node-internal default).
    pub optional: bool,
}

/// The semantic type of a node slot. Used by the scheduler to verify connected
/// slots are type-compatible, checked at job submission, not at execution time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SlotType {
    Model,
    Clip,
    Vae,
    Conditioning,
    Latent,
    Image,
    String,
    Int,
    Float,
    Bool,
    /// Disables type checking for this slot.
    Any,
}
```

**Key design decisions, unchanged from v3 (these were not the source of any v3
failure):**
- `SlotType` is architecture-agnostic. `LoadModel` always outputs `SlotType::Model`
  regardless of whether the loaded model is ZiT or Flux 2 Klein.
- `LoadModel` outputs **only** `SlotType::Model`. It never implicitly provides a VAE.
  `VaeDecode` requires `SlotType::Vae` as an explicit input, always wired from a
  separate `LoadVae` node. No hidden state propagation between nodes, ever.

### 5.7 Worker Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WorkerInfo {
    pub id: String,
    pub device_index: u32,
    pub device_name: String,
    pub status: WorkerStatus,
    pub current_job_id: Option<Uuid>,
    pub vram_used_mib: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum WorkerStatus {
    Initializing,
    Idle,
    Busy,
    Dead,
    Respawning,
}

/// Python environment health report, populated at startup preflight.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EnvReport {
    pub python_path: Option<String>,
    pub python_version: Option<String>,
    pub torch_version: Option<String>,
    pub provisioning: ProvisioningState,
    pub preflight_ok: bool,
    pub reason: Option<String>,
    pub node_types: Vec<NodeTypeDescriptor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum ProvisioningState {
    Ready,
    Provisioning,
    Failed,
    NotStarted,
}
```

### 5.8 WebSocket Event Types

```rust
/// All event types broadcast over the WebSocket stream.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEvent {
    JobQueued { job_id: Uuid, queue_position: u32 },
    JobStarted { job_id: Uuid, worker_id: String },
    JobProgress { job_id: Uuid, step: u32, total_steps: u32, preview_b64: Option<String> },
    JobImageReady { job_id: Uuid, artifact_hash: String, width: u32, height: u32, seed: i64, steps: u32 },
    JobCompleted { job_id: Uuid, elapsed_ms: u64 },
    JobFailed { job_id: Uuid, error: String },
    JobCancelled { job_id: Uuid },
    WorkerStatusChanged { worker_id: String, status: WorkerStatus, device_index: u32 },
    SystemStats { cpu_pct: f32, ram_used_mib: u64, workers: Vec<WorkerInfo> },
    ProvisioningProgress { message: String, pct: u8 },
}
```

---

## 6. Hardware Detection (`anvilml-hardware`)

### 6.1 Two Separate Questions — Do Not Conflate Them

There are two different questions and two different owners. Confusing them was an
unresolved gap in v3.

| Question | Who answers it | When | How authoritative |
|:---------|:----------------|:-----|:-------------------|
| "What GPU devices physically exist, and how much VRAM do they have?" | `anvilml-hardware` (Rust) | Once at server startup, before any worker spawns | Authoritative for enumeration and VRAM. |
| "Can THIS device, with THIS installed torch build, actually run fp8/bf16/fp16 compute?" | The Python worker, via `worker/capability.py` | Every time a worker starts (`worker_main()`), before reporting `Ready` | Authoritative for compute capability. Rust's pre-spawn guess is never trusted for this. |

`anvilml-hardware` enumerates devices and VRAM. It does **not** claim to know whether
a device can run FP8 compute — it can look up a PCI-ID table for a *hint*, but the
table is right about silicon support and can still be wrong about what an actual
installed PyTorch build supports on that silicon. Only `torch` itself, imported and
queried inside the worker process, knows that.

### 6.2 Design Principles

Detection is SDK-free. The Vulkan loader ships with every modern GPU driver; it is
not an SDK. No `nvidia-smi`, no `rocm-smi`, no `lspci`, no CUDA/ROCm toolkits required.

Detection never panics. If the Vulkan loader is absent, the function returns
`Ok(vec![])`. If no GPU is detected, a CPU device is synthesised. Result is always
`Ok(HardwareInfo)` with at least one device.

Detection runs once at startup. VRAM is refreshed on each dispatch.

### 6.3 Module Layout

```
anvilml-hardware/src/
├── lib.rs          # re-exports; ≤ 80 lines
├── detect.rs       # detect_all_devices(), DeviceDetector trait, orchestration
├── cpu.rs          # CpuDetector: always returns one CPU device
├── vulkan.rs       # VulkanDetector: headless Vulkan enumeration via ash
├── dxgi.rs         # DxgiDetector: Windows DXGI IDXGIFactory1 (Windows only)
├── sysfs.rs        # SysfsPciDetector: /sys/bus/pci/devices/* (Linux only)
└── mock.rs         # MockDetector: env-var driven stubs (mock-hardware feature only)
```

```
anvilml-hardware/tests/
├── vulkan_tests.rs
└── mock_tests.rs
```

**No hardcoded `device_db.rs` in this crate.** v3 had an in-Rust-source PCI-ID →
capability hint table here. v4 removes it: the same hint data now has exactly one
authored source, `docs/SUPPORTED_DEVICES_DB.md` (plain Markdown tables), generated
once into `database/seeds/devices.sql`, and queried at runtime through
`anvilml-registry`'s `DeviceCapabilityStore` (§7.5) — not duplicated as a second,
independently-maintained Rust match table that could drift from the SQL seed. A task
must not reintroduce a `device_db.rs`-shaped module "for convenience" — if Rust code
needs a PCI-ID hint lookup, it queries `DeviceCapabilityStore` via the already-passed
`SqlitePool` (see `detect_all_devices`'s signature, §6.4), the same as every other
piece of persisted data in this system.

NVML is removed from v4's module list (it was Linux-only VRAM refresh in v3 and is
not required for the MVP's CUDA/ROCm target — VRAM refresh during dispatch can use
Vulkan's own memory-heap query, which is already cross-platform). If a future task
finds Vulkan's VRAM query insufficient, add `nvml.rs` then with a stated reason; do
not add it speculatively.

### 6.4 Detection Orchestration

```rust
pub async fn detect_all_devices(
    cfg: &ServerConfig,
    pool: &SqlitePool,
) -> Result<HardwareInfo, AnvilError>
```

Priority order:
1. **Hardware override** (config `[hardware_override]`) — CI / isolated test envs.
2. **Mock** (`mock-hardware` cargo feature) — driven by `ANVILML_MOCK_*` env vars.
3. **Vulkan** (primary real-hardware path, all platforms).
4. **DXGI** (Windows fallback when Vulkan returns empty).
5. **PCI sysfs** (Linux fallback when Vulkan returns empty).
6. **CPU** (always appended as the final fallback device).

### 6.5 `DeviceDetector` Trait

```rust
pub trait DeviceDetector: Send + Sync {
    fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError>;
    /// Returns (total_mib, free_mib).
    fn refresh_vram(&self, index: u32) -> Result<(u32, u32), AnvilError>;
}
```

### 6.6 Runtime Capability Self-Detection (Python worker, mandatory)

This mechanism did not exist in v3 — it was identified as a required follow-up and
never built. It is a hard requirement in v4, scoped into the earliest worker-startup
phase, not deferred.

**Contract:** `worker/capability.py` exposes one function:

```python
def probe_capabilities(device_type: str, device_index: int) -> dict:
    """Probe the actual torch-level compute capability of one device.

    Called once during worker_main() startup, after torch is imported and the
    device is selected, before the Ready event is sent. Never called with mock
    mode active — mock mode skips this entirely and uses fixed synthetic values
    (see §14.3).

    Args:
        device_type: "cuda", "rocm", or "cpu" — the device this worker owns.
        device_index: The device index (ignored for "cpu").

    Returns:
        dict with keys fp32, fp16, bf16, fp8, fp4, flash_attention (all bool),
        matching InferenceCaps' field names exactly so it serialises directly
        into the Ready event's capability fields.
    """
```

**What "probe" means, concretely** — this is the mechanical contract every
implementation of `probe_capabilities` must follow, regardless of device type:

| Capability | How to probe it (not a hint lookup) |
|:-----------|:-------------------------------------|
| `fp16`/`bf16` | Construct a tiny `torch.nn.Linear` at that dtype on the target device, run one forward pass on a small tensor, catch the failure type if it raises. |
| `fp8` | Same pattern at `torch.float8_e4m3fn`. **On CPU this always raises `NotImplementedError` today** — confirmed directly in v3's investigation. `fp8 = False` on CPU is the correct, expected result, not a bug to "fix." |
| `flash_attention` | Attempt the lightest available flash-attention call path for the installed torch/backend combination at small size; `False` on any exception. |

A probe function that returns a hardcoded `True` for any field without running the
above check is non-compliant, regardless of how the device-table hint says the
silicon "should" support it. This is the literal pattern that produced v3's
unresolved gap: the database/PCI-table can correctly say "this silicon supports FP8"
while the actually-installed torch build cannot use it.

**Where this plugs in:**
- `worker_main()` calls `probe_capabilities()` once at startup (real mode only).
- The result populates the `fp16`/`bf16`/`fp8`/`fp4`/`flash_attention` fields of the
  `Ready` event (§8.5) with `capabilities_source = PyTorch`.
- Every arch module's `load()` (§11.3; the name is identical across diffusion, CLIP,
  and VAE arch modules — see §11.8) makes its dtype decision (§11.5) from the
  **worker's own probed capabilities passed in via `NodeContext`**, never from the
  PCI-ID hint table behind `DeviceCapabilityStore` (§7.5), and never by assuming
  "this is the production checkpoint so it must support fp8."

### 6.7 Mock Feature

When compiled with `--features mock-hardware`, `MockDetector` reads:

| Env var | Default | Description |
|:--------|:--------|:------------|
| `ANVILML_MOCK_DEVICE_TYPE` | `cpu` | `cuda`, `rocm`, or `cpu` |
| `ANVILML_MOCK_VRAM_MIB` | `8192` | VRAM to report |
| `ANVILML_MOCK_DEVICE_NAME` | `Mock GPU` | Device name to report |

Forwarded through `anvilml-worker`, `anvilml-scheduler`, `anvilml-server`, `backend`.
All CI runs use this feature. Real-hardware runs must never use it.

---

## 7. Model Registry (`anvilml-registry`)

### 7.1 Module Layout

```
anvilml-registry/src/
├── lib.rs              # re-exports; ≤ 80 lines
├── db.rs                # SqlitePool creation, migrations, ghost-job reset on startup
├── scanner.rs           # ModelScanner: directory walk, ModelMeta derivation
├── store.rs             # ModelStore: CRUD for ModelMeta
├── device_store.rs      # DeviceCapabilityStore: PCI-ID rows (HINT table — see §6.1)
└── seed_loader.rs       # SeedLoader: SHA256-gated SQL seed runner
```

### 7.2 Model ID Derivation

A model's stable identifier is the lower-case hex SHA256 of the first 1 MiB of its
file content. Fast for large files, stable across renames, collision-resistant for
models (which differ significantly in their first megabyte). Files smaller than
1 MiB are hashed whole.

### 7.3 `ModelKind` Inference

Inferred from the filesystem path's directory component relative to the configured
model root:

| Directory | Inferred `ModelKind` |
|:----------|:---------------------|
| `diffusion/` | `Diffusion` |
| `text_encoders/` | `TextEncoder` |
| `vae/` | `Vae` |
| (any other) | `Unknown` |

`Lora`, `ControlNet`, and `Upscale` kinds remain in the `ModelKind` enum (§5.4) for
forward compatibility but have no scanner directory mapping in the MVP — there is no
LoRA/ControlNet/Upscale node in scope (§10.3). A task must not add scanning support
for these without a corresponding node-system task; the enum variant existing is not
itself a license to wire it up.

`ModelDtype` is inferred from the filename: substrings `fp8`, `fp16`, `bf16`, `fp32`
(case-insensitive) set the corresponding variant. `fp8_e4m3fn`/`fp8_e5m2` both match
`Fp8`. Ambiguous filenames default to `Unknown`.

### 7.4 Scanner Behavior

Non-recursive by default; depth configurable via `model_scan_depth` (§15). Scanner
runs at startup and on `POST /v1/models/rescan`. A file already in the database with
an unchanged size and mtime is not re-hashed.

### 7.5 Device Capability Hint Table: One-Time Conversion, Then Frozen

**`docs/SUPPORTED_DEVICES_DB.md` is a one-time data source, not a build input.** It
contains the PCI-ID → capability hint tables in plain Markdown, used exactly once by
one task to hand-produce `database/seeds/devices.sql`. After that task closes:

- `database/seeds/devices.sql` becomes the sole runtime source for this data, loaded
  the same way as before (the existing SHA256-gated `SeedLoader`, §7.1) — no change
  to how the server consumes it.
- **`docs/SUPPORTED_DEVICES_DB.md` stays in the repository permanently as a human
  reference, but is never read by any code, never reprocessed, and never deleted by
  any task, agent, or automated step, for any reason.** There is no `defers_to`, no
  cleanup task, no "stale doc" sweep that may remove it. If a future task is tempted
  to delete it because "nothing reads it anymore," that observation is correct and
  irrelevant — its presence is itself the requirement.
- Any future change to device capability data (a new GPU added, a corrected
  capability flag) is made directly to `devices.sql` (or via whatever external
  process the project owner uses going forward) — **never** by editing
  `SUPPORTED_DEVICES_DB.md` and expecting a regeneration step to apply it. No such
  regeneration step exists in this design, on purpose. A task that adds a Markdown→
  SQL converter, a CI drift gate comparing the two, or any other automated link
  between these two files is out of scope and must not be built — that mechanism was
  explicitly removed from this design in favor of a single one-time hand conversion.

**Format of `docs/SUPPORTED_DEVICES_DB.md`** (informational — this is what the
one-time conversion task reads, not a schema any code parses):

```markdown
## NVIDIA

| PCI Vendor ID | PCI Device ID | Name | Arch | FP32 | FP16 | BF16 | FP8 | FP4 | Flash Attn |
|:--------------|:---------------|:-----|:-----|:-----|:-----|:-----|:----|:----|:-----------|
| 0x10DE | 0x2684 | GeForce RTX 4090 | Ada Lovelace | true | true | true | true | false | true |

## AMD

| PCI Vendor ID | PCI Device ID | Name | Arch | FP32 | FP16 | BF16 | FP8 | FP4 | Flash Attn |
|:--------------|:---------------|:-----|:-----|:-----|:-----|:-----|:----|:----|:-----------|
| 0x1002 | 0x7480 | Radeon RX 9070 | RDNA 4 | true | true | true | true | false | true |
```

**The one-time conversion task itself:** read every vendor table in
`docs/SUPPORTED_DEVICES_DB.md`, emit one `INSERT` per row into
`database/seeds/devices.sql`, each preceded by a comment naming the source vendor
heading and row for traceability. This is a single task's worth of work (writing a
short throwaway script or doing it by hand is the task author's choice — nothing
about it needs to be a maintained crate, since it runs once and is then never run
again). The task's report confirms the row count in the Markdown matches the row
count of `INSERT`s produced, then the task closes — there is no follow-on
"regenerate" task ever scheduled.

**This removes the need for `anvilml-hardware/src/device_db.rs` entirely** (§6.3) —
there is no hand-maintained Rust table to keep in sync, because the only Rust-side
consumer of this data (`anvilml-registry`'s `DeviceCapabilityStore`, queried by
`anvilml-hardware`'s detection orchestration via the `SqlitePool` argument already
present in `detect_all_devices`'s signature, §6.4) reads it from SQLite like
everything else the registry owns. A task must not add a Rust-source fallback table
"in case the database seed is missing" — if the seed is missing, the correct
behavior is the same as any unknown device: fall through to a CPU-equivalent
`Fallback`-sourced `InferenceCaps` (§5.5), not a silently-reintroduced second source
of truth.

---

## 8. IPC Protocol (`anvilml-ipc`)

### 8.0 Read This Before Writing Any Code In `anvilml-ipc` Or `anvilml-worker`

The IPC layer was rewritten three times across v2 and v3 (stdio pipes → OS named
pipes/Unix sockets → ZeroMQ DEALER/DEALER → ZeroMQ ROUTER/DEALER), and even the final
ROUTER/DEALER design required multiple rounds of manual, human-driven fixes after
agent sessions left it non-functional: a non-cloneable socket forced an awkward
combined `select!` loop; `ManagedWorker::run(self)` consumed the struct by value while
the pool needed it `Arc`-wrapped, making it impossible to ever call `run()`; a single
shared mutex around both send and receive caused a shutdown deadlock; a demultiplexer
had no deregistration path and leaked routing entries across every respawn.

**Every one of those defects came from the design leaving an ownership question
unanswered, which the agent then had to invent an answer to under task pressure.**
This section exists to remove every such question in advance. If a task in this area
finds itself needing to decide "who owns this lock" or "does this method take `self`
or `&self`," the answer is already written below — re-read this section before
inventing one.

### 8.1 Crate Boundary (do not blur this)

`anvilml-ipc` knows **only**:
- The wire protocol (`WorkerMessage`, `WorkerEvent`, msgpack framing).
- How to bind a ROUTER socket and send/receive framed messages by worker identity.

`anvilml-ipc` knows **nothing** about:
- Subprocess lifecycle, `ManagedWorker`, `WorkerPool`, respawn policy, or keepalive
  timers. All of that is `anvilml-worker`'s concern (§9).

If a task in `anvilml-ipc` needs to reference a worker's lifecycle *status*, that is
a sign the type belongs in `anvilml-worker` or `anvilml-core`, not `anvilml-ipc`. Do
not add a `WorkerStatus`-aware method to anything in `anvilml-ipc`.

### 8.2 Topology

```
┌───────────────────────────┐
│   Rust Supervisor          │
│   zmq.ROUTER                │
│   bind("tcp://127.0.0.1:0") │  ← OS-assigned port; happens ONCE, before any spawn
└──────────────┬─────────────┘
               │  TCP loopback
   ┌───────────┼───────────┐
   ▼           ▼           ▼
┌──────────┐ ┌──────────┐ ┌──────────┐
│ zmq.DEALER│ │ zmq.DEALER│ │ zmq.DEALER│   ← one per Python worker subprocess
│ identity= │ │ identity= │ │ identity= │      identity = worker_id, set BEFORE connect
│ "0"       │ │ "1"       │ │ "2"       │
└──────────┘ └──────────┘ └──────────┘
```

Key properties, each one a direct fix for a v2/v3 incident:
- **The supervisor binds before spawning any worker.** No race on bind-vs-connect.
  (v2's stdio/named-pipe generations both had a connect-before-bind race; this
  topology removes the race by construction, not by timing.)
- **Workers connect before doing any work.** ZeroMQ queues outbound messages until
  the peer is ready — there is no "send before the other side is listening" failure
  mode with this transport, unlike named pipes.
- **Each worker's ZeroMQ identity is its `worker_id` string, set on the DEALER socket
  before `connect()`.** This is not a convention to remember — it is the only
  mechanism that lets one ROUTER socket address N workers without N separate sockets.
- **ROUTER is the correct socket type for this topology**, not DEALER/DEALER (v2's
  mistake) or PAIR (not available in the `zeromq` crate at all, which is what caused
  v2's initial design to be unbuildable). ROUTER/DEALER is N-clients-to-one-server
  with identity-based addressing — exactly this topology.

### 8.3 The Ownership Model (this is the part that was missing in v3)

**`RouterTransport` owns the ROUTER socket. Nothing else ever touches the raw
`zeromq::RouterSocket` directly.**

```rust
/// The Rust-side ZeroMQ ROUTER socket wrapper. Binds on construction.
///
/// Ownership rule: `RouterTransport` is constructed exactly once, by `WorkerPool`,
/// and shared via `Arc<RouterTransport>`. No other code holds the socket directly.
pub struct RouterTransport {
    /// Split into independent send/receive halves at construction time —
    /// NOT a single shared mutex around both directions. A single shared mutex
    /// around send+recv is the exact root cause of v3's shutdown deadlock
    /// (a blocked recv() held the lock that a concurrent shutdown send() needed).
    /// `zeromq::RouterSocket::split()` (or the equivalent in whatever zeromq crate
    /// version is resolved per §6 of FORGE_AGENT_RULES.md) is used here specifically
    /// because it removes this failure mode, not as a stylistic preference.
    sender: Arc<tokio::sync::Mutex<RouterSocketSendHalf>>,
    receiver: Arc<tokio::sync::Mutex<RouterSocketRecvHalf>>,
    pub port: u16,
}

impl RouterTransport {
    /// Bind to a ROUTER socket on an OS-assigned TCP loopback port.
    pub async fn bind() -> Result<Self, AnvilError>;

    /// Send a message to the worker identified by `worker_id`. Locks only the
    /// send half — never blocks on or is blocked by a concurrent `recv()`.
    pub async fn send(&self, worker_id: &str, msg: &WorkerMessage) -> Result<(), AnvilError>;

    /// Receive the next event from any worker. Returns (worker_id, event).
    /// Locks only the receive half.
    pub async fn recv(&self) -> Result<(String, WorkerEvent), AnvilError>;
}
```

**Why split, stated explicitly so it is never re-derived under task pressure:** the
send path and the receive path are used by two different async contexts (a writer
task draining an outbound channel, and a reader task pumping inbound events into a
demux). If both share one lock, a `recv()` that is currently blocked waiting for a
message holds the same lock a concurrent `send()` needs to deliver a `Shutdown`
message — this is exactly v3's shutdown deadlock. Splitting into two locks, one per
direction, removes the possibility structurally. Any future change to this struct
that reintroduces a single combined lock around both `send` and `recv` is a
regression of a previously-fixed incident, not a simplification.

### 8.4 Module Layout

```
anvilml-ipc/src/
├── lib.rs          # re-exports; ≤ 80 lines
├── error.rs        # IPC-specific error types
├── messages.rs     # WorkerMessage and WorkerEvent enums
├── transport.rs    # RouterTransport — see §8.3 for the exact ownership contract
└── ws/
    ├── mod.rs
    └── broadcaster.rs  # EventBroadcaster: tokio::sync::broadcast wrapper
                          # (placed here, not in anvilml-worker or anvilml-server,
                          #  specifically to avoid a worker↔server crate cycle — see §3.2)
```

```
anvilml-ipc/tests/
├── roundtrip_tests.rs    # msgpack roundtrip for all message variants
└── stress_test.rs        # 1000-round-trip ROUTER/DEALER stress test — see §17.2
```

### 8.5 `WorkerMessage` (Rust → Python)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "_type")]
pub enum WorkerMessage {
    /// Keepalive ping. Worker must reply with Pong { seq }.
    Ping { seq: u64 },
    /// Graceful shutdown. Worker should finish current step, then exit 0.
    Shutdown,
    /// Execute a generation job.
    Execute {
        job_id: Uuid,
        graph: serde_json::Value,
        settings: JobSettings,
        device_index: u32,
    },
    /// Cancel an in-flight job cooperatively.
    CancelJob { job_id: Uuid },
    /// Query the worker's current memory usage.
    MemoryQuery,
}
```

There is no `InitializeHardware` message. Hardware initialisation happens inside the
Python worker at startup using `ANVILML_DEVICE_INDEX` from the environment — there is
no runtime hardware-initialisation round trip.

### 8.6 `WorkerEvent` (Python → Rust)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "_type")]
pub enum WorkerEvent {
    /// Worker startup complete. Reports REAL, probed capabilities (§6.6) and
    /// registered node types. Never synthetic placeholder values in real mode.
    Ready {
        worker_id: String,
        device_index: u32,
        device_name: String,
        device_type: String,        // "cuda" | "rocm" | "cpu"
        vram_total_mib: u32,
        vram_free_mib: u32,
        torch_version: String,
        fp16: bool,
        bf16: bool,
        fp8: bool,
        flash_attention: bool,
        capabilities_source: String,  // "pytorch" (real mode) | "mock" (mock mode)
        node_types: Vec<NodeTypeDescriptor>,
    },
    Pong { seq: u64 },
    Dying { reason: String },
    MemoryReport { vram_used_mib: u32, ram_used_mib: u64 },
    Progress { job_id: Uuid, step: u32, total_steps: u32, preview_b64: Option<String> },
    ImageReady {
        job_id: Uuid,
        image_b64: String,
        width: u32,
        height: u32,
        format: String,     // "png"
        seed: i64,
        steps: u32,
    },
    Completed { job_id: Uuid, elapsed_ms: u64 },
    Failed { job_id: Uuid, error: String, traceback: Option<String> },
    Cancelled { job_id: Uuid },
}
```

**Change from v3:** `Ready` adds `capabilities_source`. This is not optional
metadata — the scheduler's `422 device_does_not_support_fp8` check (§11.7) and any
operator-facing diagnostics must be able to tell whether a capability flag came from
a real torch probe or from mock mode, given that mock mode is now a permanent,
equally-tested mode rather than a stand-in for an unbuilt real path (§10.6).

### 8.7 Serialisation

All messages are msgpack via `rmp-serde`. The `_type` discriminator is a flat dict
key, not nested. Python: `msgpack.unpackb(data, raw=False)`. Rust:
`rmp_serde::to_vec_named` / `rmp_serde::from_slice`, flat-dict deserialiser.

No custom length-prefix framing — ZeroMQ's message layer handles framing natively.
One `send()` call is exactly one logical message.

---

## 9. Worker Lifecycle (`anvilml-worker`)

### 9.1 The `ManagedWorker` Ownership Conflict — Resolved By Design, Not By Convention

v3's `ManagedWorker` had `run(self)` and `shutdown(self)` methods that each consumed
the struct by value, while `WorkerPool` needed to hold every worker behind an `Arc`
to share status across a polling task and an API handler. **An `Arc`-wrapped value
can never have a by-value method called on it.** This meant `run()` was never
actually invoked anywhere in the pool — a regression that went undetected because
nothing forced a compile-time check that it was called. The eventual fix folded
`shutdown()`'s logic into `run()`'s own `tokio::select!` loop, triggered by a
per-worker `oneshot` channel the pool holds and fires, and replaced
`Arc<ManagedWorker>` with a `WorkerHandle` struct that does not require consuming the
worker to interact with it.

**v4 specifies this resolved shape directly, so no future task re-derives it under
pressure:**

```rust
/// Held by WorkerPool. Does NOT wrap ManagedWorker in Arc.
/// Cloning a WorkerHandle is cheap — it shares the status lock and the
/// shutdown-trigger sender, not the worker itself.
#[derive(Clone)]
pub struct WorkerHandle {
    pub worker_id: String,
    /// Shared, independently lockable — readable from a status-polling task
    /// or an API handler without touching the running worker task at all.
    status: Arc<RwLock<WorkerStatus>>,
    /// Fired exactly once to request graceful shutdown. Consumed by run()'s
    /// own select! loop — there is no separate externally-callable shutdown()
    /// method that competes with run() for ownership of self.
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    /// Awaited by WorkerPool::shutdown_all() with a bounded timeout (§9.5).
    join_handle: tokio::task::JoinHandle<()>,
}

/// Spawned once per worker by WorkerPool::spawn_all(). Takes ownership of
/// itself for the duration of its own task — this is fine, because nothing
/// outside this function needs to call a method ON the ManagedWorker struct
/// directly; everything external goes through the WorkerHandle's status lock
/// and shutdown_tx instead.
impl ManagedWorker {
    pub async fn run(mut self, mut shutdown_rx: tokio::sync::oneshot::Receiver<()>) {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    // Graceful shutdown path — was the separate shutdown(self)
                    // method in v3; folded in here so run() owns the entire
                    // lifecycle and nothing else needs by-value access.
                    self.send_shutdown_and_wait().await;
                    break;
                }
                event = self.next_event() => { /* dispatch */ }
            }
        }
    }
}
```

**The rule this generalizes to:** if a struct needs to be shared across multiple
independent readers/tasks (status polling, API handlers, the worker's own run loop),
do not wrap the whole struct in `Arc`. Split it into (a) a cheap, `Clone`-able handle
holding only the shared, lockable state and a trigger channel, and (b) the owning
task that holds the real struct by value for its own lifetime. Re-deriving this split
from scratch under task pressure is exactly what produced the `run()`-never-called
regression — use the shape above directly.

### 9.2 Design Principles

- One `ManagedWorker` owns exactly one Python subprocess for the lifetime of its
  `run()` task.
- `WorkerPool` owns `Vec<WorkerHandle>` and the shared `Arc<RouterTransport>`.
- A crashed worker is automatically respawned after a configurable delay (default
  2 seconds).
- A worker that fails to reach `Idle` within 60 seconds is killed and respawned.
- Keepalive pings every 30 seconds; no pong within 10 seconds → declared dead.

### 9.3 Module Layout

```
anvilml-worker/src/
├── lib.rs          # re-exports WorkerPool, WorkerHandle, ManagedWorker; ≤ 80 lines
├── pool.rs         # WorkerPool: Vec<WorkerHandle>, spawn_all(), shutdown_all()
├── managed.rs      # ManagedWorker: owns the run() loop — see §9.1
├── spawn.rs        # Subprocess Command construction + env injection
├── bridge.rs       # Two independent reader/writer tasks against RouterTransport's
│                   #   already-split send/recv halves (§8.3) — bridge.rs does not
│                   #   introduce its own additional lock around either half.
├── keepalive.rs    # Ping/Pong heartbeat + timeout watchdog
├── demux.rs        # Demultiplexes incoming WorkerEvents by job_id/type.
│                   #   MUST expose deregister(worker_id) — see §9.4.
├── env.rs          # WorkerEnv: builds environment variable map for subprocess
├── job_object.rs   # Windows Job Object orphan-cleanup wrapper (Windows only)
└── respawn.rs      # RespawnPolicy: backoff logic for repeated crashes
```

```
anvilml-worker/tests/
├── pool_tests.rs
├── managed_tests.rs
├── demux_tests.rs       # MUST include a deregistration test — see §9.4
├── env_tests.rs
└── respawn_tests.rs
```

### 9.4 Demux Deregistration (mandatory; v3 shipped without it)

`demux.rs`'s routing table maps `worker_id → channel` so incoming `WorkerEvent`s
reach the right consumer. v3 shipped `register()` with no matching `deregister()`,
so every crash + respawn cycle added a new entry without ever removing the stale
one — the table only ever grew for the life of the process.

**v4 requires `deregister(worker_id: &str)` to exist from the same task that adds
`register()`, and `ManagedWorker::run()` must call it on every exit path** (graceful
shutdown, crash, and the 60-second Initializing timeout) — not only on the graceful
path. A task implementing demux routing without a paired, called deregistration path
is incomplete.

### 9.5 `ManagedWorker` State Machine

```
          spawn()
             │
             ▼
       ┌─────────────┐
       │ Initializing │──── timeout (60s) ────► Dead (demux.deregister called)
       └─────────────┘
             │
         Ready event
             │
             ▼
          ┌──────┐ ◄──── Pong received ─── keepalive
          │ Idle │
          └──────┘
             │
         dispatch()
             │
             ▼
          ┌──────┐
          │ Busy │──── Completed / Failed / Cancelled event ──► Idle
          └──────┘
             │
          crash / Dying / pong timeout
             │
             ▼
          ┌──────┐
          │ Dead │ (demux.deregister called)
          └──────┘
             │
         respawn delay
             │
             ▼
       ┌─────────────┐
       │ Respawning  │──── spawn() ──► Initializing (demux.register called again)
       └─────────────┘
```

### 9.6 IPC Bridge Task

Two separate tokio tasks, each locking only its own half of the already-split
`RouterTransport` (§8.3) — there is no combined `select!` loop and no socket-clone
problem, because nothing here needs to clone a socket:

```rust
// writer_task: drains the mpsc channel, sends via RouterTransport::send()
tokio::spawn(async move {
    while let Some(msg) = rx.recv().await {
        transport.send(&worker_id, &msg).await?;
    }
});

// reader_task: pumps RouterTransport::recv(), routes via demux
tokio::spawn(async move {
    loop {
        let (id, event) = transport.recv().await?;
        demux.route(&id, event).await?;
    }
});
```

### 9.7 Environment Variables Injected Into Worker

| Variable | Value |
|:---------|:------|
| `ANVILML_IPC_PORT` | TCP port of the ROUTER socket (u16 decimal) |
| `ANVILML_WORKER_ID` | Bare device index as a decimal string (e.g. `"0"`) — also the ZMQ DEALER identity |
| `ANVILML_DEVICE_INDEX` | GPU device index (u32 decimal) |
| `ANVILML_DEVICE_TYPE` | `"cuda"`, `"rocm"`, or `"cpu"` |
| `ANVILML_WORKER_MOCK` | `"1"` if the `mock-hardware` cargo feature is active, else unset |
| `ANVILML_FORCE_WORKER_MOCK` | Runtime override — forces `ANVILML_WORKER_MOCK="1"` regardless of compiled features |
| `ANVILML_LOG_LEVEL` | Inherited from server config |
| `ANVILML_MAX_IPC_PAYLOAD_MIB` | Maximum IPC message size in MiB |

---

## 10. Generic Node System

### 10.1 Design Philosophy

There is no `ZitLoadPipeline`, `Flux2KleinSampler`, or any architecture-prefixed node
type. v4 keeps v3's generic, architecture-agnostic node design: `LoadModel` returns
`MODEL`; `ClipTextEncode` accepts `CLIP`; `Sampler` accepts
`MODEL + CONDITIONING + LATENT`. The graph wires generic nodes together; each node
dispatches internally to an architecture-specific module based on the loaded model's
reported architecture string.

```
LoadModel(model_id=...) ──────────────────────────────► MODEL
LoadVae(model_id=...) ────────────────────────────────► VAE
LoadClip(model_id=...) ───────────────────────────────► CLIP
ClipTextEncode(clip=CLIP, positive_text=..., negative_text=...) ──► CONDITIONING
EmptyLatent(width=1024, height=1024) ────────────────► LATENT
Sampler(model=MODEL, conditioning=CONDITIONING,
        latent=LATENT, steps=4, cfg=1.0) ────────────► LATENT, SEED
VaeDecode(vae=VAE, latent=LATENT) ───────────────────► IMAGE
SaveImage(image=IMAGE, seed=SEED)
```

`VAE` is always an explicit graph dependency from `LoadVae` into `VaeDecode`. The MVP
targets standalone safetensors files for every component — diffusion model, text
encoder, VAE are each a separate file, each loaded by its own loader node. No
all-in-one checkpoint files are in scope.

### 10.2 Node Registration

Every Python node class is decorated with `@register`, adding it to `NODE_REGISTRY`
(`type_name → class`). On worker startup, the worker serialises `NODE_REGISTRY` into
`Vec<NodeTypeDescriptor>` and includes it in the `Ready` event. The Rust scheduler
populates its dynamic `NodeTypeRegistry` from this — there is no compile-time node
type list anywhere. On respawn, the worker re-reports its node types.

### 10.3 Baseline Node Types (MVP)

| Node Type | Category | Inputs | Outputs | Notes |
|:----------|:---------|:-------|:--------|:------|
| `LoadModel` | Loaders | `model_id: String` | `model: Model` | Loads a diffusion model from a safetensors file via raw shape-inferred construction (§11). Outputs `MODEL` only. |
| `LoadVae` | Loaders | `model_id: String` | `vae: Vae` | Loads a VAE from a standalone safetensors file via raw shape-inferred construction (§11). Always an explicit graph dependency. |
| `LoadClip` | Loaders | `model_id: String, clip_type: String?` | `clip: Clip` | Loads a Qwen3 text encoder from a safetensors file via raw shape-inferred construction (§11). `clip_type` hint: `"qwen3"` (only value in MVP scope). |
| `ClipTextEncode` | Conditioning | `clip: Clip, positive_text: String, negative_text: String?` | `conditioning: Conditioning` | Encodes a text prompt using any loaded CLIP-compatible encoder. Architecture-agnostic. |
| `EmptyLatent` | Latents | `width: Int, height: Int, batch_size: Int?, model: Model?` | `latent: Latent` | Creates a blank noise latent tensor. The optional `model` input is required in real mode: dispatches to the loaded model's arch module's `compute_latent_shape()` — the shape *formula*, not just a scale factor, is architecture-specific. Mock mode ignores this input. |
| `Sampler` | Sampling | `model: Model, conditioning: Conditioning, clip: Clip, latent: Latent, steps: Int, cfg: Float, seed: Int` | `latent: Latent, seed: Int` | Dispatches internally to the matched arch module. Returns the denoised latent and the actual seed used (`-1` resolves to a random seed). |
| `VaeDecode` | Decoding | `vae: Vae, latent: Latent` | `image: Image` | Decodes a denoised latent to a PIL image using the explicitly provided VAE. |
| `ImageResize` | Images | `image: Image, width: Int, height: Int, method: String?` | `image: Image` | Resizes a PIL image. `method` defaults to `"lanczos"`. |
| `SaveImage` | Output | `image: Image, seed: Int?, steps: Int?` | *(none)* | Encodes to PNG, writes to artifact store, emits `ImageReady`. |

### 10.4 Architecture Dispatch — Three Parallel Families

v4 has three dispatch families instead of v3's two, because VAE loading is now its
own arch family rather than living inside the diffusion arch module (§11.4 explains
why):

```
worker/nodes/arch/
├── diffusion/   # zit.py, flux2klein.py — diffusion transformer load + sample
├── clip/        # qwen3.py — text encoder load
└── vae/         # zit_vae.py, flux2_vae.py — VAE load + decode
```

Each family follows the same dispatch contract shape, auto-imported via
`pkgutil.iter_modules()`:

```python
def can_handle(key: Any) -> bool:
    """Return True if this module handles the given dispatch key.
    diffusion/vae: key is an arch string read from safetensors metadata or a
    path-derived fallback. clip: key is the clip_type string."""

def get_module(key: Any) -> ModuleType | None:
    """Return the matching module for `key`, or None. Shared scan logic across
    all three families — do not write three separate iteration implementations."""
```

`LoadModel`, `LoadVae`, and `LoadClip` each call their family's `get_module(key)`,
then call `.load(model_id, caps, device)` on the result (§11 specifies this load
contract in full — the dispatch mechanism above only routes to the right module, it
does not specify how that module constructs the model).

`Sampler` calls `diffusion.get_module(model.arch).sample(...)`. `VaeDecode` calls
`vae.get_module(vae_obj.arch).decode(...)`. `EmptyLatent` calls
`diffusion.get_module(model.arch).compute_latent_shape(...)`.

**Method names are fixed and identical across every arch module in every family —
this is a hard naming contract, not an emergent convention.** v3 had `load_model()`
in one arch module and a bare `load()` in another, plus other ad-hoc name variants
across files; an agent reading one file as a template for the next then propagated
whichever name it last saw, compounding the inconsistency. v4 closes this by stating
the exact required signature once, here, for every arch module regardless of family:

| Method | Required in | Signature | Never named instead |
|:-------|:-------------|:----------|:----------------------|
| `can_handle(key)` | diffusion, clip, vae | `(key: Any) -> bool` | `matches()`, `supports()`, `is_arch()` |
| `get_module(key)` | diffusion, clip, vae (shared dispatcher, not per-module) | `(key: Any) -> ModuleType \| None` | n/a — one shared implementation, not reimplemented per module |
| `load(model_id, caps, device)` | diffusion, clip, vae | `(model_id: str, caps: dict, device: str) -> Any` | `load_model()`, `load_vae()`, `load_clip()`, `load_transformer()`, or any other family-prefixed variant |
| `sample(...)` | diffusion only | per §11.6 (pipeline-level call) | `generate()`, `run()`, `denoise()` |
| `decode(...)` | vae only | per §11.6 | `vae_decode()`, `to_image()` |
| `compute_latent_shape(...)` | diffusion only | per §10.3's `EmptyLatent` row | `latent_shape()`, `get_latent_dims()` |

Every arch module — `zit.py`, `flux2klein.py`, `qwen3.py`, `zit_vae.py`,
`flux2_vae.py`, and any added later — exposes `load()` with this exact name, this
exact parameter order, regardless of which family it belongs to. A task introducing
a new arch module copies this table's left column verbatim; it does not invent a
family-specific variant for "clarity," and it does not rename an existing module's
method to something it considers clearer without updating every caller and this
table in the same task.

### 10.5 Tokenizer Assets

Qwen3's tokenizer is vendored locally under `worker/assets/qwen3_tokenizer/` —
committed to git, never downloaded from a model hub at worker runtime. This is what
keeps the worker fully offline-capable. `worker/tools/seed_tokenizers.sh`/`.ps1`
re-seed this directory from a stated upstream source and record the provenance
reasoning for that source choice.

### 10.6 Mock/Real Parity — Mandatory Marker, Mechanically Checked

This is new in v4 and exists specifically because of two compounding v3 findings:
(a) mock mode and real mode silently diverged for an entire phase because nothing
forced them to be checked together, and (b) an agent can produce a convincing prose
justification for skipping real work that is false but goes unchallenged (the
`anvilml-artifacts` relocation incident, §3.2). Prose claims of "mock and real are
equivalent" are not trustworthy on their own; this section makes that claim
mechanically checkable.

**The rule:** every node's `execute()` method, and every arch module's `load()` /
`sample()` / `decode()` function, carries a structural pair of test IDs, declared as
a module-level constant next to the function:

```python
# worker/nodes/sampler.py
class Sampler(BaseNode):
    # REAL_PATH_VERIFIED: worker/tests/test_sampler.py::test_sample_real_zit_fixture
    # MOCK_PATH_VERIFIED: worker/tests/test_sampler.py::test_sample_mock_returns_sentinel
    NODE_TYPE = "Sampler"
    ...
    def execute(self, ctx: NodeContext, **inputs) -> dict:
        ...
```

**Validation, mechanical, run by the §9a.1-equivalent sweep (see
`FORGE_AGENT_RULES.md` for the exact enforcement procedure once it is updated to
reference this marker):**

1. `grep -rn "REAL_PATH_VERIFIED:" worker/nodes/` — every match must name a test
   function that actually exists (`pytest --collect-only` resolves it) and that test
   must not be skipped, marked `xfail`, or gated behind `ANVILML_WORKER_MOCK=1`.
2. `grep -rn "MOCK_PATH_VERIFIED:" worker/nodes/` — same check, but the named test
   must run **with** `ANVILML_WORKER_MOCK=1`.
3. Every public `execute()`/`load()`/`sample()`/`decode()` function that lacks **both**
   markers is a finding, treated with the same severity as an unmarked
   `NotImplementedError`/`TODO` under `FORGE_AGENT_RULES.md §9a.1` — this is the same
   sweep, extended to also catch "real path silently absent with no stub to grep for,"
   which is exactly how P904's mock-only gate survived four phases undetected: there
   was no `TODO` to find because the missing branch was not a stub, it was an entire
   code path that was never written and never had to admit it was missing.
4. A task that adds or modifies a node's `execute()` (or an arch module's load/sample/
   decode) without adding or updating both markers, and the tests they name, is
   incomplete. This applies even when the task's stated scope only mentions one mode
   — if a task changes real-path behavior, it must also confirm (and update the
   marker for) the corresponding mock-path test, and vice versa, because a marker
   pointing at a stale test is worse than no marker: it is a false mechanical
   guarantee, which is precisely the P902 failure mode (a checkable claim that turns
   out false) applied to test coverage instead of architecture.

This marker is additive to, not a replacement for, the normal test-writing
obligations in `FORGE_AGENT_RULES.md §5` and `docs/ENVIRONMENT.md §11`.

---

## 11. Model Loading Contract

### 11.1 What Changed From v3, And Why

v3 loaded models by constructing a `diffusers`/`transformers` model class from a
hardcoded local config, then remapping checkpoint keys using `diffusers`' own
internal `convert_*_checkpoint_to_diffusers` functions
(e.g. `convert_z_image_transformer_checkpoint_to_diffusers`). This avoided
`from_single_file()`'s network-touching `fetch_diffusers_config()` fallback, but it
still depended on `diffusers`-internal conversion functions that are not a public,
versioned API — they can change shape or move between `diffusers` releases without
notice, and tying correctness to them was the kind of unstated, undocumented
assumption this rewrite is meant to remove (the handoff calls this the "external
proposal verification" risk: an unverified claim about library internals very nearly
produced a silently-wrong model in P904).

**v4 replaces this with full ComfyUI-style raw construction**: read tensor shapes
directly out of the safetensors header, infer the architecture's hyperparameters from
those shapes, construct the target `nn.Module` directly (still using `diffusers`'/
`transformers`' own layer classes for the actual tensor math — attention, conv,
normalization — see §11.2), and remap checkpoint key names to the constructed
module's own `state_dict()` key names by hand. No `convert_*_checkpoint_to_diffusers`
call. No `from_single_file()`. No `from_pretrained()`. No network access, ever,
regardless of `local_files_only`.

### 11.2 Library Boundary (do not blur this — it was explicitly decided)

| Allowed | Not allowed in the load path |
|:--------|:------------------------------|
| `diffusers`'/`transformers`' layer/block classes used as building blocks (attention modules, normalization, conv layers, scheduler classes) | `diffusers.DiffusionPipeline.from_pretrained()` / `from_single_file()` |
| `safetensors.torch.load_file()` / `safetensors.safe_open()` for reading tensors and headers | `transformers.AutoModel.from_pretrained()` or any hub-aware loader |
| `torch.nn.Module` constructed directly by the arch module, with config values computed from inferred shapes | Any `diffusers`-internal `convert_*_checkpoint_to_diffusers` function |
| Tokenizer classes from `transformers`, loaded from the vendored local asset directory (§10.5) | Any function that can fall back to a Hugging Face Hub lookup, even one gated by a flag |

The distinction: AnvilML does not reimplement attention math or normalization layers
— that would be reinventing `diffusers`/`transformers` for no benefit. It does
reimplement the **loading mechanism** — deciding what shape the model is and getting
weights into it — which is the part that conflicts with offline-only operation and
the part that drove every loading bug in P904.

### 11.3 The Loading Contract, Per Arch Module

Every diffusion (`arch/diffusion/*.py`), CLIP (`arch/clip/*.py`), and VAE
(`arch/vae/*.py`) module implements the same four-step contract in its `load()`
function. This section states the contract and the reason for each step; the exact
algorithm (which keys to inspect, what the remap table looks like for a specific
checkpoint) is worked out per architecture at TASKS_PHASE/implementation time — it is
not specified here, because it differs per model family and belongs with the task
that implements that family.

1. **Open the safetensors file header only** (`safe_open(..., framework="pt")`,
   reading `.keys()` and `.get_slice(key).get_shape()` — not loading tensor data
   yet). Infer every architecture hyperparameter the constructor needs (channel
   counts, layer counts, head dimensions, patch size, etc.) from these shapes. Read
   **every** key, not a truncated sample — P904's first shape-inference attempt used
   `list(f.keys())[:30]` and silently missed two of three layer stacks; a partial key
   scan that looks complete is the standing risk this step exists to prevent.
2. **Construct the target `nn.Module` on `torch.device("meta")`**, with the dtype
   chosen per §11.5 (never hardcoded), using only the shape-inferred hyperparameters
   from step 1 — no `config.json`, local or remote. Meta-device construction means no
   real memory is allocated for parameters yet — this is the step that fixed P904's
   ~15GB-on-construction crash.
3. **Materialize via `to_empty()`, then remap and load.** Build the checkpoint-key
   → constructed-module-key mapping by hand (informed by inspecting both key sets
   directly — never assumed from a prior model version or another architecture's
   mapping). Load with `load_state_dict(..., assign=True)`. Tensors must already be
   cast to the target dtype **before** this call — `assign=True` bypasses dtype
   coercion, so casting after the call does not work; this exact ordering mistake is
   what caused P904's dtype-safety incident.
4. **Return the constructed module** with an `.arch` attribute set to the
   architecture string this module's `can_handle()` matches, so later dispatch
   (`Sampler`, `VaeDecode`, `EmptyLatent`) can route correctly without re-deriving
   the architecture.

A `load()` implementation that skips step 1 and hardcodes shapes for "the" production
checkpoint is non-compliant even if it happens to work for that one file — the whole
point of this contract is that it tolerates a differently-shaped checkpoint (a test
fixture, a future model variant) without an `AssertionError` or a silent wrong
construction. This is the exact defect class `axes_dims` hardcoding produced in P904.

### 11.4 Why VAE Is Its Own Arch Family In v4 (not nested under diffusion)

v3 placed `load_vae()` inside the diffusion arch module (`arch/diffusion/zit.py`
owned both the transformer and its VAE). v4 splits VAE into `arch/vae/` because:
- A VAE's shape-inference and key-remap logic is independent of the diffusion
  transformer's — they are different `nn.Module` families with no shared
  hyperparameters, so nesting them saved no code.
- ZiT and Flux 2 Klein (4B and 9B) use only **two** distinct VAE architectures
  between three diffusion models — `vae/` modules are reused across diffusion arch
  modules, which a nested layout could not express without duplication.
- `LoadVae` dispatching directly to `vae.get_module(arch)` (§10.4) mirrors
  `LoadModel`'s and `LoadClip`'s own dispatch shape exactly, removing the asymmetry
  v3 had between CLIP's string-keyed dispatch and diffusion's object-keyed dispatch
  for VAE specifically.

### 11.5 Dtype Selection (depends on §6.6 — read that section first)

`load()` never hardcodes a compute dtype. The caller (`LoadModel`/`LoadVae`/
`LoadClip`) passes the worker's own probed `InferenceCaps` (§6.6) into `load()` via
`NodeContext`. The arch module picks the dtype by this fixed precedence, the same for
every architecture:

1. If `caps.fp8` is `True` **and** the checkpoint's native dtype is FP8 → load and
   compute at FP8 directly (no upcast).
2. Else if `caps.bf16` is `True` → upcast to `bfloat16`.
3. Else if `caps.fp16` is `True` → upcast to `float16`.
4. Else → `float32` (always supported; the universal fallback).

On torch CPU today, step 1 always fails (`fp8 = False` from the real probe — §6.6)
and step 2 succeeds, landing on `bfloat16` — this is why CPU real-mode tests always
observe bf16 construction regardless of the checkpoint's native FP8 dtype, and that is
correct, not a workaround to special-case away.

### 11.6 Component vs. Pipeline Caching

`LoadModel`, `LoadVae`, `LoadClip` each cache their own raw component (transformer,
VAE, text encoder) via `pipeline_cache.get_or_load(model_id, ...)`. None of the three
loader nodes call a `diffusers`/`transformers` *pipeline* class directly — that
responsibility belongs entirely to the diffusion arch module's `sample()` function,
which on its first call for a given `model_id` assembles the runnable pipeline object
from the already-cached components and caches that assembled pipeline itself under
`f"{model_id}:pipeline"`. Subsequent `sample()` calls reuse it. This keeps
`LoadModel`/`LoadVae`/`LoadClip` decoupled from any specific pipeline class.

### 11.7 FP8 Capability Gate At Dispatch Time

If the scheduler dispatches a job whose graph references an FP8 checkpoint to a
worker whose `InferenceCaps.fp8` is `False` (per the worker's own real probe, not a
device-table hint), the scheduler returns `422 device_does_not_support_fp8` at
dispatch time — never inside the worker after a partially-loaded model has already
allocated memory.

---

## 12. Job Scheduler (`anvilml-scheduler`)

### 12.1 Module Layout

```
anvilml-scheduler/src/
├── lib.rs          # re-exports JobScheduler and public types; ≤ 80 lines
├── scheduler.rs    # JobScheduler: owns queue, ledger; dispatch loop
├── queue.rs        # JobQueue: FIFO with O(1) cancel; sorted by priority+created_at
├── ledger.rs       # VramLedger: per-device VRAM accounting
├── dag.rs          # GraphValidator: validate_graph() — collect-all-errors mode
├── types.rs        # ValidatedGraph newtype; GraphError enum
└── event_loop.rs   # Subscribes to WorkerEvent broadcast; updates job status in DB
```

```
anvilml-scheduler/tests/
├── queue_tests.rs
├── ledger_tests.rs
├── dag_tests.rs
└── node_registry_tests.rs
```

### 12.2 Node Registry (Dynamic)

`NodeTypeRegistry` lives in `anvilml-core` (§5.1) — both the scheduler (graph
validation) and the server (`GET /v1/nodes`) need it, and `anvilml-core` is the
shared dependency both already have without creating a new edge.

```rust
pub struct NodeTypeRegistry {
    types: Arc<RwLock<HashMap<String, NodeTypeDescriptor>>>,
}

impl NodeTypeRegistry {
    pub async fn update_from_worker(&self, types: Vec<NodeTypeDescriptor>);
    pub async fn get(&self, type_name: &str) -> Option<NodeTypeDescriptor>;
    pub async fn all_types(&self) -> Vec<NodeTypeDescriptor>;
}
```

If the registry is empty (no worker has reached `Ready` yet), all job submissions
return `503 workers_unavailable`.

### 12.3 Graph Validation

Non-fail-fast: all errors collected and returned together.

1. Root JSON is an object with a `"nodes"` array.
2. No duplicate node `id` values.
3. Every node `type` exists in `NodeTypeRegistry`.
4. Every edge reference resolves to a node that exists and declares that output slot.
5. Every edge's `slot_type` is compatible with the receiving input (exact match, or
   either side is `SlotType::Any`).
6. The graph is acyclic (Kahn's algorithm).

A graph passing all checks yields a `ValidatedGraph(serde_json::Value)` newtype. Only
a `ValidatedGraph` may be enqueued.

### 12.4 VRAM Ledger

Tracks VRAM reservation per device. Reserved at dispatch, released at
`Completed`/`Failed`/`Cancelled`. Reservations are conservative estimates from model
metadata (if known to the registry) or a configurable default.

**Advisory, not enforced.** A real OOM is still possible mid-execution; the worker
emits `Failed`, the scheduler releases the reservation. The ledger prevents
over-scheduling, not VRAM sufficiency.

### 12.5 Dispatch Loop

A background tokio task wakes on (a) a new job enqueued, or (b) any worker
transitioning to `Idle`. Per wake, iterate the queue front-to-back, dispatch every
job with a suitable idle worker, stop when the queue is exhausted or no idle workers
remain.

Worker selection:
1. If `job.settings.device_preference` matches an `Idle` worker, use it.
2. Else rank `Idle` workers by `vram_free_mib` descending; pick the top candidate.
3. If none `Idle`, leave the job queued.

---

## 13. HTTP & WebSocket Server (`anvilml-server`)

### 13.1 Module Layout

```
anvilml-server/src/
├── lib.rs              # build_router() → axum::Router; AppState construction; ≤ 80 lines
├── state.rs            # AppState struct
└── handlers/
    ├── mod.rs
    ├── health.rs        # GET /health
    ├── system.rs        # GET /v1/system, /v1/system/env, /v1/system/versions
    ├── jobs.rs          # POST/GET/DELETE /v1/jobs*
    ├── models.rs        # GET /v1/models*, POST /v1/models/rescan
    ├── workers.rs       # GET /v1/workers, POST /v1/workers/:id/restart
    ├── artifacts.rs     # GET /v1/artifacts*
    └── nodes.rs         # GET /v1/nodes
```

`AnvilError`'s `IntoResponse` impl lives in `anvilml-core/src/error.rs` (§5.2) — no
separate `error.rs` in `anvilml-server`.

```
anvilml-server/src/ws/
├── mod.rs
├── broadcaster.rs   # EventBroadcaster: tokio::sync::broadcast wrapper
├── handler.rs       # GET /v1/events WebSocket upgrade handler
└── stats_tick.rs    # Background task: emits SystemStats every 5 seconds
```

### 13.2 `AppState`

```rust
#[derive(Clone)]
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
    pub node_registry: Arc<NodeTypeRegistry>,
}
```

### 13.3 Middleware Stack (outermost first)

1. `CorsLayer::permissive()` — local-only use.
2. `TraceLayer` — structured logging via `tracing`.
3. `SetRequestIdLayer` — injects `X-Request-Id` UUID.
4. `CompressionLayer` — gzip/br above a configurable threshold.

### 13.4 REST Routes

| Method | Path | Description | Success |
|:-------|:-----|:------------|:--------|
| GET | `/health` | Liveness probe | 200 `{ status, version, uptime_s }` |
| GET | `/v1/system` | Full hardware snapshot | 200 `HardwareInfo` |
| GET | `/v1/system/env` | Python environment health + provisioning | 200 `EnvReport` |
| GET | `/v1/system/versions` | Per-component version report | 200 `ComponentVersions` |
| POST | `/v1/jobs` | Submit job; validate; enqueue | 202 `{ job_id, queue_position }` |
| GET | `/v1/jobs` | List jobs (`?status=` `?limit=` `?before=`) | 200 `Vec<Job>` |
| GET | `/v1/jobs/:id` | Get one job | 200 `Job` |
| POST | `/v1/jobs/:id/cancel` | Cancel queued or running job | 202 |
| DELETE | `/v1/jobs/:id` | Delete terminal job + its artifacts | 204 |
| DELETE | `/v1/jobs` | Bulk clear (`?status=completed\|failed\|cancelled\|all`) | 200 `{ removed: u32 }` |
| GET | `/v1/models` | List models (`?kind=`) | 200 `Vec<ModelMeta>` |
| GET | `/v1/models/:id` | Get one model | 200 `ModelMeta` |
| POST | `/v1/models/rescan` | Trigger rescan | 202 |
| GET | `/v1/workers` | List workers + status | 200 `Vec<WorkerInfo>` |
| POST | `/v1/workers/:id/restart` | Restart a worker | 202 |
| GET | `/v1/artifacts` | List artifacts (`?job_id=`) | 200 `Vec<ArtifactMeta>` |
| GET | `/v1/artifacts/:hash` | Serve artifact PNG | 200 `image/png` |
| GET | `/v1/nodes` | List registered node types with slot descriptors | 200 `Vec<NodeTypeDescriptor>` |
| GET | `/v1/events` | WebSocket upgrade | 101 Switching Protocols |

### 13.5 Error Response Shape

```json
{
  "error": "invalid_graph",
  "message": "unknown_node_type: FooNode; cycle_detected: n0, n1",
  "request_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

| Code | Meaning |
|:-----|:--------|
| 400 | Bad request (malformed JSON body) |
| 404 | Not found (job, model, worker, artifact) |
| 409 | Conflict (job not cancellable) |
| 422 | Unprocessable (graph validation failure; FP8 capability mismatch — §11.7) |
| 503 | Service unavailable (no workers ready; provisioning in progress) |

### 13.6 WebSocket Event Stream

`GET /v1/events` upgrades to a WebSocket. On connect: (1) subscribe to
`EventBroadcaster`; (2) send pending `SystemStats` immediately; (3) forward all
subsequent `WsEvent`s as JSON text frames. Slow consumers are dropped after the
broadcast buffer (1024 events) overflows; the client must reconnect.

---

## 14. Python Worker Process (`worker/`)

### 14.1 There Is No Mock-Only Gate (this is the single biggest change from v3)

v3's `worker_main.py` had this at the top:

```python
if os.environ.get("ANVILML_WORKER_MOCK") != "1":
    print("... Non-mock mode not yet implemented.", file=sys.stderr)
    sys.exit(1)
```

This meant the real code path **never ran, even once**, for the entire v3 effort —
not a partial implementation, an absent one, hidden behind a flag that was always
set in every test and every CI run. v4 has no equivalent of this gate anywhere, in
any phase, from the first task that creates `worker_main.py` onward. `worker_main.py`
always attempts real startup when `ANVILML_WORKER_MOCK` is unset; mock mode is an
explicit, equally-maintained alternate branch (§14.3), not a placeholder for a
real branch that gets written later. A task that adds a "not yet implemented" exit
gate of this shape, for any reason, at any point, is non-compliant with §10.6 and
`FORGE_AGENT_RULES.md §9.7a` — there is no `defers_to` entry that could legitimately
back deferring the entire worker startup path.

### 14.2 Startup Sequence (real mode)

```
worker_main.py receives env vars from Rust
    │
    ├── ipc.connect(ANVILML_IPC_PORT, ANVILML_WORKER_ID)
    │       sets up zmq.DEALER socket, identity = worker_id
    │
    ├── _import_torch_and_select_device()
    │       imports torch; torch.cuda.set_device(device_index) or rocm equivalent;
    │       "cpu" device_type skips device selection entirely
    │
    ├── capability.probe_capabilities(device_type, device_index)   # §6.6 — REAL probe
    │       never synthetic in this branch
    │
    ├── _import_nodes()
    │       triggers auto-import of all modules in nodes/
    │       builds Vec<NodeTypeDescriptor> from NODE_REGISTRY
    │
    └── ipc.send_event(Ready { ..., capabilities_source: "pytorch", node_types: [...] })
            ── enters message dispatch loop ──►
```

### 14.3 Startup Sequence (mock mode, `ANVILML_WORKER_MOCK=1`)

```
worker_main.py receives env vars from Rust
    │
    ├── ipc.connect(...)            # identical to real mode — IPC is not mocked
    │
    ├── _mock_probe_capabilities()
    │       returns fixed synthetic values, NEVER imports torch
    │
    ├── _import_nodes()             # identical to real mode — node registration
    │                                 does not depend on torch being importable
    │
    └── ipc.send_event(Ready { ..., capabilities_source: "mock", node_types: [...] })
            ── enters message dispatch loop ──►
```

The two sequences differ in exactly one step (capability probing) plus each node's
own `execute()` branching on `ANVILML_WORKER_MOCK` internally (§14.4). IPC connection,
node import, and the dispatch loop itself are identical code, not separately
maintained mock/real copies — this is what makes the §10.6 parity marker meaningful:
there genuinely is one real branch and one mock branch per node, not an entire
parallel worker implementation.

### 14.4 `ipc.py`

```python
"""ZeroMQ DEALER transport for AnvilML worker IPC.

The Rust supervisor binds a ROUTER socket. This worker connects a DEALER socket
with a stable identity equal to ANVILML_WORKER_ID. Identity frames are handled
automatically by ZeroMQ; application code sends/receives plain msgpack dicts.
"""

import zmq
import msgpack

_ctx: zmq.Context | None = None
_sock: zmq.Socket | None = None


def connect(port: int, worker_id: str) -> None:
    """Connect DEALER socket to the ROUTER at *port*, using *worker_id* as identity.

    Must be called exactly once before any send/recv operation.

    Args:
        port: TCP port on 127.0.0.1 where the Rust ROUTER is bound.
        worker_id: Stable worker identity string — the bare device index as
            injected via ANVILML_WORKER_ID in production (e.g. "0").
    """
    global _ctx, _sock
    _ctx = zmq.Context.instance()
    _sock = _ctx.socket(zmq.DEALER)
    _sock.setsockopt(zmq.IDENTITY, worker_id.encode())
    _sock.connect(f"tcp://127.0.0.1:{port}")


def send_event(data: dict) -> None:
    """Send a WorkerEvent dict to the Rust supervisor.

    Args:
        data: Dict with '_type' key and event fields.

    Raises:
        RuntimeError: If connect() has not been called.
    """
    if _sock is None:
        raise RuntimeError("ipc: not connected — call connect() first")
    _sock.send(msgpack.packb(data, use_bin_type=True))


def recv_message() -> dict:
    """Receive the next WorkerMessage from the Rust supervisor. Blocks until a
    message arrives.

    Returns:
        Dict with '_type' key and message fields.

    Raises:
        RuntimeError: If connect() has not been called.
    """
    if _sock is None:
        raise RuntimeError("ipc: not connected — call connect() first")
    data = _sock.recv()
    return msgpack.unpackb(data, raw=False)
```

### 14.5 Node Base Class

```python
"""Base node ABC and registration decorator."""

from __future__ import annotations
from abc import ABC, abstractmethod
from typing import Any
from dataclasses import dataclass

NODE_REGISTRY: dict[str, type["BaseNode"]] = {}


def register(cls: type) -> type:
    """Register a node class in NODE_REGISTRY.

    The class must define NODE_TYPE (str), CATEGORY (str), DISPLAY_NAME (str),
    DESCRIPTION (str), INPUT_SLOTS (list[SlotSpec]), and OUTPUT_SLOTS (list[SlotSpec]).

    Raises:
        TypeError: If any required attribute is missing.
    """
    required = ("NODE_TYPE", "CATEGORY", "DISPLAY_NAME", "DESCRIPTION",
                "INPUT_SLOTS", "OUTPUT_SLOTS")
    for attr in required:
        if not hasattr(cls, attr):
            raise TypeError(f"@register: {cls.__name__} missing {attr}")
    NODE_REGISTRY[cls.NODE_TYPE] = cls
    return cls


@dataclass
class SlotSpec:
    """Declares one input or output slot on a node."""
    name: str
    slot_type: str          # Must match a SlotType value (e.g. "MODEL", "CLIP")
    optional: bool = False


class NodeContext:
    """Runtime context passed to every node's execute() method.

    Attributes:
        job_id: The UUID string of the currently executing job.
        device: The torch device string (e.g. "cuda:0", "cpu"). Unused in mock mode.
        caps: The worker's own InferenceCaps dict from capability.probe_capabilities()
            (or the mock equivalent). Arch modules read dtype decisions from this —
            never from a Rust-side hint — per §6.6/§11.5.
        cancel_flag: threading.Event; set when the job is cancelled.
        emit: Callable for emitting WorkerEvent dicts back to the supervisor.
        pipeline_cache: The shared LRU model/pipeline cache.
        mock: bool — True if ANVILML_WORKER_MOCK=1. Nodes branch on this exactly
            once, at the top of execute(), never deeper inside arch dispatch.
    """
    def __init__(self, job_id, device, caps, cancel_flag, emit, pipeline_cache, mock):
        self.job_id = job_id
        self.device = device
        self.caps = caps
        self.cancel_flag = cancel_flag
        self.emit = emit
        self.pipeline_cache = pipeline_cache
        self.mock = mock
```

### 14.6 Mock Mode Node Behavior

When `ctx.mock` is `True`:
- Every node's `execute()` branches at the top: a fast sentinel path returns
  placeholder outputs without running any model code.
- `SaveImage` emits `ImageReady` with a 64×64 black PNG.
- `ANVILML_MOCK_NODE_DELAY_MS` introduces artificial latency in mock execute paths
  (used to test cancellation/timeout behavior).

This branch is not a stand-in for an unwritten real branch — both branches are
written, tested, and marked per §10.6 from the same task that introduces the node.

---

## 15. Configuration

Layered precedence, lowest to highest: compiled-in defaults → `anvilml.toml` →
environment variables (`ANVILML_*`, nested fields via `__`) → CLI flags. Full field
reference lives in `docs/ENVIRONMENT.md §3–§4`, generated and kept in sync with
`anvilml-core/src/config.rs` by the config-drift CI gate (§18.5).

Key fields relevant to this design (non-exhaustive — see ENVIRONMENT.md for the rest):

| Field | Default | Notes |
|:------|:--------|:------|
| `host` | `127.0.0.1` | Bind address |
| `port` | `8488` | HTTP port |
| `db_path` | `./anvilml.db` | SQLite database path |
| `artifact_dir` | `./artifacts` | Generated image storage |
| `model_scan_depth` | `2` | Non-recursive scanner depth (§7.4) |
| `venv_path` | `./worker/.venv` | Python venv root |

---

## 16. Logging Standards

### 16.1 Level Assignment

| Level | Use |
|:------|:----|
| `ERROR` | Unrecoverable failures causing a subsystem to abort. Always include `error=`. |
| `WARN` | Recoverable anomalies. Include `error=` only when it adds information. |
| `INFO` | Operational lifecycle: startup, shutdown, worker spawn/ready/dead, model scan complete, job dispatch. Always visible at default log level. |
| `DEBUG` | Internal state: message sent/received, IPC frame, job state transition, VRAM reservation, capability probe result (§6.6). |
| `TRACE` | Per-iteration detail. Use sparingly. |

### 16.2 Mandatory INFO Events

| Event | Required fields |
|:------|:----------------|
| Server bind | `addr=%addr` |
| Graceful shutdown initiated | `reason=%reason` |
| Worker spawned | `worker_id=%id, device_index=%idx, pid=%pid` |
| Worker Ready | `worker_id=%id, device=%name, torch_version=%ver, fp8=%bool, capabilities_source=%src` |
| Worker Dead | `worker_id=%id, exit_code=%code` |
| Worker Respawning | `worker_id=%id, delay_ms=%ms` |
| Job dispatched | `job_id=%id, worker_id=%wid` |
| Job completed | `job_id=%id, elapsed_ms=%ms` |
| Job failed | `job_id=%id, error=%err` |
| Model scan complete | `count=%n, dir=%dir` |
| Hardware detection result | `device=%name, vram_mib=%n, type=%t` per GPU |
| Capability probe result (real mode) | `worker_id=%id, fp8=%bool, bf16=%bool, fp16=%bool` |

### 16.3 Mandatory DEBUG Events

| Event | Required fields |
|:------|:----------------|
| IPC message sent | `worker_id=%id, msg_type=%t` |
| IPC event received | `worker_id=%id, event_type=%t` |
| Job state transition | `job_id=%id, from=%old, to=%new` |
| VRAM reservation | `device=%idx, reserved_mib=%n, free_mib=%n` |
| Hardware detection fallback | `fallback=%method` |
| Node registry update | `worker_id=%id, node_count=%n` |
| Dtype selected for load | `model_id=%id, dtype=%dtype, reason=%caps_field` |

### 16.4 Structured Logging Format

```rust
// Correct:
tracing::info!(worker_id = %worker_id, device = %device_name, "worker ready");
// Wrong:
tracing::info!("worker {} is ready on {}", worker_id, device_name);
```

Python workers use `logging` at the same levels, structured fields as keyword args
where the handler supports it. Python worker logs forward via stdout to the Rust
`tracing` subscriber.

---

## 17. Testing Strategy

### 17.1 Test Catalogue

`docs/TESTS.md` is mandatory, maintained in parallel with the codebase. One entry per
test:

```markdown
## test_name (crate or module)

**File:** `path/to/test_file.rs` or `worker/tests/test_file.py`
**Context:** What system state or precondition this test requires.
**Tests:** What behaviour or invariant is being verified.
**Mode:** mock | real | both (real-mode tests use a fixture checkpoint — name it)
**Inputs:** What data or configuration is used.
**Expected output:** What the test asserts.
```

The `Mode` field is new in v4 — given §10.6's parity-marker requirement, every entry
covering a node or arch-module function states which mode it exercises, so
`docs/TESTS.md` and the markers stay independently cross-checkable.

### 17.2 Test Categories

| Category | Location | Description |
|:---------|:---------|:------------|
| Rust unit tests | `crates/*/tests/*.rs` | Single function/struct, no I/O, no subprocess, no network |
| Rust integration tests | `backend/tests/*.rs` | Running server with mock hardware; in-process `axum::serve` |
| IPC stress test | `anvilml-ipc/tests/stress_test.rs` | 1000-round-trip ROUTER/DEALER under mock-hardware — **gates Phase IPC-baseline**: no subsequent phase begins until this passes (§20) |
| Python mock-mode tests | `worker/tests/test_*.py`, `ANVILML_WORKER_MOCK=1` | No torch import; sentinel outputs |
| Python real-mode tests | `worker/tests/test_*.py`, `ANVILML_WORKER_MOCK` unset | Real torch CPU + tiny fixture checkpoints (§17.5). Never the production-size model (§2.2). |
| End-to-end smoke | `docs/PROOF_phase*.md` | Manual runnable proof; not automated; required per phase |

### 17.3 Mock AND Real Are Both Mandatory, Every Phase

This is the structural answer to v3's single largest gap. Stated as an explicit rule
because "test it" alone was not sufficient in v3 — the gate has to name what "it"
means:

- A phase that adds or modifies a node, an arch module, or any worker startup
  behavior is not complete until **both** a mock-mode test and a real-mode test
  (against a fixture, §17.5) exist and pass for that change, **and** both are named
  in that function's `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` markers (§10.6).
- Mock-mode and real-mode tests may be separate tasks within the same phase. They
  must not be separate phases — a phase whose mock-mode tasks are complete and whose
  real-mode tasks are not yet started, or vice versa, is not complete, and the
  phase's closing/integration task must block on this exactly as it blocks on the
  `defers_to` audit (`FORGE_AGENT_RULES.md §9a`).
- There is no "real-mode comes later" phase for node/loader/worker-startup work.
  Real-GPU-only verification (the one thing the Forge agent genuinely cannot run —
  §2.2) is the sole exception, and even that exception only ever applies to running
  on actual GPU silicon — it never excuses skipping the CPU-with-fixture real path.

### 17.4 Test Isolation Rules

1. Every test using a database gets its own `open_in_memory()` connection.
2. Every test setting an env var restores it unconditionally on exit, even on panic.
   Capture the pre-existing value first.
3. Every test binding a network socket uses port `0` (OS-assigned) and reads the
   actual port after bind.
4. `#[serial]` only for physically singular shared resources (hardware mock env vars
   qualify; nothing else does — port conflicts, DB locks, temp file collisions are
   isolation defects to fix structurally, not `#[serial]` candidates).
5. No `#[ignore]` in committed code. A test that cannot pass is fixed or deleted.
6. Any test that spawns a subprocess and blocks on its IPC output (`recv()`,
   `proc.wait()`, `proc.communicate()`) sets an explicit timeout and surfaces
   captured stderr on timeout. See `docs/ENVIRONMENT.md §11.5` for the exact pattern
   — this rule exists because of a real incident (`test_mock_startup_sends_ready`
   hanging forever on a worker that died silently) and is non-negotiable.
7. Never use `sys.modules.pop("torch")` + `importlib.reload()` to test "this module
   doesn't import torch at the top level," or any equivalent forced-unload-and-reload
   of a module that transitively imports a native-extension-heavy package. This
   crashed the WSL2 agent VM at the OS level (not a clean test failure) twice in
   P904. Use subprocess isolation instead: spawn a fresh Python process, assert
   `"torch" not in sys.modules` inside that subprocess, check its exit code from the
   test. This is slower per-test but cannot take down the host process.

### 17.5 Real-Mode Fixture Checkpoints

`worker/tests/fixtures/` holds tiny synthetic `.safetensors` files — never real
downloaded model weights — built to exercise the loading contract (§11.3) at a scale
safe for a 10GB CI/agent VM. A fixture's tensor shapes are deliberately *not* a
miniaturized copy of the real model's shapes verbatim; they are chosen to be
structurally valid for the architecture's shape-inference formula while being small
enough to construct in well under 1GB peak RAM. Building a new fixture is part of
the same task that implements a new arch module's `load()` — a `load()` function
merged without a fixture exercising it has no real-mode test to satisfy §17.3.

**Mandatory regression case:** at least one fixture per diffusion/CLIP/VAE family
must have a non-recognizable key prefix and no `arch` metadata key, so the metadata
fallback path in shape/arch inference is actually exercised. (v3 shipped a
`st.metadata` vs `st.metadata()` call-as-property bug, `worker/nodes/loader.py:702`
in that codebase, that was never caught because every real fixture used so far had a
recognizable prefix and never hit the fallback. v4 closes this by requiring the
fallback path to be exercised by at least one fixture from the start, not by hoping a
future test happens to use an unusual checkpoint.)

### 17.6 Mock Layers (Rust + Python, orthogonal)

| Layer | Activation | Scope |
|:------|:-----------|:------|
| `mock-hardware` cargo feature | Compile-time | Replaces GPU detection with `MockDetector` (§6.7) |
| `ANVILML_WORKER_MOCK=1` env var | Runtime | Python worker skips torch/capability-probe, returns sentinel values (§14.3) |

In CI, both are active for the mock-mode test run; for the real-mode test run,
`mock-hardware` stays active (no real GPU exists in CI either — §2.2) but
`ANVILML_WORKER_MOCK` is unset, so the worker subprocess takes the real startup path
on a CPU device against fixture checkpoints.

---

## 18. Build & Toolchain

### 18.1 Toolchain & Edition Pin

**Rust 1.96.0, edition 2024.** Both are explicit, exact pins — not "stable" and not
"2021." A task that bumps either pin without being asked to is out of scope; a task
that targets an edition-2021 construct where a 2024 equivalent exists (e.g. the 2024
edition's tightened `unsafe` block/`extern` requirements, RPIT lifetime capture
rules) must use the 2024-correct form.

`rust-toolchain.toml` (workspace root):
```toml
[toolchain]
channel = "1.96.0"
components = ["rustfmt", "clippy"]
```

Workspace root `Cargo.toml` sets the edition once for every member to inherit —
no per-crate `edition = "2021"` (or any other value) anywhere in the workspace:
```toml
[workspace]
resolver = "2"
members = ["crates/*", "backend"]

[workspace.package]
edition = "2024"
rust-version = "1.96.0"
```

Every crate's own `Cargo.toml` inherits rather than restating the value:
```toml
[package]
name = "anvilml-core"
edition.workspace = true
rust-version.workspace = true
```

A `Cargo.toml` that hardcodes `edition = "2021"` or omits `edition.workspace = true`
is non-compliant — this is exactly the kind of stale-pin drift that is easy for an
agent to copy forward from an example seen during training rather than verified
against this section.

**Dependency compatibility, verified by the project owner against live registries
(2026-06-25) — this verification was deliberately not delegated to the Forge agent,
per the version-floor/API-fabrication rules in `FORGE_AGENT_RULES.md` §6, because the
agent re-deriving MSRV/edition compatibility from training-data memory is the exact
drift pattern those rules exist to prevent:**

| Crate | Verified current version | Stated MSRV | Compatible with 1.96.0 / edition 2024? |
|:------|:--------------------------|:------------|:------------------------------------------|
| `tokio` | 1.51.x (current LTS) | 1.71 | Yes |
| `axum` | 0.8.9 | 1.80 | Yes |
| `zeromq` | 0.6.0 (latest published; unchanged from v3) | not formally stated; tracks current stable | Yes |
| `sqlx` | 0.9.0 | 1.94.0 | Yes on MSRV — **but see the flagged change below before any task pins this version** |

**Flagged: `sqlx` 0.9.0 is a breaking release from v3's assumed 0.8.x baseline.** Two
changes a task in `anvilml-registry`/`anvilml-artifacts` must account for, stated here
so the agent does not have to discover them by trial and error:
1. The project moved registries (`launchbadge/sqlx` → `transact-rs/sqlx`); this does
   not change the crate name on crates.io, only documentation/issue-tracker links.
2. `query!`/`query_as!`/etc. now take a `SqlSafeStr`-bound parameter instead of a bare
   `&str`. By default only `&'static str` implements `SqlSafeStr`. Any query string
   built at runtime (e.g. a dynamically-constructed `WHERE` clause in `scanner.rs` or
   `store.rs`) must be wrapped in `sqlx::AssertSqlSafe(..)` explicitly. A task that
   hits a `SqlSafeStr` trait-bound compiler error must apply this wrapper — that is
   the correct fix, not a downgrade to `sqlx = "0.8"` (the version-floor rule in
   `FORGE_AGENT_RULES.md` §6 applies here by name).

If a future task's `cargo update` moves any of these crates to a version not
reflected in this table, the task's own MCP/registry lookup (mandatory per
`FORGE_AGENT_RULES.md` §6) is authoritative over this table, and the table should be
corrected to match in the same task.

### 18.2 Cargo Features

| Feature | Crates | Effect |
|:--------|:-------|:-------|
| `mock-hardware` | `anvilml-hardware` (forwarded by worker, scheduler, server, backend) | Enables `MockDetector`. All CI runs use this. Never in releases. |

Forwarding rule — every crate depending (directly or transitively) on
`anvilml-hardware` declares:
```toml
[features]
mock-hardware = ["anvilml-hardware/mock-hardware"]
```

### 18.3 CI Matrix (GitHub Actions)

Six jobs, run on every push to `main` and every PR. This is more detailed than v3's
CI section because real-mode CPU testing in CI is now a first-class, designed
requirement rather than an aspiration — see §2.2 for why this is possible (the
fixture-checkpoint approach keeps real-mode tests small enough for a standard
GitHub-hosted runner).

| Job | Runner(s) | Commands |
|:----|:----------|:---------|
| `rust-linux` | `ubuntu-latest` | `cargo fmt --all -- --check`, `cargo clippy --workspace --features mock-hardware -- -D warnings`, `cargo test --workspace --features mock-hardware` |
| `rust-windows` | `windows-latest` | `cargo clippy --workspace --features mock-hardware -- -D warnings`, `cargo test --workspace --features mock-hardware` (no `cargo fmt --check` — Linux-only per v3's existing convention) |
| `worker-linux-mock` | `ubuntu-latest` | Install `requirements/base.txt` (no torch). `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v -m "not real_mode"` |
| `worker-linux-real` | `ubuntu-latest` | Install `requirements/base.txt`, then `requirements/cpu-runner-reqs.txt` (torch CPU wheel). `python -m pytest worker/tests -v -m real_mode` — `ANVILML_WORKER_MOCK` unset; exercises real startup + fixture-checkpoint loading (§17.5) on CPU. |
| `worker-windows-mock` | `windows-latest` | Same as `worker-linux-mock`, Windows paths/venv activation. |
| `worker-windows-real` | `windows-latest` | Same as `worker-linux-real`, Windows paths/venv activation. |
| `openapi-drift` | `ubuntu-latest` | Regenerate `openapi.json`; `git diff --exit-code api/openapi.json` |
| `config-drift` | `ubuntu-latest` | `cargo test -p anvilml --features mock-hardware -- config_reference` |

**Why mock and real are separate jobs, not one job with two pytest invocations:** a
real-mode CPU test failure (e.g. a meta-device construction bug) must never be masked
by, or block on, an unrelated mock-mode failure in the same job's exit code — keeping
them as separate jobs means the CI status check for each is independently
attributable, which matters for §17.3's phase-closing gate (the agent needs to be
able to point at exactly which job is red).

**pytest marker convention:** real-mode-only tests are marked `@pytest.mark.real_mode`
(registered in `worker/pytest.ini` or `pyproject.toml`). A test with no marker is
assumed mock-compatible and runs in both jobs unless it imports torch unconditionally
(which only `real_mode`-marked tests may do).

**No OS gate on worker job ordering.** Within `worker-linux-real` and
`worker-windows-real`, the install order is always: `base.txt` → mock-mode collection
check (`pytest --collect-only -m "not real_mode"`, confirms nothing in the mock
suite accidentally imports torch at collection time) → `cpu-runner-reqs.txt` → the
real-mode suite. This ordering is identical on both platforms; do not add a
platform-specific branch to it without a stated reason.

### 18.4 Local WSL2 Pre-Push Gate (not a CI job)

```bash
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
```

Catches Windows-incompatible code without a Windows runner on every push. Requires
`rustup target add x86_64-pc-windows-gnu` and `gcc-mingw-w64-x86-64` in WSL2.
Documented as a required pre-push step in `docs/ENVIRONMENT.md`.

This check itself produces build cache. It is covered by the same mandatory cleanup
as every other build/test command in the session — see §4.9, not a separate rule.

### 18.5 Config-Drift And OpenAPI-Drift Gates

Any task that adds/renames/removes a field on a top-level or nested config struct
must, in the same task: (a) update `anvilml.toml`; (b) update
`docs/ENVIRONMENT.md §4`; (c) confirm `config-drift` passes locally. Same pattern for
any change to a public HTTP handler signature and `openapi-drift`.

### 18.6 Worker Requirements File Discipline

`torch` must never appear in `worker/requirements/base.txt` — restated here because
it is a CI-breaking mistake, not just a style note. `base.txt` installs cleanly with
no GPU driver and no torch wheel index configured; that property is what lets
`worker-linux-mock`/`worker-windows-mock` run without ever touching a torch index.

### 18.7 Release Build

Single statically-linked executable (`anvilml`/`anvilml.exe`) via
`cargo build --release`. Python worker source lives in `worker/` relative to the
binary; the venv is created by provisioning scripts. The binary does not embed
Python source.

---

## 19. Operations & Runbook

### 19.1 First-Run Setup

1. Place model files in configured model directories (default:
   `./models/{diffusion,text_encoders,vae}/`).
2. Run `./scripts/install_worker_deps.sh` (Linux) / `install_worker_deps.ps1`
   (Windows) to create the Python venv, or let AnvilML auto-provision on startup.
3. Start: `./anvilml` (or `anvilml.exe`). Binds `127.0.0.1:8488`.

### 19.2 Ghost Job Reset

On startup, any job in `Queued` or `Running` state from a previous run is reset to
`Failed` with `error: "server_restart"`.

### 19.3 Graceful Shutdown

On `SIGINT`/`SIGTERM` (Linux) or Ctrl-C (Windows):
1. HTTP server stops accepting new connections.
2. All workers receive `Shutdown` IPC messages.
3. Server waits up to 30 seconds for workers to exit.
4. Any worker not exited is killed.
5. SQLite WAL is checkpointed.
6. Process exits 0.

### 19.4 Worker Crash Recovery

1. `ManagedWorker` detects exit via the child process wait future.
2. Status transitions to `Dead`; demux deregisters the worker (§9.4);
   `WorkerStatusChanged` is broadcast.
3. Any in-flight job on that worker is marked `Failed` with `error: "worker_crashed"`.
4. After `respawn_delay_ms` (default 2000), the worker is respawned.
5. On `Ready`, the worker is available for new jobs.

If a worker crashes more than `respawn_max_attempts` (default 5) times within
`respawn_window_s` (default 300), respawn halts; the worker stays `Dead` until a
manual `/v1/workers/:id/restart` call.

---

## 20. Implementation Roadmap

Implementation proceeds as vertical slices. Each phase delivers a runnable binary
with one new observable capability, verified by a Runnable Proof command. No phase is
complete until its Runnable Proof passes — and, for any phase touching nodes,
loaders, or worker startup, until both its mock-mode and real-mode tests pass
(§17.3). The authoritative phase breakdown lives in `docs/PHASES.md`; this is a
high-level grouping.

| Phase Group | Capability | Mock/Real note |
|:------------|:-----------|:----------------|
| **Scaffold** | Repository structure; server starts and answers `/health` | N/A |
| **Core Infrastructure** | Config, shutdown, domain types, hardware detection, SQLite, model registry | Hardware detection ships with `mock-hardware` from the start; no separate "add mocking" phase later |
| **IPC Baseline** | WebSocket events; ZeroMQ ROUTER transport; worker spawn, handshake, keepalive | **Gate: the 1000-round-trip ROUTER/DEALER stress test (§17.2) must pass before any later phase begins.** This is unchanged from v3 — it was correct there; v3's IPC failures were ownership-model gaps the spec now closes (§8.3, §9.1), not a missing stress test. |
| **Real Worker Startup** | `worker_main.py` real-mode startup (no mock gate, §14.1), real capability probe (§6.6), `Ready` event with `capabilities_source=pytorch`, against a CPU device with no nodes registered yet | **This phase did not exist as a named phase in v3 — it is new and load-bearing.** Both a mock-mode and a real-mode startup test exist before this phase closes. Nothing in a later phase may assume real startup "will be built later." |
| **Generic Node Groundwork** | `BaseNode`/`@register`/`SlotSpec`, the three `arch/` dispatch packages (`diffusion/`, `clip/`, `vae/`) with their shared `can_handle`/`get_module` scan logic but **no concrete arch modules yet**, `NodeContext` with `caps`/`mock` fields, the §10.6 marker convention itself | Mock and real tests here cover dispatch-with-zero-modules-registered and dispatch-with-one-stub-module — not yet a real checkpoint, since no arch module exists yet yet to load one. |
| **Dynamic Node System** | Worker reports node types at Ready; Rust stores in dynamic registry; `/v1/nodes` endpoint live | Depends on Generic Node Groundwork existing first. |
| **Graph Validation** | DAG validator uses dynamic registry; slot type checking; full error collection | |
| **Job Queue** | Job submission, persistence, queue management, VRAM ledger | |
| **Dispatch & Execute** | Scheduler dispatches to worker; a trivial real node (e.g. a no-op pass-through) proves end-to-end real dispatch, not just mock | Unlike v3, this phase's "proves IPC correctness" claim must be demonstrated with a real (if trivial) node, not only a mock execution path. |
| **Artifact Storage** | PNG content-addressed storage; `/v1/artifacts/:hash` | |
| **Live Events** | Progress, ImageReady, Completed/Failed events via WebSocket | |
| **Cancellation** | Cooperative cancel: Queued (immediate) and Running (IPC signal) | |
| **ZiT Diffusion + Qwen3 CLIP + ZiT VAE** | `arch/diffusion/zit.py`, `arch/clip/qwen3.py`, `arch/vae/zit_vae.py` — full §11 loading contract, real fixture-checkpoint tests, real `Sampler`/`VaeDecode` dispatch for this one architecture family | First phase exercising §11 end to end. Fixture checkpoints for ZiT/Qwen3-4B/ZiT-VAE are built here. |
| **Flux 2 Klein 4B Diffusion + Flux 2 VAE** | `arch/diffusion/flux2klein.py` (4B variant), `arch/vae/flux2_vae.py` — reuses `qwen3.py` for its text encoder (already built) | Confirms the generic node layer genuinely needed no changes to add a second diffusion architecture — if it does need changes, that is a design defect to flag, not silently patch. |
| **Flux 2 Klein 9B + Qwen3-8B CLIP variant** | Extends `flux2klein.py` for the 9B FP8 variant; extends `qwen3.py`'s dispatch for the 8B FP8-mixed encoder size | Confirms the same arch module can serve two model sizes via shape inference (§11.3 step 1) rather than needing a second file. |
| **End-to-End Validation** | Full real generation run on real GPU hardware (manual, project owner only — §2.2): ZiT → real PNG; Flux 2 Klein (4B, 9B) → real PNG | The only phase whose Runnable Proof is explicitly manual and explicitly excluded from CI, per §2.2. |
| **Distribution** | Auto-provisioning; version introspection; release packaging | |
| **Documentation** | mdBook documentation site; API reference; node SDK guide | |

**Critical path items:**
- IPC Baseline's stress test gate (above) is unchanged from v3 and remains correct.
- Real Worker Startup is the new critical-path item: no node, loader, or arch-module
  phase begins before it closes, because every one of them depends on `NodeContext`
  carrying real `caps` and a real-vs-mock `mock` flag that only exists once this
  phase has shipped both branches.
- Generic Node Groundwork must close before any per-architecture phase (ZiT, Flux 2
  Klein 4B, Flux 2 Klein 9B) starts, per your explicit phasing decision: groundwork
  first, concrete architectures as successive phases.
- Any task that would introduce a hardcoded node type name outside test fixtures, or
  a hardcoded architecture string outside an arch module's own `can_handle()`, is a
  defect (unchanged from v3 — this was correct there).

---

## Appendix A: v3 → v4 Change Table

| Area | v3 | v4 | Why |
|:-----|:---|:---|:----|
| Real-path policy | Mock-only gate (`ANVILML_WORKER_MOCK != "1"` → exit 1); real startup never ran | No gate; real startup is the default branch, mock is the explicit alternate (§14.1) | P904's headline finding: real code had never executed once, structurally untested rather than under-tested |
| Model loading | `diffusers` model classes + `diffusers`-internal `convert_*_checkpoint_to_diffusers` functions (§10.5 in v3) | Raw shape-inferred `nn.Module` construction + hand-written key remap; `diffusers`/`transformers` used only for layer building blocks, never as a hub-aware loader (§11) | Relying on `diffusers`-internal, non-public conversion functions risked silent breakage; full offline guarantee needed regardless of `local_files_only` |
| Hardware capability | Single `InferenceCaps`, implicitly trusted regardless of source | `CapabilitySource` enum distinguishes `PyTorch` (real probe, authoritative) from `DeviceTable`/`Fallback` (pre-spawn hints only); `worker/capability.py` mandatory from the Real Worker Startup phase (§6.6) | v3 left "worker self-detects real capability" as an acknowledged, unscheduled gap |
| Mock/real divergence | No structural check; P904 found mock mode "real" and real mode "never run" for four phases undetected | `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` markers per function, mechanically grep-checked (§10.6) | Closes the exact loophole — an entire missing code path with no stub to grep for |
| IPC ownership | `ManagedWorker::run(self)`/`shutdown(self)` consumed by value while pool needed `Arc`-wrapping; single mutex around ROUTER send+recv caused a shutdown deadlock; demux had no `deregister()` | `WorkerHandle` (cheap, `Clone`-able) separated from `ManagedWorker` (owns its own `run()` task); `RouterTransport` splits send/recv into independent locks at construction (§8.3, §9.1); `deregister()` mandatory from the same task as `register()` (§9.4) | These exact defects required multiple rounds of manual, non-agent intervention to fix in v3 |
| VAE arch dispatch | Nested inside `arch/diffusion/{zit,flux}.py` | Own family, `arch/vae/` (§11.4) | Two VAE architectures are shared across three diffusion models in the v4 matrix; nesting duplicated code |
| CLIP scope | `qwen3`, `clip_l`, `t5` clip_type families | `qwen3` only (4B and 8B variants) | v4 model matrix (ZiT, Flux 2 Klein 4B/9B) uses only Qwen3 text encoders |
| Test catalogue | No `Mode` field | `Mode: mock \| real \| both` per entry (§17.1) | Makes mock/real coverage independently auditable against the §10.6 markers |
| NVML detector | Present (Linux VRAM refresh) | Removed; Vulkan memory-heap query covers VRAM refresh | Not required for the CUDA/ROCm-only v4 target; avoid speculative scope |
| `Ready` event | No `capabilities_source` field | Added (§8.6) | Scheduler/operator diagnostics must distinguish a real probe from mock/synthetic values |
| CI worker jobs | Two-OS matrix (`worker` on ubuntu/windows), mock only | Four jobs: `worker-{linux,windows}-{mock,real}` (§18.3) | Real-mode CPU testing in CI is now a first-class requirement, not deferred to manual verification |
| Roadmap | Real-mode worker startup was not a named phase at all | "Real Worker Startup" is its own named phase, gating every later node/loader phase (§20) | This was the single largest unscoped gap the handoff identified |
| Toolchain pin | Rust 1.95.0, edition 2021 | Rust 1.96.0, edition 2024 (§18.1). Dependency MSRV/compatibility verified against live registries before this pin was adopted — `sqlx` 0.9.0's `SqlSafeStr` breaking change flagged explicitly so no task rediscovers it via a failed downgrade attempt | Explicit version bump; workspace-level pin via `workspace.package`, not restated per crate |
| Build cache discipline | No mandated cleanup; accumulated over 200GB of `cargo`/Python cache across the life of v2/v3 on the agent VM | `cargo clean` (workspace-wide) + Python cache cleanup mandatory as the last step of every ACT session that ran a build/test command (§4.9) | Direct, quantified fix for a recorded disk-exhaustion risk on the 10GB-RAM WSL2 agent VM |
| Device capability hint table | Hardcoded `device_db.rs` Rust match table, separately maintained from the SQLite seed | `docs/SUPPORTED_DEVICES_DB.md` hand-converted once into `database/seeds/devices.sql` by a single one-time task; `device_db.rs` removed entirely; the Markdown file is kept permanently as a human reference but never re-read by any code (§6.3, §7.5) | Removes a second, independently-maintained copy of the same hint data that could silently drift from the SQL seed |
| Arch module method naming | `load_model()` in one arch module, bare `load()` in another, other ad-hoc variants across files | One fixed method-name contract — `can_handle()`, `get_module()`, `load()`, `sample()`/`decode()`/`compute_latent_shape()` — identical across every diffusion/clip/vae arch module, stated as a table (§10.4) | Removes the copy-the-last-file-I-saw drift pattern that produced inconsistent names in v3 |
| `ClipTextEncode` input naming | `text` / `negative_text` | `positive_text` / `negative_text` (§10.3) | Symmetric naming makes the positive/negative pairing explicit at the API level |

---

## Appendix B: MVP Model Matrix And Example Graph

### B.1 Model Matrix

| Diffusion model | Diffusion arch module | Text encoder | CLIP arch module | VAE family | VAE arch module |
|:----------------|:------------------------|:-------------|:--------------------|:-----------|:------------------|
| Z-Image Turbo (ZiT), FP8 | `arch/diffusion/zit.py` | Qwen3 4B | `arch/clip/qwen3.py` | ZiT-compatible | `arch/vae/zit_vae.py` |
| Flux 2 Klein 4B, FP8 | `arch/diffusion/flux2klein.py` | Qwen3 4B | `arch/clip/qwen3.py` | Flux 2-compatible | `arch/vae/flux2_vae.py` |
| Flux 2 Klein 9B, FP8 | `arch/diffusion/flux2klein.py` | Qwen3 8B (FP8-mixed) | `arch/clip/qwen3.py` | Flux 2-compatible | `arch/vae/flux2_vae.py` |

Each diffusion model requires three separate standalone `.safetensors` files
(diffusion model, text encoder, VAE) — no all-in-one checkpoint files are in scope.
Flux 2 Klein's 4B and 9B variants share one diffusion arch module and one VAE arch
module; only the text-encoder size differs, and that is handled by `qwen3.py`'s own
shape inference (§11.3 step 1), not a second CLIP module.

### B.2 Example Graph (architecture-agnostic — works for any row in B.1)

The `model_id` values are SHA256 hex digests the job **submitter** provides — the
same digest the model scanner computes and `GET /v1/models` reports (§7.2). The
scheduler resolves each hash to its registered filesystem path immediately before
dispatching `WorkerMessage::Execute`, rewriting the graph's `LoadModel`/`LoadVae`/
`LoadClip` `model_id` inputs in place. The Python worker's loader nodes always
receive a real filesystem path, never the hash — they perform no hash lookup
themselves. Submitting a hash unknown to the registry fails the job before dispatch.

```json
{
  "nodes": [
    {
      "id": "model",
      "type": "LoadModel",
      "inputs": { "model_id": "<sha256-of-diffusion-model-safetensors>" }
    },
    {
      "id": "vae",
      "type": "LoadVae",
      "inputs": { "model_id": "<sha256-of-vae-safetensors>" }
    },
    {
      "id": "encoder",
      "type": "LoadClip",
      "inputs": {
        "model_id": "<sha256-of-text-encoder-safetensors>",
        "clip_type": "qwen3"
      }
    },
    {
      "id": "latent",
      "type": "EmptyLatent",
      "inputs": {
        "width": 1024,
        "height": 1024,
        "model": { "node_id": "model", "output_slot": "model" }
      }
    },
    {
      "id": "cond",
      "type": "ClipTextEncode",
      "inputs": {
        "clip": { "node_id": "encoder", "output_slot": "clip" },
        "positive_text": "a photograph of a red fox in a snowy forest"
      }
    },
    {
      "id": "sampled",
      "type": "Sampler",
      "inputs": {
        "model": { "node_id": "model", "output_slot": "model" },
        "conditioning": { "node_id": "cond", "output_slot": "conditioning" },
        "clip": { "node_id": "encoder", "output_slot": "clip" },
        "latent": { "node_id": "latent", "output_slot": "latent" },
        "steps": 20,
        "cfg": 3.5,
        "seed": -1
      }
    },
    {
      "id": "decoded",
      "type": "VaeDecode",
      "inputs": {
        "vae": { "node_id": "vae", "output_slot": "vae" },
        "latent": { "node_id": "sampled", "output_slot": "latent" }
      }
    },
    {
      "id": "saved",
      "type": "SaveImage",
      "inputs": {
        "image": { "node_id": "decoded", "output_slot": "image" },
        "seed": { "node_id": "sampled", "output_slot": "seed" }
      }
    }
  ]
}
```

`LoadModel` outputs only `MODEL`. `VaeDecode` requires `vae` as an explicit input —
no implicit state propagation from any other node. `EmptyLatent`'s `model` input is
required in real mode so it can dispatch to the correct architecture's
`compute_latent_shape()` (§10.3) — every row in B.1 has a structurally different
patch-packing formula, so this input is never optional once a real model is involved.

`Sampler` dispatches internally to whichever `arch/diffusion/*.py` module's
`can_handle()` matches the loaded model's `.arch` attribute. Seed `-1` resolves to a
cryptographically random integer; the resolved seed is returned as the `seed` output
slot and wired into `SaveImage` so the stored artifact records the actual seed used.

---

*End of document. `docs/ARCHITECTURE.md` is a navigational summary of this document.
In case of conflict, this document is authoritative.*