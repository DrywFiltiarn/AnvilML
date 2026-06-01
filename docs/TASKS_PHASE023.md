# Tasks: Phase 023 — Auto-Provisioning & Workspace Release Version

| Field | Value |
|-------|-------|
| Phase | 023 |
| Name | Auto-Provisioning & Workspace Release Version |
| Milestone group | Distribution readiness |
| Depends on phases | 1-22 |
| Task file | `forge/tasks/tasks_phase023.json` |
| Tasks | 9 |

## Overview

Phase 23 makes AnvilML self-provisioning, adds version introspection, and establishes the product release version. On a clean machine with no Python venv, the binary now binds and serves the API immediately, then installs the worker dependencies in the **background** — surfacing live state (`NotStarted → InProgress → Ready / Failed`) over `GET /v1/system/env` and a new `provisioning.progress` WebSocket event — and brings the worker pool up automatically once ready. Job submission returns 503 until provisioning completes. This is the prerequisite for a distributable build: an end user can unzip and run without manually building a venv first. It also exposes `GET /v1/system/versions`, which reports the AnvilML release version alongside the individual version of every crate (backend, core, hardware, registry, ipc, worker, scheduler, server, openapi) and the Python worker — so a running instance can state exactly what it is built from. The phase also adds a workspace-level **release version** (`[workspace.package] version`) that is independent of the individual crate versions — crates keep their own versions and may diverge; the workspace version is the single value that, when bumped, drives a release in Phase 24.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P23-A1 | `Cargo.toml ([workspace.package])` | anvilml: workspace release-version field (independent of crate versions) |
| P23-A2 | `crates/anvilml-core/src/types/worker.rs` | anvilml-core: ProvisioningState type and EnvReport extension |
| P23-A3 | `crates/anvilml-core/src/types/events.rs` | anvilml-core: ProvisioningProgress WS event variant |
| P23-A4 | `crates/anvilml-worker/src/provisioner.rs` | anvilml-worker: provisioner module runs install_worker_deps as a child process |
| P23-A5 | `backend/src/main.rs` | anvilml: first-run detection triggers background provisioning |
| P23-A6 | `crates/anvilml-server/src/handlers/jobs.rs` | anvilml-server: job submission 503 while provisioning |
| P23-A7 | `docs/PROOF_phase023.md` | anvilml: document provisioning lifecycle proof |
| P23-B1 | `crates/*/src/lib.rs + anvilml-core/src/types/version.rs` | anvilml: per-crate VERSION constants and ComponentVersions aggregate type |
| P23-B2 | `crates/anvilml-server/src/handlers/system.rs` | anvilml-server: GET /v1/system/versions handler |

## Task details

#### P23-A1: anvilml: workspace release-version field (independent of crate versions)

- **Prereqs:** P22-A6
- **Tags:** reasoning

In workspace root Cargo.toml add [workspace.package] version="0.1.0" as the PRODUCT RELEASE VERSION. Crates KEEP their own independent [package] version (do NOT set version.workspace=true) - per-crate versions may diverge from the release version. Expose the workspace release version to the backend (e.g. a tiny anvilml-version crate, or a build script) so GET /health and the launcher banner report the RELEASE version, not the backend crate version. Verify: cargo metadata shows workspace.package.version; curl /health version equals it.

#### P23-A2: anvilml-core: ProvisioningState type and EnvReport extension

- **Prereqs:** P23-A1
- **Tags:** —

Extend src/types/worker.rs (or types/env.rs): add ProvisioningState enum (NotStarted, InProgress{percent:Option<u8>, message:String}, Ready, Failed{reason:String}). Add provisioning: ProvisioningState field to EnvReport (default NotStarted). Update anvilml.toml + ENVIRONMENT.md if any new config key is introduced (none expected). Derive Serialize/Deserialize/Clone/Debug + utoipa::ToSchema. cargo test -p anvilml-core -- provisioning exits 0; config_reference drift test still passes.

#### P23-A3: anvilml-core: ProvisioningProgress WS event variant

- **Prereqs:** P23-A2
- **Tags:** —

Add a WsEvent variant ProvisioningProgress in src/types/events.rs serializing as event='provisioning.progress' with fields {state, percent:Option<u8>, message, timestamp}. Mirrors ProvisioningState. Update the known event list in ARCHITECTURE.md §7. cargo test -p anvilml-core -- events exits 0: assert provisioning.progress serializes with the correct event tag.

#### P23-A4: anvilml-worker: provisioner module runs install_worker_deps as a child process

- **Prereqs:** P23-A3
- **Tags:** reasoning

