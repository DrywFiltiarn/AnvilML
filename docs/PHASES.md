# PHASES.md — AnvilML Phase Registry (Rebuild — Vertical Slices)

**Location:** `forge/docs/PHASES.md`
**Supersedes:** the previous 10-phase horizontal-layer task set (archived).
**Design authority:** `ANVILML_DESIGN.md` Rev 3.

## Why this structure differs from the previous one

The previous task set was organised by **architectural layer** (core → ipc → hardware → registry → workers → scheduler → server → launcher). Every layer was unit-tested, but the binary did nothing runnable until the second-to-last phase. Two failures resulted: (1) tasks touched too much code at once and exhausted the agent's context during debugging, and (2) there was no observable milestone — only green unit tests — until the very end.

This rebuild is organised by **vertical slice**. Phase 1 produces a binary you can start and curl. Every subsequent phase thickens that running binary with one new observable capability. Two hard rules:

1. **One module/file per task.** A task implements one file (plus its test) or one endpoint. Never "all job endpoints" — each endpoint is its own task. This keeps the code touched per task small enough that a debug session stays well under the context limit.
2. **Every phase ends with a Runnable Proof.** Each phase document ends with explicit commands (curl, browser, CLI) and the observable result that proves the phase works by *running the binary*, not by running `cargo test`.

`cargo test` and `cargo clippy` remain per-task gates. They are necessary but no longer sufficient: a phase is only done when its Runnable Proof passes.

## Naming conventions

- Task IDs: short phase number, no leading zero: `P1-A1`, `P12-C3`.
- File names: zero-padded three digits: `tasks_phase001.json`, `TASKS_PHASE001.md`.
- A task's `prereqs` may reference task IDs in any earlier phase (merged-DAG resolution, Forge `--phase` flag).

## Phase Map

| Phase | Name | Vertical slice delivered | Runnable proof (summary) |
|------:|------|--------------------------|--------------------------|
| 000 | Repository Preamble | `.gitignore`, `.gitattributes`, pinned `rust-toolchain.toml` (1.95.0) | `git status` ignores `target/`/`*.db`; `rustc` is 1.95.0 |
| 001 | Walking Skeleton | Workspace + binary that binds axum and serves `/health` | `curl /health` → 200 `{status,version,uptime_s}` |
| 002 | Config & Graceful Shutdown | Layered config load; Ctrl-C / SIGTERM clean exit | Start with custom `--port`; Ctrl-C exits 0 cleanly |
| 003 | Core Domain Types | All `anvilml-core` types + error model, behind `/v1/system/env` stub | `curl /v1/system/env` → 200 `EnvReport` (stub values) |
| 004 | Hardware Detection | Real device detection surfaced via REST | `curl /v1/system` → 200 real `HardwareInfo` |
| 005 | SQLite Persistence | DB opens, migrates, ghost-job reset on startup | DB file created; tables exist; restart resets ghosts |
| 006 | Model Registry | Scan model dirs, list/get models via REST | Drop a file in `models/`; `curl /v1/models` lists it |
| 007 | WebSocket Event Stream | `/v1/events` WS + `system.stats` tick | Browser/`websocat` sees `system.stats` every 5 s |
| 008 | IPC Framing | `anvilml-ipc` messages + framing, round-trip tested via a CLI probe | `cargo run -p ipc-probe` round-trips a frame |
| 009 | Worker Spawn & Handshake | Spawn mock Python worker; Ping→Pong; status via REST | `curl /v1/workers` shows a live Idle worker |
| 010 | Worker Crash Recovery | Watchdog: kill worker → Dead → respawn → Idle | Kill worker PID; `/v1/workers` shows respawn to Idle |
| 011 | Graph Validation | DAG validator + `KNOWN_NODE_TYPES`; reject bad graphs | `POST /v1/jobs` bad graph → 422 with error list |
| 012 | Job Submission & Queue | Submit valid job → Queued → persisted; list/get | `POST /v1/jobs` → 202; `curl /v1/jobs/:id` → Queued |
| 013 | Dispatch & Execute | Scheduler dispatches to worker; mock returns image | Submit job → `curl /v1/jobs/:id` reaches Completed |
| 014 | Artifact Storage | Save PNG, content-addressed; serve via REST | After a job, `curl /v1/artifacts/:hash` returns a PNG |
| 015 | Live Job Events | `job.*` WS events through full lifecycle | WS shows queued→started→image_ready→completed |
| 016 | Job Cancellation | Cancel queued + running (cooperative) | `POST /v1/jobs/:id/cancel` → job reaches Cancelled |
| 017 | Job & Artifact Management | Delete single/bulk jobs + artifacts | `DELETE /v1/jobs/:id` → 204; bulk clear works |
| 018 | Worker Restart API | `POST /v1/workers/:id/restart`; env health repair | Restart a worker via REST; it re-initialises |
| 019 | Frontend Serving | Local / Remote / Headless modes; SPA fallback | Browser loads served frontend (or warning page) |
| 020 | OpenAPI & Launcher Polish | `openapi.json` generation; browser auto-open; CI diff gate | `cargo run -p anvilml-openapi`; binary opens browser |
| 021 | Real Python Worker — ZiT | Replace mock: real ZiT pipeline nodes | ZiT model → real generated image end-to-end |
| 022 | Real Python Worker — SDXL & Hardening | SDXL nodes; OOM trap; cross-platform CI green | SDXL model → image; full CI green both OSes |
| 023 | Auto-Provisioning & Workspace Release Version | Background worker-dep install with live state; workspace release version | Clean run: API up immediately, `provisioning.progress` → Ready |
| 024 | Release Packaging & Automation | Version-bump → auto-tag → signed cross-platform GitHub Release zips | Bump workspace version → published Linux+Windows zips + SHA256SUMS + GPG |
| 025 | Documentation Site | Full mdBook user/operator manual, auto-deployed to GitHub Pages | `mdbook build docs-site`; site publishes to Pages |

