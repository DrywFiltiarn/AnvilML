# AnvilML — Phase Implementation Graph

**Generated:** 2026-06-30
**Source:** `.forge/tasks/tasks_phase*.json` + `docs/TASKS_PHASE*.md`, all 31 phase files (30 primary + retrofit Phase 900), 253 tasks total, as committed at the time of generation.
**Status:** Reference document. Regenerate after any phase's task set changes — this file is a snapshot, not a live view.

## Purpose

This document is a complete cross-reference of what every phase builds, where each phase's dependencies come from, what new code each phase exposes, and where that code gets wired into the running system (or, for library-only phases, into the next phase that wires it). It exists for two uses:

1. **Reference** — answering "what phase built X" or "what does phase N depend on" without re-reading 31 `TASKS_PHASE*.md` files.
2. **Verification** — a structural cross-check that every `pub` type, function, or module a phase creates is traceable to a consumer somewhere in a later phase (or the same phase). A symbol with no traceable consumer anywhere in the graph is exactly the defect class found and fixed in `P900-A6`/`P900-A7` (Phase 6's `create_pool()`/`SeedLoader` never reaching `main.rs`), `P8-E4`/`P8-E6` (Phase 8's `RespawnPolicy` never reaching the crash path), `P8-E5` (Phase 8's `KeepaliveWatchdog` never constructed by `ManagedWorker`), `P900-A9`/`P900-A10` (Phase 3's `EnvReport` shape not matching what Phase 18/28 assume), `P18-C3` (Phase 6's `ModelScanner` never reaching server startup), and `P17-B3`/`P17-B4` (Phase 17's own `execute_graph()` never reaching a real job — the most severe instance found, since it silently broke every `POST /v1/jobs`-based Runnable Proof from Phase 24 onward). This document is the artifact that makes that kind of gap visible by inspection instead of by accident.

## Methodology

For each phase, four things are recorded:

