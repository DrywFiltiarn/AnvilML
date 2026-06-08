# ENVIRONMENT.md — AnvilML Configuration & Environment Reference

**Document:** `docs/ENVIRONMENT.md`
**Location in repo:** `AnvilML/docs/ENVIRONMENT.md`
**Authoritative source:** `ANVILML_DESIGN.md` §3 (Configuration), §5 (Hardware Detection), §6 (Python Environment), §8.3 (Worker Env), §21 (Build/Provisioning)
**Read by:** OpenCode forge-plan and forge-act agents at the start of every session.

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
# seeds_path = "./seeds"  # default: <exe_dir>/seeds; falls back to backend/seeds/ in debug builds
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
# AnvilML is headless by default. BloomeryUI is run as a SEPARATE server by SindriStudio,
# NOT served by AnvilML. local/remote exist only for serving a CUSTOM frontend standalone.
mode = "headless"           # headless (default) | local | remote
# path = "./frontend"       # custom frontend dir, for mode = "local"  (not BloomeryUI)
# url  = "http://localhost:5173"  # custom frontend dev server, for mode = "remote"

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

| Variable                 | Config field      | Default              | Notes                              |
|--------------------------|-------------------|----------------------|------------------------------------|
| `ANVILML_HOST`           | `host`            | `127.0.0.1`          | Bind address                       |
| `ANVILML_PORT`           | `port`            | `8488`               | HTTP port                          |
| `ANVILML_DB_PATH`        | `db_path`         | `./anvilml.db`       | SQLite database path               |
| `ANVILML_ARTIFACT_DIR`   | `artifact_dir`    | `./artifacts`        | Where generated images are stored  |
| `ANVILML_VENV_PATH`      | `venv_path`       | `./venv`             | Python venv root                   |
| `ANVILML_WORKER_LOG_DIR` | `worker_log_dir`  | `./logs`             | Worker stderr capture directory    |
| `ANVILML_SEEDS_PATH`     | `seeds_path`      | `<exe_dir>/seeds`    | SQL seed files directory           |

### 3.2 Threading

| Variable                      | Config field          | Default | Notes                        |
|-------------------------------|-----------------------|---------|------------------------------|
| `ANVILML_NUM_THREADS`         | `num_threads`         | `14`    | PyTorch intra-op threads     |
| `ANVILML_NUM_INTEROP_THREADS` | `num_interop_threads` | `4`     | PyTorch inter-op threads     |

### 3.3 Frontend

| Variable                  | Config field      | Default    | Notes                                  |
|---------------------------|-------------------|------------|----------------------------------------|
| `ANVILML_FRONTEND__MODE`  | `frontend.mode`   | `headless` | `headless` \| `local` \| `remote`     |

### 3.4 GPU Selection

| Variable                              | Config field                    | Default | Notes                                    |
|---------------------------------------|---------------------------------|---------|------------------------------------------|
| `ANVILML_GPU_SELECTION__DEFAULT_DEVICE` | `gpu_selection.default_device` | `auto`  | `auto` \| `cpu` \| integer device index |

### 3.5 Logging

| Variable              | Purpose                                    | Default |
|-----------------------|--------------------------------------------|---------|
| `ANVILML_LOG`         | `tracing` filter directive                 | `info`  |
| `RUST_LOG`            | Fallback when `ANVILML_LOG` is unset       | `info`  |

`ANVILML_LOG` takes precedence over `RUST_LOG`. Output format is selected by the
`--log-format plain|json` CLI flag (default `plain`), not by an environment variable.

The `--verbose` CLI flag (when implemented) maps to `ANVILML_LOG=debug` and enables
all DEBUG-level instrumentation. Until that flag exists, set `ANVILML_LOG=debug`
directly to see debug output.

Common filter examples:
```
ANVILML_LOG=debug          # all crates at DEBUG
ANVILML_LOG=info           # default — operational events only
ANVILML_LOG=anvilml=debug  # AnvilML crates at DEBUG, dependencies at INFO
```

See §9 for the logging conventions all code in this project must follow.

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
| `ANVILML_IPC_SOCKET`       | Unix domain socket path (Linux/macOS) or Windows named pipe path. Worker must connect to this path at startup before processing any IPC frames. Format: `{tmp}/anvilml-{pid}/worker-{index}.sock` on Unix; `\\.\pipe\anvilml-worker-{index}-{pid}` on Windows. |
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

The `venv_path` directory is provisioned automatically by AnvilML on first run if it is absent
or `import torch` fails. The server binds immediately and the API is responsive at `:8488`
throughout provisioning. State is observable via `GET /v1/system/env` (`.provisioning` field)
and `WS /v1/events` (`provisioning.progress` frames). Jobs return `503 provisioning` until
provisioning reaches `Ready`.