Create crates/anvilml-worker/src/provisioner.rs: async fn provision(cfg, progress_tx: mpsc::Sender<ProvisioningState>) -> Result<(),AnvilError>. Resolve platform script (install_worker_deps.sh unix / .ps1 windows), spawn as child, stream stdout/stderr lines, map recognizable lines to InProgress{message} (percent may be None), set Ready on exit 0 else Failed{reason}. Cancel-safe; no panic on missing script. cargo test -p anvilml-worker --features mock-hardware -- provisioner exits 0 (fake script fixture). Also pass: cargo check --target x86_64-pc-windows-gnu --features mock-hardware.

#### P23-A5: anvilml: first-run detection triggers background provisioning

- **Prereqs:** P23-A4
- **Tags:** reasoning

In backend startup: after preflight, if venv missing OR torch import fails AND ANVILML_WORKER_MOCK unset, set EnvReport.provisioning=InProgress and tokio::spawn provisioner::provision in the background - the server MUST bind immediately and stay responsive. Forward each ProvisioningState to AppState.env_report and broadcast ProvisioningProgress. On Ready (re)spawn the WorkerPool; on Failed leave provisioning=Failed. Verify: with no venv, cargo run binds immediately; /v1/system/env shows InProgress then Ready; workers come up after Ready.

#### P23-A6: anvilml-server: job submission 503 while provisioning

- **Prereqs:** P23-A5
- **Tags:** —

In handlers/jobs.rs submit_job: if EnvReport.provisioning is InProgress or NotStarted, return 503 with body {error:'provisioning', message, request_id}; if Failed return 503 {error:'workers_unavailable'}. Only accept jobs when Ready (or when ANVILML_WORKER_MOCK=1, which bypasses provisioning entirely). Wire the check ahead of validation. Verify: during provisioning curl POST /v1/jobs -> 503 provisioning; after Ready -> 202.

#### P23-A7: anvilml: document provisioning lifecycle proof

- **Prereqs:** P23-A6
- **Tags:** —

No code. Write docs/PROOF_phase023.md: exact steps to observe background provisioning on a clean machine - delete venv; run binary (no mock); show server answering /health immediately; websocat /v1/events streaming provisioning.progress; /v1/system/env transitioning NotStarted->InProgress->Ready; POST /v1/jobs returning 503 then 202 once Ready. Complete when a human observes the full lifecycle with the API responsive throughout.

#### P23-B1: anvilml: per-crate VERSION constants and ComponentVersions aggregate type

- **Prereqs:** P23-A1
- **Tags:** —

In EACH workspace crate (anvilml-core,-hardware,-registry,-ipc,-worker,-scheduler,-server,-openapi) add to lib.rs (main.rs for openapi): pub const VERSION: &str = env!("CARGO_PKG_VERSION");. In anvilml-core create src/types/version.rs: ComponentVersions struct { anvilml (workspace release version), backend, core, hardware, registry, ipc, worker, scheduler, server, openapi: String } + python_worker: Option<String>. Derive Serialize/Deserialize/Clone/Debug + utoipa::ToSchema. cargo test -p anvilml-core -- version exits 0; config_reference drift test still passes.

#### P23-B2: anvilml-server: GET /v1/system/versions handler

- **Prereqs:** P23-B1
- **Tags:** —

Create handlers/system.rs get_versions(State)->Json<ComponentVersions>: populate anvilml = workspace release version (from P23-A1), backend = its own CARGO_PKG_VERSION, and each crate field from that crate's pub VERSION const (anvilml_core::VERSION etc). python_worker: read worker/__init__.py __version__ at startup into AppState (Option, None if absent). Wire GET /v1/system/versions. Add #[utoipa::path] so it lands in openapi.json. Verify: curl /v1/system/versions returns 200 with anvilml + every crate version and python_worker.


## Runnable Proof

On a clean checkout with no venv, confirm the API is responsive while dependencies install in the background.

```bash
rm -rf ./venv
./target/release/anvilml --no-browser          # real mode (no ANVILML_WORKER_MOCK)
# immediately, in another terminal — API is already up:
curl -s http://127.0.0.1:8488/health           # 200 right away
curl -s http://127.0.0.1:8488/v1/system/env | jq .provisioning   # InProgress{...}
websocat ws://127.0.0.1:8488/v1/events          # shows provisioning.progress frames
# submit during provisioning:
curl -s -X POST .../v1/jobs -d @valid_zit_job.json -H 'content-type: application/json' -i | head -1   # 503 provisioning
# wait for completion:
curl -s http://127.0.0.1:8488/v1/system/env | jq .provisioning   # Ready
curl -s http://127.0.0.1:8488/v1/workers | jq '.[0].status'      # idle
```

Also confirm `curl -s .../health | jq .version` equals the `[workspace.package] version` in the root `Cargo.toml`. Then `curl -s http://127.0.0.1:8488/v1/system/versions | jq` should list `anvilml` (the release version) plus a version for each crate and `python_worker`. Phase done when the API stays responsive throughout a real background provisioning cycle, state transitions are observable, jobs are gated until Ready, and `/health` reports the workspace release version.