> **Phase numbers 900–999 are reserved for retrofit phases.** See the Retrofit Phases section below.

## Milestone groupings (for reporting)

| Group | Phases | Theme |
|-------|--------|-------|
| Pre-flight | 000 | Repository hygiene: ignore rules, line endings, pinned toolchain |
| Runnable server skeleton | 001–002 | Binary starts, configurable, shuts down cleanly |
| Observable system state | 003–007 | Hardware, DB, models, live stats all visible via REST/WS |
| Worker lifecycle | 008–010 | Workers spawn, handshake, recover from crashes |
| End-to-end generation (mock) | 011–017 | Full job flow with mock worker: submit → image → manage |
| Production surface | 018–020 | Worker admin, frontend, OpenAPI, launcher polish |
| Real inference | 021–022 | Real ZiT + SDXL pipelines; hardening; CI green |
| Distribution readiness | 023–025 | Self-provisioning on first run; signed cross-platform releases |
| Retrofit | 900–999 | Post-hoc corrections and adjustments inserted between primary phases |

## Retrofit Phases (900–999)

Phase numbers 900–999 are reserved exclusively for retrofit, correction, and adjustment work that must be inserted between already-executed primary phases. They are never part of the original development plan — they are authored on demand when a gap is identified in the committed codebase (for example: a new rule added to `FORGE_AGENT_RULES.md` after earlier phases ran, a production bug that must be fixed before the next primary phase begins, or a cross-cutting concern that spans multiple already-completed files).

**This section does not enumerate every retrofit phase.** Retrofit phases are self-documenting: each has its own `tasks_phase9NN.json` and `TASKS_PHASE9NN.md` which describe what was retrofitted and why. To see what retrofit phases exist, list the `9xx` entries in `.forge/tasks/`. PHASES.md is not updated when a new retrofit phase is added — only when a retrofit phase is first introduced (to record the convention) or when a retrofit phase affects the Registered Projects list.

**Execution order** is determined entirely by `prereqs`, not by the phase number. A retrofit phase inserted between primary phases P and Q must have its first task prereq the terminal task of P, and the first task of Q must prereq the terminal task of the retrofit phase.

**Known retrofit phases:**

| Phase | Name | Inserted between | Addresses |
|------:|------|-----------------|-----------|
| 900 | Logging Retrofit | 008 → 009 | Retrofit `FORGE_AGENT_RULES.md §11` logging obligations to phases 000–008 |

---

## Dependency principle

Phase N's first task depends on the terminal task of phase N−1. Within a phase, tasks form a short chain or fan-out. Because the binary is always runnable, later phases that add an endpoint depend only on the specific earlier tasks that built the state they read — not on the entire prior phase. Cross-phase prereqs are explicit in each task's `prereqs`.

## Registered project

`anvilml` → AnvilML repository. `bloomeryui` and `sindristudio` remain out of scope.