The provisioning scripts may also be run manually for venv repair or to swap torch versions:

```bash
# Linux / macOS
bash backend/scripts/install_worker_deps.sh

# Windows
powershell -ExecutionPolicy Bypass -File backend\scripts\install_worker_deps.ps1
```

**Interpreter resolution:**
- Linux / macOS: `{venv_path}/bin/python3`
- Windows: `{venv_path}\Scripts\python.exe`

These scripts detect the available hardware backend (CUDA / ROCm / CPU) **and the OS** — SDK-free,
via `anvilml --print-hardware` (Vulkan/DXGI enumeration; falls back to PCI sysfs on Linux or
`Get-CimInstance Win32_VideoController` on Windows) — then install the matching torch build on top
of `base.txt`:

- **CUDA** → `worker/requirements/cuda.txt`
- **ROCm on Linux** → `worker/requirements/rocm-linux.txt` (pip ROCm index, stable or nightly)
- **ROCm on Windows** → `worker/requirements/rocm-windows.txt` — AMD's *PyTorch on Windows* package
  (ROCm ≥ 7.2), **not** the Linux pip ROCm index
- **CPU** → `worker/requirements/cpu.txt`

**ROCm on Windows is a mandatory MVP backend.** It requires AMD's *PyTorch on Windows* distribution
(AMD Adrenalin / PyTorch-on-Windows driver package, **ROCm ≥ 7.2**) on a supported AMD Radeon
RX 7000/9000-series GPU or select Ryzen AI APU; hardware outside AMD's supported list falls back to
CPU. (Authoritative: `ANVILML_DESIGN.md` §5, §6, §21.)

**Preflight checks at startup (§6.4):**
1. Interpreter exists and is executable.
2. Python version is `3.12.x` (warning only if not).
3. `import torch` succeeds (failure → workers `Dead`, server starts, jobs return `503`).

---

## 5. The Forge — Specific Variables (CI / local orchestration only)

These control `forge.py` and are never read by AnvilML itself.

| Variable                   | Purpose                                               | Default                             |
|----------------------------|-------------------------------------------------------|-------------------------------------|
| `FORGE_DISCORD_TOKEN`      | Discord bot token for approval notifications          | (required for Discord)              |
| `FORGE_DISCORD_GUILD_ID`   | Discord server ID                                     | (required for Discord)              |
| `FORGE_OPENCODE_BIN`       | Path to OpenCode CLI binary                           | `opencode`                          |
| `FORGE_OPENCODE_TIMEOUT`   | Max seconds per OpenCode session                      | `7200` (120 min)                    |
| `FORGE_OPENCODE_RETRIES`   | Retry count on OpenCode failure (llama.cpp crash)     | `3`                                 |
| `FORGE_OPENCODE_RETRY_DELAY` | Base seconds between retries                        | `60`                                |
| `FORGE_MODEL_PLANNING`     | OpenCode model for PLAN sessions                      | `llama.cpp/Qwen3.6-35B-A3B:planning` |
| `FORGE_MODEL_CODING`       | OpenCode model for ACT sessions                       | `llama.cpp/Qwen3.6-35B-A3B:coding`  |
| `FORGE_CONTEXT_WINDOW`     | Model context window size (tokens)                    | `262144` (256k)                     |
| `FORGE_POLL_INTERVAL`      | Discord approval poll interval (seconds)              | `10`                                |
| `FORGE_APPROVAL_TIMEOUT`   | Discord approval timeout (seconds)                    | `86400` (24 h)                      |

---

## 6. Build, Format, and Lint Commands

These are the canonical commands for all ACT sessions working on AnvilML.

### Formatter Commands (two-pass contract — `FORGE_AGENT_RULES.md §5.9`)

The ACT agent runs the formatter **twice** per session. The two modes are distinct commands:

| Pass                        | Mode        | Command                        | When                                          |
|-----------------------------|-------------|--------------------------------|-----------------------------------------------|
| Pass 1                      | In-place    | `cargo fmt --all`              | Immediately after IMPLEMENT                   |
| Pass 2 (gate)               | Check-only  | `cargo fmt --all -- --check`   | Immediately before `git add -A`               |
| Pass 1 re-run (conditional) | In-place    | `cargo fmt --all`              | Only if pass 2 exits non-zero (drift found)   |

**Pass 2 exit codes:**
- `0` — no formatting drift; proceed to staging
- `1` — drift found; run pass 1 re-run (`cargo fmt --all`), then immediately run the
  post-reformat compile check below to verify compilation, then re-run pass 2 to confirm

**Post-reformat compile check** (run after pass 1 re-run only, when pass 2 was non-zero):
```bash
cargo check --workspace --features mock-hardware
```
If this exits non-zero after the pass 1 re-run, set Status=BLOCKED and STOP — do not stage.

