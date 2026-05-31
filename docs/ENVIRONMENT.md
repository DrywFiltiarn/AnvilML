# ENVIRONMENT.md — AnvilML Configuration & Environment Reference

**Document:** `docs/ENVIRONMENT.md`
**Location in repo:** `AnvilML/docs/ENVIRONMENT.md`
**Authoritative source:** `ANVILML_DESIGN.md` §3 (Configuration), §5 (Hardware Detection), §6 (Python Environment), §8.3 (Worker Env), §21 (Build/Provisioning)
**Read by:** Cline at the start of every PLAN and ACT session.

---

## 1. Configuration Resolution Order

Lowest to highest priority — each level overrides the previous:

```
built-in defaults → anvilml.toml → ANVILML_* environment variables → CLI flags
```

The config file path defaults to `./anvilml.toml` (adjacent to the binary) and can be
overridden with `--config <path>`.

---

## 2. `anvilml.toml` — Full Reference

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
kind = "diffusion"          # optional: diffusion | vae | lora | controlnet

[[model_dirs]]
path = "./models/vae"
kind = "vae"

[rocm]
use_hipblaslt = true
# hsa_override_gfx_version = "10.3.0"   # Linux ROCm only; for unsupported gfx arch (ignored on Windows)

[frontend]
mode = "local"              # local | remote | headless
# path = "./bloomery"       # for mode = "local"  (default: ./bloomery)
# url  = "http://localhost:5173"  # for mode = "remote"

[gpu_selection]
default_device = "auto"     # auto | cpu | <integer device index>

[limits]
max_ipc_payload_mib = 64
list_default_limit  = 100
list_max_limit      = 1000
ws_broadcast_capacity = 256
```

---

## 3. Environment Variable Reference

`ANVILML_*` variables override the matching config field. Nested fields use double
underscores (`__`). All variables are optional; built-in defaults apply when unset.

### 3.1 Server & Storage

| Variable              | Config field      | Default              | Notes                              |
|-----------------------|-------------------|----------------------|------------------------------------|
| `ANVILML_HOST`        | `host`            | `127.0.0.1`          | Bind address                       |
| `ANVILML_PORT`        | `port`            | `8488`               | HTTP port                          |
| `ANVILML_DB_PATH`     | `db_path`         | `./anvilml.db`       | SQLite database path               |
| `ANVILML_ARTIFACT_DIR`| `artifact_dir`    | `./artifacts`        | Where generated images are stored  |
| `ANVILML_VENV_PATH`   | `venv_path`       | `./venv`             | Python venv root (user-managed)    |
| `ANVILML_WORKER_LOG_DIR` | `worker_log_dir` | `./logs`           | Worker stderr capture directory    |

### 3.2 Threading

| Variable                      | Config field          | Default | Notes                        |
|-------------------------------|-----------------------|---------|------------------------------|
| `ANVILML_NUM_THREADS`         | `num_threads`         | `14`    | PyTorch intra-op threads     |
| `ANVILML_NUM_INTEROP_THREADS` | `num_interop_threads` | `4`     | PyTorch inter-op threads     |

### 3.3 Frontend

| Variable                  | Config field      | Default  | Notes                                  |
|---------------------------|-------------------|----------|----------------------------------------|
| `ANVILML_FRONTEND__MODE`  | `frontend.mode`   | `local`  | `local` \| `remote` \| `headless`     |

### 3.4 GPU Selection

| Variable                              | Config field                    | Default | Notes                                    |
|---------------------------------------|---------------------------------|---------|------------------------------------------|
| `ANVILML_GPU_SELECTION__DEFAULT_DEVICE` | `gpu_selection.default_device` | `auto`  | `auto` \| `cpu` \| integer device index |

### 3.5 Logging

| Variable              | Purpose                          | Default |
|-----------------------|----------------------------------|---------|
| `ANVILML_LOG`         | `tracing` filter (§19)           | `info`  |
| `RUST_LOG`            | Fallback for `ANVILML_LOG`       | `info`  |

`ANVILML_LOG` takes precedence over `RUST_LOG`. Output format is selected by the
`--log-format plain|json` CLI flag (default `plain`), not by an environment variable.

### 3.6 Mock & CI Variables

These are used in CI and local development only. Never set in production.

| Variable                  | Purpose                                               | Default |
|---------------------------|-------------------------------------------------------|---------|
| `ANVILML_WORKER_MOCK`     | Set to `1` to run Python worker in stub/mock mode.    | unset   |
| `ANVILML_MOCK_DEVICE_TYPE`| Device type reported by mock hardware detector.       | `cpu`   |
| `ANVILML_MOCK_VRAM_MIB`   | VRAM (MiB) reported by mock hardware detector.        | `8192`  |
| `ANVILML_MOCK_GFX_ARCH`   | GPU arch string reported by mock hardware detector.   | `gfx1100` |
| `ANVILML_MOCK_NODE_DELAY_MS` | Per-node sleep injected by the mock executor so cancel/crash tests can act mid-job. | unset (0) |
| `ANVILML_PING_INTERVAL_MS`| Worker keepalive ping interval override (tests only).  | `30000` |
| `ANVILML_PONG_TIMEOUT_MS` | Worker Pong-response timeout override (tests only).    | `10000` |
| `ANVILML_RESPAWN_DELAY_MS`| Delay before respawning a dead worker (tests only).    | `2000`  |

### 3.7 Per-Worker Variables (injected by Rust, not set by user)

These are set by `anvilml-worker::env::build_worker_env` and injected only into each
worker child process. Do not set these manually.

| Variable                   | Purpose                                           |
|----------------------------|---------------------------------------------------|
| `ANVILML_WORKER_ID`        | Logical worker identifier (`worker-{index}`)      |
| `ANVILML_DEVICE_INDEX`     | GPU device index this worker owns                 |
| `ANVILML_NUM_THREADS`      | Intra-op thread count (from `num_threads`)        |
| `ANVILML_NUM_INTEROP_THREADS` | Inter-op thread count (from `num_interop_threads`) |
| `ANVILML_WORKER_MOCK`      | Propagated to the child when set on the server (mock mode) |
| `CUDA_VISIBLE_DEVICES`     | CUDA isolation (CUDA workers only)                |
| `HIP_VISIBLE_DEVICES`      | ROCm device isolation — ROCm workers, Linux **and** Windows |
| `ROCBLAS_USE_HIPBLASLT`    | ROCm performance flag (from `rocm.use_hipblaslt`) |
| `HSA_OVERRIDE_GFX_VERSION` | ROCm gfx override (from `rocm.hsa_override_gfx_version`) — **Linux ROCm runtime only**, not applicable on Windows |
| `OMP_NUM_THREADS`          | OpenMP threading (from `num_threads`)             |
| `MKL_NUM_THREADS`          | MKL threading (from `num_threads`)                |
| `OPENBLAS_NUM_THREADS`     | OpenBLAS threading (from `num_threads`)           |
| `VECLIB_MAXIMUM_THREADS`   | vecLib threading (macOS, from `num_threads`)      |

---

## 4. Python Venv

The `venv_path` directory is **user-managed**. AnvilML does not create or modify it;
it only resolves the interpreter from it.

**Interpreter resolution:**
- Linux / macOS: `{venv_path}/bin/python3`
- Windows: `{venv_path}\Scripts\python.exe`

**Provisioning scripts (run once, before starting AnvilML):**
```bash
# Linux / macOS
bash backend/scripts/install_worker_deps.sh

