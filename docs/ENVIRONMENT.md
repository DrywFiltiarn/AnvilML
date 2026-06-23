# ENVIRONMENT.md — AnvilML Build, Test, and Configuration Reference

**Document:** `docs/ENVIRONMENT.md`
**Location in repo:** `AnvilML/docs/ENVIRONMENT.md`
**Read by:** The Forge `forge-plan` and `forge-act` agents at the start of every session.
**Authoritative for:** all build, format, lint, cross-check, test, and gate commands;
  environment variable reference; config field reference; version bump procedure;
  logging field conventions; and the inline documentation obligation.

Agents: read this document in full before writing any plan or any code. All commands
used in ACT sessions come from this document. Do not invent commands, flags, or paths.

---

## Table of Contents

1. [Development Environment Prerequisites](#1-development-environment-prerequisites)
2. [Repository Bootstrap](#2-repository-bootstrap)
3. [Environment Variable Reference](#3-environment-variable-reference)
4. [Config Field Reference (`anvilml.toml`)](#4-config-field-reference-anvilmltoml)
5. [Python Worker Runtime](#5-python-worker-runtime)
6. [Build, Format, Lint, and Test Commands](#6-build-format-lint-and-test-commands)
7. [Platform Cross-Check (Local WSL2 Gate)](#7-platform-cross-check-local-wsl2-gate)
8. [Project Gates](#8-project-gates)
9. [Logging Conventions](#9-logging-conventions)
10. [Inline Documentation Obligation](#10-inline-documentation-obligation)
11. [Test File Conventions](#11-test-file-conventions)
12. [Crate Version Bump Convention](#12-crate-version-bump-convention)

---

## 1. Development Environment Prerequisites

### Rust

```bash
# Install rustup (if not present)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# The correct toolchain is pinned in rust-toolchain.toml and activated automatically.
# Verify:
rustc --version   # must match rust-toolchain.toml channel
cargo --version
```

Required components (installed automatically from `rust-toolchain.toml`):
- `rustfmt` — code formatter
- `clippy` — linter

Required targets:
```bash
# Windows cross-check target (WSL2 local gate only — not for CI)
rustup target add x86_64-pc-windows-gnu

# On Linux/WSL2, the mingw linker is also required:
sudo apt-get install gcc-mingw-w64-x86-64
```

Required system packages (Ubuntu/Debian):
```bash
sudo apt-get install pkg-config libssl-dev
```

### Python

Python 3.12.x is required. The version is user-managed; AnvilML does not install Python.

```bash
python3.12 --version   # must be 3.12.x
```

The Python worker venv lives at `./worker/.venv` by default (configurable via `venv_path`).
Provisioning scripts create and populate it:

```bash
# Linux / macOS
bash scripts/install_worker_deps.sh

# Windows (PowerShell)
powershell -ExecutionPolicy Bypass -File scripts\install_worker_deps.ps1
```

These scripts detect the available hardware backend (CUDA / ROCm / CPU) and install
the matching torch build. See `ANVILML_DESIGN.md §18.1` for the full provisioning flow.

### sqlx-cli (for migrations)

```bash
cargo install sqlx-cli --no-default-features --features sqlite
```

This is only needed for authoring new migrations, not for running the server.

---

## 2. Repository Bootstrap

After cloning:

```bash
# 1. Build the workspace (verifies toolchain and resolves dependencies)
cargo build --workspace --features mock-hardware

# 2. Run all Rust tests to confirm baseline green
cargo test --workspace --features mock-hardware

# 3. Provision the Python worker venv
bash scripts/install_worker_deps.sh     # Linux
# powershell -ExecutionPolicy Bypass -File scripts\install_worker_deps.ps1   # Windows

# 4. Run Python tests (invoke the venv interpreter directly — do not use bare python)
# Linux / macOS:
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v
# Windows:
# ANVILML_WORKER_MOCK=1 worker\.venv\Scripts\python -m pytest worker/tests/ -v

# 5. Install the WSL2 Windows cross-check target (once, on WSL2 only)
rustup target add x86_64-pc-windows-gnu
sudo apt-get install gcc-mingw-w64-x86-64
```

---

## 3. Environment Variable Reference

`ANVILML_*` variables override the matching config field. Nested fields use double
underscores (`__`). All variables are optional; compiled-in defaults apply when unset.

### 3.1 Server & Storage

| Variable | Config field | Default | Notes |
|:---------|:-------------|:--------|:------|
| `ANVILML_HOST` | `host` | `127.0.0.1` | Bind address |
| `ANVILML_PORT` | `port` | `8488` | HTTP port |
| `ANVILML_DB_PATH` | `db_path` | `./anvilml.db` | SQLite database path |
| `ANVILML_ARTIFACT_DIR` | `artifact_dir` | `./artifacts` | Generated image storage |
| `ANVILML_VENV_PATH` | `venv_path` | `./worker/.venv` | Python venv root |
| `ANVILML_SEEDS_PATH` | `seeds_path` | `./database/seeds` | SQL seed files directory |
| `ANVILML_MAX_IPC_PAYLOAD_MIB` | `max_ipc_payload_mib` | `256` | Max IPC message size |
| `ANVILML_NUM_THREADS` | `num_threads` | unset (= num_cpus) | Tokio worker thread count |

### 3.2 GPU Selection

| Variable | Config field | Default | Notes |
|:---------|:-------------|:--------|:------|
| `ANVILML_GPU_SELECTION__DEFAULT_DEVICE` | `gpu_selection.default_device` | `auto` | `auto` \| `cpu` \| integer device index |

### 3.3 Logging

| Variable | Purpose | Default |
|:---------|:--------|:--------|
| `ANVILML_LOG` | `tracing` filter directive | `info` |
| `RUST_LOG` | Fallback when `ANVILML_LOG` is unset | `info` |

`ANVILML_LOG` takes precedence over `RUST_LOG`. Output format is controlled by the
`--log-format plain|json` CLI flag (default `plain`), not by an environment variable.

### 3.4 Worker Process Variables (injected by Rust supervisor, not user-set)

These are written into the subprocess environment by `WorkerEnv` in `anvilml-worker`.
Do not set them manually in production; they are listed here for test reference.

| Variable | Type | Set by | Description |
|:---------|:-----|:-------|:------------|
| `ANVILML_IPC_PORT` | u16 decimal | `env.rs` | TCP port of the ROUTER socket |
| `ANVILML_WORKER_ID` | string | `env.rs` | Bare device index as a string (e.g. `"0"`) — this is also the ZMQ DEALER identity the worker registers with the ROUTER. NOT the `"worker-N"` display label used elsewhere for logging/UI. |
| `ANVILML_DEVICE_INDEX` | u32 decimal | `env.rs` | GPU device index |
| `ANVILML_DEVICE_TYPE` | string | `env.rs` | `"cuda"`, `"rocm"`, or `"cpu"` |
| `ANVILML_LOG_LEVEL` | string | `env.rs` | Forwarded from server config |
| `ANVILML_MAX_IPC_PAYLOAD_MIB` | u32 decimal | `env.rs` | Forwarded from server config |

### 3.5 Test / CI / Mock Variables

| Variable | Purpose | Values |
|:---------|:--------|:-------|
| `ANVILML_WORKER_MOCK` | Python worker mock mode — no torch, sentinel outputs | `1` = mock; unset = real |
| `ANVILML_MOCK_DEVICE_TYPE` | Mock hardware device type | `cuda`, `rocm`, `cpu` |
| `ANVILML_MOCK_VRAM_MIB` | Mock hardware VRAM | integer MiB |
| `ANVILML_MOCK_DEVICE_NAME` | Mock hardware device name | any string |
| `ANVILML_MOCK_NODE_DELAY_MS` | Artificial delay per mock node execute | integer ms |
| `ANVILML_FORCE_WORKER_MOCK` | Forces `ANVILML_WORKER_MOCK=1` into the worker subprocess even when compiled without `mock-hardware`. Set in the supervisor's own shell env before launching `anvilml`. | `1` = force mock; unset or any other value = no effect |

`ANVILML_WORKER_MOCK` is set by the Rust supervisor when the `mock-hardware` cargo
feature is active. Tests that spawn real worker subprocesses must set it themselves
within the test scope and restore it unconditionally on exit (see §11.3).
`ANVILML_FORCE_WORKER_MOCK` is a second, independent trigger for the same effect, checked at runtime regardless of the `mock-hardware` feature. It exists for manually pairing a real-hardware binary with a mock Python worker (e.g. local testing without GPU/torch). It does not affect `anvilml-hardware`'s GPU detection — only the Python worker side is mocked.

---

## 4. Config Field Reference (`anvilml.toml`)

The checked-in `anvilml.toml` at the repo root is the canonical reference config.
Every field in `ServerConfig` must appear in `anvilml.toml` at its documented default.
The `config_reference` test (Gate 1 in §8) enforces this automatically.

Config precedence (lowest to highest):
1. Compiled-in defaults (`ServerConfig::default()`)
2. `anvilml.toml` (path set by `--config`, default `./anvilml.toml`)
3. `ANVILML_*` environment variables
4. CLI flags (`--host`, `--port`, `--config`)

### Top-level scalar fields

| Field | Type | Default | Description |
|:------|:-----|:--------|:------------|
| `host` | string (IP) | `"127.0.0.1"` | Bind address |
| `port` | u16 | `8488` | HTTP port |
| `db_path` | path | `"./anvilml.db"` | SQLite database file |
| `artifact_dir` | path | `"./artifacts"` | Generated image storage directory |
| `num_threads` | u32? | `null` (= num_cpus) | Tokio worker thread count |
| `venv_path` | path | `"./worker/.venv"` | Python venv root |
| `max_ipc_payload_mib` | u32 | `256` | Maximum IPC message payload in MiB |
| `seeds_path` | path | `"./database/seeds"` | SQL seed files directory |
| `log_level` | string | `"info"` | Logging level forwarded to worker subprocesses |

### `[[model_dirs]]` (array of tables)

Each entry configures one model directory:

| Field | Type | Description |
|:------|:-----|:------------|
| `path` | path | Directory to scan |
| `recursive` | bool | Scan subdirectories (default `false`) |
| `max_depth` | u32? | Maximum scan depth when `recursive = true` |

### `[gpu_selection]`

| Field | Type | Default | Description |
|:------|:-----|:--------|:------------|
| `default_device` | string | `"auto"` | `"auto"` \| `"cpu"` \| integer device index as string |

### `[limits]`

| Field | Type | Default | Description |
|:------|:-----|:--------|:------------|
| `max_queued_jobs` | u32 | `100` | Maximum jobs allowed in Queued state simultaneously |
| `max_concurrent_jobs` | u32 | `1` | Maximum jobs dispatched simultaneously (one per GPU) |

### `[rocm]` (optional)

| Field | Type | Default | Description |
|:------|:-----|:--------|:------------|
| `hsa_override_gfx_version` | string? | `null` | Override `HSA_OVERRIDE_GFX_VERSION` for unsupported GFX targets |

### `[hardware_override]` (optional — CI and isolated test use only)

| Field | Type | Description |
|:------|:-----|:------------|
| `device_type` | string | `"cuda"`, `"rocm"`, or `"cpu"` |
| `vram_total_mib` | u32 | VRAM to report |

**NEVER** include `[hardware_override]` in a release build or production config.

---

## 5. Python Worker Runtime

### Interpreter paths

| Platform | Path |
|:---------|:-----|
| Linux / macOS | `{venv_path}/bin/python3` |
| Windows | `{venv_path}\Scripts\python.exe` |

### Preflight checks (run at server startup)

1. Interpreter path exists and is executable.
2. Python version is `3.12.x` (WARN logged if not, but does not abort).
3. When `ANVILML_WORKER_MOCK` is unset: `import torch` succeeds. On failure, workers
   remain `Dead` and job submissions return `503 workers_unavailable`.

### Worker startup sequence

```
env vars injected by Rust
    ↓
ipc.connect(ANVILML_IPC_PORT, ANVILML_WORKER_ID)
    ↓
_probe_hardware()   ← reads device; no-op in mock mode
    ↓
_import_nodes()     ← triggers NODE_REGISTRY population
    ↓
ipc.send_event(Ready { node_types: [...all NodeTypeDescriptors...] })
    ↓
message dispatch loop
```

The `Ready` event is the synchronisation point between Rust and Python. The Rust supervisor
transitions the worker to `Idle` only on receipt of a valid `Ready` event. A worker that
does not emit `Ready` within 60 seconds is killed and respawned.

---

## 6. Build, Format, Lint, and Test Commands

ACT agents run these commands in the sequence below. Exit codes are binding: non-zero
means blocked. Record verbatim output in the implementation report for every step.

### Step 1 — Implement

Write source code, tests, and CI changes per the approved plan scope.

### Step 2 — Version bump

For every crate whose source files were modified, bump the patch version.
See §12 for the exact procedure and manifest locations.

### Step 3 — Format (pass 1, in-place)

```bash
cargo fmt --all
```

Run immediately after implementing. If the formatter exits non-zero, fix the cause
before proceeding. Do not continue with unformatted code.

### Step 4 — Lint

```bash
# With mock-hardware (required — all CI runs use this)
cargo clippy --workspace --features mock-hardware -- -D warnings

# With real-hardware (required — verifies cfg-gated real paths)
cargo clippy --bin anvilml -- -D warnings
```

Zero warnings required. Fix every warning, including pre-existing ones in any file
this task modifies. Document pre-existing fixes in `## Deviations from Plan`.
Never skip a warning by documenting it.

### Step 5 — Platform cross-check

See §7. Run all four checks. Record verbatim output.

### Step 6 — Rust tests

```bash
cargo test --workspace --features mock-hardware
```

Zero failures required. If a test passes on retry, diagnose — see §11.3 for isolation
requirements. Do not accept flakiness without diagnosis.

### Step 7 — Python syntax/compile check (mandatory, run before Step 8)

```bash
# Linux / macOS:
worker/.venv/bin/python -m py_compile $(git ls-files 'worker/*.py')
# Windows:
# worker\.venv\Scripts\python -m py_compile (git ls-files 'worker/*.py')
```

Must exit 0 before Step 8 (pytest) is invoked. This step exists because a `SyntaxError`
in any module that a test reaches via subprocess (e.g. `worker_main.py`, invoked as
`python -m worker.worker_main`) does not surface as a normal test failure: the subprocess
dies on import, before it connects IPC or sends its first message. A test that then blocks
on a synchronous `socket.recv()` waiting for that message hangs indefinitely rather than
failing — this is exactly what happened with `test_mock_startup_sends_ready` when a
malformed `elif` was introduced after an `except` block in `worker_main.py`. `py_compile`
catches the same defect in milliseconds, before any subprocess is spawned, and names the
exact file and line in its error output.

This step is **mandatory for every task that creates or modifies any `.py` file** under
`worker/`, not only tasks that touch `worker_main.py` directly — any module reachable by
import from a subprocessed entry point shares this failure mode. If this step fails, fix
the syntax error and re-run it before proceeding to Step 8. Do not attempt to diagnose a
hung or failing pytest run before confirming this step passes.

### Step 8 — Python tests

```bash
# Linux / macOS:
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v
# Windows:
# ANVILML_WORKER_MOCK=1 worker\.venv\Scripts\python -m pytest worker/tests/ -v
```

Zero failures required. Run from the repo root. Invoke the venv interpreter directly —
do not use bare `python`. **Never run this step before Step 7 has exited 0.** Running
pytest against a syntactically broken module is the proximate cause of the indefinite
hang Step 7 exists to prevent.

If a Python test that spawns a worker subprocess (any test using `subprocess.Popen`
with `worker.worker_main` or another worker entry point) appears to hang for more than
~10 seconds, do not wait it out — abort it, re-run Step 7, and treat a Step 7 failure as
the most likely cause before investigating IPC timing or socket logic.

### Step 9 — Project gates

Run all applicable gates from §8. Record verbatim output.

### Step 10 — Format (pass 2, check-only gate)

```bash
cargo fmt --all -- --check
```

Must exit 0 before staging. If non-zero (drift introduced by lint or test fixes):
1. Run `cargo fmt --all` (pass 3 in-place).
2. Run `cargo check --workspace --features mock-hardware` — if this fails, set
   Status=BLOCKED and STOP. Do not stage code that does not compile after formatting.
3. Run `cargo fmt --all -- --check` again to confirm.

### Step 11 — Stage

```bash
git add -A
```

Do **NOT** `git commit` or `git push`. The Forge owns all git operations.

### Step 12 — Report, state update, STOP

Write the implementation report and update `.forge/state/CURRENT_TASK.md`.

### GitHub CI job matrix (reference)

The following jobs run automatically on every push to `main` (`.github/workflows/ci.yml`).
They are not steps the agent executes — they are listed here so agents understand what
the CI infrastructure validates.

| Job | Runner (matrix) | Steps |
|:----|:----------------|:------|
| `rust` | ubuntu-latest, windows-latest | Provision worker venv; `cargo fmt --all -- --check` (Linux only); `cargo clippy --workspace --features mock-hardware -- -D warnings`; `cargo test --workspace --features mock-hardware` |
| `worker` | ubuntu-latest, windows-latest | Provision worker venv; `ANVILML_WORKER_MOCK=1 <matrix-python> -m pytest worker/tests -v` |
| `openapi-drift` | ubuntu-latest | `cargo run -p anvilml-openapi`; `git diff --exit-code api/openapi.json` |
| `config-drift` | ubuntu-latest | `cargo test -p anvilml --features mock-hardware -- config_reference` |

**The real `worker` CI job does not run a `py_compile` step.** Step 7's mandatory
local `py_compile` check exists for exactly the reason described above (the
`test_mock_startup_sends_ready` incident), but CI currently has no equivalent backstop —
if Step 7 is skipped locally, a syntax error reaches `pytest worker/tests -v` directly in
CI, with whatever hang or failure mode that produces. Do not assume CI catches what a
skipped Step 7 would have caught; this gap is real, not theoretical, and should be raised
with the project owner rather than silently relied upon as a safety net.

The `worker` job's Windows matrix entry is required because the Python worker runs on
Windows in production (ROCm on Windows is a mandatory MVP backend). Pytest failures on
Windows that do not reproduce on Linux indicate platform-specific path handling,
line-ending issues, or Windows-only socket behaviour in the worker code.

The `rust` job provisions `worker/.venv` via `scripts/install_worker_deps.sh` (Linux) or
`scripts\install_worker_deps.ps1` (Windows) before any test step, even though it runs no
Python tests itself — this ensures the venv interpreter and `base.txt` dependencies are
present for any Rust test that spawns a Python subprocess. The `rust` job does not set
`ANVILML_WORKER_MOCK` itself — the `mock-hardware` feature flag causes
`build_worker_env()` to inject `ANVILML_WORKER_MOCK=1` into the spawned subprocess
automatically.

---

## 7. Platform Cross-Check (Local WSL2 Gate)

This gate runs locally in WSL2 before every push to `main`. It is **not** a GitHub CI
job. Run all four commands in order and record verbatim output in `## Platform Cross-Check`.

```bash
# 1. Mock-hardware Linux (exercises #[cfg(unix)] scaffold and mock paths)
cargo check --workspace --features mock-hardware

# 2. Mock-hardware Windows (exercises #[cfg(windows)] code paths)
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu

# 3. Real-hardware Linux (exercises real Vulkan/sysfs paths, no mock)
cargo check --bin anvilml

# 4. Real-hardware Windows (exercises real DXGI/NVML paths on Windows target)
cargo check --bin anvilml --target x86_64-pc-windows-gnu
```

All four must exit 0. A failure on any check blocks staging. If check 3 or 4 fails due
to a missing platform API, the fix is a `#[cfg(...)]` guard or a fallback implementation —
never a skip of the check.

---

## 8. Project Gates

### Gate 1 — Config Surface Sync

**Trigger:** any task that adds, renames, or removes a field on `ServerConfig` or any
nested config struct.

**Required actions (same task, not a follow-up):**
1. Update `anvilml.toml` with the new key at its documented default.
2. Update `docs/ENVIRONMENT.md §4` with the new field.
3. Run the gate and confirm it passes.

```bash
cargo test -p anvilml --features mock-hardware -- config_reference
```

Must exit 0. This test asserts that the key set of `ServerConfig::default()` (serialised
to TOML) exactly matches the key set of the checked-in `anvilml.toml`.

### Gate 2 — OpenAPI Drift

**Trigger:** any task that modifies:
- Handler function signatures in `anvilml-server/src/handlers/*.rs`
- `#[utoipa::path]` annotations or `ToSchema` derives
- `AppState` fields used in response types

```bash
cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json
```

If `git diff` is non-empty, the `openapi.json` is stale. Regenerate and stage:

```bash
cargo run -p anvilml-openapi
git add api/openapi.json
```

Re-run the gate to confirm idempotency (must exit 0).

**Skip only if** `api/openapi.json` does not yet exist (prior to the phase that
introduces the `anvilml-openapi` binary). Once it exists, the gate is always required
when the trigger conditions are met.

### Gate 3 — Node Parity

**Trigger:** any task that adds, removes, or renames a node type in `worker/nodes/`,
or modifies `crates/anvilml-core/src/node_registry.rs`.

```bash
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_parity.py -v
```

This test verifies that the set of types in `NODE_REGISTRY` (Python) matches the set
expected by the scheduler's integration tests. It does not enforce a compile-time
constant — it enforces runtime consistency.

---

## 9. Logging Conventions

All code must follow the logging conventions in `FORGE_AGENT_RULES.md §11`. This section
provides the AnvilML-specific mandatory log points that §11 requires.

### Level assignment

| Level | Use for |
|:------|:--------|
| `ERROR` | Unrecoverable failures causing a subsystem to abort. Always include `error=`. |
| `WARN` | Recoverable anomalies — execution continues. Include `error=` only when it adds information beyond what the other structured fields already convey. A "file not found" error on a `path=` field that already names the file: omit `error=`. A permission-denied or unexpected OS error: include `error=`. |
| `INFO` | Operational lifecycle events, always visible at the default log level. See mandatory list below. |
| `DEBUG` | Internal state useful for diagnosis. See mandatory list below. |
| `TRACE` | Per-iteration or per-byte detail. Use sparingly. |

### Mandatory INFO log points

Every task that touches the relevant subsystem must verify these log calls exist.
If absent in any file the task modifies, add them. A task is not complete if a
mandatory INFO log point is absent.

| Subsystem | Event | Required fields |
|:----------|:------|:----------------|
| Database | SQLite file created (did not previously exist) | `path=` |
| Database | Each migration applied | `migration=`, `version=` |
| Database | All migrations already up-to-date (no-op) | `migrations_applied=0` |
| Seeds | Seed file applied (SHA256 changed or first run) | `file=`, `sha256=` |
| Seeds | Seed file skipped (SHA256 unchanged) | `file=`, `status=up-to-date` |
| Server | Bind address on successful listen | `addr=` |
| Server | Graceful shutdown initiated | `reason=` |
| Hardware | Each detected device at startup | `index=`, `name=`, `device_type=`, `vram_total_mib=`, `fp8=` |
| Workers | Worker spawned | `worker_id=`, `device_index=`, `pid=` |
| Workers | Worker reached Ready | `worker_id=`, `device=`, `torch_version=`, `fp8=`, `node_count=` |
| Workers | Worker exited unexpectedly | `worker_id=`, `exit_code=` |
| Workers | Worker respawning | `worker_id=`, `attempt=`, `delay_ms=` |
| Scheduler | Job dispatched | `job_id=`, `worker_id=` |
| Scheduler | Job completed | `job_id=`, `elapsed_ms=` |
| Scheduler | Job failed | `job_id=`, `error=` |
| Model scan | Scan completed | `count=`, `dir=` |
| Provisioning | Provisioning started | `reason=` |
| Provisioning | Provisioning completed | `elapsed_ms=` |

### Mandatory DEBUG log points

| Subsystem | Event | Required fields |
|:----------|:------|:----------------|
| IPC | Message sent to a worker | `worker_id=`, `msg_type=` |
| IPC | Event received from a worker | `worker_id=`, `event_type=` |
| Scheduler | Job state transition | `job_id=`, `from=`, `to=` |
| Scheduler | VRAM reservation | `device_index=`, `reserved_mib=`, `free_after_mib=` |
| Hardware | Detection fallback used | `fallback=` (e.g. `"dxgi"`, `"sysfs"`) |
| Node registry | Registry updated from worker Ready | `worker_id=`, `node_count=` |
| Model scan | Each file examined | `path=`; if skipped: `reason=` |

### Structured field notation

Always use structured field notation — never format values into the message string:

```rust
// Correct:
tracing::info!(worker_id = %worker_id, device = %device_name, "worker ready");

// Wrong — not indexable by log aggregators:
tracing::info!("worker {} is ready on {}", worker_id, device_name);
```

Python workers use the `logging` module at the same levels. Use `extra={}` for structured
fields where the handler supports it.

### `#[tracing::instrument]` obligation

Apply `#[tracing::instrument]` to every async function that represents a meaningful unit
of work: migration runner, seed loader, worker spawn, job dispatch, model scan.

Span names must be lowercase `snake_case` matching the function name. Do not instrument
tight inner loops or per-frame functions.

---

## 10. Inline Documentation Obligation

This obligation is **unconditional** and applies to every source file touched by a task,
whether or not the task's primary purpose is documentation. A task is not complete if
these requirements are not met in any file it modifies.

### Rust

**Every `pub` item** (`pub fn`, `pub struct`, `pub enum`, `pub trait`, `pub const`,
`pub type`, `pub mod`) **must have a `///` doc comment** that describes:
- What it *does* (not what it *is*).
- Any non-obvious preconditions or postconditions.
- For `fn`: what each argument represents and what is returned or what error variants
  can be returned.

**Every non-trivial decision point** in function bodies must have an inline `//` comment
explaining *why* the branch was taken, the value was chosen, or the fallback was selected.
"Non-trivial" means: anything that would not be immediately obvious to a competent Rust
developer unfamiliar with this codebase reading the code for the first time. If uncertain,
comment it.

**Examples of required inline comments:**

```rust
// WRONG — no explanation of why this specific limit exists:
if payload_len > max_bytes {
    return Err(AnvilError::PayloadTooLarge(...));
}

// CORRECT — explains the constraint's origin:
// Enforce the payload cap before allocating. A malicious or buggy worker could
// send a frame header claiming gigabytes of payload; we reject before alloc.
// The limit is configurable via max_ipc_payload_mib in ServerConfig.
if payload_len > max_bytes {
    return Err(AnvilError::PayloadTooLarge(...));
}
```

```rust
// WRONG — a match arm with no explanation for a non-obvious case:
DeviceType::Cpu => None,

// CORRECT:
// CPU devices never have a PCI vendor ID; they are synthesised by CpuDetector.
DeviceType::Cpu => None,
```

**`lib.rs` files** contain only `pub mod`, `pub use`, and crate-level `//!` doc comments.
No implementation code. Crate-level doc comment is mandatory and must describe what the
crate owns and its hard constraints (e.g. "Zero I/O. Zero async.").

### Python

**Every `class` and every non-trivial `def`** must have a docstring using Google style:
```python
def connect(port: int, worker_id: str) -> None:
    """Connect DEALER socket to the ROUTER at *port*.

    Must be called exactly once before any send/recv operation.

    Args:
        port: TCP port on 127.0.0.1 where the Rust ROUTER is bound.
        worker_id: Stable worker identity string — the bare device index
            as injected via ANVILML_WORKER_ID in production (e.g. "0").

    Raises:
        RuntimeError: If called more than once.
    """
```

**Every non-trivial decision point** in function bodies must have an inline `#` comment
explaining the *why*. The same standard as Rust applies: comment what is not immediately
obvious to a competent Python developer unfamiliar with the codebase.

### What "non-trivial" means in practice

These patterns always require a comment:
- A guard condition that prevents an edge case (explain the edge case).
- A fallback path (explain why the primary failed and what the fallback does).
- A `cfg` attribute or conditional import (explain what the condition guards).
- A magic number or constant (explain its origin or meaning).
- A `#[allow(...)]` suppression (explain why it is legitimate here).
- Any `unsafe` block (explain the invariant being upheld).
- A platform-specific code path (explain what the platform constraint is).
- A ZeroMQ socket option setting (explain what behaviour it controls).

---

## 11. Test File Conventions

### 11.1 Rust test file placement

**Inline `#[cfg(test)]` blocks are not permitted** in Rust source files except for trivial
unit tests of a single pure function (≤ 20 lines total, no external test helpers, no I/O).

All other tests go in `crates/{name}/tests/` as separate Rust test crate files. These
are compiled as independent test crates and use the crate's public API — which forces
correct API design and catches visibility mistakes.

```
crates/anvilml-worker/
├── src/
│   └── managed.rs      ← no #[cfg(test)] block here unless trivial
└── tests/
    └── managed_tests.rs ← all managed.rs tests live here
```

Integration tests that require the full server live in `backend/tests/`.

### 11.2 Python test file placement

All Python tests live in `worker/tests/`. One test file per source module:

```
worker/
├── ipc.py
└── tests/
    └── test_ipc.py     ← tests for ipc.py
```

Test files import only the public interface of the module under test. If a test needs
to reach private internals, the internals are not correctly encapsulated.

`conftest.py` contains only shared pytest fixtures. It must not contain test functions.

### 11.3 Test isolation rules

These rules are mandatory. A task that introduces an isolation defect must fix it before
marking the task complete.

**Database isolation:** every test that uses SQLite must get its own `open_in_memory()`
connection. Tests must never share a database connection.

**Environment variable isolation:** every test that sets `std::env::set_var` (Rust) or
`os.environ[...] =` (Python) must:
1. Capture the pre-existing value before mutating (or record that it was absent).
2. Restore the original value — or remove the var if it was absent — as an
   **unconditional final step** outside any conditional or assertion block, so
   teardown runs even on panic or early return.
3. Never rely on inherited env state from a prior test in the same runner process.
4. Be annotated `#[serial]` (Rust, via the `serial_test` crate) or placed in a
   `serial` pytest group (Python) — because `std::env` and `os.environ` are
   process-global. Capture-and-restore prevents leaking state between *sequential*
   tests but does not prevent a *concurrent* test thread from observing the mutated
   value mid-flight. `#[serial]` serialises execution of all tests in the same binary
   that share this annotation, eliminating the race window.

```rust
// Correct Rust pattern:
#[serial]  // required — env vars are process-global; concurrent tests race on set_var
fn test_example() {
    let prior = std::env::var("ANVILML_MOCK_VRAM_MIB").ok();
    std::env::set_var("ANVILML_MOCK_VRAM_MIB", "16384");
    // ... test body ...
    match prior {
        Some(v) => std::env::set_var("ANVILML_MOCK_VRAM_MIB", v),
        None => std::env::remove_var("ANVILML_MOCK_VRAM_MIB"),
    }
}
```

**`#[serial]` usage:** mandatory for any test that mutates process-global state
(env vars, process-wide signal handlers, global singletons). Also permitted for tests
where the shared resource is physically singular (e.g. a hardware device detected from
the OS). It must not be used for any other reason — port conflicts, database locks, and
temp file collisions are isolation defects that must be fixed structurally. When used,
it must be justified in `## Deviations from Plan`.

**`#[ignore]`:** not permitted in committed code. A test that cannot pass is either
fixed or deleted. An ignored test is a silent failure and will be treated as a
task defect.

### 11.4 Test documentation obligation

Every test must have a doc comment (`///` in Rust, docstring in Python) that describes:
- What behaviour or invariant the test verifies.
- What precondition or setup state it requires.
- What the expected outcome is.

Additionally, every test **must be catalogued in `docs/TESTS.md`** using the format
defined in `ANVILML_DESIGN.md §16.1`. This catalogue is updated as part of the same
task that adds or modifies the test. A task that adds tests but does not update
`docs/TESTS.md` is incomplete.

### 11.5 Bounded waits in subprocess/IPC tests (mandatory)

Any Python test that spawns a worker subprocess and then blocks on a socket call to
observe its output (`router.recv()`, `proc.wait()`, `proc.communicate()`, or equivalent)
**must bound that wait**. An unbounded blocking call on a subprocess's IPC output has no
graceful failure mode if the subprocess dies before producing that output — it hangs
forever instead of failing the test. This is not hypothetical: it is the exact mechanism
behind the `test_mock_startup_sends_ready` incident, where a `SyntaxError` in
`worker_main.py` caused the worker subprocess to exit immediately, and the test's
unguarded `router.recv()` blocked indefinitely waiting for a `Ready` event that could
never arrive.

**Required pattern for ZeroMQ ROUTER/DEALER tests:**

```python
# Set a receive timeout before any blocking recv() call.
router.setsockopt(zmq.RCVTIMEO, 5000)  # milliseconds; 5s is the project default
try:
    identity = router.recv()
    raw = router.recv()
except zmq.Again:
    # No message arrived within the timeout. Surface the subprocess's stderr —
    # this is almost always a worker startup failure, not a slow worker.
    proc.terminate()
    stdout, stderr = proc.communicate(timeout=5)
    pytest.fail(
        f"worker did not send expected message within timeout. "
        f"stderr={stderr.decode(errors='replace')}"
    )
```

**Required pattern for `subprocess.Popen.wait()` calls:** always pass an explicit
`timeout=` argument (the existing `test_shutdown_exits_cleanly` test already does this
correctly — `proc.wait(timeout=10)` — and is the reference pattern). Never call
`.wait()` or `.communicate()` without a `timeout=`.

**Why surface stderr on timeout, not just fail:** a bare `pytest.fail("timed out")`
forces the next person to re-run the test under a debugger to find a defect that the
subprocess already reported on its own stderr at the moment it died. Capturing and
including `stderr` in the failure message turns a multi-minute manual investigation
into an immediate, self-explanatory diagnosis from the first failed CI run.

This rule applies retroactively: any task that touches a test file already containing
an unguarded blocking call on subprocess IPC must add a timeout as part of that task,
even if the timeout is unrelated to the task's stated goal. Record this under
`## Deviations from Plan` per the usual convention for incidental fixes.

---

## 12. Crate Version Bump Convention

Every task that modifies source files inside a Rust crate must increment that crate's
patch version (`Z` in `X.Y.Z`) before staging. Only `Z` changes; `X` and `Y` are
manually controlled. The workspace release version (`[workspace.package] version` in
the root `Cargo.toml`) is **read-only** — never modify it in a task.

### Version file locations

| Crate / Package | Manifest | Version field |
|:----------------|:---------|:--------------|
| `backend` (binary) | `backend/Cargo.toml` | `[package] version` |
| `anvilml-core` | `crates/anvilml-core/Cargo.toml` | `[package] version` |
| `anvilml-hardware` | `crates/anvilml-hardware/Cargo.toml` | `[package] version` |
| `anvilml-registry` | `crates/anvilml-registry/Cargo.toml` | `[package] version` |
| `anvilml-artifacts` | `crates/anvilml-artifacts/Cargo.toml` | `[package] version` |
| `anvilml-ipc` | `crates/anvilml-ipc/Cargo.toml` | `[package] version` |
| `anvilml-worker` | `crates/anvilml-worker/Cargo.toml` | `[package] version` |
| `anvilml-scheduler` | `crates/anvilml-scheduler/Cargo.toml` | `[package] version` |
| `anvilml-server` | `crates/anvilml-server/Cargo.toml` | `[package] version` |
| `anvilml-openapi` | `crates/anvilml-openapi/Cargo.toml` | `[package] version` |

### Bump procedure

```bash
# 1. Read the current version for the crate being modified:
grep '^version' crates/anvilml-registry/Cargo.toml
# → version = "0.1.4"

# 2. Write back with Z+1, X.Y unchanged:
# → version = "0.1.5"
# Target only the [package] version line.
```

Do not edit `Cargo.lock` — cargo regenerates it on the next build. Cross-crate path
dependencies in this workspace carry no version pins, so no cascade update to sibling
`Cargo.toml` files is needed when bumping a dependency's version.

### Python packages

There is no separate version file for Python worker code. Python version management
is handled at the workspace level via `worker/requirements/*.txt`. Individual Python
modules do not carry their own version identifiers.