# AnvilML

[![CI](https://github.com/DrywFiltiarn/AnvilML/actions/workflows/ci.yml/badge.svg)](https://github.com/DrywFiltiarn/AnvilML/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust 1.96.0](https://img.shields.io/badge/rust-1.96.0-orange.svg)](rust-toolchain.toml)
[![Python 3.12](https://img.shields.io/badge/python-3.12-3776AB.svg)]()

AnvilML is the headless Rust backend binary (`anvilml` / `anvilml.exe`) of the
**SindriStudio** local AI image-generation platform. It is the engine room: a
single binary that supervises GPU/CPU worker processes, runs an offline,
ComfyUI-style node-graph image generation pipeline, and exposes the whole thing as
one versioned REST + WebSocket API — with no UI of its own and no dependency on
any external service.

> **Status note:** this README describes AnvilML's complete, designed end state —
> the system as specified in [`docs/ANVILML_DESIGN.md`](docs/ANVILML_DESIGN.md),
> not only the part that exists today. The project is implemented incrementally,
> phase by phase, against this same design throughout. See
> [Implementation Status](#implementation-status) for exactly what currently runs.

---

## Table of Contents

- [What AnvilML Is](#what-anvilml-is)
- [How It Works](#how-it-works)
- [The API](#the-api)
- [Architecture](#architecture)
- [Repository Layout](#repository-layout)
- [Implementation Status](#implementation-status)
- [Getting Started](#getting-started)
- [Configuration](#configuration)
- [Testing](#testing)
- [CI](#ci)
- [Documentation Index](#documentation-index)
- [Contributing](#contributing)
- [License](#license)

---

## What AnvilML Is

AnvilML is the backend half of **SindriStudio**, a local, offline AI image
generation platform. SindriStudio itself is just a launcher: it starts AnvilML and
**BloomeryUI** (a separate reference frontend) as two independent sibling
processes. AnvilML and BloomeryUI never talk to each other directly or share code —
the only thing connecting them is AnvilML's HTTP/WebSocket API, the same API any
other client could use instead. AnvilML has no idea BloomeryUI exists, and never
will; this separation is a permanent architectural boundary, not a temporary
implementation detail.

What AnvilML actually does:

- **Detects and manages hardware.** On startup (and on demand via a CLI probe), it
  enumerates every GPU in the system — NVIDIA, AMD, and CPU fallback — and
  self-tests each one's real compute capabilities (FP8/FP16/BF16 support, available
  VRAM) directly at the torch level, rather than trusting a hardware ID lookup
  table to guess what a device can do.
- **Owns a model registry.** It scans configured directories for model checkpoint
  files, fingerprints and catalogs them in SQLite, and serves that catalog over the
  API — so a client can ask "what models are available" without ever touching the
  filesystem itself.
- **Spawns and supervises Python worker processes.** One worker per detected GPU,
  or a single CPU worker if no GPU is present. Workers are real OS subprocesses,
  spoken to over a local ZeroMQ IPC channel — not threads, not embedded Python, not
  a shared address space. If a worker dies, AnvilML detects it and respawns it.
- **Executes node-graph image generation jobs.** A client submits a directed graph
  of generation nodes (load a model, encode a text prompt, sample, decode to an
  image, save) as JSON. AnvilML validates the graph, schedules it onto an available
  worker, and streams back progress and the final image — in the same spirit as
  ComfyUI's node graph, but defined and executed entirely server-side.
- **Stores and serves the results.** Generated images are saved as
  content-addressed artifacts (hashed by content, not by job ID), retrievable
  later by any client, independent of which job produced them.
- **Runs completely offline.** No call to Hugging Face Hub, no telemetry, no update
  check, ever, in any code path that ships. Every model AnvilML uses must already
  be a local file on disk.

AnvilML's MVP model support spans three concrete combinations: **Z-Image Turbo +
Qwen3 4B**, **Flux 2 Klein 4B + Qwen3 4B**, and **Flux 2 Klein 9B + Qwen3 8B** — see
`ANVILML_DESIGN.md` Appendix B for the full matrix and example graph.

For the precise contract behind every claim above — type shapes, error semantics,
the IPC wire protocol, the loading contract — `docs/ANVILML_DESIGN.md` is the
single source of truth. This README never tries to restate that detail; it only
gives the shape of the thing.

---

## How It Works

A generation request's life cycle, end to end:

1. **A client submits a job.** `POST /v1/jobs` with a JSON graph — a set of nodes
   (`LoadModel`, `ClipTextEncode`, `Sampler`, `VaeDecode`, `SaveImage`, etc.) and
   the edges connecting their inputs and outputs. AnvilML validates the graph
   (unknown node types, type-mismatched edges, cycles) before accepting it.
2. **The scheduler queues and dispatches it.** A background dispatch loop picks an
   idle worker — preferring one matching the job's device preference, otherwise the
   one with the most free VRAM — and sends the graph to it over IPC. VRAM is
   reserved (advisory, not OS-enforced) for the duration of execution.
3. **A Python worker executes the graph.** Each node runs in topological order
   against real, raw `nn.Module` constructions — there are no `diffusers` or
   `transformers` model classes anywhere in the load path; weights are loaded and
   remapped directly from the checkpoint's tensor keys, ComfyUI-style. The same
   worker code path runs identically whether the underlying hardware is an NVIDIA
   GPU, an AMD GPU, or plain CPU.
4. **Progress streams back live.** A client connected to `GET /v1/events`
   (WebSocket) receives real-time job progress, per-node execution events, and
   periodic system stats — no polling required, though polling `GET /v1/jobs/:id`
   remains available for clients that prefer it.
5. **The result lands as a retrievable artifact.** The finished image is hashed and
   stored once; `GET /v1/artifacts/:hash` serves it to any client, any time after.

Mock and real execution are two equally-maintained code paths, not a mock
standing in for unfinished real work — every node ships both, and CI runs both
independently. See `ANVILML_DESIGN.md §10` and §14 for the full mock/real parity
rule and the node system contract.

---

## The API

AnvilML's REST + WebSocket API is the *only* integration surface — there is no
shared library, no embedded scripting hook, nothing else a client can reach into.
The complete route table (`ANVILML_DESIGN.md §13.4`):

```
GET  /health                          Liveness probe
GET  /v1/system                       Full hardware snapshot
GET  /v1/system/env                   Python environment health + provisioning
GET  /v1/system/versions              Per-component version report
GET  /v1/nodes                        Registered node types + slot descriptors
POST /v1/jobs                         Submit a generation job (graph + settings)
GET  /v1/jobs                         List jobs (?status= ?limit= ?before=)
GET  /v1/jobs/:id                     Get one job's status/result
POST /v1/jobs/:id/cancel              Cancel a queued or running job
DEL  /v1/jobs/:id                     Delete a terminal job + its artifacts
DEL  /v1/jobs                         Bulk clear (?status=)
GET  /v1/models                       List discovered models (?kind=)
GET  /v1/models/:id                   Get one model's metadata
POST /v1/models/rescan                Trigger a model-directory rescan
GET  /v1/workers                      List workers + their status
POST /v1/workers/:id/restart          Restart a worker
GET  /v1/artifacts                    List generated artifacts (?job_id=)
GET  /v1/artifacts/:hash              Retrieve a generated image by content hash
GET  /v1/events                       WebSocket upgrade — live job/system events
```

A generated OpenAPI 3.1 spec (`api/openapi.json`) is the machine-readable version
of this same table, built directly from the server's own type definitions — so it
can never describe a shape the server doesn't actually return. Full request/response
schemas, error codes, and the WebSocket event catalogue are in
`ANVILML_DESIGN.md §13`.

---

## Architecture

```
                    ┌──────────────────────────┐
   REST/WS client ─►│   anvilml (Rust binary)  │
                    │  ┌────────────────────┐  │
                    │  │ HTTP/WS server     │  │
                    │  │ Job scheduler      │  │      ZeroMQ (loopback TCP)
                    │  │ Model registry     │──┼──────────────┐
                    │  │ Artifact storage   │  │              │
                    │  │ (all via SQLite)   │  │              ▼
                    │  └────────────────────┘  │   ┌──────────────────────┐
                    └──────────────────────────┘   │  Python worker(s)    │
                                                   │  one per GPU, or     │
                                                   │  one CPU fallback    │
                                                   │  runs the node graph │
                                                   └──────────────────────┘
```

Rust owns everything except the actual model math: process orchestration, the
public API, persistence, scheduling, and IPC framing. Python owns only node
execution — loading checkpoints, running inference, encoding/decoding tensors.
The two communicate over a local ZeroMQ ROUTER/DEALER socket, never a shared
address space; a crashed worker cannot take the server down with it, and is simply
respawned.

The codebase is a Rust Cargo workspace of nine crates, each owning one concern:

| Crate | Owns |
|:------|:-----|
| `anvilml-core` | Pure domain types, config schema, and the error enum — zero I/O, zero async, depended on by everything else |
| `anvilml-hardware` | GPU/CPU detection, refreshable VRAM snapshots, and pre-spawn capability hints |
| `anvilml-registry` | Scans model directories, persists model metadata to SQLite, and the device-capability hint table |
| `anvilml-artifacts` | Content-addressed PNG artifact storage, persisted to SQLite |
| `anvilml-ipc` | The ZeroMQ ROUTER transport wrapper and wire-protocol message enums between Rust and Python |
| `anvilml-worker` | Spawns Python worker subprocesses, manages their lifecycle, respawns on crash |
| `anvilml-scheduler` | Accepts submitted job graphs, validates them, maintains the job queue, tracks VRAM, dispatches to workers |
| `anvilml-server` | The axum router, every HTTP/WebSocket handler, and the OpenAPI annotations behind them |
| `anvilml-openapi` | Generates `api/openapi.json` from the server's own type definitions |

Alongside the Rust workspace, `worker/` holds the Python process that actually
executes node graphs — a generic node-dispatch engine plus per-architecture
modules (one Python file per model family) that each implement the same four-step
loading contract and the same fixed method names, so adding support for a new
model architecture never requires touching the generic engine.

For the full crate dependency graph, module-by-module file layout, and the worker
lifecycle state machine, see [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — a
navigational summary that always points back to `ANVILML_DESIGN.md` for the
authoritative detail.

---

## Repository Layout

```
AnvilML/
├── backend/                   # The anvilml binary's entry point (main.rs, cli.rs)
├── crates/                    # The nine-crate Rust workspace — see Architecture above
│   ├── anvilml-core/
│   ├── anvilml-hardware/
│   ├── anvilml-registry/
│   ├── anvilml-artifacts/
│   ├── anvilml-ipc/
│   ├── anvilml-worker/
│   ├── anvilml-scheduler/
│   ├── anvilml-server/
│   └── anvilml-openapi/
├── worker/                    # Python worker process — node graph execution
├── scripts/                   # Python venv provisioning (install_worker_deps.sh/.ps1)
├── database/migrations/       # SQL schema migrations (sqlx)
├── api/                       # Generated OpenAPI spec (api/openapi.json)
├── docs/                      # Design, environment, phase, and task documentation
└── .forge/tasks/              # Machine-readable per-phase task definitions (JSON)
```

This is the repository's intended full layout. `worker/` and `scripts/` in
particular don't exist in the repository yet — see
[Implementation Status](#implementation-status) below for exactly what's present
today.

---

## Implementation Status

AnvilML is a ground-up v4 rewrite, built incrementally against the design
described above. The full implementation roadmap — 30 phases, plus one retrofit
phase — is completely designed and task-authored
(see [`docs/PHASES.md`](docs/PHASES.md)). **Authored is not the same as built:**
as of this writing, implementation has reached early **Phase 6** (Model Registry &
Artifacts) of 30. Everything described above is the target; not all of it exists
in code yet.

What's genuinely live today, if you clone and build this repo right now:

| Capability | Status |
|:-----------|:-------|
| `cargo build -p anvilml` / `cargo run -p anvilml` | ✅ Builds and runs the server |
| `GET /health` | ✅ Wired, returns `200` (JSON body pending — see `docs/TASKS_PHASE900.md`) |
| `anvilml hw-probe` (CLI) | ✅ Prints real `HardwareInfo` JSON via the full detector chain |
| Config loading (`anvilml.toml` → env vars → CLI flags) | ✅ Full four-layer precedence |
| Model registry (`anvilml-registry`) | 🚧 In progress — SQLite schema and seed data exist; scanning/querying is mid-implementation |
| `worker/` (Python inference process) | ❌ Does not exist yet — Phase 9+ |
| Job submission, dispatch, generation (`POST /v1/jobs`) | ❌ Does not exist yet — Phase 14+ |
| Most of the route table above (`/v1/nodes`, `/v1/models`, `/v1/artifacts`, `/v1/events`, etc.) | ❌ Not wired yet — later phases |

For the authoritative, per-phase breakdown of what's planned and in what order,
see [`docs/PHASES.md`](docs/PHASES.md). For literal, copy-pasteable proof commands
— bash and PowerShell — of each phase's actual deliverable as it lands, see
[`docs/RUNNABLE_PROOF.md`](docs/RUNNABLE_PROOF.md).

---

## Getting Started

### Prerequisites

- **Rust 1.96.0**, pinned exactly via [`rust-toolchain.toml`](rust-toolchain.toml)
  (installed automatically by `rustup` if you have it; otherwise see
  [rustup.rs](https://rustup.rs)).
- **SQLite 3** command-line tools (`sqlite3`), for inspecting the database directly
  if needed.
- Linux or Windows. macOS and ARM are explicitly out of scope
  (`ANVILML_DESIGN.md` §2.1).

Python 3.12.x and a GPU toolchain (CUDA/ROCm) will be required once the Python
worker exists, but are **not needed yet** — nothing in the repository today
depends on them.

### Build and run

```bash
git clone https://github.com/DrywFiltiarn/AnvilML.git
cd AnvilML
cargo build --release -p anvilml --features mock-hardware
./target/release/anvilml
```

```powershell
git clone https://github.com/DrywFiltiarn/AnvilML.git
cd AnvilML
cargo build --release -p anvilml --features mock-hardware
.\target\release\anvilml.exe
```

By default the server binds `127.0.0.1:8488`. With it running:

```bash
curl http://127.0.0.1:8488/health
```

To probe detected hardware instead of starting the server:

```bash
./target/release/anvilml hw-probe
```

The `mock-hardware` feature enables `ANVILML_MOCK_DEVICE_TYPE` /
`ANVILML_MOCK_VRAM_MIB` env vars for exercising GPU-detection code paths without
real GPU hardware — useful for development and is what CI uses throughout. See
[`docs/RUNNABLE_PROOF.md`](docs/RUNNABLE_PROOF.md) for further worked examples, in
both bash and PowerShell, of every capability as it lands.

---

## Configuration

AnvilML loads configuration through a four-layer precedence chain (lowest to
highest): **compiled-in defaults → `anvilml.toml` → `ANVILML_*` environment
variables → CLI flags**. The checked-in [`anvilml.toml`](anvilml.toml) at the repo
root is the canonical reference — every field documented there is enforced to stay
in sync with the compiled-in defaults by an automated drift test
(`backend/tests/config_reference.rs`).

```bash
# Override the bound port via an environment variable
ANVILML_PORT=9999 ./target/release/anvilml

# Or via a CLI flag
./target/release/anvilml --port 9999

# Or point at a different config file entirely
./target/release/anvilml --config ./my-config.toml
```

The full environment-variable and config-field reference lives in
[`docs/ENVIRONMENT.md`](docs/ENVIRONMENT.md) §3–4.

---

## Testing

```bash
# Full Rust workspace test suite
cargo test --workspace --features mock-hardware

# Format and lint checks (must pass before any commit)
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
```

Real-path implementation is mandatory throughout this project — there is no
mock-only code path anywhere in the design. Once the Python worker exists (Phase
9+), both a mock-mode and a real-mode Python test suite will need to pass
independently; see `docs/ENVIRONMENT.md` §2 and §17 for the full discipline once
that applies.

---

## CI

Every push to `main` runs the matrix defined in
[`.github/workflows/ci.yml`](.github/workflows/ci.yml):

| Job | Runner | Checks |
|:----|:-------|:-------|
| `rust-test` | ubuntu-latest / windows-latest | `cargo fmt --check` (Linux only), clippy, full test suite |
| `worker-test` | ubuntu-latest / windows-latest | Python mock/real test matrix (placeholder until `worker/` exists) |
| `config-drift` | ubuntu-latest | Confirms `anvilml.toml` matches compiled-in defaults |
| `openapi-drift` | ubuntu-latest | Confirms `api/openapi.json` matches the live `ToSchema` definitions |

This matrix grows as implementation proceeds — see `docs/ENVIRONMENT.md` §18 for
the complete, eventual CI job matrix (some jobs listed there, such as the four
split `worker-*-mock`/`worker-*-real` jobs, land once their corresponding phases
are implemented).

---

## Documentation Index

| Document | What it covers |
|:---------|:----------------|
| [`docs/ANVILML_DESIGN.md`](docs/ANVILML_DESIGN.md) | The complete functional & technical design — single source of truth for all types, API shapes, IPC protocol, and contracts |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Navigational summary: where things live, what each crate owns |
| [`docs/ENVIRONMENT.md`](docs/ENVIRONMENT.md) | Build/test/lint commands, env var and config reference, CI matrix |
| [`docs/PHASES.md`](docs/PHASES.md) | The full phase registry — every phase, its task count, and its Runnable Proof summary |
| [`docs/RUNNABLE_PROOF.md`](docs/RUNNABLE_PROOF.md) | Literal, copy-pasteable proof commands (bash + PowerShell) per phase |
| [`docs/TESTS.md`](docs/TESTS.md) | Test file catalogue and conventions |
| [`docs/SUPPORTED_DEVICES_DB.md`](docs/SUPPORTED_DEVICES_DB.md) | The frozen GPU PCI-ID capability reference table |
| `docs/TASKS_PHASE*.md` | Per-phase task definitions, in the same format `.forge/tasks/tasks_phase*.json` encodes machine-readably |

`docs/FORGE_AGENT_RULES.md` and `docs/FORGE_TASK_AUTHORING_SPEC.md` document the
conventions for **The Forge**, the autonomous task-execution agent this project
uses during development — they're not needed to build or run AnvilML itself.

---

## Contributing

This project is developed by its owner using an autonomous local-LLM agent (The
Forge) working through the task definitions in `.forge/tasks/`, plan-then-act, one
task at a time, against the design in `docs/ANVILML_DESIGN.md`. If you're looking
to understand *why* something is built a particular way, the relevant
`docs/TASKS_PHASE0NN.md` document's "Overview" and "Known Constraints and
Gotchas" sections usually explain the reasoning, not just the what.

External contributions: please open an issue describing the change before
submitting a PR, since most implementation order and scope is locked in by the
phase plan in `docs/PHASES.md` — a change that looks small in isolation may
conflict with a dependency a later phase already assumes.

---

## License

[MIT](LICENSE) © 2026 Trinity3D Technologies
