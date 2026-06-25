# ENVIRONMENT.md — AnvilML Build, Test, and Configuration Reference

**Document:** `docs/ENVIRONMENT.md`
**Location in repo:** `AnvilML/docs/ENVIRONMENT.md`
**Read by:** The Forge `forge-plan` and `forge-act` agents at the start of every session.
**Authoritative for:** all build, format, lint, cross-check, test, and gate commands;
  environment variable reference; config field reference; cache cleanup procedure;
  version bump procedure; logging field conventions; and the inline documentation
  obligation.

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
13. [Mandatory Build Cache Cleanup](#13-mandatory-build-cache-cleanup)

---

## 1. Development Environment Prerequisites

### Rust

```bash
# Install rustup (if not present)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# The correct toolchain is pinned in rust-toolchain.toml and activated automatically.
# Verify:
rustc --version   # must print 1.96.0
cargo --version
```

**Toolchain pin: Rust 1.96.0, edition 2024.** Both are exact pins — not "stable" and
not "2021." Do not bump either without being asked. See `ANVILML_DESIGN.md §18.1` for
the verified dependency-compatibility table; if a `cargo update` moves a dependency
to a version not reflected there, look it up live (MCP/registry) rather than from
training-data memory, per `FORGE_AGENT_RULES.md §6.`

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
the matching torch build from the matching `worker/requirements/*.txt` file. They
never install `torch` from `base.txt` — `torch` never appears there, by design (see
`ANVILML_DESIGN.md §3.1`). See `ANVILML_DESIGN.md §18.6` for the full requirements-file
discipline.

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

# 4. Run Python mock-mode tests (invoke the venv interpreter directly — never bare python)
# Linux / macOS:
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v -m "not real_mode"
# Windows:
# ANVILML_WORKER_MOCK=1 worker\.venv\Scripts\python -m pytest worker/tests/ -v -m "not real_mode"

# 5. Run Python real-mode tests (no ANVILML_WORKER_MOCK; needs requirements/cpu-linux-agent.txt
#    or requirements/cpu-runner-reqs.txt installed first — see §5)
# Linux / macOS:
worker/.venv/bin/python -m pytest worker/tests/ -v -m real_mode
# Windows:
# worker\.venv\Scripts\python -m pytest worker/tests/ -v -m real_mode

# 6. Install the WSL2 Windows cross-check target (once, on WSL2 only)
rustup target add x86_64-pc-windows-gnu
sudo apt-get install gcc-mingw-w64-x86-64
```

**Both Step 4 and Step 5 are mandatory.** Real-path implementation is not optional in
this project (`ANVILML_DESIGN.md §10.6`, §17.3) — running only the mock suite does
not confirm a working environment. If Step 5 fails because `torch` is not installed,
that is a real environment gap to fix (install `requirements/cpu-linux-agent.txt`),
not a step to skip.

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
| `ANVILML_MODEL_SCAN_DEPTH` | `model_scan_depth` | `2` | Non-recursive scanner depth |
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

These are written into the subprocess environment by `WorkerEnv` in `anvilml-worker`
(`ANVILML_DESIGN.md §9.7`). Do not set them manually in production; they are listed
here for test reference.

| Variable | Type | Description |
|:---------|:-----|:------------|
| `ANVILML_IPC_PORT` | u16 decimal | TCP port of the ROUTER socket |
| `ANVILML_WORKER_ID` | string | Bare device index as a string (e.g. `"0"`) — this is also the ZMQ DEALER identity the worker registers with the ROUTER. |
| `ANVILML_DEVICE_INDEX` | u32 decimal | GPU device index |
| `ANVILML_DEVICE_TYPE` | string | `"cuda"`, `"rocm"`, or `"cpu"` |
| `ANVILML_LOG_LEVEL` | string | Forwarded from server config |
| `ANVILML_MAX_IPC_PAYLOAD_MIB` | u32 decimal | Forwarded from server config |

### 3.5 Test / CI / Mock Variables

| Variable | Purpose | Values |
|:---------|:--------|:-------|
| `ANVILML_WORKER_MOCK` | Python worker mock mode — skips the real torch capability probe, uses sentinel outputs (`ANVILML_DESIGN.md §14.3`) | `1` = mock; unset = real |
| `ANVILML_MOCK_DEVICE_TYPE` | Mock hardware device type | `cuda`, `rocm`, `cpu` |
| `ANVILML_MOCK_VRAM_MIB` | Mock hardware VRAM | integer MiB |
| `ANVILML_MOCK_DEVICE_NAME` | Mock hardware device name | any string |
| `ANVILML_MOCK_NODE_DELAY_MS` | Artificial delay per mock node execute | integer ms |
| `ANVILML_FORCE_WORKER_MOCK` | Forces `ANVILML_WORKER_MOCK=1` into the worker subprocess even when compiled without `mock-hardware`. Set in the supervisor's own shell env before launching `anvilml`. | `1` = force mock; unset = no effect |

`ANVILML_WORKER_MOCK` is set automatically by the Rust supervisor when the
`mock-hardware` cargo feature is active. `ANVILML_FORCE_WORKER_MOCK` is a second,
independent trigger for the same effect, checked at runtime regardless of the
`mock-hardware` feature — it exists for manually pairing a real-hardware binary with
a mock Python worker (e.g. local testing without GPU/torch). Neither variable
affects `anvilml-hardware`'s GPU detection — only the Python worker side is mocked.

**There is no environment variable that disables the real-path code branch
entirely.** `ANVILML_WORKER_MOCK` selects between two equally-maintained branches
(`ANVILML_DESIGN.md §14.1`); it never gates an unwritten or stubbed-out real path.

---

## 4. Config Field Reference (`anvilml.toml`)

The checked-in `anvilml.toml` at the repo root is the canonical reference config.
Every field in `ServerConfig` must appear in `anvilml.toml` at its documented default.
The `config_reference` test (Gate 1 in §8) enforces this automatically.

Config precedence (lowest to highest), per `ANVILML_DESIGN.md §15`:
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
| `venv_path` | path | `"./worker/.venv"` | Python venv root |
| `model_scan_depth` | u32 | `2` | Non-recursive scanner depth (`ANVILML_DESIGN.md §7.4`) |
| `max_ipc_payload_mib` | u32 | `256` | Maximum IPC message payload in MiB |
| `num_threads` | u32? | `null` (= num_cpus) | Tokio worker thread count |

### `[[model_dirs]]` (array of tables)

Each entry configures one model directory:

| Field | Type | Description |
|:------|:-----|:------------|
| `path` | path | Directory to scan |
| `recursive` | bool | Scan subdirectories (default `false`) |
| `max_depth` | u32? | Maximum scan depth when `recursive = true` — caps at `model_scan_depth` if both set |

### `[gpu_selection]`

| Field | Type | Default | Description |
|:------|:-----|:--------|:------------|
| `default_device` | string | `"auto"` | `"auto"` \| `"cpu"` \| integer device index as string |

### `[limits]`

| Field | Type | Default | Description |
|:------|:-----|:--------|:------------|
| `max_queued_jobs` | u32 | `100` | Maximum jobs allowed in `Queued` state simultaneously |

### `[rocm]` (optional)

| Field | Type | Default | Description |
|:------|:-----|:--------|:------------|
| `hsa_override_gfx_version` | string? | `null` | Override `HSA_OVERRIDE_GFX_VERSION` for unsupported GFX targets |

### `[hardware_override]` (optional — CI and isolated test use only)

| Field | Type | Description |
|:------|:-----|:------------|
| `device_type` | string | `"cuda"`, `"rocm"`, or `"cpu"` |
| `vram_total_mib` | u32 | VRAM to report |

Read first in detection priority order (`ANVILML_DESIGN.md §6.4`, step 1).
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
   remain `Dead` and job submissions return `503 workers_unavailable`. This is a
   genuine environment defect to fix (install the matching `requirements/*.txt`),
   never a reason to fall back to mock mode silently — `ANVILML_DESIGN.md §14.1`
   prohibits any code path that substitutes mock behaviour for a failed real-mode
   precondition.

### Worker startup sequence — real mode (`ANVILML_WORKER_MOCK` unset)

```
env vars injected by Rust
    ↓
ipc.connect(ANVILML_IPC_PORT, ANVILML_WORKER_ID)
    ↓
import torch; select device (torch.cuda.set_device / ROCm equivalent; "cpu" skips this)
    ↓
capability.probe_capabilities(device_type, device_index)   ← REAL torch-level probe
    ↓
_import_nodes()     ← triggers NODE_REGISTRY population
    ↓
ipc.send_event(Ready { capabilities_source: "pytorch", node_types: [...] })
    ↓
message dispatch loop
```

### Worker startup sequence — mock mode (`ANVILML_WORKER_MOCK=1`)

```
env vars injected by Rust
    ↓
ipc.connect(ANVILML_IPC_PORT, ANVILML_WORKER_ID)     ← identical to real mode
    ↓
_mock_probe_capabilities()   ← fixed synthetic values; never imports torch
    ↓
_import_nodes()               ← identical to real mode
    ↓
ipc.send_event(Ready { capabilities_source: "mock", node_types: [...] })
    ↓
message dispatch loop
```

---

## 6. Build, Format, Lint, and Test Commands

ACT agents run these commands in the sequence below. Exit codes are binding: non-zero
means blocked. Record verbatim output in the implementation report for every step.

### Step 1 — Implement

Write source code, tests, and CI changes per the approved plan scope. If the task
touches a node's `execute()` or an arch module's `load()`/`sample()`/`decode()`, both
a mock-mode and a real-mode test are part of this step, not a follow-up
(`ANVILML_DESIGN.md §10.6`, §17.3).

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

### Step 8 — Python mock-mode tests

```bash
# Linux / macOS:
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v -m "not real_mode"
# Windows:
# ANVILML_WORKER_MOCK=1 worker\.venv\Scripts\python -m pytest worker/tests/ -v -m "not real_mode"
```

Zero failures required. Run from the repo root. Invoke the venv interpreter directly —
do not use bare `python`. **Never run this step before Step 7 has exited 0.** Running
pytest against a syntactically broken module is the proximate cause of the indefinite
hang Step 7 exists to prevent.

If a Python test that spawns a worker subprocess (any test using `subprocess.Popen`
with `worker.worker_main` or another worker entry point) appears to hang for more than
~10 seconds, do not wait it out — abort it, re-run Step 7, and treat a Step 7 failure as
the most likely cause before investigating IPC timing or socket logic.

### Step 9 — Python real-mode tests (mandatory — never optional, never deferred)

```bash
# Linux / macOS:
worker/.venv/bin/python -m pytest worker/tests/ -v -m real_mode
# Windows:
# worker\.venv\Scripts\python -m pytest worker/tests/ -v -m real_mode
```

`ANVILML_WORKER_MOCK` must be **unset** for this step — setting it would silently
skip the real torch-level code path this step exists to exercise. Zero failures
required. This step is not conditional on the task's stated scope: per
`ANVILML_DESIGN.md §10.6` and §17.3, any task touching a node or arch module ships
both a mock-mode and a real-mode test, and this is where the real-mode half runs.
A task that only ran Step 8 has not completed its own testing obligation, regardless
of how the task description was phrased.

If this step fails because `torch` is not importable, see §5's preflight note — fix
the environment, do not skip the step or substitute a mock-mode pass for it.

### Step 10 — Project gates

Run all applicable gates from §8. Record verbatim output.

### Step 11 — Format (pass 2, check-only gate)

```bash
cargo fmt --all -- --check
```

Must exit 0 before staging. If non-zero (drift introduced by lint or test fixes):
1. Run `cargo fmt --all` (pass 3 in-place).
2. Run `cargo check --workspace --features mock-hardware` — if this fails, set
   Status=BLOCKED and STOP. Do not stage code that does not compile after formatting.
3. Run `cargo fmt --all -- --check` again to confirm.

### Step 12 — Stage

```bash
git add -A
```

Do **NOT** `git commit` or `git push`. The Forge owns all git operations.

### Step 13 — Build cache cleanup (mandatory, every session — see §13)

```bash
cargo clean
find . -type d -name "__pycache__" -exec rm -rf {} +
find . -type d -name ".pytest_cache" -exec rm -rf {} +
rm -rf .mypy_cache .ruff_cache
```

Run after staging, as the last action before the report. This is not optional and
is not scoped to the crate(s) this task touched — see §13 for the full rule and the
reason it exists (a recorded 200GB cache accumulation across v2/v3).

### Step 14 — Report, state update, STOP

Write the implementation report and update `.forge/state/CURRENT_TASK.md`.

### GitHub CI job matrix (reference)

The following jobs run automatically on every push to `main`
(`.github/workflows/ci.yml`). They are not steps the agent executes — they are
listed here so agents understand what the CI infrastructure validates. Full detail:
`ANVILML_DESIGN.md §18.3`.

| Job | Runner | Steps |
|:----|:-------|:------|
| `rust-linux` | ubuntu-latest | `cargo fmt --all -- --check`, clippy, full Rust test suite (`--features mock-hardware`) |
| `rust-windows` | windows-latest | clippy, full Rust test suite (`--features mock-hardware`) — no `fmt --check` |
| `worker-linux-mock` | ubuntu-latest | Install `base.txt` (no torch); `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v -m "not real_mode"` |
| `worker-linux-real` | ubuntu-latest | Install `base.txt`, then `cpu-runner-reqs.txt`; `python -m pytest worker/tests -v -m real_mode` (mock unset) |
| `worker-windows-mock` | windows-latest | Same as `worker-linux-mock`, Windows paths |
| `worker-windows-real` | windows-latest | Same as `worker-linux-real`, Windows paths |
| `openapi-drift` | ubuntu-latest | `cargo run -p anvilml-openapi`; `git diff --exit-code api/openapi.json` |
| `config-drift` | ubuntu-latest | `cargo test -p anvilml --features mock-hardware -- config_reference` |

**The four `worker-*` jobs are split mock/real specifically so a real-mode CPU
failure is never masked by an unrelated mock-mode failure's exit code in the same
job** (`ANVILML_DESIGN.md §18.3` explains the attribution reasoning). CI runners
are ephemeral and discard their filesystem at job end — §13's cache cleanup rule
does not apply to CI, only to the persistent Forge agent VM and local dev.

The `worker-*-real` jobs' install order is fixed: `base.txt` → a mock-suite
collection check (`pytest --collect-only -m "not real_mode"`, confirms nothing in
the mock suite accidentally imports torch at collection time) → `cpu-runner-reqs.txt`
→ the real-mode suite. This order is identical on Linux and Windows.

The `rust-*` jobs provision `worker/.venv` before any test step, even though they
run no Python tests themselves — this ensures the venv interpreter and `base.txt`
dependencies are present for any Rust test that spawns a Python subprocess. They do
not set `ANVILML_WORKER_MOCK` themselves — the `mock-hardware` feature flag causes
`build_worker_env()` to inject it into the spawned subprocess automatically.

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

# 4. Real-hardware Windows (exercises real DXGI paths on Windows target)
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

### Gate 4 — Mock/Real Parity Markers

**Trigger:** any task that adds or modifies a node's `execute()`, or an arch module's
`load()`/`sample()`/`decode()`/`compute_latent_shape()` (`ANVILML_DESIGN.md §10.4`'s
fixed method-name contract).

```bash
# 1. Every REAL_PATH_VERIFIED / MOCK_PATH_VERIFIED marker names a real, collectible test
grep -rn "REAL_PATH_VERIFIED:\|MOCK_PATH_VERIFIED:" worker/nodes/ \
  | sed -E 's/.*(REAL|MOCK)_PATH_VERIFIED: *//' \
  | xargs -I{} worker/.venv/bin/python -m pytest --collect-only "{}" -q

# 2. Every public load()/sample()/decode()/compute_latent_shape()/execute() in
#    worker/nodes/ has BOTH markers present in the same file, near the function
grep -L "REAL_PATH_VERIFIED:" worker/nodes/**/*.py | grep -v __init__ | grep -v base.py
grep -L "MOCK_PATH_VERIFIED:" worker/nodes/**/*.py | grep -v __init__ | grep -v base.py
```

Both `grep -L` commands (files **lacking** a marker) must return empty for any file
defining a node or arch-module function in scope. A non-empty result is a finding,
treated with the same severity as an unmarked stub (`FORGE_AGENT_RULES.md §9a.1`) —
see `ANVILML_DESIGN.md §10.6` for the full rule and why it exists. This gate is new
in v4; there is no v3 equivalent because the marker convention itself is new.

---

## 9. Logging Conventions

All code must follow the logging conventions in `FORGE_AGENT_RULES.md §11`. The
mandatory AnvilML-specific log points that §11 requires are the full INFO/DEBUG
tables in `ANVILML_DESIGN.md §16.2–§16.3` — read those tables directly; they are not
duplicated here to avoid the two documents drifting out of sync with each other.

### Level assignment

| Level | Use for |
|:------|:--------|
| `ERROR` | Unrecoverable failures causing a subsystem to abort. Always include `error=`. |
| `WARN` | Recoverable anomalies — execution continues. Include `error=` only when it adds information beyond what the other structured fields already convey. |
| `INFO` | Operational lifecycle events, always visible at the default log level. See `ANVILML_DESIGN.md §16.2`. |
| `DEBUG` | Internal state useful for diagnosis. See `ANVILML_DESIGN.md §16.3`. |
| `TRACE` | Per-iteration or per-byte detail. Use sparingly. |

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
of work: migration runner, seed loader, worker spawn, job dispatch, model scan,
capability probe (`ANVILML_DESIGN.md §6.6`).

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

Every arch module's `load()` must document its dtype decision in the same comment
style — explain which `caps` field drove the choice (`ANVILML_DESIGN.md §11.5`), not
just state the chosen dtype:

```python
# WRONG — states the outcome with no reasoning:
target_dtype = torch.bfloat16

# CORRECT — names the capability field that drove the choice and the fallback chain:
# caps.fp8 is False on CPU (real probe always returns False here — never a bug to
# "fix"); caps.bf16 is True, so we upcast to bf16 per the fixed precedence in
# ANVILML_DESIGN.md §11.5.
target_dtype = torch.bfloat16
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
- A dtype/capability branch in an arch module's `load()` (explain which `caps` field
  drove the choice — see the Python example above).

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

