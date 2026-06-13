# PHASES.md — AnvilML v3 Phase Registry

**Document:** `docs/PHASES.md`
**Design authority:** `docs/ANVILML_DESIGN.md`

## Structure

Implementation proceeds as **vertical slices**. Each phase delivers a runnable binary with
one new observable capability verified by an explicit Runnable Proof. `cargo test` and
`cargo clippy` are per-task gates; they are necessary but not sufficient — a phase is only
done when its Runnable Proof passes.

Two rules apply to every phase:
1. **One file or one endpoint per task.** A task implements one source file plus its tests,
   or one HTTP endpoint. Never "all endpoints in a handler."
2. **Every phase ends with a Runnable Proof.** Commands and expected output are documented in
   each `TASKS_PHASE<NNN>.md`.

## Naming conventions

- Task IDs: short phase number without leading zeros: `P1-A1`, `P12-C3`.
- File names: zero-padded three digits: `tasks_phase001.json`, `TASKS_PHASE001.md`.
- A task's `prereqs` may reference IDs from any earlier phase.

## Phase Map

| Phase | Name | Capability delivered | Runnable Proof (summary) |
|------:|------|---------------------|--------------------------|
| 000 | Repository Preamble | Repo hygiene: `.gitignore`, `.gitattributes`, `rust-toolchain.toml`, workspace crate skeletons, CI workflow stubs | `rustc --version` matches toolchain; `cargo build --workspace --features mock-hardware` exits 0 |
| 001 | Walking Skeleton | axum server binds and serves `GET /health` | `curl http://127.0.0.1:8488/health` → 200 `{"status":"ok","version":"0.1.0","uptime_s":N}` |
| 002 | Config & Graceful Shutdown | Layered config (toml → env → CLI); `--host`/`--port` flags; Ctrl-C/SIGTERM exits 0 | Start with `--port 9000`; `curl :9000/health` → 200; Ctrl-C exits 0 |
| 003 | Core Domain Types | All `anvilml-core` types, `AnvilError`, `ServerConfig`; stub `/v1/system/env` | `curl /v1/system/env` → 200 `EnvReport` stub; `cargo test -p anvilml-core` exits 0 |
| 004 | Hardware Detection | SDK-free GPU detection (Vulkan primary); `GET /v1/system` returns real `HardwareInfo` | `curl /v1/system` → 200 with at least one device; `cargo test -p anvilml-hardware` exits 0 |
| 005 | SQLite Persistence | DB open/migrate; ghost-job reset; registry `db.rs` and `seed_loader.rs` | DB file created on first run; all tables present; restart resets ghost jobs |
| 006 | Model Registry | Scanner + store; `GET /v1/models`, `GET /v1/models/:id`, `POST /v1/models/rescan` | Place `.safetensors` in `models/diffusion/`; `curl /v1/models` lists it with correct `kind` and `dtype` |
| 007 | WebSocket Event Stream | `/v1/events` WS upgrade; `EventBroadcaster`; `SystemStats` tick every 5 s | `websocat ws://127.0.0.1:8488/v1/events` receives `system.stats` frames continuously |
| 008 | ZeroMQ IPC Transport | `anvilml-ipc`: `RouterTransport` bind/send/recv; msgpack roundtrip; 1000-trip stress test | `cargo test -p anvilml-ipc` exits 0; stress test completes 1000 roundtrips with 0 errors |
| 009 | Worker Spawn & Handshake | `WorkerPool` spawns mock Python worker; IPC `Ready`; keepalive; `GET /v1/workers` | `curl /v1/workers` shows `status: "Idle"` within 30 s of server start |
| 010 | Worker Crash Recovery | Watchdog detects exit; Dead → Respawning → Idle; crashed job marked Failed | Kill worker PID; `/v1/workers` shows Dead then Idle within 10 s |
| 011 | Dynamic Node Registry | Worker reports `NodeTypeDescriptor[]` in `Ready`; `NodeTypeRegistry`; `GET /v1/nodes` | `curl /v1/nodes` returns JSON array containing all 9 baseline node type names |
| 012 | Graph Validation | DAG validator using dynamic registry; slot-type checking; `POST /v1/jobs` 422 on invalid | `POST /v1/jobs` unknown type → 422; cycle → 422; valid graph → 202 |
| 013 | Job Queue & Persistence | Valid graph → 202 + `job_id`; SQLite persistence; `GET /v1/jobs`, `GET /v1/jobs/:id` | `POST /v1/jobs` → 202; `curl /v1/jobs/:id` → `{"status":"Queued"}` |
| 014 | Dispatch & Mock Execute | VRAM ledger; dispatch loop; mock worker executes graph; Completed | Submit valid mock job; `curl /v1/jobs/:id` reaches `"status":"Completed"` |
| 015 | Artifact Storage | PNG content-addressed store; `GET /v1/artifacts`, `GET /v1/artifacts/:hash` | After completed job, `curl /v1/artifacts/:hash` returns `image/png` bytes |
| 016 | Live Job Events | `Progress`, `ImageReady`, `Completed`, `Failed` over WebSocket | `websocat` receives job lifecycle events in sequence during mock job run |
| 017 | Cancellation | `POST /v1/jobs/:id/cancel`; Queued instant; Running cooperative IPC; 409 on terminal | Cancel queued job → 202 + `Cancelled`; cancel running job → 202; worker stops cooperatively |
| 018 | ZiT Generic Nodes | Real Python nodes: `LoadModel`, `LoadVae`, `LoadClip`, `ClipTextEncode`, `EmptyLatent`, `Sampler`, `VaeDecode`, `SaveImage`; ZiT FP8 arch dispatch | Real ZiT FP8 workflow produces a PNG artifact |
| 019 | Flux 2 Klein Nodes | `arch/flux.py`; Qwen3 8B FP8-mixed text encoder; same generic nodes run Flux graph | Real Flux 2 Klein FP8 workflow produces a PNG artifact |
| 020 | End-to-End Validation | CI green on all 6 jobs; crash recovery integration test; both model families validated | All 6 CI jobs pass; ZiT + Flux each produce a real PNG on target hardware |
| 021 | Auto-Provisioning & Versions | Background venv auto-provision; `GET /v1/system/versions`; `ComponentVersions` type | `curl /v1/system/versions` returns rust/python/torch/worker version strings |
| 022 | Release Packaging | `cargo build --release`; GitHub Release workflow; SHA256SUMS; install scripts validated | Release zip produced; `sha256sum --check SHA256SUMS` exits 0 |
| 023 | Documentation Site | mdBook site; API reference; Node SDK guide; Configuration reference; Operations guide | `mdbook build` exits 0; all chapters render without broken links |

## Critical path notes

Phase 008 (ZeroMQ IPC) is the highest-risk phase. No subsequent phase begins until the
1000-trip stress test passes on both Linux and the Windows cross-check target. This gate
is mandatory in Phase 008's Runnable Proof and is checked before any Phase 009 task begins.

Phase 011 (Dynamic Node Registry) permanently replaces any compile-time node type list.
No task written after Phase 011 may introduce a hardcoded node type name outside of test
fixtures — any such occurrence is a structural defect requiring a retrofit phase.