# Windows
powershell -ExecutionPolicy Bypass -File backend\scripts\install_worker_deps.ps1
```

These scripts detect the available hardware backend (CUDA / ROCm / CPU) **and the OS**, then
install the matching torch build on top of `base.txt`:

- **CUDA** → `worker/requirements/cuda.txt`
- **ROCm on Linux** → `worker/requirements/rocm.txt` (pip ROCm index, stable or nightly)
- **ROCm on Windows** → `worker/requirements/rocm-windows.txt` — AMD's *PyTorch on Windows* package
  (ROCm ≥ 7.2), **not** the Linux pip ROCm index
- **CPU** → `worker/requirements/cpu.txt`

**ROCm on Windows is a mandatory MVP backend.** It requires AMD's *PyTorch on Windows* distribution
— the AMD Adrenalin / PyTorch-on-Windows driver package built on **ROCm ≥ 7.2** — on a supported
AMD Radeon RX 7000/9000-series GPU or select Ryzen AI APU. Hardware outside AMD's supported-GPU
list falls back to CPU. (Authoritative: `ANVILML_DESIGN.md` §5, §6, §21.)

**Preflight checks at startup (§6.1):**
1. Interpreter exists and is executable.
2. Python version is `3.12.x` (warning only if not).
3. `import torch` succeeds (failure → workers `Dead`, server starts, jobs return `503`).

---

## 5. Forge-Specific Variables (CI / local orchestration only)

These control `forge.py` and are never read by AnvilML itself.

| Variable                | Purpose                                              | Default              |
|-------------------------|------------------------------------------------------|----------------------|
| `FORGE_DISCORD_TOKEN`   | Discord bot token for approval notifications         | (required for Discord) |
| `FORGE_DISCORD_GUILD_ID`| Discord server ID                                    | (required for Discord) |
| `FORGE_CLINE_BIN`       | Path to Cline CLI binary                             | `cline`              |
| `FORGE_CLINE_TIMEOUT`   | Max seconds per Cline session                        | `5400` (90 min)      |
| `FORGE_CLINE_RETRIES`   | Retry count on Cline failure (llama.cpp crash)       | `3`                  |
| `FORGE_CLINE_RETRY_DELAY` | Base seconds between retries                       | `60`                 |
| `FORGE_MODEL_PLANNING`  | llama-swap model ID for PLAN sessions                | `Qwen3.6-35B-A3B:planning` |
| `FORGE_MODEL_CODING`    | llama-swap model ID for ACT sessions                 | `Qwen3.6-35B-A3B:coding`   |
| `FORGE_POLL_INTERVAL`   | Discord approval poll interval (seconds)             | `10`                 |
| `FORGE_APPROVAL_TIMEOUT`| Discord approval timeout (seconds)                   | `86400` (24 h)       |