**`pytest.ini` / `pyproject.toml` registers the `real_mode` marker** so pytest does not
warn on unregistered marker use:

```ini
[pytest]
markers =
    real_mode: exercises real torch-level code against a fixture checkpoint (no torch import in mock-mode collection)
```

A test with no marker is assumed mock-compatible and runs in both the mock and real
CI jobs (`ANVILML_DESIGN.md §18.3`) unless it imports `torch` unconditionally at
module level — only `real_mode`-marked tests may do that.

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

**Never `sys.modules.pop("torch")` + `importlib.reload()`** to test "this module
doesn't import torch at top level." This crashed the WSL2 agent VM at the OS level
(not a clean test failure) twice during prior development
(`ANVILML_DESIGN.md §17.4` rule 7). Use subprocess isolation instead: spawn a fresh
Python process, assert `"torch" not in sys.modules` inside that subprocess, check
its exit code from the test.

```python
# WRONG — can crash the host process, not just fail the test:
import sys, importlib
sys.modules.pop("torch", None)
import worker.nodes.loader  # re-trigger import side effects
assert "torch" not in sys.modules

# CORRECT — isolated in a subprocess; a crash there cannot take down the test runner:
import subprocess, sys
result = subprocess.run(
    [sys.executable, "-c", "import worker.nodes.loader; import sys; "
                            "assert 'torch' not in sys.modules; print('OK')"],
    capture_output=True, text=True, timeout=10,
)
assert result.returncode == 0, result.stderr
```

