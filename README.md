# AnvilML

[![CI](https://github.com/DrywFiltiarn/AnvilML/actions/workflows/ci.yml/badge.svg)](https://github.com/DrywFiltiarn/AnvilML/actions/workflows/ci.yml)

**The headless Rust + Python inference backend of the SindriStudio project.**

AnvilML is the backend engine: a concurrent **Rust** orchestrator (API, scheduling, state,
worker supervision) paired with an isolated **Python** worker process per GPU that runs the actual
tensor math. It is headless and UI-agnostic — frontends integrate **only** through a versioned
REST + WebSocket API.

> **AnvilML is one component of three.** It is *not* the whole product:
>
> | Component | Role | Location |
> | :-- | :-- | :-- |
> | **SindriStudio** | The root project — a one-click executable that launches the backend and the frontend together. This is what end users run. | separate / root |
> | **AnvilML** | The headless backend inference engine (Rust + Python). | **this repository** |
> | **BloomeryUI** | The reference frontend. | [BloomeryUI repo](https://github.com/DrywFiltiarn/BloomeryUI) |
>
> This repository builds **AnvilML only**. SindriStudio bundles and launches AnvilML alongside
> BloomeryUI; it is packaged separately.

> **Project status: early development.** The architecture is specified in
> [`ANVILML_DESIGN.md`](./ANVILML_DESIGN.md) and implementation is in progress against the roadmap
> in that document. Interfaces may change before the first tagged release.

---

## Why AnvilML

- **Hybrid for the right reasons.** Rust handles everything that benefits from safety and
  concurrency (HTTP/WebSocket, job queue, DAG scheduling, SQLite registry, worker supervision).
  Python handles the ML, where the ecosystem lives (PyTorch, diffusers, transformers).
- **Crash-resilient.** A native worker crash (driver segfault, OOM-killed process) is caught by a
  watchdog; the failed job is reported, the worker respawns in ~2 s, and the server never goes down.
- **Zero UI coupling.** The frontend is interchangeable. The only contract is the documented API and
  the generated `openapi.json`.
- **Cross-platform.** First-class support for **Linux and Windows**; CPU-only on macOS.

## Where AnvilML fits

```
SindriStudio  (one-click launcher — separate component)
   ├─ starts ─► AnvilML backend        ◄── this repository
   └─ starts ─► BloomeryUI frontend

Runtime data flow:

BloomeryUI            REST /v1/*  +  WebSocket /v1/events
(or any client) ───────────────────────────────────►  anvilml (Rust backend)
                                                       ├─ axum HTTP/WS server
                                                       ├─ job scheduler + DAG validation
                                                       ├─ SQLite registry (models/jobs/artifacts)
                                                       └─ worker pool (1 process per GPU)
                                                               │  framed msgpack over stdio
                                                               ▼
                                                       Python worker(s)
                                                       └─ executor → ZiT / SDXL pipeline nodes
```

The AnvilML backend is a Cargo **workspace** of eight crates (`anvilml-core`, `-hardware`,
`-registry`, `-ipc`, `-worker`, `-scheduler`, `-server`, `-openapi`); the Python worker lives in
`worker/`. See [`ANVILML_DESIGN.md`](./ANVILML_DESIGN.md) for the full functional and technical
design.

## Features (target capability)

- Text-to-image generation via the **ZiT** (distilled/turbo) and **SDXL** pipelines.
- Hardware backends: **NVIDIA CUDA**, **AMD ROCm** (Linux), and **CPU** fallback.
- Multi-GPU: one worker per device; jobs run concurrently across devices.
- In-worker LRU pipeline cache so repeat jobs skip model reloads.
- Content-addressed PNG artifacts served over REST; live progress over WebSocket.
- Model registry with on-disk scanning; cooperative job cancellation; graceful crash recovery.

## Requirements

| Component | Requirement |
| :-- | :-- |
| OS | Linux (x86_64) or Windows (x86_64); macOS = CPU-only |
| Rust | Toolchain pinned by `rust-toolchain.toml` (build from source) |
| Python | 3.12.x, in a **user-managed** virtual environment |
| GPU (optional) | NVIDIA + CUDA, or AMD + ROCm (Linux). No GPU → CPU worker |

## Installation

> Running AnvilML directly (below) is for backend development and headless use. End users normally
> run **SindriStudio**, which launches AnvilML and BloomeryUI together.

AnvilML does **not** manage the Python environment for you — the heavy, hardware-specific ML stack
stays under your control.

```bash
# 1. Build the backend
git clone https://github.com/DrywFiltiarn/AnvilML.git
cd AnvilML
cargo build --release            # -> target/release/anvilml

# 2. Provision the Python worker venv (detects CUDA / ROCm / CPU)
#    Linux / macOS:
./backend/scripts/install_worker_deps.sh
#    Windows (PowerShell):
#    powershell -ExecutionPolicy Bypass -File .\backend\scripts\install_worker_deps.ps1
```

## Configuration

Configuration loads from `anvilml.toml`, with every field overridable by an `ANVILML_*` environment
variable and then by CLI flags. A minimal config:

```toml
host = "127.0.0.1"
port = 8488
venv_path = "./venv"

[[model_dirs]]
path = "./models/diffusion"
kind = "diffusion"

[frontend]
mode = "local"      # local | remote | headless
```

See [`ANVILML_DESIGN.md` §3](./ANVILML_DESIGN.md#3-configuration-anvilml-core) for the complete
config and environment-variable reference.

## Running

```bash
./target/release/anvilml                 # uses ./anvilml.toml
./target/release/anvilml --port 9000 --no-browser
```

Quick health check and a job submission:

```bash
curl http://127.0.0.1:8488/health
curl -X POST http://127.0.0.1:8488/v1/jobs \
  -H 'content-type: application/json' \
  -d '{ "graph": { "nodes": [ ... ] }, "settings": { "seed": -1, "steps": 8, "guidance_scale": 0.0, "width": 1024, "height": 1024 } }'
```

Subscribe to `ws://127.0.0.1:8488/v1/events` for live progress; fetch results from
`GET /v1/artifacts/:hash`.

## Frontend

[BloomeryUI](https://github.com/DrywFiltiarn/BloomeryUI) is the reference frontend and is
distributed separately; SindriStudio bundles it with this backend. AnvilML can serve a built
frontend from a local directory (`local` mode), reverse-proxy a dev server (`remote` mode), or run
API-only (`headless` mode). Any client that honours the API contract works.

## Repository layout

```
anvilml/
├── backend/        # launcher binary (anvilml), migrations, scripts, integration tests
├── crates/         # the eight workspace crates
├── worker/         # Python inference worker (executor, nodes, requirements)
├── ANVILML_DESIGN.md
├── README.md
├── LICENSE
├── CODE_OF_CONDUCT.md
├── CONTRIBUTING.md
├── SECURITY.md
└── .github/        # issue + PR templates, CI
```

## Development

All tests run **without a real GPU** via mock modes.

```bash
cargo fmt --all --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test  --workspace --features mock-hardware
cargo run   -p anvilml-openapi        # regenerate backend/openapi.json

# Python worker tests (mock mode)
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests
```

See [CONTRIBUTING.md](./CONTRIBUTING.md) before opening a pull request.

## Roadmap

Milestones follow the crate dependency order (M0 scaffold → M6 SDXL + hardening). The full table
is in [`ANVILML_DESIGN.md` §23](./ANVILML_DESIGN.md#23-implementation-roadmap). Deferred scope
(per-step progress, additional backends, auth, sub-graph chunking) is tracked in §25.

## Contributing

Contributions are welcome. Please read [CONTRIBUTING.md](./CONTRIBUTING.md) and our
[Code of Conduct](./CODE_OF_CONDUCT.md). Security issues must be reported privately — see
[Security](#security).

## Security

Please report vulnerabilities **privately** — see [SECURITY.md](./SECURITY.md). Open a
[private GitHub Security Advisory](https://github.com/DrywFiltiarn/AnvilML/security/advisories/new)
or email `trinity3dtech@gmail.com`. Do **not** open public issues for security problems.

## License

Released under the [MIT License](./LICENSE).

## Acknowledgements

AnvilML is the backend of the **SindriStudio** project and is built on the Rust and PyTorch
ecosystems — `tokio`, `axum`, `sqlx`, `diffusers`, `transformers`, and many others.