### All Build and Lint Commands

| Step | Command | Notes |
|------|---------|-------|
| Format (in-place) | `cargo fmt --all` | Pass 1 — apply formatting |
| Format (check-only) | `cargo fmt --all -- --check` | Pass 2 gate — must exit 0 before staging |
| Post-reformat compile check | `cargo check --workspace --features mock-hardware` | Only if pass 2 was non-zero |
| Lint (mock-hardware) | `cargo clippy --workspace --features mock-hardware -- -D warnings` | Zero warnings |
| Lint (real-hardware) | `cargo clippy --bin anvilml -- -D warnings` | Zero warnings |
| Test (Rust) | `cargo test --workspace --features mock-hardware` | Zero failures |
| Test (Python worker) | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` | Zero failures |
| Config drift gate | `cargo test -p backend --features mock-hardware -- config_reference` | See §8 Gate 1 |
| OpenAPI drift gate | `cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json` | On handler/schema changes only |

The OpenAPI drift gate applies only when `anvilml-server` handler signatures or
`utoipa` annotations are modified. It is not required for every task.

---

## 7. Platform Cross-Check (`FORGE_AGENT_RULES.md §5.7`)

AnvilML targets Linux and Windows as co-equal MVP platforms. Before writing the
implementation report, run all four of the following checks in order:

```bash
# 1. Mock-hardware Linux check (exercises #[cfg(unix)] scaffold and cfg-gated mock paths)
cargo check --workspace --features mock-hardware

# 2. Mock-hardware Windows cross-check (exercises #[cfg(windows)] scaffold and cfg-gated mock paths)
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu

# 3. Real-hardware Linux check (exercises #[cfg(unix)] detection paths)
cargo check --bin anvilml

# 4. Real-hardware Windows cross-check (exercises #[cfg(windows)] detection paths)
cargo check --bin anvilml --target x86_64-pc-windows-gnu
```

The `x86_64-pc-windows-gnu` target and `gcc-mingw-w64` linker are installed in the
local build environment. Checks 2 and 4 run on Linux via cross-compilation.
Checks 1 and 3 are native Linux. **All four must exit 0.** Neither mock-hardware nor
real-hardware alone is sufficient — `mock-hardware` elides all real-hardware
`#[cfg(windows)]` and `#[cfg(unix)]` detection paths, while the real-hardware checks
exercise code that `mock-hardware` never compiles.

Record the verbatim output of all four commands in `## Platform Cross-Check` in
the implementation report.

---

## 8. Project Gates (`FORGE_AGENT_RULES.md §5.8`)

### Gate 1 — Config Surface Sync

Any task that adds, renames, or removes a field on `ServerConfig` or any nested config
struct **must** in the same task:

- Update `./anvilml.toml` with the matching key at its documented default value
- Update `docs/ENVIRONMENT.md §2` with the new/changed field description

Enforce with:

```bash
cargo test -p backend --features mock-hardware -- config_reference
```

This test asserts that the committed `./anvilml.toml` key-set matches
`ServerConfig::default()` recursively. Fix `anvilml.toml` to make it pass — do NOT
weaken or skip the test. **Skip only** if task P3-B2 has not yet been implemented
(i.e. `backend/tests/config_reference.rs` does not yet exist).

Record the verbatim output in `## Project Gates` in the implementation report.
---

## 9. Logging Conventions (`FORGE_AGENT_RULES.md §11`)

All code in this project must follow the logging conventions defined in
`FORGE_AGENT_RULES.md §11`. This section lists the AnvilML-specific mandatory
log points that §11 requires to be present at INFO level.

### Mandatory INFO log points

These events must always be logged at INFO. They are unconditionally visible at the
default log level and must never be demoted to DEBUG or removed:

| Subsystem | Event | Required fields |
|-----------|-------|-----------------|
| Database | SQLite database file created (did not previously exist) | `path=` |
| Database | Each migration applied | `migration=`, `version=` (or migration filename) |
| Database | All migrations already up to date (no-op) | `migrations_applied=0` or equivalent |
| Seeds | Seed file applied (SHA256 changed or first run) | `file=`, `sha256=` |
| Seeds | Seed file skipped (SHA256 unchanged) | `file=`, `status=up-to-date` |
| Server | Bind address and port on successful listen | `addr=` |
| Server | Graceful shutdown initiated (signal received) | signal name |
| Hardware | Each detected device on startup | `index=`, `name=`, `device_type=`, `vram_total_mib=` |
| Workers | Worker spawned | `worker_id=`, `device_index=` |
| Workers | Worker respawned after unexpected exit | `worker_id=`, `exit_code=` or `signal=` |
| Workers | Worker reached Ready state | `worker_id=` |
| Model scan | Scan completed | `models_scanned=` |
| Provisioning | Provisioning started | `reason=` (absent venv, torch import failure, etc.) |
| Provisioning | Provisioning completed | `duration_ms=` |