### 11.4 Test documentation obligation

Every test must have a doc comment (`///` in Rust, docstring in Python) that describes:
- What behaviour or invariant the test verifies.
- What precondition or setup state it requires.
- What the expected outcome is.

Additionally, every test **must be catalogued in `docs/TESTS.md`** using the format
defined in `ANVILML_DESIGN.md §17.1` — including the `Mode: mock | real | both` field,
which is new in v4 and must be filled in for every entry. This catalogue is updated
as part of the same task that adds or modifies the test. A task that adds tests but
does not update `docs/TESTS.md` is incomplete.

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
`timeout=` argument. Never call `.wait()` or `.communicate()` without a `timeout=`.

**Why surface stderr on timeout, not just fail:** a bare `pytest.fail("timed out")`
forces the next person to re-run the test under a debugger to find a defect that the
subprocess already reported on its own stderr at the moment it died. Capturing and
including `stderr` in the failure message turns a multi-minute manual investigation
into an immediate, self-explanatory diagnosis from the first failed CI run.

This rule applies retroactively: any task that touches a test file already containing
an unguarded blocking call on subprocess IPC must add a timeout as part of that task,
even if the timeout is unrelated to the task's stated goal. Record this under
`## Deviations from Plan` per the usual convention for incidental fixes.

### 11.6 Real-mode fixture checkpoints

