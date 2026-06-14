# AnvilML

**AnvilML** is the Rust backend binary of the [SindriStudio](https://github.com/DrywFiltiarn/SindriStudio) image-generation platform. It spawns and supervises Python inference workers (one per GPU), exposes a versioned REST + WebSocket API, and manages job scheduling, model registry, and artifact storage via SQLite.

AnvilML is **headless only**. It is a pure API server. It does not serve a web UI and does not embed BloomeryUI. SindriStudio is the separate launcher that starts AnvilML and BloomeryUI as independent sibling processes.

---

## Status

**v3 — active development.** This is a ground-up rebuild informed by lessons from the v2 implementation. See `docs/ANVILML_DESIGN.md §Appendix A` for the v2 post-mortem.

---

## Architecture at a glance

```
SindriStudio (launcher)
├── AnvilML          ← this repo; Rust API server + Python worker supervisor
└── BloomeryUI       ← separate repo; reference web frontend
```

AnvilML spawns one Python worker subprocess per GPU. Workers communicate with the supervisor over ZeroMQ (ROUTER ↔ DEALER over TCP loopback). The REST + WebSocket API is the sole integration surface for any client.

```
Client (BloomeryUI / curl / any HTTP client)
    │  REST + WebSocket (port 8488)
    ▼
anvilml (Rust)
├── anvilml-server      axum HTTP/WS server
├── anvilml-scheduler   job queue, DAG validation, VRAM ledger, dispatch
├── anvilml-worker      worker pool: spawn, supervise, keepalive, respawn
├── anvilml-registry    model scanner + SQLite store
├── anvilml-hardware    SDK-free GPU detection (Vulkan primary)
├── anvilml-ipc         ZeroMQ ROUTER transport + WorkerMessage/WorkerEvent
└── anvilml-core        domain types, config, errors
    │  ZeroMQ tcp://127.0.0.1:{port}
    ▼
Python worker (per GPU)
├── worker_main.py      entry point; message dispatch loop
├── executor.py         graph topological sort + node execution
├── nodes/              generic node set (LoadModel, LoadVae, LoadClip, ...)
│   └── arch/           architecture dispatch (zit.py, flux.py, ...)
└── pipeline_cache.py   LRU model cache
```

---

## Supported hardware

| GPU backend | Linux | Windows |
|-------------|:-----:|:-------:|
| NVIDIA CUDA | ✓ | ✓ |
| AMD ROCm | ✓ | ✓ |
| CPU (fallback) | ✓ | ✓ |

Hardware detection is SDK-free. No `nvidia-smi`, `rocm-smi`, or CUDA/ROCm toolkits are required. The Vulkan loader (bundled with every modern GPU driver) is sufficient.

---

## MVP model support

Both model families use standalone `.safetensors` files. There are no all-in-one checkpoints.

| Model | Files required |
|-------|---------------|
| Z-Image Turbo (ZiT) FP8 | ZiT FP8 diffusion model + Qwen3 4B text encoder + ZiT VAE |
| Flux 2 Klein FP8 | Flux 2 Klein 9B FP8 diffusion model + Qwen3 8B FP8-mixed text encoder + Flux VAE |

The generic node graph is identical for both — only `model_id` values change.

---

## Quick start

### Prerequisites

- Rust stable toolchain (installed via `rustup` — `rust-toolchain.toml` pins the version automatically)
- Python 3.12.x (user-managed; AnvilML does not install Python)
- GPU driver with Vulkan support (for GPU inference; CPU fallback requires no driver)

### Build and run

```bash
# Clone
git clone https://github.com/DrywFiltiarn/AnvilML.git
cd AnvilML

# Provision Python worker venv (detects CUDA/ROCm/CPU automatically)
bash backend/scripts/install_worker_deps.sh          # Linux / macOS
# powershell -ExecutionPolicy Bypass -File backend\scripts\install_worker_deps.ps1  # Windows

# Build
cargo build --release

# Place model files
mkdir -p models/diffusion models/text_encoders models/vae
# Copy your .safetensors files into the appropriate subdirectory

# Run
./target/release/anvilml
# Server binds http://127.0.0.1:8488 by default

# Test
curl http://127.0.0.1:8488/health
curl http://127.0.0.1:8488/v1/workers
curl http://127.0.0.1:8488/v1/nodes
curl http://127.0.0.1:8488/v1/models
```

### Submit a job

```bash
curl -X POST http://127.0.0.1:8488/v1/jobs \
  -H 'Content-Type: application/json' \
  -d @docs/example_workflows/zit_fp8.json

# Poll status
curl http://127.0.0.1:8488/v1/jobs/<job_id>

# Fetch result
curl http://127.0.0.1:8488/v1/artifacts/<hash> --output result.png
```

### Watch events in real time

```bash
websocat ws://127.0.0.1:8488/v1/events
```

---

## Configuration

AnvilML reads configuration from (lowest to highest precedence):

1. Compiled-in defaults
2. `anvilml.toml` (path set by `--config`, default `./anvilml.toml`)
3. `ANVILML_*` environment variables
4. CLI flags (`--host`, `--port`, `--config`)

Key options:

| Flag / Env var | Default | Description |
|----------------|---------|-------------|
| `--host` / `ANVILML_HOST` | `127.0.0.1` | Bind address |
| `--port` / `ANVILML_PORT` | `8488` | HTTP port |
| `ANVILML_DB_PATH` | `./anvilml.db` | SQLite database path |
| `ANVILML_ARTIFACT_DIR` | `./artifacts` | Generated image storage |
| `ANVILML_VENV_PATH` | `./worker/.venv` | Python venv root |

See `docs/ENVIRONMENT.md §3–4` for the full reference.

---

## API overview

All routes are under `/v1/` except `/health`.

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Liveness probe |
| GET | `/v1/system` | Hardware snapshot |
| GET | `/v1/system/env` | Python environment health |
| GET | `/v1/system/versions` | Component version report |
| GET | `/v1/nodes` | Registered node types |
| POST | `/v1/jobs` | Submit generation job |
| GET | `/v1/jobs` | List jobs |
| GET | `/v1/jobs/:id` | Get job status |
| POST | `/v1/jobs/:id/cancel` | Cancel job |
| GET | `/v1/models` | List scanned models |
| POST | `/v1/models/rescan` | Trigger model rescan |
| GET | `/v1/workers` | Worker pool status |
| GET | `/v1/artifacts/:hash` | Fetch generated image |
| GET | `/v1/events` | WebSocket event stream |

Full OpenAPI spec: `backend/openapi.json`

---

## Node system

AnvilML uses a **generic, architecture-agnostic node graph**. There are no `ZitSampler` or `FluxTextEncode` nodes — only generic nodes that dispatch internally based on the loaded model's architecture.

**Baseline nodes:**

| Node | Inputs → Output |
|------|----------------|
| `LoadModel` | `model_id` → `MODEL` |
| `LoadVae` | `model_id` → `VAE` |
| `LoadClip` | `model_id`, `clip_type?` → `CLIP` |
| `ClipTextEncode` | `CLIP`, `text`, `negative_text?` → `CONDITIONING` |
| `EmptyLatent` | `width`, `height` → `LATENT` |
| `Sampler` | `MODEL`, `CONDITIONING`, `LATENT`, `steps`, `cfg`, `seed` → `LATENT`, `seed` |
| `VaeDecode` | `VAE`, `LATENT` → `IMAGE` |
| `ImageResize` | `IMAGE`, `width`, `height` → `IMAGE` |
| `SaveImage` | `IMAGE`, `seed?` → *(emits ImageReady event)* |

`GET /v1/nodes` returns the live registry at runtime.

---

## Development

### Build for development

```bash
cargo build --workspace --features mock-hardware
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v
```

### Pre-push gate (WSL2)

```bash
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
```

This gate catches Windows-incompatible code without a Windows runner. The target must be installed once: `rustup target add x86_64-pc-windows-gnu`.

### CI

GitHub Actions runs 6 jobs on every push to `main`:

| Job | Runner |
|-----|--------|
| `rust-linux` | Ubuntu latest |
| `rust-windows` | Windows latest |
| `worker-linux` | Ubuntu latest |
| `worker-windows` | Windows latest |
| `openapi-drift` | Ubuntu latest |
| `config-drift` | Ubuntu latest |

---

## Project structure

```
AnvilML/
├── backend/            Binary crate (anvilml executable)
├── crates/
│   ├── anvilml-core        Domain types, config, errors
│   ├── anvilml-hardware    GPU detection
│   ├── anvilml-registry    Model scanner + SQLite
│   ├── anvilml-ipc         ZeroMQ transport
│   ├── anvilml-worker      Worker pool
│   ├── anvilml-scheduler   Job scheduling
│   ├── anvilml-server      HTTP/WS server
│   └── anvilml-openapi     OpenAPI generator (build-time)
├── worker/             Python inference workers
│   ├── nodes/              Generic node implementations
│   │   └── arch/           Architecture dispatch modules
│   └── requirements/       Platform-specific torch installs
├── docs/               Design documents, phase registry, test catalogue
└── .forge/             Forge orchestrator task files and state
```

---

## Documentation

| Document | Purpose |
|----------|---------|
| `docs/ANVILML_DESIGN.md` | Full functional and technical specification |
| `docs/ARCHITECTURE.md` | Repository layout and component guide |
| `docs/ENVIRONMENT.md` | Build commands, env vars, config reference |
| `docs/PHASES.md` | Implementation phase registry |
| `docs/TESTS.md` | Test catalogue |

---

## License

MIT — see [LICENSE](LICENSE)

**Contact:** trinity3dtech@gmail.com  
**Repository:** https://github.com/DrywFiltiarn/AnvilML