### Mandatory DEBUG log points

These events must exist at DEBUG level and are visible only when
`ANVILML_LOG=debug` (or `--verbose` once implemented):

| Subsystem | Event | Required fields |
|-----------|-------|-----------------|
| Database | Each SQL query executed (if feasible via sqlx instrumentation) | query summary |
| IPC | Each message sent to a worker | `worker_id=`, `message_type=` |
| IPC | Each event received from a worker | `worker_id=`, `event_type=` |
| Model scan | Each file examined (accepted or skipped) | `path=`, `reason=` if skipped |
| Job scheduler | Job dispatched to worker | `job_id=`, `worker_id=` |
| Job scheduler | Job state transition | `job_id=`, `from=`, `to=` |
| Hardware | Detection fallback used (Vulkan unavailable, falling back to DXGI/sysfs) | `fallback=` |

### WARN and ERROR conventions

| Level | Use for | Field discipline |
|-------|---------|-----------------|
| `WARN` | Recoverable anomalies — the system continues but something unexpected happened | Include `path=` or relevant identifier; include `error=` **only** when the error message adds information beyond what the other fields already convey. A "not found" error on a path field that already names the path is redundant — omit `error=`. A permission denied or unexpected OS error on a named path is not redundant — include `error=`. |
| `ERROR` | Unrecoverable failures that cause a subsystem or operation to fail | Always include `error=` |

**WARN field example — redundant (do not write):**
```rust
tracing::warn!(path = %entry.path().display(), error = %e,
    "scanner: skipping unreadable entry");
// When e is "The system cannot find the file specified. (os error 2)" and
// path already names the missing file — the error adds nothing.
```

**WARN field example — correct:**
```rust
tracing::warn!(path = %entry.path().display(),
    "scanner: skipping missing path");
// If the error is something unexpected (e.g. permission denied), include it:
tracing::warn!(path = %entry.path().display(), error = %e,
    "scanner: skipping unreadable entry");
```

### Span and context conventions

- Use `tracing::instrument` on async functions that represent a meaningful
  unit of work (migration runner, seed loader, worker spawn, job dispatch).
- Span names must be lowercase snake_case and match the function or subsystem name.
- Do not instrument tight inner loops or per-frame/per-packet functions.

---

## 10. Crate Version Bump Convention (`FORGE_AGENT_RULES.md §12`)

Every task that modifies source files in a crate must increment that crate's patch version
per `FORGE_AGENT_RULES.md §12`. This section defines the AnvilML-specific locations.

### Version file locations

| Crate / Package | Manifest file | Version field |
|----------------|--------------|---------------|
| `backend` (anvilml binary) | `backend/Cargo.toml` | `[package] version` |
| `anvilml-core` | `crates/anvilml-core/Cargo.toml` | `[package] version` |
| `anvilml-hardware` | `crates/anvilml-hardware/Cargo.toml` | `[package] version` |
| `anvilml-registry` | `crates/anvilml-registry/Cargo.toml` | `[package] version` |
| `anvilml-ipc` | `crates/anvilml-ipc/Cargo.toml` | `[package] version` |
| `anvilml-worker` | `crates/anvilml-worker/Cargo.toml` | `[package] version` |
| `anvilml-scheduler` | `crates/anvilml-scheduler/Cargo.toml` | `[package] version` |
| `anvilml-server` | `crates/anvilml-server/Cargo.toml` | `[package] version` |
| `anvilml-openapi` | `crates/anvilml-openapi/Cargo.toml` | `[package] version` |

### Read-only fields — never modify

| Field | Location | Why read-only |
|-------|----------|---------------|
| `[workspace.package] version` | root `Cargo.toml` | Product release version — triggers GitHub Release pipeline. Manually controlled only. |
| Major (`X`) and minor (`Y`) digits | Any crate `Cargo.toml` | Manually controlled. Only `Z` (patch) is agent-writable. |

### Bump procedure

```bash
# 1. Read the current version for the crate being modified, e.g. anvilml-registry:
grep '^version' crates/anvilml-registry/Cargo.toml
# → version = "0.1.4"

# 2. Compute Z+1, write back — only the [package] version line, X.Y unchanged:
# → version = "0.1.5"
```

Target only the `version = "X.Y.Z"` line in the `[package]` section. Do not edit
`Cargo.lock` — cargo regenerates it on the next build. Cross-crate path dependencies
in this workspace carry no version pins, so no cascade update to sibling `Cargo.toml`
files is needed.