`worker/tests/fixtures/` holds tiny synthetic `.safetensors` files, never real
downloaded model weights — see `ANVILML_DESIGN.md §17.5` for full detail and the
mandatory metadata-fallback regression case. Building a new fixture is part of the
same task that implements a new arch module's `load()`; a `load()` merged without a
fixture exercising it has no real-mode test, which fails Gate 4 (§8) and the dual
mock/real requirement (`ANVILML_DESIGN.md §17.3`).

A fixture file is never the production-size checkpoint, even scaled down "for
realism" beyond what the shape-inference formula needs to construct correctly
(`ANVILML_DESIGN.md §17.5`) — oversized fixtures are exactly what produced the
near-OOM incident on the 10GB agent VM that this convention exists to prevent.

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

---


## 13. Mandatory Build Cache Cleanup

Full rule and rationale: `ANVILML_DESIGN.md §4.9`. Summary: across the life of prior
development on this project, uncleaned `cargo`/Python build caches accumulated over
200GB of disk on the Forge agent's 10GB-RAM WSL2 VM. This is now a mandatory last
step of every ACT session that ran any build or test command — see §6 Step 13.

### Required commands, run in this order, at the end of every ACT session

```bash
# Rust — workspace-wide, regardless of which crate(s) the task touched
cargo clean

# Python — regardless of whether the task touched worker/ at all
find . -type d -name "__pycache__" -exec rm -rf {} +
find . -type d -name ".pytest_cache" -exec rm -rf {} +
rm -rf .mypy_cache .ruff_cache
```

### Rules

1. **Unconditional.** Runs at the end of every ACT session that ran a
   `cargo build`/`cargo test`/`cargo check`/`pytest` command — in practice, every
   session, since verifying the task's own change requires at least one of these.
   The only exemption is a task that built nothing (e.g. pure documentation).
2. **Workspace-wide, never `-p <crate>` scoped.** Most of the 200GB-class
   accumulation comes from shared dependency compilation artifacts, not from any one
   crate — a scoped clean would leave the bulk of it in place.
3. **Not deferrable.** This is part of the task's own ACT session. A `defers_to`
   entry pointing this obligation at a future task is non-compliant with
   `FORGE_AGENT_RULES.md §9.7a`, the same as deferring any other mandatory step.
4. **Cold-start cost is accepted.** The next task's ACT session starts from a cold
   build cache. This is a deliberate tradeoff (slower first build) for bounded disk
   usage — not a regression to "optimize away."
5. **Does not apply to CI.** GitHub-hosted runners discard their entire filesystem
   at job end; there is nothing to clean there. This rule is scoped to the Forge
   agent's persistent WSL2 VM and the project owner's local development environment.