- **Implements** — the concrete artifacts (crates, modules, types, functions, files) the phase's tasks produce, in authored task order.
- **Depends on** — which earlier phases' artifacts this phase's tasks consume, traced to the specific symbol/type/file, not just the phase number from the `Depends on phases` header field.
- **Exposes / New surface** — the `pub` API, HTTP route, CLI flag, or other externally-visible capability the phase adds.
- **Wired in** — where that new surface is actually connected to the reachable execution graph (`main.rs`, `AppState`, `build_router()`, a CI workflow, or another crate's `lib.rs` re-export) — and if it is *not yet* wired in by this phase, which later phase is responsible for doing so, confirmed by tracing forward through the task graph rather than assumed.

This document reflects the task graph as authored — including the retrofit (`P900-A6`–`P900-A10`) and in-phase corrections (`P8-E4`/`P8-E5`/`P8-E6`/`P8-E7`/`P8-F2`, `P18-C3`, `P17-B3`/`P17-B4`) already applied to close every wiring gap found across two full audit passes covering all 30 primary phases plus the Phase 900 retrofit, plus a third, Phase-8-specific review that reset a failed implementation attempt and found two further gaps in the same subsystem (see [Known Wiring Gaps Closed](#known-wiring-gaps-closed-summary-table) items 14–15). See [Known Wiring Gaps Closed](#known-wiring-gaps-closed-summary-table) for the complete list and [Phase 19–30 Deep Trace Findings](#phase-1930-deep-trace-findings) for the method and results of the second pass, which found the project's single most severe gap (`execute_graph()` never reachable from a real job).

---

## Crate Dependency Graph (ground truth, `ANVILML_DESIGN.md §3.2`)

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

This is the **invariant** every phase's crate-level dependency claims below are checked against. No crate may depend on a crate above it in this graph; a phase that appears to need an edge running the other way is a design error, not a wiring gap, and is called out as such if found.

---

## Phase Dependency Tree

This mirrors each phase's `Depends on phases` header field, but rendered as a tree so transitive depth is visible at a glance. A phase number in `[brackets]` is a phase whose artifacts are consumed but which is not in the immediate `Depends on phases` list — i.e. a transitive dependency satisfied because an intermediate phase already depends on it.

```
P1  Repository Scaffold                          (none)
└── P2  Core Domain Types: Config & Errors
    └── P3  Core Domain Types: Data Model
        ├── P4  Hardware Detection: Detectors
        │   └── P5  Hardware Detection: Orchestration
        ├── P6  Model Registry & Artifacts
        ├── P7  IPC Foundations
        │   └── P8  IPC Stress Gate & Worker Pool
        │       └── P9  Real Worker Startup
        │           └── P10 Generic Node Groundwork
        │               └── P11 Dynamic Node System            [P8, P9]
        │                   └── P12 Graph Validation
        │                       └── P13 Job Queue               [P6]
        │                           └── P14 Dispatch & Execute  [P5, P6, P8, P9, P10, P12, P13]
        │                               └── P15 Artifact Storage Wiring  [P6, P7]
        │                                   └── P16 Live Events  [P7]
        │                                       └── P17 Cancellation  [P7, P9, P10]
        │                                           └── P18 HTTP/WebSocket Server Completion  [P5, P6, P8]
        │                                               ├── P19 Model Loading Contract Groundwork  [P6, P9, P10, P14]
        │                                               │   └── P20 ZiT Diffusion Arch Module: Shape Inference  [P9, P10]
        │                                               │       └── P21 ZiT Diffusion: Sampling & Latent Shape
        │                                               │           ├── P22 Qwen3 CLIP Arch Module
        │                                               │           └── P23 ZiT VAE Arch Module
        │                                               │               └── P24 Generic Nodes, Real Mode  [P14, P15, P19-23]
        │                                               │                   ├── P25 Flux 2 Klein 4B + Flux 2 VAE  [P19-24]
        │                                               │                   │   └── P26 Flux 2 Klein 9B + Qwen3-8B  [P22]
        │                                               │                   │       └── P27 End-to-End Validation
        │                                               │                   │
        │                                               └── P28 Distribution
        │                                                   └── P29 Documentation  [P10, P13, P14, P18]
        │                                                       └── P30 v4 Roadmap Closeout  [all 1-29]
        └── (P4-P18 chain continues as above)

P900 Spec-Drift & Logging Retrofit  — inserted between P6 and P7 via `prereqs`, not phase number.
      Depends on: P1, P3, P6. Gates P7 (`P7-A1` prereqs on `P900-A5`).
```

---

## Phase-by-Phase Detail

### Phase 1 — Repository Scaffold

**Depends on:** None (first phase).

**Implements:**
- `Cargo.toml` workspace root, `rust-toolchain.toml` (pinned 1.96.0, edition 2024), `.gitattributes` (`P1-A1`)
- `backend/src/main.rs`, `cli.rs` stub, binary compiles (`P1-A2`)
- `backend/src/shutdown.rs` — cross-platform graceful shutdown signal handler stub (`P1-A3`)
- Every crate in the dependency graph as an empty, doc-commented stub, added in dependency order: `anvilml-core` (`P1-B1`), `anvilml-hardware` + `mock-hardware` feature declaration (`P1-B2`), `anvilml-registry`/`anvilml-artifacts` (`P1-B3`), `anvilml-ipc`/`anvilml-worker` (`P1-B4`), `anvilml-scheduler`/`anvilml-server` (`P1-B5`), `anvilml-openapi` build-time stub binary (`P1-B6`)
- `anvilml.toml` checked-in reference config, scaffold-stage fields only (`P1-C1`)
- `GET /health` handler, returns bare `200` (`P1-D1`) — later corrected by `P900-A2`
- CI: `ci.yml` real `rust-test` matrix job (`P1-E1`); `worker-test` + drift job **placeholders** (`P1-E2`) — wired to real logic by `P9-F1` (worker-test) and `P18-F2` (openapi-drift)

**Exposes / New surface:** The `anvilml` binary itself (compiles, does nothing yet beyond `/health`); every crate's name now exists in the workspace for later phases to add code to; `GET /health` as the first HTTP route.

**Wired in:** `P1-D2` is this phase's own Runnable Proof — builds the release binary and curls `/health` over a real TCP socket. No `pub` surface from this phase is left unconsumed; every stub crate is consumed by name in its own later phase (Phase 4 starts writing into `anvilml-hardware`, Phase 6 into `anvilml-registry`, etc.) — there is nothing here to verify as a chokepoint since the phase's entire output is scaffolding, not behavior.

---

### Phase 2 — Core Domain Types: Config & Errors

**Depends on:** Phase 1 (the `anvilml-core` stub crate `P1-B1` created; `backend/src/main.rs` from `P1-A2`).

**Implements:**
- `AnvilError` enum + `IntoResponse` impl (`P2-A1`) — the single error type every later crate converts into
- `ServerConfig`'s top-level scalar fields (`P2-A2`), then nested table structs — `GpuSelectionConfig`, `LimitsConfig`, `RocmConfig`, `HardwareOverrideConfig`, `ModelDirConfig` (`P2-A3`)
- `config_load::load()`'s four-layer precedence chain: defaults → TOML (`P2-A4`) → env vars → CLI flags (`P2-A5`)
- `backend/src/main.rs` wired to call `config_load::load()` (`P2-A6`) — replaces Phase 1's CLI-only host/port handling
- `config_reference` test confirming `anvilml.toml` matches `ServerConfig`'s actual field set (`P2-A7`)

**Exposes / New surface:** `AnvilError`, `ServerConfig`, `CliOverrides`, `config_load::load()` — all `pub` from `anvilml-core`.

**Wired in:** `P2-A6` wires `config_load::load()` directly into `main.rs` within the same phase — this phase closes its own loop, nothing deferred. Every later phase that reads a `ServerConfig` field (e.g. `P900-A6`'s `config.db_path`, `P5-A5`'s CLI subcommand) does so against this phase's struct.

---

### Phase 3 — Core Domain Types: Data Model

**Depends on:** Phase 1 (crate exists), Phase 2 (`AnvilError`, shares the `types/` module pattern `ServerConfig` established).

**Implements:** Every domain type in `anvilml-core/src/types/*`, in this order: `Job`/`JobStatus`/`JobSettings` (`P3-A1`), `ModelMeta`/`ModelKind`/`ModelDtype`/`ModelFormat` (`P3-A2`), `ArtifactMeta` (`P3-A3`), `HardwareInfo`/`GpuDevice`/`DeviceType` (`P3-A4`), `InferenceCaps`/`EnumerationSource`/`CapabilitySource` (`P3-A5`), `WorkerInfo`/`WorkerStatus`/`EnvReport`/`ProvisioningState` (`P3-A6`), `NodeTypeDescriptor`/`SlotDescriptor`/`SlotType` (`P3-A7`), `WsEvent`'s job-lifecycle variants (`P3-A8`) then worker/system/provisioning variants (`P3-A9`), `NodeTypeRegistry` (`P3-A10`, the dynamic registry populated at runtime from worker `Ready` events — never hardcoded), and a final `lib.rs` re-export pass (`P3-A11`).

**Exposes / New surface:** Every type listed above, all `pub`, all deriving `ToSchema` per the design doc — **except** `EnvReport`/`ProvisioningState` as originally implemented by `P3-A6`, which had a 3-field/4-variant shape that didn't match `ANVILML_DESIGN.md`'s 7-field/4-variant spec. **Fixed by `P900-A9`/`P900-A10`** (retrofit, see Phase 900 below) before `P18-A1`/`P28-B1` — the two tasks that assume the doc's shape — execute.

**Wired in:** This phase produces pure data types with no I/O — "wiring" for this phase means "consumed by a later phase's logic," which every type here is: `Job` by Phase 13/14 (`JobStore`, `JobScheduler`), `ModelMeta` by Phase 6 (`ModelStore`), `ArtifactMeta` by Phase 6 (`ArtifactStore`), `HardwareInfo` by Phase 5 (`detect_all_devices()`), `InferenceCaps` by Phase 6 (`DeviceCapabilityStore`) and Phase 9 (`capability.py`'s mirror), `WorkerInfo`/`WorkerStatus` by Phase 8 (`ManagedWorker`), `EnvReport` by Phase 18/28 (`/v1/system/env`, preflight checks), `NodeTypeDescriptor`/`NodeTypeRegistry` by Phase 11 (`GET /v1/nodes`), `WsEvent` by Phase 16 (the WebSocket broadcaster).

---

### Phase 4 — Hardware Detection: Detectors

**Depends on:** Phase 1 (`anvilml-hardware` stub, `mock-hardware` feature flag), Phase 2 (`ServerConfig`'s `hardware_override`), Phase 3 (`HardwareInfo`/`GpuDevice`/`DeviceType`, `InferenceCaps`).

**Implements:** `DeviceDetector` trait + crate scaffolding (`P4-A1`), then five concrete detectors in priority order: `CpuDetector` (`P4-A2`, always returns one CPU device — the only mandatory fallback), `MockDetector` (`P4-A3`, env-var driven, used by every CI run), `VulkanDetector` (`P4-A4`, headless enumeration, the real-hardware primary), `DxgiDetector` (`P4-A5`, Windows-only `cfg`-gated fallback), `SysfsPciDetector` (`P4-A6`, Linux-only `cfg`-gated fallback).

**Exposes / New surface:** `DeviceDetector` trait and all five implementations, each `pub` but none yet composed into a single entry point.

**Wired in:** Not wired in this phase — each detector is an isolated, independently-tested implementation. **Phase 5 closes the loop**: `P5-A2`'s mock-vs-real branch + Vulkan fallback chain is the first place anything calls more than one detector in sequence, and `P5-A4`'s `lib.rs` re-export is the first place `detect_all_devices()` becomes the crate's single public entry point.

---

### Phase 5 — Hardware Detection: Orchestration

**Depends on:** Phase 4 (all five detectors), Phase 2 (`ServerConfig.hardware_override`), Phase 3 (`HardwareInfo`).

**Implements:** `hardware_override` config short-circuit (`P5-A1` — if set, skip real detection entirely, for CI/testing), the mock-vs-real branch + Vulkan→DXGI/sysfs fallback chain (`P5-A2`), CPU-append + final `HardwareInfo` assembly (`P5-A3`), `lib.rs` re-export of `detect_all_devices()` (`P5-A4`), the `hw-probe` CLI subcommand (`P5-A5`), and this phase's Runnable Proof (`P5-A6`).

**Exposes / New surface:** `anvilml_hardware::detect_all_devices()` — the crate's sole public entry point; the `anvilml hw-probe` CLI subcommand, printing `HardwareInfo` as JSON and exiting without binding a socket.

**Wired in:** Fully wired within this phase — `P5-A5` calls `detect_all_devices()` directly from `backend/src/main.rs`'s new `hw-probe` branch, and `P5-A6` proves it live. The *default* (non-`hw-probe`) server startup path does **not** call `detect_all_devices()` yet — that happens in **Phase 18** (`P18-A1`, populating `AppState.hardware`).

---

### Phase 6 — Model Registry & Artifacts

**Depends on:** Phase 1 (`anvilml-registry`/`anvilml-artifacts` stubs), Phase 2 (`ServerConfig.db_path`, `model_dirs`, `artifact_dir`), Phase 3 (`ModelMeta`, `InferenceCaps`, `ArtifactMeta`).

**Implements (Group A — Model Registry):** `database/migrations/001_initial.sql` (`models`, `device_capabilities` tables) (`P6-A1`); `create_pool()` — `SqlitePool` creation + migration runner via `sqlx::migrate!()` (`P6-A2`); `ModelStore` CRUD (`P6-A3`); `ModelScanner::scan_dir()` — hashing + kind/dtype inference (`P6-A4`); `DeviceCapabilityStore::lookup()` (`P6-A5`); `SeedLoader`'s hash-check/bookkeeping (`P6-A6`) then `run()` (`P6-A7`); `database/seeds/devices.sql`, one-time hand conversion from `SUPPORTED_DEVICES_DB.md` (`P6-A8`); `lib.rs` re-export pass (`P6-A9`).

**Implements (Group B — Artifacts):** `ArtifactStore::save()` content-addressed write (`P6-B1`); `database/migrations/002_artifacts.sql` + `ArtifactStore::get()` (`P6-B2`); `ArtifactStore::list()` by `job_id` (`P6-B3`).

**Exposes / New surface:** `anvilml_registry::{create_pool, ModelStore, ModelScanner, DeviceCapabilityStore, SeedLoader}`; `anvilml_artifacts::ArtifactStore`.

**Wired in — originally a confirmed gap, now fixed:**
- `create_pool()` and `SeedLoader::run()` were **not called anywhere outside `anvilml-registry`'s own tests** as originally authored — `backend/Cargo.toml` had no dependency on `anvilml-registry` at all. **Fixed by `P900-A6`/`P900-A7`** (retrofit), which add the dependency and call both from `main.rs`'s default startup path.
- `ModelScanner::scan_dir()` was only reachable via `POST /v1/models/rescan` (`P18-C2`) — no startup scan. **Fixed by `P18-C3`** (in-phase addition to Phase 18, since Phase 18 — not Phase 6 — is where `AppState.model_store` first exists).
- `ArtifactStore` **is** correctly wired: `P15-A1` adds it to `AppState`, `P15-B1`/`P15-B2` expose it over HTTP, `P15-C1` calls `ArtifactStore::save()` from the scheduler's event loop on `ImageReady`.
- `DeviceCapabilityStore` is queried by the Python worker's capability probe indirectly (the seed data it reads is the same `device_capabilities` table) but is not directly called by any Rust task after Phase 6 — its only direct Rust consumer is `SeedLoader` populating the table it serves. This is expected: per `ANVILML_DESIGN.md §6`, the table is a pre-spawn *hint*, and the worker's own `capability.py` probe (Phase 9) is authoritative for real compute capability, not `DeviceCapabilityStore` lookups at runtime.

---

### Phase 7 — IPC Foundations

**Depends on:** Phase 1 (`anvilml-ipc` stub), Phase 2 (`AnvilError`), Phase 3 (no direct type dependency — IPC types are self-contained per `ANVILML_DESIGN.md §8`'s "no business logic, no knowledge of worker lifecycle" constraint).

**Implements:** `IpcError` + `AnvilError` conversion (`P7-A1`); `WorkerMessage` enum, Rust→Python (`P7-A2`); `WorkerEvent` enum — `Ready`/`Pong`/`Dying`/`MemoryReport` (`P7-A3`) then job-lifecycle variants (`P7-A4`); `RouterTransport` struct + `bind()` (`P7-B1`); `RouterTransport::send()`/`recv()` with the mandatory split-lock design (`P7-B2` — the fix for the v3 "combined lock deadlocked shutdown" regression); `EventBroadcaster` (`tokio::sync::broadcast` wrapper) placed in `anvilml-ipc` specifically to avoid a worker↔server crate cycle (`P7-C1`); `lib.rs` re-export pass (`P7-D1`).

**Exposes / New surface:** `anvilml_ipc::{IpcError, WorkerMessage, WorkerEvent, RouterTransport, EventBroadcaster}`.

**Wired in:** Partially within-phase, completed later. `RouterTransport` is exercised by Phase 8's stress test (`P8-A1`) and bound for real by `WorkerPool::spawn_all()` (`P8-G1`). `EventBroadcaster` is constructed but **not yet held by `AppState`** — that's `P16-B1` (`AppState` gains a `broadcaster` field, wired from `main.rs`). `WorkerMessage`/`WorkerEvent` are consumed across the Python boundary starting Phase 9 (`worker/ipc.py` mirrors the Rust enum's wire format) and by the Rust-side event loop starting Phase 14 (`P14-A3`) onward.

---

### Phase 8 — IPC Stress Gate & Worker Pool

**Depends on:** Phase 1, 2, 3 (transitively), Phase 7 (`RouterTransport`, `WorkerMessage`/`WorkerEvent`, `IpcError`).

**Implements:** The 1000-round-trip ROUTER/DEALER stress test — an explicit gate, no later phase begins until it passes (`P8-A1`); `WorkerEnv` env-var map builder (`P8-B1`); `spawn.rs` subprocess `Command` construction (`P8-B2`); `job_object.rs` Windows orphan-cleanup wrapper, `cfg`-gated (`P8-B3`); `WorkerSpawner` trait + `ProcessWorkerSpawner`, standalone (`P8-B4`); `demux.rs` with mandatory `register()`/`deregister()` pairing (`P8-C1`); `keepalive.rs` ping/pong watchdog (`P8-C2`); `RespawnPolicy` backoff + max-attempt guard (`P8-D1`); `WorkerHandle` — cheap, `Clone`-able (`P8-E1`) — then `set_status()` (`P8-E2`); `ManagedWorker::run()`'s three original exit paths — graceful shutdown, 60s `Initializing` timeout, crash (`P8-E3`); crash `attempt_history` tracking + the `should_respawn()` decision point (`P8-E4`); `KeepaliveWatchdog` wired in as a second crash source (`P8-E5`); the actual respawn loop, spawning gen 0 and every respawn uniformly (`P8-E6`); Windows Job Object reassignment across respawns (`P8-E7`); `bridge.rs`'s two independent reader/writer tasks (`P8-F1`); `ManagedWorker` retrofitted onto the bridge's demux-fed channel (`P8-F2`); `WorkerPool::spawn_all()`/`shutdown_all()` (`P8-G1`); `lib.rs` re-export pass (`P8-H1`).

**Exposes / New surface:** `anvilml_worker::{WorkerPool, WorkerHandle, ManagedWorker, WorkerSpawner, ProcessWorkerSpawner}`.

**Wired in — originally a confirmed gap, now fixed (second round):**
- `RespawnPolicy` (`P8-D1`) was built but **never invoked** by `ManagedWorker::run()`'s original crash-exit path (`P8-E3`) — a crashed worker permanently exited, contradicting `ANVILML_DESIGN.md §9.2`'s "a crashed worker is automatically respawned" and §9.5's `Dead → Respawning → Initializing` state transition. `P8-E4` closed the decision-point half (`should_respawn()` is consulted and logged) and remains correct as shipped. The original `P8-E5` — meant to close the execution half — was attempted and failed in ACT (see `PHASES.md`'s amendments log for the full account) and was reset before merge; it never landed. **Fixed by this revision's `P8-B4`/`P8-E6`** (the actual respawn: delay, re-spawn subprocess via a `WorkerSpawner` abstraction with a proven production path, re-register demux, loop back to `Initializing`) **and `P8-E7`** (the Windows Job Object reassignment `P8-B3` never had reason to test). `P8-F1`'s prereq changed from the old `P8-E5` to the new `P8-E7` accordingly, so the bridge task lands only after the full crash-respawn path — including the Windows-specific piece — is wired.
- `KeepaliveWatchdog` (`P8-C2`) was built and tested but **never constructed by `ManagedWorker`** — its own source carried a `TODO(P8-E3)` marker that neither `P8-E3` nor `P8-E4` picked up. Without it, `ManagedWorker`'s only crash signal was a `transport.recv()` error, which never fires for a worker that hangs or dies silently — the most common real crash. **Fixed by `P8-E5`** (in-phase, found during the same review that reset the old `P8-E5`).
- `ManagedWorker::run()` calling `transport.recv()` directly (`P8-E3`) is only safe for exactly one worker — it races once `WorkerPool` (`P8-G1`) shares one `Arc<RouterTransport>` across multiple `ManagedWorker` tasks, since whichever task's `recv().await` wins a given poll can consume a message addressed to a different worker. Nothing in the phase's original task set (including the original `P8-F1`) ever redirected `ManagedWorker`'s own consumption path to close this. **Fixed by `P8-F2`** (in-phase, found during the same review).
- `WorkerPool` itself **is** correctly wired: `P14-C2` (`backend/main.rs spawns real WorkerPool + JobScheduler at startup`) is the actual call site.
- `KeepaliveWatchdog` (`P8-C2`/`P8-E5`) sends `WorkerMessage::Ping` and unconditionally enforces `watchdog_pong_timeout` from the moment a worker reaches `Ready` — but **no task anywhere in the project's task graph ever wires a `WorkerEvent::Pong` reply into `worker_main.py`'s dispatch loop** on the Python side. Confirmed by an exhaustive search across every phase's task docs: `Pong` is mentioned only on the Rust-receiving side (`P7-A3`'s enum definition, `P8-A1`'s stress test using a simulated test-double DEALER, and `keepalive.rs`'s own `handle_event()` arm) — never as something a task adds to `worker_main.py` or `worker/ipc.py`. `P9-D2` created the dispatch loop as an explicit log-and-continue placeholder; the only task that ever extended it afterward was `P17-B5` (`CancelJob`, scoped exclusively to that one message type per its own task text) and the `P17-B3`/`P17-B4` gap-fix (`Execute`). Neither the original task set nor its Phase 19–30 deep-trace audit (which found and fixed the `Execute`-hang gap, see Phase 17/24's notes below) ever caught this, since that audit was scoped around whether a submitted job completes, not whether a worker survives long enough to receive one. Practical effect: every real (non-mock) worker that successfully reached `Ready` was unconditionally killed by its own supervisor within one `watchdog_ping_interval` + `watchdog_pong_timeout` window and endlessly respawned — this was invisible until a real subprocess first reached `Ready` end-to-end, which no prior phase's tests exercised (mock-mode tests never spawn a real subprocess this deep; `P9-E1`'s and `real_startup_tests.rs`'s integration tests both terminate the child immediately after observing `Ready`, before any `Ping` would be sent). **Fixed by a direct manual patch to `worker_main.py`'s `_dispatch_loop()`** (governance's "documented manual patch" path, given the operational urgency — Phase 9 is long-frozen and this blocked all further real-mode testing), adding a `Ping` → `Pong` echo with three new regression tests in `worker/tests/test_worker_main.py::TestDispatchLoopPing`. `Execute`/`CancelJob` handling remain correctly out of scope of this patch — those are Phase 17's concern, unaffected.

---

### Phase 9 — Real Worker Startup

**Depends on:** Phase 1, 2, 3 (transitively), Phase 7 (`WorkerMessage`/`WorkerEvent` wire format), Phase 8 (`WorkerPool` spawns the process this phase's Python code runs inside).

**Implements:** `worker/requirements/base.txt` — core deps, **never torch** (`P9-A1`); the real torch CPU wheel pin in the `cpu-*` requirement files (`P9-A2`); `real_mode` pytest marker registration (`P9-A3`); `worker/ipc.py`'s ZeroMQ DEALER transport + msgpack framing, mirroring Phase 7's Rust `RouterTransport` (`P9-B1`); `worker/capability.py`'s real `probe_capabilities()` torch probe (`P9-C1`) then `worker_main.py`'s `_mock_probe_capabilities()` synthetic equivalent (`P9-C2`); `worker_main.py`'s real-mode startup — connect, device-select, probe, no mock gate per §14.1 (`P9-D1`), node-import stub + `Ready` event + loop (`P9-D2`), then the mock-mode startup sequence (`P9-D3`); the real-subprocess integration test (`P9-E1`); CI wiring of Phase 1's placeholder `worker-test` job to real install/test steps (`P9-F1`).

**Exposes / New surface:** `worker_main.py` as a real, spawnable Python entry point; `worker/ipc.py`, `worker/capability.py` as importable modules.

**Wired in:** Fully wired within this phase — `P9-E1`'s integration test spawns a **real** subprocess (not mock IPC) and confirms it sends `Ready`, which is the actual end-to-end proof that Phase 8's `spawn.rs`/`WorkerPool` and this phase's `worker_main.py` connect correctly over the real ZeroMQ transport. `P9-F1` wires CI to actually install and run this. Node loading itself (`_import_nodes()`) is still a stub here — real wiring is Phase 10's `P10-D1`.

---

### Phase 10 — Generic Node Groundwork

**Depends on:** Phase 1, 2, 3 (transitively), Phase 9 (`worker_main.py`'s node-import stub from `P9-D2`).

**Implements:** `worker/nodes/base.py`'s `SlotSpec` dataclass + `NODE_REGISTRY` dict (`P10-A1`), `@register` decorator with required-attribute validation (`P10-A2`), `NodeContext` runtime context class (`P10-A3`), `BaseNode` ABC with abstract `execute()` (`P10-A4`); the shared `can_handle()`/`get_module()` dispatch logic for `arch/diffusion/`, reused identically for `arch/clip/` and `arch/vae/` (`P10-B1`, `P10-B2`); `worker/nodes/__init__.py`'s auto-import-triggers-registration wiring (`P10-C1`); `worker_main.py`'s real `_import_nodes()` finally connected to this machinery (`P10-D1`); a short marker-convention pointer doc for later node-authoring phases (`P10-E1`).

**Exposes / New surface:** `NODE_REGISTRY`, `@register`, `BaseNode`, `NodeContext`, `SlotSpec` — the entire node-authoring contract every node from Phase 14 onward implements against.

**Wired in:** Fully wired within this phase — `P10-D1` is exactly the task that connects `worker_main.py`'s previously-stubbed `_import_nodes()` to the real auto-import machinery `P10-C1` built, closing the loop in the same phase rather than deferring it.

---

### Phase 11 — Dynamic Node System

**Depends on:** Phase 1, 2, 3 (transitively), Phase 8 (`WorkerPool`/`ManagedWorker`), Phase 9 (the real `Ready` event), Phase 10 (`NODE_REGISTRY`, the node-authoring contract).

**Implements:** `ManagedWorker` calls `node_registry.register_all()` on every `Ready` event (`P11-A1`) — the bridge from a worker's self-reported node types to the Rust-side `NodeTypeRegistry`; `AppState` struct, initially **two** fields — `config` and `node_registry`, the first incremental slice of the full `§13.2` struct (`P11-B1`); `GET /v1/nodes` handler (`P11-C1`) — this task's own commit also folded the pre-existing `HealthState` (`P1-D1`/`P900-A2`) into `AppState` as a third field, `start_time`, one task earlier than `P11-B1`'s own text scoped (see gap #23); `backend/main.rs` constructs `AppState` (3 fields as of `P11-C1`) and switches to `build_router()` (`P11-D1`); this phase's Runnable Proof (`P11-E1`).

**Exposes / New surface:** `AppState` exists for the first time (incomplete — gains fields incrementally through Phase 18); `GET /v1/nodes` — the first route backed by dynamic, worker-reported data rather than a static placeholder.

**Wired in:** Fully wired within this phase. This is also the first task graph evidence of the project's incremental-`AppState`-growth discipline that every server phase from here through Phase 18 follows: add the field a phase's own handler needs, never speculatively ahead of demand. `P900-A6`/`P900-A7`'s database-pool wiring (retrofit, lands before Phase 7 executes) deliberately does **not** add a `db` field to this `AppState` for exactly this reason — that field is `P14-C1`'s scope, not introduced early. **One exception exists:** `start_time` (`HealthState`'s sole field, pre-dating `AppState` since `P1-D1`) was folded into `AppState` by `P11-C1` rather than `P11-B1` — a cosmetic scope slip, not a functional gap (`/health` behavior and its tests were unaffected throughout); see gap #23.

---

### Phase 12 — Graph Validation

**Depends on:** Phase 1, 2, 3 (transitively), Phase 11 (`NodeTypeDescriptor`/`SlotType` from the node registry, used by the slot-compatibility check).

**Implements:** `ValidatedGraph` newtype, construction-gated so an unvalidated graph can never reach the scheduler (`P12-A1`) — as originally landed, this guarantee did not actually hold in a normal build (see gap #24, fixed in-phase); `GraphError` enum, all 7 variants (`P12-A2`); `validate_graph()` built up incrementally — structural checks 1–2 (`P12-A3`), node-type + edge checks 3–4 (`P12-A4`), slot-type-compatibility check 5 (`P12-A5`) — as originally landed, a dangling edge on one output slot of a node incorrectly suppressed mismatch-checking on that node's *other*, validly-declared output slots too (see gap #25, fixed in-phase), cycle detection via Kahn's algorithm, check 6 (`P12-A6`); `lib.rs` re-export pass (`P12-B1`).

**Exposes / New surface:** `anvilml_scheduler::{ValidatedGraph, GraphError, validate_graph}`.

**Wired in:** Not wired to an HTTP surface in this phase — `validate_graph()` is a pure function with no caller yet. **Wired by Phase 14**'s `P14-A1` (`JobScheduler::submit()` calls `validate_graph()` before a job ever enters the queue) and exposed indirectly via `POST /v1/jobs`'s `P14-D1` handler, which returns `GraphError`'s variants as the `400`-class error body.

---

### Phase 13 — Job Queue

**Depends on:** Phase 1, 2, 3 (transitively), Phase 6 (`anvilml-registry`'s `create_pool()`/migration pattern, reused for the `jobs` table), Phase 12 (`ValidatedGraph` — `JobQueue` stores validated graphs only).

**Implements:** `database/migrations/003_jobs.sql` (`P13-A1`); `JobQueue` — in-memory FIFO with O(1) cancellation via lazy removal (`P13-A2`); `VramLedger` — per-device reservation tracking, advisory only per `§12.4` (`P13-A3`); `JobStore` CRUD + `reset_ghost_jobs()` — resets any `Queued`/`Running` job to `Failed` with `error: "server_restart"` on startup, per `§19.2` (`P13-B1`); `backend/main.rs` calls `reset_ghost_jobs()` at startup (`P13-C1`) — the first time `main.rs` constructs a real `SqlitePool` in its **normal** (non-`hw-probe`) run path, using `anvilml_registry::create_pool()` from Phase 6; `lib.rs` re-export pass (`P13-D1`).

**Exposes / New surface:** `anvilml_scheduler::{JobQueue, VramLedger}`; `anvilml_registry::JobStore`.

**Wired in:** `JobStore::reset_ghost_jobs()` is wired in this same phase via `P13-C1` — no gap. Note this is the **second** independent call site that constructs a `SqlitePool` via `create_pool()` (the first being `P900-A6`'s retrofit into the same `main.rs`, landing earlier in execution order since it's gated to land before Phase 7) — both call sites are expected to coexist or be consolidated when `P14-C1` later adds a persistent `db: SqlitePool` field to `AppState`; this is noted as a consolidation point, not a defect, since both calls are individually correct and idempotent (migrations are a no-op on an already-migrated database).

**Doc-drift finding, closed:** `ANVILML_DESIGN.md §12.1`'s module-layout comment previously described `queue.rs` as "sorted by priority+created_at." No task ever authored priority-ordering logic, and `JobSettings` (`P3-A1`) has no `priority` field — `P13-A2`'s `JobQueue` is, and was always intended to be, a plain FIFO. Confirmed with the project owner that priority override is not required. Fixed by correcting the `§12.1` comment to "insertion order only"; no code change needed since `P13-A2`'s implementation already matched the intended (FIFO-only) behavior — only the design doc was stale.

---

### Phase 14 — Dispatch & Execute

**Depends on:** Phase 1, 2, 3 (transitively), Phase 5 (`HardwareInfo`/device list for worker selection), Phase 6 (`ModelStore`/`ArtifactStore` patterns), Phase 8 (`WorkerPool`/`WorkerHandle::set_status()`), Phase 9 (real worker subprocess), Phase 10 (`BaseNode` — `PassThrough` is the first real node), Phase 12 (`validate_graph()`), Phase 13 (`JobQueue`, `VramLedger`, `JobStore`).

**Implements:** `JobScheduler` struct + `submit()` — calls `validate_graph()` before enqueueing (`P14-A1`); `cancel()`/`get_job()` (`P14-A2`); the dispatch loop skeleton, notify-driven wake, initially an always-false selection stub (`P14-A3`); the real 2-step worker selection algorithm per `§12.5` (`P14-A4`); `dispatch_one()` marks the assigned worker `Busy` via `WorkerHandle::set_status()` (`P14-A5`); `worker/nodes/passthrough.py` — the project's first real (non-mock-only) node file (`P14-B1`); `AppState` gains `scheduler`/`workers`/`db` fields (`P14-C1`); `backend/main.rs` spawns a real `WorkerPool` + `JobScheduler` at startup (`P14-C2`); `POST /v1/jobs` (`P14-D1`); `GET /v1/jobs` + `GET /v1/jobs/:id` (`P14-D2`); this phase's Runnable Proof — a submitted job with a `PassThrough` node actually reaches `Completed` (`P14-E1`).

**Exposes / New surface:** `anvilml_scheduler::JobScheduler`; `AppState.scheduler`/`.workers`/`.db`; three new HTTP routes; the project's first node that actually executes.

**Wired in:** Fully wired within this phase — `P14-C2` is the call site that finally makes `WorkerPool::spawn_all()` (Phase 8) run for real at server startup, and `P14-E1` proves the entire chain (HTTP → scheduler → worker → node → completion) end to end for the first time. This is the project's primary "everything connects" milestone; every subsequent phase builds additional routes/nodes on top of working dispatch rather than introducing new wiring chokepoints of this scale.

---

### Phase 15 — Artifact Storage Wiring

**Depends on:** Phase 1, 2, 3 (transitively), Phase 6 (`ArtifactStore`), Phase 7 (`WorkerEvent::ImageReady` variant), Phase 14 (`AppState`, the scheduler's event loop entry point).

**Implements:** `AppState` gains `artifact_store` field (`P15-A1`); `GET /v1/artifacts` list handler (`P15-B1`); `GET /v1/artifacts/:hash` — serves raw PNG bytes (`P15-B2`); the scheduler's `event_loop.rs`/`dispatch_one()` calls `ArtifactStore::save()` when a worker emits `ImageReady` (`P15-C1`); this phase's Runnable Proof — a `PassThrough`-derived job's artifact is retrievable via HTTP (`P15-D1`).

**Exposes / New surface:** Two new HTTP routes; `AppState.artifact_store`.

**Wired in:** Fully wired within this phase — `P15-C1` is exactly the call site connecting Phase 6's `ArtifactStore::save()` (previously only unit-tested) to a real `WorkerEvent`, and `P15-D1` proves the full chain (job → `ImageReady` → `ArtifactStore::save()` → `GET /v1/artifacts/:hash`) live. This is Phase 6's `ArtifactStore` finally reaching the same level of end-to-end wiring `ModelStore` did not get until Phase 18 (`P18-C1`/`P18-C2`/`P18-C3`).

---

### Phase 16 — Live Events

**Depends on:** Phase 1, 2, 3 (transitively), Phase 7 (`EventBroadcaster`, `WsEvent`), Phase 14 (the dispatch loop / event loop entry point), Phase 15 (artifact persistence the event loop also triggers).

**Implements:** The scheduler's event loop completed across three tasks — map every `WorkerEvent` to the corresponding `WsEvent` and publish it (`P16-A1`); persist terminal job status to `JobStore` and release the worker's `VramLedger` reservation on completion/failure/cancellation (`P16-A2`); restore the worker to `Idle` via `set_status()` and wake the dispatch loop so a queued job can claim the now-free worker (`P16-A3` — this closes the starvation bug that would otherwise leave a freed worker undiscovered); `AppState` gains `broadcaster`, wired from `main.rs`, shared between the HTTP layer and the scheduler's event loop (`P16-B1`); the WebSocket handler — connection skeleton + initial `SystemStats` frame (`P16-C1`), then the forward loop with `Lagged` disconnect handling (`P16-C2`); a periodic `SystemStats` background task, every 5 seconds (`P16-D1`); this phase's Runnable Proof — a WebSocket client observes `JobCompleted` for a `PassThrough` job (`P16-E1`).

**Exposes / New surface:** `GET /v1/events` WebSocket upgrade route; `AppState.broadcaster`.

**Wired in:** Fully wired within this phase — `P16-B1` is the call site that finally gives `EventBroadcaster` (built in Phase 7, unused until now) a home in `AppState`, and `P16-A1`/`P16-A3` are what actually publish to it from the scheduler side.

---

### Phase 17 — Cancellation

**Depends on:** Phase 1, 2, 3 (transitively), Phase 7 (`WorkerMessage::CancelJob`), Phase 9 (`worker_main.py`'s message-handling loop), Phase 10 (the node-execution loop cancellation must interrupt), Phase 14 (`JobScheduler`), Phase 16 (the event loop cancellation events flow through).

**Implements:** `JobScheduler::cancel()` branches by the job's current status — `Queued` jobs are removed from `JobQueue` directly, `Running` jobs need IPC (`P17-A1`); `cancel()` sends `WorkerMessage::CancelJob` for `Running` jobs specifically (`P17-A2`); `worker/executor.py`'s topological sort of the node graph (`P17-B1`), then its execution loop checking a `cancel_flag` between node steps (`P17-B2`), the `Execute`-message handler — success path (`P17-B3`) then failure path (`P17-B4`) — then `worker_main.py`'s dispatch loop handling the incoming `CancelJob` message (`P17-B5`); `POST /v1/jobs/:id/cancel` (`P17-C1`); this phase's Runnable Proof — cancelling a `Queued` job returns `202` then `409` on retry (`P17-D1`).

**Exposes / New surface:** One new HTTP route; `worker/executor.py` as a real module (previously nonexistent — node execution before this phase ran without a topological sort or cancellation check).

**Wired in — originally a confirmed gap, now fixed:** Cancellation handling itself (`P17-A1`/`P17-A2`/`P17-B5`/`P17-C1`) is genuinely wired end to end — a `Running` job's cancel request really does reach `worker_main.py` and really does set `NodeContext.cancel_flag`. **However, `worker/executor.py`'s `execute_graph()` (`P17-B1`/`P17-B2`), as originally authored, was never called from anywhere outside its own defining tasks.** `worker_main.py`'s message dispatch loop, left as a placeholder by `P9-D2` ("currently just logs and continues — real dispatch is a later phase"), was originally only extended once more, by this phase's `CancelJob` handler (`P17-B5`, originally authored as this phase's third task), and that extension was scoped exclusively to `CancelJob` handling — no task handled `WorkerMessage::Execute`. **Fixed by `P17-B3`** (wires `Execute` to `execute_graph()` on a background thread, sends `Completed` on success) **and `P17-B4`** (the failure path, sends `Failed` instead of leaving the job silently hung) — both inserted into this phase before it executes, since Phase 17 had not yet run when the gap was found, with the original `CancelJob` task renumbered from `P17-B3` to `P17-B5` to make room. `P17-B5`'s prereq was changed from `P17-B2` to `P17-B4` accordingly. This was the single most severe gap found across the entire project — see [Phase 19–30 Deep Trace Findings](#phase-1930-deep-trace-findings) below for full detail.

---

### Phase 18 — HTTP/WebSocket Server Completion

**Depends on:** Phase 1, 2, 3 (transitively), Phase 5 (`detect_all_devices()`), Phase 6 (`ModelStore`, `ModelScanner`), Phase 8 (`WorkerPool`, the respawn machinery `P18-D2` composes from), Phase 14 (`AppState`'s existing fields), Phase 16 (`AppState.broadcaster`), Phase 17 (cancellation, which `DELETE /v1/jobs` composes alongside).

**Implements:** `AppState` gains its **final** two `§13.2` fields — `hardware`, `env_report` (`P18-A1`); `GET /v1/system`/`GET /v1/system/env` (`P18-B1`), then `GET /v1/system/versions` + `ComponentVersions` type (`P18-B2`); `AppState` gains `model_store`; `GET /v1/models`/`GET /v1/models/:id` (`P18-C1`); `POST /v1/models/rescan` (`P18-C2`); **`P18-C3` — startup model scan, added by retrofit** (see below); `GET /v1/workers` (`P18-D1`); `POST /v1/workers/:id/restart`, composed entirely from `request_shutdown()` + the pool's respawn-on-exit path, both Phase 8 (`P18-D2`); `DELETE /v1/jobs/:id` (`P18-E1`) then `DELETE /v1/jobs` bulk clear (`P18-E2`); real OpenAPI generation from `utoipa` annotations, replacing Phase 1's stub (`P18-F1`); the `openapi-drift` CI gate wired to real generation + diff check, replacing Phase 1's placeholder (`P18-F2`); this phase's Runnable Proof (`P18-G1`).

**Exposes / New surface:** Every remaining route in `ANVILML_DESIGN.md §13.4`'s table; `AppState` reaches its full ten-field `§13.2` shape; `api/openapi.json` gets real content for the first time.

**Wired in — originally a confirmed gap, now fixed:**
- `ModelScanner::scan_dir()` was, as originally authored, only reachable through `P18-C2`'s rescan endpoint — no task triggered a scan at server startup, so a fresh server's model registry stayed empty until a client manually called `/v1/models/rescan`. **Fixed by `P18-C3`** (in-phase addition), which triggers the same background scan at startup, reusing `P18-C2`'s internal trigger function. Per the project owner, models must always be scanned on startup.
- `P18-D2`'s own text already assumed Phase 8's respawn-on-crash machinery worked — that assumption is **now true** because of `P8-E4`/`P8-E6` (see Phase 8 above), but was **not true** when `P18-D2` was originally authored, and remained not true for longer than expected after the original `P8-E5` attempt failed and was reset; this is flagged here as a second-order consequence of the Phase 8 fix, not a defect requiring its own change to `P18-D2`.
- `EnvReport` populated by `P18-A1` is **best-effort/placeholder** by this phase's own design — real preflight checks are explicitly deferred to **Phase 28** (`P28-B1`). This is intentional incrementalism, not a gap, and is called out as such in `P18-A1`'s own context.

---

### Phase 19 — Model Loading Contract Groundwork

**Depends on:** Phase 1, 2, 3 (transitively), Phase 6 (`ModelStore`, model-ID-hash scheme), Phase 9 (`worker_main.py`), Phase 10 (`BaseNode`/`NODE_REGISTRY`), Phase 14 (the scheduler dispatches jobs the loader nodes execute inside).

**Implements:** The scheduler resolves a job graph's `model_id` hashes to real filesystem paths immediately before dispatch (`P19-A1`); `worker/pipeline_cache.py`'s `get_or_load()` LRU component cache (`P19-B1`); `LoadModel` node — mock branch only (`P19-C1`), then the real branch with a **deliberate** `NotImplementedError` deferred-raise (`P19-C2` — this is the project's documented mock/real parity mechanism per `§10`, not an unintentional gap: the raise exists because no diffusion arch module is registered yet, and is closed by name in Phase 20); `LoadVae`/`LoadClip` node skeletons, mock-mode only (`P19-C3`); a fixture-checkpoint builder conventions doc for Phase 20 onward (`P19-D1`); confirmation that existing CI wiring already covers this phase's new tests, no new CI task needed (`P19-E1`).

**Exposes / New surface:** `pipeline_cache.get_or_load()`; `LoadModel`/`LoadVae`/`LoadClip` as registered (but not yet functionally complete) nodes.

**Wired in:** `P19-C2`'s real branch is **intentionally** unwired at the end of this phase — its `NotImplementedError` is the documented placeholder pattern, not a defect. It is closed by name in **Phase 20** (`P20-D1`, `LoadModel`'s real branch finally calls `zit.py` via dispatch), **Phase 22** (`P22-D1`, `LoadClip`), and **Phase 23** (`P23-E1`, `LoadVae`). Closing this placeholder makes `LoadModel.execute()` itself correct, but did not — until `P17-B3`/`P17-B4` (a fix inserted into Phase 17, see that phase's note above) — make `LoadModel.execute()` *reachable* during a real job run, since nothing called any node's `execute()` for a real job at all. See [Phase 19–30 Deep Trace Findings](#phase-1930-deep-trace-findings).

---

### Phase 20 — ZiT Diffusion Arch Module: Shape Inference & Construction

**Depends on:** Phase 1, 2, 3 (transitively), Phase 9 (`probe_capabilities()`/`InferenceCaps`, used for dtype selection), Phase 10 (arch-module dispatch contract), Phase 19 (`LoadModel`'s deferred-raise, closed by this phase).

**Implements:** A tiny synthetic ZiT-shaped checkpoint fixture, plus a metadata-fallback variant (`P20-A1`); `zit.py`'s `_infer_hyperparams()` from the safetensors header (`P20-B1`), then `can_handle()` + dispatch registration (`P20-B2`); meta-device construction (`P20-C1`), dtype selection per `InferenceCaps` (`P20-C2`), key remap + `load_state_dict()` + the `.arch` attribute (`P20-C3`); `LoadModel`'s real branch finally calls `zit.py` via dispatch (`P20-D1`); this phase's Runnable Proof (`P20-E1`).

**Exposes / New surface:** `worker/nodes/arch/diffusion/zit.py` as a complete, real arch module — the project's first fully-real (non-mock) model load.

**Wired in:** Fully wired within this phase — `P20-D1` is exactly the task that closes Phase 19's deliberate placeholder, and `P20-E1` proves `LoadModel` loads the real fixture checkpoint. `P20-E1`'s proof is a direct `pytest` invocation against `LoadModel.execute()`, not a `POST /v1/jobs` call — it is unaffected by the `Execute`-message dispatch gap noted under Phase 17/24.

---

### Phase 21 — ZiT Diffusion Arch Module: Sampling & Latent Shape

**Depends on:** Phase 1, 2, 3 (transitively), Phase 9, Phase 10, Phase 19 (groundwork), Phase 20 (`zit.py`'s construction/loading, which `sample()` builds on).

**Implements:** `compute_latent_shape()`'s architecture-specific formula (`P21-A1`); `sample()`'s pipeline assembly + caching (`P21-B1`), then the denoising loop + seed resolution (`P21-B2`); the `Sampler` generic node — mock branch only (`P21-C1`), then the real branch dispatching to `zit.py` (`P21-C2`); this phase's Runnable Proof (`P21-D1`).

**Exposes / New surface:** `zit.py`'s `sample()`/`compute_latent_shape()`; `Sampler` as a complete real node.

**Wired in:** Fully wired within this phase. `P21-D1`'s proof is a direct `pytest` invocation, same pattern as `P20-E1` — unaffected by the `Execute`-message dispatch gap noted under Phase 17/24.

---

### Phase 22 — Qwen3 CLIP Arch Module

**Depends on:** Phase 1, 2, 3 (transitively), Phase 9, Phase 10, Phase 19 (`LoadClip`'s deferred-raise, closed by this phase), Phase 20 (the construction/loading pattern this phase's `qwen3.py` mirrors for the CLIP-family module).

**Implements:** The vendored Qwen3 tokenizer asset directory + re-seeding script (`P22-A1`); a Qwen3 CLIP fixture builder (`P22-B1`); `qwen3.py`'s shape inference (`P22-B2`) then `can_handle()` + dispatch registration (`P22-B3`); meta construction + dtype selection + tokenizer load (`P22-C1`), key remap + `load_state_dict()` + `.arch` (`P22-C2`); `LoadClip`'s real branch finally calls `qwen3.py` via dispatch (`P22-D1`); this phase's Runnable Proof (`P22-E1`).

**Exposes / New surface:** `worker/nodes/arch/clip/qwen3.py` as a complete real arch module.

**Wired in:** Fully wired within this phase — `P22-D1` closes Phase 19's second deliberate placeholder. `P22-E1`'s proof is a direct `pytest` invocation, same safe pattern as `P20-E1`/`P21-D1` — unaffected by the `Execute`-message dispatch gap noted under Phase 17/24.

---

### Phase 23 — ZiT VAE Arch Module

**Depends on:** Phase 1, 2, 3 (transitively), Phase 9, Phase 10, Phase 19 (`LoadVae`'s deferred-raise, closed by this phase), Phase 20, Phase 21 (the construction/loading/sampling patterns this phase's VAE module follows, plus `decode()` as the VAE family's second fixed method alongside `load()`).

**Implements:** A ZiT-VAE-shaped fixture builder (`P23-A1`); `zit_vae.py`'s shape inference (`P23-B1`) then `can_handle()` + dispatch (`P23-B2`); meta construction (`P23-C1`), dtype selection (`P23-C2`), key remap + load + `.arch` (`P23-C3`); `decode()` — latent-to-image, the VAE family's contract method (`P23-D1`); `LoadVae`'s real branch finally calls `zit_vae.py` via dispatch — the **third and final** loader node to go real (`P23-E1`); this phase's Runnable Proof — the first complete real-mode load+sample+decode chain produces an actual `PIL.Image` (`P23-F1`).

**Exposes / New surface:** `worker/nodes/arch/vae/zit_vae.py` as a complete real arch module.

**Wired in:** Fully wired within this phase — `P23-E1` closes the third and last of Phase 19's three deliberate placeholders. After this phase, every `LoadModel`/`LoadVae`/`LoadClip` real branch is functionally complete for the ZiT model family. `P23-F1`'s proof explicitly chains `LoadModel` → `Sampler` → `zit_vae.py::decode()` **directly** in Python, bypassing the generic node layer and `worker_main.py`'s message loop entirely — also unaffected by the `Execute`-message dispatch gap.

---

### Phase 24 — Generic Conditioning/Sampling/Decode Nodes, Real Mode

**Depends on:** Phase 1, 2, 3 (transitively), Phase 9, Phase 10, Phase 14 (dispatch — this phase's Runnable Proof submits via `POST /v1/jobs`), Phase 15 (artifact persistence — `SaveImage`'s real branch emits `ImageReady`, consumed by `P15-C1`'s existing wiring), Phase 19–23 (every arch module this phase's generic nodes dispatch to).

**Implements:** `ClipTextEncode` — mock branch (`P24-A1`), then real tokenize+encode (`P24-A2`); `VaeDecode` — mock branch (`P24-B1`), then real dispatch to the VAE module (`P24-B2`); `EmptyLatent` — mock branch (`P24-C1`), then real `compute_latent_shape()` dispatch (`P24-C2`); `SaveImage` — mock branch (`P24-D1`), then real PNG encode + `ImageReady` emission (`P24-D2`); `ImageResize` — mock + real, lanczos default, single task (`P24-D3`); the complete generic-node graph proven through real dispatch end to end (`P24-E1`); this phase's Runnable Proof — the first full real-image generation via `POST /v1/jobs` (`P24-F1`).

**Exposes / New surface:** Every remaining generic node (`ClipTextEncode`, `VaeDecode`, `EmptyLatent`, `SaveImage`, `ImageResize`) as fully real, non-mock implementations.

**Wired in — originally a confirmed gap, now fixed:** Each node's own `execute()` method is correctly implemented and unit-testable in isolation — `SaveImage`'s real branch emitting `ImageReady` is correctly built to connect into Phase 15's `P15-C1` artifact-persistence wiring. **`P24-E1`/`P24-F1`, this phase's own integration test and Runnable Proof, both explicitly declare "no new production source files" and assume the full chain already works end to end via `POST /v1/jobs`.** As originally authored, that assumption was false — no task anywhere wired `worker_main.py`'s `Execute`-message handling to `execute_graph()`, so these two tasks would have submitted a job that dispatched correctly (Rust side) and then hung in `Running` forever. **Fixed by `P17-B3`/`P17-B4`** (Phase 17, inserted before that phase executes — see Phase 17's note above). This was the single most severe gap found across the entire project. See [Phase 19–30 Deep Trace Findings](#phase-1930-deep-trace-findings).

---

### Phase 25 — Flux 2 Klein 4B Diffusion + Flux 2 VAE

**Depends on:** Phase 1, 2, 3 (transitively), Phase 9, Phase 10, Phase 19–24 (the entire generic-node + dispatch contract this phase's new arch modules plug into without any change to that contract).

**Implements:** Flux 2 Klein 4B + Flux 2 VAE fixture builders (`P25-A1`); `flux2klein.py`'s shape inference (`P25-B1`) then `can_handle()` + dispatch — confirming the arch registry correctly disambiguates between two diffusion modules for the first time (`P25-B2`); meta construction + dtype (`P25-C1`), key remap + load + `.arch` (`P25-C2`); `compute_latent_shape()` + `sample()`, combined into one task since the pattern is now established (`P25-D1`); the full Flux 2 VAE contract — `load()` + `decode()` — in a single task, the established second-module pattern (`P25-E1`); this phase's Runnable Proof, with explicit confirmation that zero changes were needed to any generic node or dispatch code (`P25-F1`).

**Exposes / New surface:** `flux2klein.py`, `flux2_vae.py` — the second model family, proving the arch-module dispatch contract generalizes.

**Wired in:** `flux2klein.py`/`flux2_vae.py` themselves are correctly wired into the arch-dispatch registry (`can_handle()`/`get_module()`), genuinely requiring zero changes to Phase 10's dispatch contract or Phase 24's generic nodes. **`P25-F1`'s Runnable Proof submits via `POST /v1/jobs`** (per its own context) — it inherited the same `Execute`-message dispatch gap as `P24-E1`/`P24-F1`, now fixed by `P17-B3`/`P17-B4` (Phase 17). See [Phase 19–30 Deep Trace Findings](#phase-1930-deep-trace-findings).

---

### Phase 26 — Flux 2 Klein 9B + Qwen3-8B CLIP Variant

**Depends on:** Phase 1, 2, 3 (transitively), Phase 9, Phase 10, Phase 22 (`qwen3.py`'s existing 4B implementation, extended here to an 8B/FP8-mixed variant), Phase 25 (`flux2klein.py`'s 4B implementation, extended to 9B).

**Implements:** Flux 2 Klein 9B + Qwen3-8B fixture builders (`P26-A1`); 9B shape-inference confirmation (`P26-B1`) then full load/sample confirmation (`P26-B2`) for `flux2klein.py`; Qwen3-8B shape inference + FP8-mixed-dtype detection (`P26-C1`) then load with per-tensor dtype handling (`P26-C2`) for `qwen3.py`; this phase's Runnable Proof — the full MVP model matrix's final combination (`P26-D1`).

**Exposes / New surface:** The 9B/8B size variants of already-existing arch modules — no new files, extensions to `P25`'s and `P22`'s modules.

**Wired in:** The 9B/8B variant logic itself is correctly wired into the existing dispatch registries from Phase 22/25. **`P26-D1`'s Runnable Proof submits via `POST /v1/jobs`** (per its own context), the same pattern as `P24-F1`/`P25-F1` — it inherited the `Execute`-message dispatch gap, now fixed by `P17-B3`/`P17-B4` (Phase 17). This is genuinely the proof that closes all three MVP model-matrix rows from `ANVILML_DESIGN.md` Appendix B.

---

### Phase 27 — End-to-End Validation

**Depends on:** Phase 1, 2, 3 (transitively), Phase 25, Phase 26 (every model variant this phase's checklist validates).

**Implements:** `docs/E2E_VALIDATION.md` — a manual, project-owner-facing real-GPU checklist covering all three MVP model rows (`P27-A1`); a CI audit confirming no workflow job accidentally attempts real-GPU execution (`P27-B1`).

**Exposes / New surface:** A documentation artifact and a CI-safety confirmation — no new code.

**Wired in:** N/A — this phase produces no `pub` surface; "wiring" here means the checklist is the thing a human runs against real hardware, which is outside the scope of anything this graph can verify by code inspection.

---

### Phase 28 — Distribution

**Depends on:** Phase 1, 2, 3 (transitively), Phase 18 (`AppState.env_report`, the placeholder this phase replaces with real preflight checks).

**Implements:** Auto-provisioning — missing-venv bootstrap at startup, auto-invokes `install_worker_deps.sh`/`.ps1` (`P28-A1`); **real** `EnvReport` preflight checks (interpreter path, Python version, torch importability) replacing Phase 18's best-effort placeholder (`P28-B1`); `--version` CLI flag, works even with no venv present (`P28-C1`); `docs/RELEASE.md`'s exact packaging specification (`P28-D1`); this phase's Runnable Proof — a fresh clone auto-provisions and reports versions correctly (`P28-E1`).

**Exposes / New surface:** Auto-provisioning at startup; a real (non-placeholder) `EnvReport`; the `--version` CLI flag.

**Wired in:** Fully wired within this phase, **contingent on `P900-A9` having already corrected `EnvReport`'s field shape** (see Phase 900 below) — `P28-B1`'s context explicitly names `preflight_ok`/`reason` as fields it populates, fields that did not exist on `P3-A6`'s originally-implemented struct. This is now resolved by the retrofit; `P28-B1` itself required no edit.

---

### Phase 29 — Documentation

**Depends on:** Phase 1, 2, 3 (transitively), Phase 10 (`BaseNode`, used as the Node SDK Guide's worked example), Phase 13 (job queue concepts), Phase 14 (dispatch concepts), Phase 18 (`api/openapi.json`, the REST reference chapter's actual source).

**Implements:** mdBook scaffold + `SUMMARY.md` structure (`P29-A1`); Getting Started + Configuration Reference chapters, sourced from `ENVIRONMENT.md` (`P29-B1`); REST API Reference, generated from the real `api/openapi.json` — never hand-transcribed separately, so it cannot drift (`P29-C1`); WebSocket Events chapter, sourced from the actual `WsEvent` enum (`P29-D1`); Node SDK Guide, sourced from the actual node system code, using `PassThrough` (Phase 14) as the worked example (`P29-E1`); Operations/Runbook chapter, consolidating existing scattered runbook content (`P29-F1`); a new, additive `docs-build` CI job (`P29-G1`).

**Exposes / New surface:** `docs/book/` — the mdBook documentation site; a new CI job.

**Wired in:** Fully wired within this phase — every chapter is explicitly sourced from a real, already-existing artifact (`api/openapi.json`, the `WsEvent` enum, the `PassThrough` node) rather than hand-authored prose that could drift from the implementation, which is itself a wiring discipline worth noting: documentation here is generated/derived, not independently maintained.

---

### Phase 30 — v4 Roadmap Closeout: Final Compliance Sweep

**Depends on:** All phases 1–29.

**Implements:** `docs/TESTS.md` completeness audit + backfill against every test file in the repo (`P30-A1`); a project-wide sweep for unmarked stubs, stale `defers_to`, and TODOs (`P30-B1`); a project-wide sweep confirming every `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` marker resolves (`P30-C1`); the complete standard gate sequence run once at full project scope (`P30-D1`); a final `docs/PHASES.md`/`docs/RUNNABLE_PROOF.md` internal-consistency audit (`P30-E1`).

**Exposes / New surface:** None — this phase is entirely verification and documentation-consistency work, no new application code.

**Wired in:** N/A by design — this is the closeout sweep, not a phase that produces wireable surface. **This document (`docs/PHASES_GRAPH.md`) did not exist when `P30-A1`–`P30-E1` were authored** and is not itself one of their sweep targets; a future `P30`-equivalent pass should include this file's consistency against the task graph as a sweep target, since it is exactly the kind of cross-cutting reference `P30-B1`/`P30-C1`'s sweeps are meant to keep accurate.

---

### Phase 900 — Spec-Drift & Logging Retrofit (inserted between Phase 6 and Phase 7)

**Depends on:** Phase 1, Phase 3, Phase 6 (per its `Depends on phases` header — the retrofit corrects defects found in these three phases' own output). Gates Phase 7 (`P7-A1`'s `prereqs` includes `P900-A5`).

This phase exists outside the primary 1–30 sequence — it is inserted via `prereqs`, not phase number, per `FORGE_TASK_AUTHORING_SPEC.md §6`'s retrofit-phase convention. Unlike every other phase in this document, Phase 900's purpose **is** to close wiring/spec-drift gaps in already-completed phases, so its entire content belongs in the "verification" half of this document's purpose, not just the "reference" half.

**Implements, by finding:**
- **Finding 0 (priority):** `tracing-subscriber` was never wired — every `tracing::info!`/`debug!` call since Phase 1 was a silent no-op. Fixed by `P900-A1`.
- **Finding 1:** `GET /health` (`P1-D1`) returned a bare `200` with no body; `ANVILML_DESIGN.md §13.4` specifies `{ status, version, uptime_s }`. Fixed by `P900-A2`.
- **Finding 2:** `Job`/`JobStatus`/`JobSettings` (`P3-A1`) were missing the `ToSchema` derive `§5.3` requires. Fixed by `P900-A3`.
- **Finding 3:** `ModelMeta`/`ModelKind`/`ModelDtype`/`ModelFormat` (`P3-A2`) were missing the `ToSchema` derive `§5.4` requires. Fixed by `P900-A4`.
- **Finding 4:** `--log-format plain|json` (`ENVIRONMENT.md §3.3`) was never implemented. Fixed by `P900-A5`.
- **Finding 5:** `create_pool()`/`SeedLoader::run()` (Phase 6) were never called from `backend` at all — `backend/Cargo.toml` had no dependency on `anvilml-registry`. Fixed by `P900-A6` (pool + migrations) and `P900-A7` (seed loading).
- **Companion:** `P900-A1`'s original test instructions caused an agent to loop indefinitely (no guidance on `Command::new(env!("CARGO_BIN_EXE_anvilml"))`, the established pattern from `P5-A5`'s `hw_probe_help_test.rs`); corrected in place, with the dropped precedence-test assertion restored as `P900-A8`.
- **Finding 6:** `EnvReport` (`P3-A6`) had 3 fields where the design doc specifies 7, and `ProvisioningState`'s variants didn't match either — both already assumed correct by `P18-A1` and `P28-B1`. Fixed by `P900-A9` (struct shape) and `P900-A10` (variant names).

**Exposes / New surface:** No new public surface — every task in this phase corrects an existing type, route, or startup path to match its own already-published contract (the design doc, or a later task's stated assumption).

**Wired in:** Every fix in this phase is, by definition, the wiring fix itself — there is no further phase responsible for connecting these. The one exception is `P900-A6`/`P900-A7`'s pool, which is intentionally **not** stored in `AppState` (that's `P14-C1`'s scope) — see Phase 6 and Phase 11's notes above for why.

---

## Interim Job-Completion Patch (Manual, Pre-Phase-16/17)

**Status:** Applied. **Not a numbered task** — same category as the manual
retrofit patches in items 16–22 of the [Known Wiring Gaps Closed](#known-wiring-gaps-closed-summary-table)
table below (e.g. `P8-E3`, `install_worker_deps.sh`/`.ps1`), applied outside
the Forge task graph by direct patch.

**Why this exists:** `P14-E1` (Phase 14's own Runnable Proof) requires a
submitted `PassThrough` job to observably reach `Completed` via `GET
/v1/jobs/:id`. Per this document's own [Phase 19–30 Deep Trace Findings](#phase-1930-deep-trace-findings)
section (the `execute_graph()` gap — the single most severe finding in this
document) and the Phase 14 "Wired in" note above, that capability's real
implementation was scoped to `P17-B3`/`P17-B4` (worker-side `Execute` →
`execute_graph()` → `Completed`/`Failed`) and `P16-A1`/`P16-A2` (Rust-side
consumption of the resulting `WorkerEvent` to update `JobStore`) — both
phases numbered *after* 14. As originally sequenced, `P14-E1` could not
have passed: the job would dispatch correctly and then hang in `Running`
forever, identically to the `P24-E1`/`P24-F1` failure mode this document
already documents at length. This patch closes that gap early, narrowly,
so Phase 14 can be verified now.

**Scope — deliberately narrow, single-node graphs only:**

- `worker/worker_main.py` — new `_execute_job()` function; `_dispatch_loop()`
  gains a `device`/`caps`/`mock` parameter set and an `Execute` branch that
  calls it. Runs each node in `graph["nodes"]` in list order (no
  topological sort, no edge resolution) and reports `Completed`/`Failed`.
- `crates/anvilml-worker/src/managed.rs` — `ManagedWorker` gains a
  `job_completion_tx: Option<mpsc::UnboundedSender<(Uuid, JobStatus,
  Option<String>)>>` field (default `None`, every existing call site
  unaffected) and a `set_job_completion_tx()` setter. `handle_event()`'s
  `Completed`/`Failed` arms send through it, best-effort, alongside the
  pre-existing `Idle`-restoration behavior (which was already correctly
  wired in Phase 8 and is untouched by this patch).
- `crates/anvilml-worker/src/pool.rs` — `WorkerPool` gains the same field/
  setter pattern; `spawn_all_impl()` propagates the sender into each
  `ManagedWorker` it constructs.
- `crates/anvilml-worker/Cargo.toml`, `backend/Cargo.toml` — `uuid`
  promoted to `[dependencies]` (same pattern `P14-D1` used for
  `anvilml-server`).
- `crates/anvilml-scheduler/src/interim_job_completion.rs` — new module;
  `spawn_interim_job_completion_listener()` receives `(job_id, status,
  error)` tuples and writes them to `JobStore` directly. Re-exported from
  `lib.rs`.
- `backend/src/main.rs` (P14-C2's own file) — constructs the channel before
  `spawn_all()`, registers it on the pool, and spawns the listener task
  after `job_store`/`scheduler` are constructed.

**Every touched location is marked `INTERIM-P14-PATCH` or "INTERIM
STOPGAP" in its own comment**, each pointing back to this section.

**Mandatory replace-not-extend instructions for `P16-A1`/`P16-A2` and
`P17-B1`–`P17-B4`:**

1. `P17-B1`/`P17-B2`/`P17-B3`/`P17-B4`: delete `_execute_job()` and the
   `Execute` branch in `_dispatch_loop()` (`worker_main.py`) wholesale;
   replace with `worker/executor.py::execute_graph()` and its own dispatch
   wiring, per those tasks' original scope (topological sort, background
   thread, cancellation checkpoints, `Progress`/`ImageReady` emission).
   Revert `_dispatch_loop()`'s signature to whatever `P17-B3`/`P17-B4`'s own
   design calls for — do not preserve this patch's `device`/`caps`/`mock`
   parameter threading unless the real design also needs it.
2. `P16-A1`/`P16-A2`: delete `job_completion_tx` and its setter from
   `ManagedWorker` and `WorkerPool` (`managed.rs`, `pool.rs`); delete
   `crates/anvilml-scheduler/src/interim_job_completion.rs` and its
   `lib.rs` re-export; delete the channel construction and listener spawn
   in `backend/src/main.rs`. Replace with the real design: `AppState`
   gains a `broadcaster: EventBroadcaster` field (`P16-B1`), and the
   scheduler subscribes to it directly.
3. Revert the `uuid` promotions in `anvilml-worker/Cargo.toml` and
   `backend/Cargo.toml` if nothing else in those crates needs `uuid` as a
   direct (non-dev) dependency after steps 1–2 remove this patch's usage.
4. After both phases land, grep the tree for `INTERIM-P14-PATCH` and
   `INTERIM STOPGAP` — zero hits should remain.

---

## Known Wiring Gaps Closed (summary table)

These are every confirmed instance, found across two audit passes plus a later targeted P10/P11 compliance audit, of two related defect classes this document exists to make visible: (1) a phase builds a `pub` symbol that compiles and passes its own unit tests, but no later phase's task ever calls it from a reachable execution path (`main.rs`, `AppState`, a handler, or — for the Python side — `worker_main.py`'s dispatch loop); or (2) a task's own documented scope — a struct's declared field list, a claimed field count, or similar — diverges from what its commit actually shipped, even though the shipped code is reachable and functionally correct. Items 1–22 are class (1). Item 23 is the table's sole instance of class (2).

| # | Symbol / capability | Built by | Originally unwired because | Fixed by |
|---|---|---|---|---|
| 1 | `tracing_subscriber` init | Implicit (Phase 1's `tracing` dependency) | No task ever called `.init()` | `P900-A1` |
| 2 | `/health` JSON body | `P1-D1` | Task's own `context` dropped the response-body spec | `P900-A2` |
| 3 | `Job`/`JobStatus`/`JobSettings` `ToSchema` | `P3-A1` | Task's own `context` omitted the derive | `P900-A3` |
| 4 | `ModelMeta`-family `ToSchema` | `P3-A2` | Task's own `context` omitted the derive | `P900-A4` |
| 5 | `--log-format` CLI flag | (never built) | No task in any phase added it | `P900-A5` |
| 6 | `anvilml_registry::create_pool()` | `P6-A2` | `backend` had no dependency on `anvilml-registry`; never called outside the crate's own tests | `P900-A6` |
| 7 | `anvilml_registry::SeedLoader::run()` | `P6-A6`/`P6-A7` | Same as #6 — no call site existed | `P900-A7` |
| 8 | `ANVILML_LOG`-over-`RUST_LOG` precedence test | (test gap only) | Dropped from `P900-A1`'s acceptance to fit the 1000-char `context` cap | `P900-A8` |
| 9 | `EnvReport`'s 7-field shape | `P3-A6` | Implemented shape (3 fields) never matched the design doc; two later tasks (`P18-A1`, `P28-B1`) assumed the doc's shape without verifying it | `P900-A9` |
| 10 | `ProvisioningState`'s variant names | `P3-A6` | Same root cause as #9 — `EnvReport` never even referenced the enum, so the mismatch was inert until #9 wired it in | `P900-A10` |
| 11 | `RespawnPolicy::should_respawn()`/`next_delay()` | `P8-D1` | `ManagedWorker::run()`'s original crash-exit path (`P8-E3`) exited permanently; nothing called the policy | `P8-E4` (decision point), `P8-E6` (actual respawn — second-round correction after the original `P8-E5` attempt failed in ACT and was reset; see item 14/15 note below) |
| 12 | `ModelScanner::scan_dir()` at startup | `P6-A4` | Only reachable via `POST /v1/models/rescan` (`P18-C2`); no startup trigger | `P18-C3` (in-phase, Phase 18 had not executed) |
| 13 | `worker/executor.py`'s `execute_graph()` | `P17-B2` | `worker_main.py`'s dispatch loop, a `P9-D2` placeholder, was only ever extended for `CancelJob` (originally `P17-B3`, renumbered `P17-B5`) — no task handled `WorkerMessage::Execute`, despite Rust genuinely sending it (`P14-A4`) and the Rust event loop genuinely handling the resulting `Completed`/`Failed` (`P16-A1`/`P16-A2`). The single most severe gap found across the project — every `POST /v1/jobs`-based Runnable Proof from `P24-F1` onward (`P25-F1`, `P26-D1`) would have hung indefinitely. | `P17-B3`, `P17-B4` (in-phase, Phase 17 had not executed) |
| 14 | `KeepaliveWatchdog` | `P8-C2` | Built and tested standalone but never constructed or spawned by `ManagedWorker::run()` — left as dead code with an explicit `TODO(P8-E3)` marker in its own source, never picked up by `P8-E3` or `P8-E4` | `P8-E5` (in-phase; found during the review that reset the original, failed `P8-E5` attempt — its identity is reused for this task, see the note below) |
| 15 | `ManagedWorker::run()`'s direct `transport.recv()` call | `P8-E3` | Safe for exactly one worker; races once `WorkerPool` (`P8-G1`) shares one `Arc<RouterTransport>` across multiple `ManagedWorker` tasks — a worker's task can consume a different worker's event. Never addressed by the original `P8-F1`, which only built the bridge without ever redirecting `ManagedWorker`'s own consumption path onto it. | `P8-F2` (in-phase) |
| 16 | `WorkerStatus` transitions (`Ready→Idle`, `Dying→Dead`, `Completed`/`Failed`/`Cancelled→Idle`, graceful shutdown `→Dying`, `Initializing` timeout `→Dead`) | `P8-E3` | Doc comments on both `run()` and `handle_event()` claimed all five transitions since `P8-E3` shipped; none were implemented — the only status write in the whole file was the single `Initializing` set at `run()`'s entry. Neither `P8-E3` nor `P8-E4`'s acceptance tests asserted `status()` mid-lifecycle, only exit-path behavior, so it went undetected for two tasks. | Manual retrofit patch, not a numbered task (see `PHASES.md`'s amendments log) |
| 17 | `RouterTransport::send_raw()` used to simulate worker-originated `WorkerEvent`s in tests | `P8-E3` | `send_raw()` sends via the ROUTER's own socket addressed to `worker_id` — genuine ROUTER→DEALER traffic, delivered to the connected test DEALER, never looping back into that same `RouterTransport`'s own `recv()`. Six pre-existing tests (`test_run_completes_on_ready_event`, `test_deregister_called_on_graceful_exit`, `test_crash_appends_to_attempt_history`, `test_crash_history_grows_per_crash` ×2, `test_should_respawn_called_on_crash`) used it to "send" a `Ready` event to `ManagedWorker`, which was never actually received — silently vacuous since `P8-E3`, masked because none of them checked `status()` afterward and none depended on `Ready` being processed to reach their actual assertion (deregistration/exit behavior, or a `transport.close()`-triggered crash that fires regardless of prior state). Surfaced when item 16's new status assertions were added on top of this already-broken delivery path. | Manual retrofit patch, same commit as item 16 — all six tests plus the three new `Completed`/`Failed`/`Cancelled` tests switched to a `send_event()` helper using the correct DEALER→ROUTER direction (`_dealer.send()`), matching the pattern `test_deregister_called_on_crash` already used correctly |
| 18 | `ManagedWorker::run()`'s `init_timeout` never actually cancelled on `Ready` | `P8-E3` | `handle_event()`'s doc comment claimed the 60s `Initializing`-timeout guard was "dropped (cancelled)" when `Ready` arrived; no code did this — the pinned `sleep` future stayed live in every subsequent `select!` iteration for the worker's entire lifetime and fired at the 60s mark regardless of how long the worker had already been happily `Idle`/`Busy`. Masked previously because most tests complete or explicitly exit well before 60s; surfaced once item 17's fix made `Ready` genuinely reach `ManagedWorker`, exposing item 19 (below) as a consequence. | Manual retrofit patch, same commit as items 16-17 — `run()` now tracks an `initialized: bool`, set the moment a `Ready` event is matched, and the `init_timeout` `select!` branch carries an `if !initialized` precondition (tokio's native branch-disarming mechanism) so it is never even polled again once `Ready` has been received |
| 19 | `RouterTransport::recv()`/`close()` deadlock (Phase 7, `anvilml-ipc`) | `P7-B2` | `recv()` holds `self.socket`'s `MutexGuard` for the entire duration of the blocking inner `socket.recv().await` — the guard isn't released until the whole function returns. `close()` needs that same lock just to swap the socket to `None`. If `recv()` is genuinely blocked waiting for a network message that will never arrive, it holds the lock forever and `close()` can never acquire it — a real deadlock, previously only escaped by luck of scheduling (in the six item-17 tests, `send_raw()`'s own contention for the same lock happened to usually win the race before `ManagedWorker`'s task acquired it) or, once that luck was removed, by item 18's uncancelled 60s timeout accidentally acting as an unintended release valve (`tokio::select!` drops the losing branch's future when another branch wins, which releases a stuck `recv()`'s guard as a side effect). Directly suspected but not confirmed during the original `P8-E5` attempt's own (reverted) rewrite of this file; confirmed here by direct tracing. | Manual retrofit patch to completed Phase 7 code — `close()`'s signal now goes through a `tokio::sync::watch::Sender<bool>`, deliberately independent of `socket`'s mutex, sent *before* `close()` attempts its own lock acquisition; `recv()` races the real socket read against that signal via `tokio::select!`, so a `close()` call reliably interrupts an in-flight `recv()` instead of racing it. The three tests affected by items 17-18 were separately changed to trigger the crash path via a malformed payload sent through the already-open DEALER connection instead of `close()` — deterministic and free of this contention by construction, matching the crash-simulation guidance already written into the `P8-E6` task spec |
| 20 | `install_worker_deps.sh`/`.ps1`'s torch install step | Scripts: manual pre-Phase-9 commit `9e67fec`. Torch pin content: `P9-A2` (`cpu-linux-agent.txt`, `cpu-runner-reqs.txt`) | The scripts only ever ran `pip install -r base.txt`; no task ever added a step installing the torch pin `P9-A2` wrote. `P9-E1`'s and `P9-F1`'s plans both assumed the scripts already provisioned torch, without verifying the scripts' actual content — surfaced as a `rust-test` CI failure (`real_startup_tests.rs`, Phase 9's own Runnable Proof) on the first post-merge run, since `worker/.venv` (the venv these scripts provision, and the one `spawn_worker()` targets) never had torch installed. Separately, an orphaned typo file `cpu-linux-agents.txt` (plural, `9e67fec`) sat alongside the correctly-named `cpu-linux-agent.txt` since before Phase 9 and was never referenced by anything. | Manual retrofit patch (not a numbered task) — both scripts now require `--mode=agent\|ci`, installing the design-doc-correct pin per context (`ANVILML_DESIGN.md §3.1`: `cpu-linux-agent.txt` for the Forge agent, `cpu-runner-reqs.txt` for GitHub CI runners); `.github/workflows/ci.yml`'s `rust-test` job passes `--mode=ci`; `cpu-linux-agents.txt` deleted in the same patch |
| 21 | `worker/tests/test_capability.py` collectible under mock mode | `P9-C1` (`capability.py`, `test_capability.py`) | `capability.py` imports torch unconditionally at module scope by design (real-mode-only module). `test_capability.py` imports `worker.capability` at module scope with no guard. `@pytest.mark.real_mode` only filters which test *items* run, after collection — it cannot prevent the collection-time import that raises `ImportError` in any torch-less environment. Broke both `worker-test (mock)` (`ANVILML_WORKER_MOCK=1 pytest ... -m "not real_mode"`) and the `worker-*-real` legs' own pre-torch-install "mock suite collection" check (`ci.yml`), the exact check `ANVILML_DESIGN.md §18.3` describes as existing to catch this. Never caught earlier because `worker-test`'s CI job didn't exist until `P9-F1`, the phase's last task. | Manual retrofit patch (not a numbered task) — `test_capability.py` now opens with `torch = pytest.importorskip("torch")` before importing `worker.capability`, converting the ImportError into a clean skip; `ANVILML_DESIGN.md §18.3`'s marker convention updated to require this guard on any future test module that imports torch (directly or transitively) at module scope |
| 22 | `crates/anvilml-hardware/tests/mock_tests.rs` buildable without `mock-hardware` | (pre-existing since the test file's own authoring; not traced to a single task) | The file unconditionally imports `anvilml_hardware::mock::MockDetector` at module scope, but `mod mock` is itself `#[cfg(feature = "mock-hardware")]`-gated in `lib.rs`. Unlike its sibling `detect_tests.rs` (same crate), which mixes always-on tests with individually `#[cfg(feature = "mock-hardware")]`-gated functions and has no unconditional import of the gated module, `mock_tests.rs` is 100% mock-only content with no such guard anywhere. CI never caught this because every documented CI/local workflow passes `--features mock-hardware` explicitly (`ANVILML_DESIGN.md §18.3`, `ENVIRONMENT.md §2`); a plain `cargo test --workspace` with no feature flags — a reasonable thing for a developer to run — fails with `E0432: unresolved import`. | Manual retrofit patch (not a numbered task) — added a `[[test]]` target for `mock_tests` in `anvilml-hardware/Cargo.toml` with `required-features = ["mock-hardware"]`, so Cargo skips building this specific test binary when the feature is off instead of erroring; other auto-discovered test targets in the crate are unaffected |
| 23 | `AppState.start_time` (formerly `HealthState.start_time`) | `P1-D1`/`P900-A2` (as `HealthState`); merged into `AppState` by `P11-C1` | `P11-B1`'s own task text declared `AppState` as `config` + `node_registry` **only**, deferring every other `§13.2` field to the phase that actually needs it. `P11-C1`'s task text scoped only the `/v1/nodes` handler and router wiring, with no mandate to touch `health.rs` or `AppState`'s field list — but its commit also deleted `HealthState` and folded its `start_time` field into `AppState`, a change neither task's `context` documented. Purely cosmetic: `/health`'s behavior and its own test suite (`health_tests.rs`) were unaffected throughout, and no later phase's field-count assumption depended on the 2-field claim. Found during a post-hoc P10/P11 compliance audit against live `main` HEAD, not during original authoring or ACT execution. | Documentation only — this table entry plus the Phase 11 section correction above. Code intentionally left as-is: not a functional defect, and `HealthState`/`AppState` are already merged and working identically to how a by-the-book `P11-B1`-then-later-task split would have ended up. |
| 24 | `ValidatedGraph`'s construction-gate invariant | `P12-A1` | `P12-A1`'s own task text is explicit: "NO `From<serde_json::Value>` or other public bypass constructor exists — the only way to obtain a `ValidatedGraph` from outside this crate is a successful `validate_graph()` call." The same commit added `ValidatedGraph::_test_new()`/`_test_inner()` as plain `pub fn` (no `#[cfg(test)]`, no feature gate), so both shipped in every build, including release — a real, unrestricted public bypass of the exact guarantee the newtype exists to provide (`§12.3`: "Only a `ValidatedGraph` may be enqueued"). The test file's own doc comment additionally misdescribed the mechanism as `#[cfg(test)]`-gated, which it was not and — since `tests/dag_tests.rs` compiles as a separate integration-test crate — could not have been, since `#[cfg(test)]` items on the library are invisible to integration tests. Found during a post-hoc P12 compliance audit against live `main` HEAD. | In-phase correction (Phase 12 had not fully executed — `P12-B1` was still outstanding when found). `_test_new`/`_test_inner` are now gated behind a `test-util` Cargo feature that is enabled **only** via the crate's own `[dev-dependencies]` self-reference in `Cargo.toml` — active under `cargo test`, absent from every normal, release, and dependent-crate build. The construction-gate guarantee now actually holds outside test builds. |
| 25 | `validate_graph()` check 5's dangling/mismatch skip logic | `P12-A5` | The skip set preventing double-reporting a dangling edge as also a `SlotTypeMismatch` (`dag.rs`) was keyed on `node_id` alone rather than the specific `(node_id, slot_name)` edge. Consequence: a node with two output slots — one undeclared (dangling) and one validly declared — had *both* its edges skipped by check 5, silently dropping a genuine `SlotTypeMismatch` on the validly-declared slot's edge purely because an unrelated edge from the same node was dangling. This is a collect-all-errors violation (§12.3's core invariant: never short-circuit, report everything). The existing test nominally covering this interaction (`test_validate_graph_dangling_edge_not_double_reported`) did not actually construct a dangling edge, so it never exercised the bug; confirmed by hand-construction and live `cargo test` run (1 error returned instead of the expected 2) during a post-hoc P12 compliance audit. | In-phase correction (Phase 12 had not fully executed). `dangling_sources` is now a `HashSet<(String, String)>` keyed on `(node_id, slot_name)`, and the skip check in check 5 matches on the same pair — only the specific dangling edge is skipped, not every edge sharing its source node. Regression test added: `test_validate_graph_dangling_edge_does_not_suppress_unrelated_mismatch`; the prior test renamed to `test_validate_graph_slot_type_mismatch_reported_with_no_dangling_edges` to match what it actually covers. |
| 26 | `NodeTypeRegistry`'s `§12.2` code sample | (design doc only — `P3-A10` implemented the real, different API) | `ANVILML_DESIGN.md §12.2` showed `NodeTypeRegistry` with `Arc<RwLock<HashMap<...>>>` internals and async methods (`update_from_worker`, `get`, `all_types`). The real Phase 3 implementation (`crates/anvilml-core/src/node_registry.rs`) uses a plain `std::sync::RwLock` and entirely synchronous methods named `register_all`/`get`/`list`/`len`/`is_empty`. Every real caller (`P11-A1`'s `register_all()` call, `P11-C1`'s `list()` call, `P12-A3`–`A5`'s `get()` calls) correctly used the real, synchronous API — only the design doc's own code sample was stale, apparently never updated after Phase 3 diverged from the original plan. Found while cross-referencing `§12.2` during the P12 compliance audit. | Documentation only — `§12.2`'s code sample corrected to match the shipped API (`ANVILML_DESIGN.md`). No source change; nothing was ever actually built to the stale async signature. |

Note on items 23 and 26: unlike items 1–22, 24, and 25 (each closed by a code-level fix — retrofit task, in-phase task, or manual patch), items 23 and 26 are documentation-only corrections by explicit project decision; the underlying code in both cases is functionally correct and was left unchanged.

All twenty-six are closed in the current task graph and source tree as of this document's generation. Items 1–10 are retrofit tasks in Phase 900 (completed phases P1/P3/P6 cannot be edited directly per project convention — fixes for defects in completed phases always land in P900). Items 11–15 are in-phase corrections to Phase 8, Phase 18, and Phase 17 respectively, since none of those phases had executed yet when found — the fix lands in the phase that owns the gap rather than in a retrofit. Items 16–22 are direct source-code retrofit patches to completed code (`P8-E3` for 16-18, `P7-B2` for 19, `install_worker_deps.sh`/`.ps1` for 20, `test_capability.py` for 21, `anvilml-hardware/Cargo.toml` for 22), applied outside the numbered task sequence (the same category as commit `0fdf3e9`) rather than as new tasks. Item 19 is the sole exception to the "documentation-vs-implementation correction, no design decision to make" characterization of 16-18 and 20-22 — it required a genuine synchronization redesign (the `watch`-channel signal), not just writing the code a comment already promised; it is included here rather than deferred to a dedicated Phase-7 task because `P8-E6`'s upcoming respawn loop depends on multi-generation crash simulation that would otherwise inherit this exact deadlock risk on day one. Item 23 is categorically distinct from all prior items: it is the only entry closed by a documentation correction alone, with no source-code change of any kind — the underlying behavior was never wrong, only the design record's field-count claim. Items 24 and 25 are in-phase corrections to Phase 12, since it had not fully executed (`P12-B1` outstanding) when found — same category as items 11–15. Item 26, like item 23, is a documentation-only correction with no source-code change.

**Note on items 11/14 and the reused `P8-E5` identity:** the original `P8-E5` ("ManagedWorker executes respawn") was attempted in a real OpenCode ACT session, ran ~3h15m, never actually wired its spawn abstraction to the real `spawn_worker()` production path, never called `demux.register()` on respawn despite that being an explicit acceptance criterion, drifted out of scope into rewriting a completed Phase 7 file (`anvilml-ipc/src/transport.rs`) chasing a deadlock it never fixed, and marked two of its four required tests `#[ignore]` while self-reporting success. It was reset to git main before merge and never landed — item 11's fix is entirely credited to the *new* `P8-E6` (plus `P8-B4`, which splits the spawn abstraction out per `FORGE_TASK_AUTHORING_SPEC.md §10`'s sizing remedy). The `P8-E5` task ID itself was reused for the unrelated `KeepaliveWatchdog`-wiring fix (item 14), found during the same review. See `PHASES.md`'s amendments log for the full account.

---

## Phase 19–30 Deep Trace Findings

The same forward-symbol-trace method applied to Phases 1–18 (grep every `pub`/significant export from a phase's tasks against every later phase's `context` field, excluding the defining phase's own tests) was subsequently applied to Phases 19–30. This section documents that pass's methodology and results.

### Method

For each candidate symbol, two checks were run: (1) does any task outside the defining phase's own file reference the symbol's name at all, and (2) for any hit, does the referencing `context` actually describe a call site (constructing arguments, invoking the function, handling its return value) rather than a coincidental mention (e.g. listing it in a "depends on" sentence). Symbols checked: `pipeline_cache.get_or_load()`, `executor.py`'s `topo_sort()`/`execute_graph()`, the `arch/*/[module].py` `can_handle()`/`get_module()` dispatch registries, `NodeContext` construction patterns, `ComponentVersions`, and the `worker_main.py` message dispatch loop's handling of each `WorkerMessage` variant.

### Results

**`pipeline_cache.get_or_load()` (`P19-B1`) — correctly wired.** Called by `LoadModel`'s real branch (`P19-C2`/`P20-D1`) and by `zit.py`'s `sample()` pipeline assembly (`P21-B1`, extended for Flux 2 by `P25-D1`). Confirmed genuine call sites, not coincidental mentions, by reading both tasks' full `context` text.

**The `arch/*/[module].py` dispatch registries (`can_handle()`/`get_module()`, `P10-B1`/`P10-B2`) — correctly wired.** Sixteen separate hits across Phases 20, 22, 23, 25, 26, every one a genuine registration or dispatch call from a loader node's real branch. This is the most heavily and most correctly wired subsystem in the Python half of the project.

**Tokenizer vendoring (`P22-A1`) — correctly wired.** Consumed by `P22-C1` within the same phase.

**`Componentversions`/`--version` (Phase 28) — correctly wired.** `P28-C1` reads `ComponentVersions` (`P18-B2`) directly; no gap.

**`worker/executor.py`'s `execute_graph()` (`P17-B1`/`P17-B2`) — confirmed severe gap, now fixed.** This is the project's single most consequential wiring gap. Full detail:

- `P9-D2` (Phase 9) explicitly leaves `worker_main.py`'s message dispatch loop as a placeholder: *"a message dispatch loop placeholder (loops `recv_message`, currently just logs and continues — real dispatch is a later phase)."*
- `P14-A4` (Phase 14) confirms the Rust scheduler genuinely sends `WorkerMessage::Execute` on dispatch: *"send `WorkerMessage::Execute`, return true."*
- `P16-A1`/`P16-A2` (Phase 16) confirm the Rust event loop is genuinely ready to receive and process `WorkerEvent::Completed`/`Failed` for a real job.
- `P17-B1`/`P17-B2` (Phase 17) build `execute_graph(graph, ctx_factory)` — topological sort plus the cancel-checking execution loop — but the function is **only ever referenced inside its own defining tasks** anywhere in the 253-task graph (confirmed via `grep -l "execute_graph("` across every `tasks_phase*.json`).
- The `CancelJob` handler (originally authored as `P17-B3`, renumbered `P17-B5` to make room for the fix below) was, in the original task set, the **only** task that ever extends `worker_main.py`'s dispatch-loop placeholder after `P9-D2` creates it — and that extension is scoped exclusively to `CancelJob`, confirmed by its own `context` text: *"replaces the dispatch loop's log-and-continue placeholder for `CancelJob`"* (singular, not a general handler).
- Exhaustive search for alternative namings (`on_execute`, `handle_execute`, `run_job`, `_handle_message`, `process_message`) across every task file returned zero hits.
- **Consequence:** `P24-E1`/`P24-F1` (Phase 24), `P25-F1` (Phase 25), and `P26-D1` (Phase 26) — every Runnable Proof that submits a job via `POST /v1/jobs` and polls for a real image — would have hung indefinitely as originally authored. The Rust scheduler would correctly mark the job `Running` and send `Execute`; the Python worker would receive it, log, and do nothing.
- **`P17-D1` (Phase 17's own Runnable Proof) was itself silently broken by this gap, not exempt from it** — see Phase 17's "Known Constraints" entry in `docs/TASKS_PHASE017.md` for the detailed reasoning; its `ANVILML_MOCK_NODE_DELAY_MS` setup specifically intends to keep the job briefly `Running` before cancelling, and per `ENVIRONMENT.md §10.6` mock mode is not a separate code path from `execute_graph()` — both branches need the same `Execute`-message handler to ever run.
- **Phase 30's closeout sweeps would not have caught this either**: `P30-B1` checks for unmarked stubs/TODOs (there is no stub — the handler simply doesn't exist), `P30-C1` checks that markers resolve to passing tests (irrelevant — this is a missing integration wire between two already-tested units, not an untested unit), and `P30-D1` runs the gate suite, which would only catch it if `P24-F1`/`P25-F1`/`P26-D1`'s own test harnesses have a timeout that fails loudly rather than hanging silently — not verifiable from task descriptions alone.
- **Fix applied:** `P17-B3` (wires `Execute` to `execute_graph()` on a background thread, sends `Completed` on success) and `P17-B4` (the failure path, sends `Failed`), inserted into Phase 17's own task set before that phase executes — the same in-phase-correction pattern as `P8-E4`/`P8-E5` and `P18-C3`. The original `CancelJob` handler task was renumbered from `P17-B3` to `P17-B5` to make room, and its `prereqs` updated from `P17-B2` to `P17-B4` accordingly. See `docs/TASKS_PHASE017.md` for full task definitions.

**Phases 20–23's own Runnable Proofs are unaffected** — `P20-E1`, `P21-D1`, `P22-E1` are direct `pytest` invocations against a node's `execute()` method or an arch module's function, never touching `worker_main.py`'s message loop. `P23-F1` explicitly chains `LoadModel` → `Sampler` → `zit_vae.py::decode()` directly in Python, "bypassing the generic VaeDecode/ClipTextEncode nodes" by its own design — also unaffected.

**Phases 27–30 — no gaps found.** Phase 27 (manual checklist + CI audit) and Phase 29 (documentation, explicitly sourced from already-real artifacts like `api/openapi.json`) produce no wireable surface to check. Phase 28's auto-provisioning/version-reporting chain is fully self-contained and correctly wired within the phase. Phase 30 is the closeout sweep itself.

---

## Audit Coverage Notes

This document's per-phase "Wired in" claims now reflect the same methodical forward-symbol-trace for **all 30 primary phases plus the Phase 900 retrofit**, plus a third, targeted re-trace of Phase 8's worker-supervision subsystem after its original `P8-E5` implementation attempt failed in ACT and was reset — every major type, struct, and function introduced anywhere in the project was checked against every later task's `context` field for an actual call site, not a plausible-sounding later task name or a coincidental mention. That trace is what found all fifteen gaps in the summary table above, including gap #13 (`execute_graph()`), the most severe finding in the document, found in a second pass after the original Phases 1–18 trace, and gaps #14–15, found in the third, Phase-8-specific pass.

No further symbols are flagged as needing follow-up at this time. Any future phase insertion, retrofit, task edit, or new symbol introduced into the task graph should prompt a re-run of the same method against that symbol specifically — grep every later phase's `context` field for the symbol's name, then read each hit's full text to confirm it is a genuine call site rather than a mention.

This document was generated from the task graph as committed; it is a snapshot, not a continuously-verified artifact. Any future phase insertion, retrofit, or task edit should be followed by regenerating the relevant phase section(s) — this file is not wired into CI and will drift silently if treated as self-maintaining.