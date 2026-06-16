# Plan Report: P8-B3

| Field       | Value                                                   |
|-------------|---------------------------------------------------------|
| Task ID     | P8-B3                                                   |
| Phase       | 008 — ZeroMQ IPC Transport                              |
| Description | scripts: install_worker_deps.sh and .ps1 — create venv and install base dependencies |
| Depends on  | none                                                    |
| Project     | anvilml                                                 |
| Planned at  | 2026-06-16T14:50:00Z                                    |
| Attempt     | 1                                                       |

## Objective

Create two idempotent provisioning scripts (`scripts/install_worker_deps.sh` and `scripts/install_worker_deps.ps1`) that verify Python 3.12 is present, create a virtual environment at a configurable path (default `./worker/.venv`), and install the base dependencies from `worker/requirements/base.txt`. After execution, `worker/.venv/bin/python3 -c "import zmq, msgpack"` must exit 0 on Linux, proving the venv is functional and the core IPC dependencies are installed. This is the provisioning entry point referenced by `ENVIRONMENT.md §1` and `ANVILML_DESIGN.md §18.1`; Phase 022 will extend it with hardware-detection and torch installation.

## Scope

### In Scope
- Create `scripts/install_worker_deps.sh` with:
  - `#!/usr/bin/env bash` shebang and `set -euo pipefail`.
  - Verification that `python3.12` is on `$PATH` (exit 1 with error message if absent).
  - Read `ANVILML_VENV_PATH` env var, defaulting to `./worker/.venv`.
  - Skip venv creation if `$venv_path/bin/python3` already exists.
  - Create venv via `python3.12 -m venv "$venv_path"` if it does not exist.
  - Activate venv and run `pip install -r worker/requirements/base.txt`.
  - Idempotent: re-running is a silent no-op with exit 0.
- Create `scripts/install_worker_deps.ps1` with:
  - `$ErrorActionPreference = 'Stop'`.
  - Verification that `py -3.12` is available (exit 1 if absent).
  - Read `$env:ANVILML_VENV_PATH`, defaulting to `.\worker\.venv`.
  - Skip venv creation if `$venv_path\Scripts\python.exe` already exists.
  - Create venv via `py -3.12 -m venv "$venv_path"` if it does not exist.
  - Activate venv and run `pip install -r worker\requirements\base.txt`.
  - Idempotent: re-running is a silent no-op with exit 0.
- Ensure `.gitattributes` line-ending rules are respected (`.sh` = LF, `.ps1` = CRLF). The existing `.gitattributes` already covers this.

### Out of Scope
- Hardware detection and torch installation (deferred to Phase 022).
- Auto-provisioning from the Rust binary on startup (deferred to Phase 021–023).
- Version introspection or release packaging (Phase 021–023).
- Any Rust code changes — this task touches no crates.
- Any CI file changes.

## Existing Codebase Assessment

No prior source exists in `scripts/` — the directory contains only a `.gitkeep` file. The `worker/requirements/base.txt` file already exists with the correct dependency list (`pyzmq>=26.0`, `msgpack>=1.0`, `pillow>=10.0`, `safetensors>=0.4`, `pytest>=8.0`). The `worker/ipc.py` module already exists and demonstrates the expected usage pattern (ZMQ DEALER + msgpack).

This task establishes the baseline provisioning pattern. The scripts must follow the convention that `ANVILML_VENV_PATH` is the single configuration point for the venv location, matching the `venv_path` config field in `ServerConfig` (per `ENVIRONMENT.md §3.1`). The scripts must not modify any Rust crate — this is a pure script task.

## Resolved Dependencies

None. This task introduces no external crates or packages. It consumes only:
- The standard library `venv` module (Python 3.12 standard library).
- The dependencies already listed in `worker/requirements/base.txt`.

## Approach

1. **Create `scripts/install_worker_deps.sh`** (Linux/macOS):
   - Start with `#!/usr/bin/env bash` and `set -euo pipefail` for strict error handling.
   - Use `command -v python3.12 >/dev/null 2>&1 || { echo "error: python3.12 is required but not found on PATH" >&2; exit 1; }` to verify the interpreter. This is a hard requirement — no fallback to other Python versions.
   - Read `ANVILML_VENV_PATH` with a default: `venv_path="${ANVILML_VENV_PATH:-./worker/.venv}"`.
   - Check if `$venv_path/bin/python3` exists: if yes, skip venv creation (idempotency). If no, run `python3.12 -m venv "$venv_path"`.
   - Activate the venv using `source "$venv_path/bin/activate"`.
   - Run `pip install -r worker/requirements/base.txt` to install base dependencies.
   - The script exits 0 on success. All commands are under `set -e` so any failure produces a non-zero exit.

2. **Create `scripts/install_worker_deps.ps1`** (Windows):
   - Start with `$ErrorActionPreference = 'Stop'` for strict error handling.
   - Verify `py -3.12` availability: `py -3.12 -c "import sys" 2>$null` — if this fails, exit 1 with an error message. The `py` launcher is installed by the standard Python 3.12 installer.
   - Read `$env:ANVILML_VENV_PATH` with default: `$venv_path = $env:ANVILML_VENV_PATH ?? ".\worker\.venv"`.
   - Check if `$venv_path\Scripts\python.exe` exists: if yes, skip venv creation (idempotency). If no, run `py -3.12 -m venv "$venv_path"`.
   - Activate the venv by dot-sourcing: `& "$venv_path\Scripts\Activate.ps1"`.
   - Run `pip install -r worker\requirements\base.txt` to install base dependencies.
   - The script exits 0 on success. `$ErrorActionPreference = 'Stop'` ensures any failure produces a non-zero exit.

3. **Ensure line endings match `.gitattributes`**:
   - The `.sh` script uses LF line endings (default for `cat` heredoc on Linux).
   - The `.ps1` script must use CRLF line endings per `.gitattributes` (`*.ps1 text eol=crlf`). On Linux, write the file with `unix2dos` or use `sed` to convert, or write it with explicit CRLF using a bash heredoc with `\r\n` — however, since we are writing from a Linux environment, the simplest approach is to write the `.ps1` content with a bash heredoc and then run `unix2dos scripts/install_worker_deps.ps1` to convert line endings. If `unix2dos` is not available, use `sed -i 's/$/\r/' scripts/install_worker_deps.ps1`.

## Public API Surface

None. This task creates shell/PowerShell scripts only — no Rust or Python public API items are introduced. No crates are modified.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `scripts/install_worker_deps.sh` | Linux/macOS venv provisioning script |
| CREATE | `scripts/install_worker_deps.ps1` | Windows venv provisioning script |

No existing files are modified. No `Cargo.toml` version bumps needed (no Rust source touched).

## Tests

This task creates scripts, not code under test. The acceptance criteria serve as the verification:

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| (manual) | Linux provisioning | `install_worker_deps.sh` creates venv and installs deps | `python3.12` on PATH, `scripts/` dir exists | `bash scripts/install_worker_deps.sh` | `worker/.venv/bin/python3 -c "import zmq, msgpack"` exits 0 | `bash scripts/install_worker_deps.sh && worker/.venv/bin/python3 -c "import zmq, msgpack"` exits 0 |
| (manual) | Windows provisioning | `install_worker_deps.ps1` creates venv and installs deps | `py -3.12` available, PowerShell 5.1+ | `powershell -ExecutionPolicy Bypass -File scripts\install_worker_deps.ps1` | `worker\.venv\Scripts\python.exe -c "import zmq, msgpack"` exits 0 | `powershell -ExecutionPolicy Bypass -File scripts\install_worker_deps.ps1 && worker\.venv\Scripts\python.exe -c "import zmq, msgpack"` exits 0 |
| (manual) | Idempotency (Linux) | Re-running the script is a no-op | venv already exists from prior run | `bash scripts/install_worker_deps.sh` (second run) | exits 0, no errors | `bash scripts/install_worker_deps.sh && echo "idempotent"` exits 0 |
| (manual) | Idempotency (Windows) | Re-running the .ps1 script is a no-op | venv already exists from prior run | `powershell -ExecutionPolicy Bypass -File scripts\install_worker_deps.ps1` (second run) | exits 0, no errors | `powershell -ExecutionPolicy Bypass -File scripts\install_worker_deps.ps1 && echo "idempotent"` exits 0 |
| (manual) | Missing python3.12 | Script rejects missing interpreter | `python3.12` removed from PATH | `bash scripts/install_worker_deps.sh` | exits 1 with error message | `PATH=/usr/bin bash scripts/install_worker_deps.sh; echo "exit: $?"` — exit code must be 1 |

## CI Impact

No CI changes required. These scripts are provisioning tools run by developers before starting the server — they are not executed by any CI job. The CI jobs (`worker-linux`, `worker-windows`) run `pytest worker/tests/` which assumes the venv already exists.

## Platform Considerations

- **Linux/macOS**: Uses `#!/usr/bin/env bash`, `set -euo pipefail`, `$venv_path/bin/python3` for interpreter path, `source` for activation. The `pip` command inside the activated venv resolves to the venv's pip.
- **Windows (PowerShell)**: Uses `$ErrorActionPreference = 'Stop'`, `$venv_path\Scripts\python.exe` for interpreter path, dot-sourcing for activation (`& "$venv_path\Scripts\Activate.ps1"`), `py -3.12` launcher for invoking Python 3.12. The `.ps1` file must use CRLF line endings per `.gitattributes`.
- **Line endings**: `.gitattributes` already specifies `*.sh text eol=lf` and `*.ps1 text eol=crlf`. The `.sh` script naturally gets LF on Linux. The `.ps1` script must be explicitly converted to CRLF when written from a Linux environment.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `python3.12` not found on PATH but Python 3.12 is installed elsewhere (e.g. Homebrew on macOS under `/opt/homebrew/bin/python3.12`) | Medium | Medium | The script uses `command -v python3.12` which checks `$PATH`. If a user has Python 3.12 installed via Homebrew but the binary is named differently (e.g. `python3`), the script will fail. Document in the script comments that users should ensure `python3.12` is on PATH or set `ANVILML_VENV_PATH` pointing to an existing venv. |
| PowerShell 5.1 on older Windows does not support `Activate.ps1` without execution policy bypass | Low | Medium | The activation line uses `& "$venv_path\Scripts\Activate.ps1"` — users must run with `-ExecutionPolicy Bypass` as documented in `ENVIRONMENT.md §1`. The error message from a blocked policy is clear enough. |
| `pip install -r` on Windows PowerShell after activating venv may not resolve `pip` correctly if the venv was created with a different Python version | Low | High | Using `py -3.12 -m venv` ensures the venv's Python matches the launcher command. Inside the activated venv, `pip` resolves to the venv's pip. This is standard venv behavior and should not be an issue in practice. |
| Script written from Linux with LF endings on `.ps1` file, causing PowerShell to fail on Windows | Medium | High | Explicitly convert `.ps1` to CRLF after writing: use `sed -i 's/$/\r/' scripts/install_worker_deps.ps1` or `unix2dos` if available. Verify with `file scripts/install_worker_deps.ps1` that it reports "CRLF" before staging. |

## Acceptance Criteria

- [ ] `bash scripts/install_worker_deps.sh` exits 0 (creates venv if absent, no-op if present)
- [ ] `worker/.venv/bin/python3 -c "import zmq, msgpack"` exits 0 (proves deps installed correctly)
- [ ] `python3.12 --version` succeeds — script rejects missing python3.12 with exit 1
- [ ] `bash scripts/install_worker_deps.sh` run twice exits 0 both times (idempotency)
- [ ] `file scripts/install_worker_deps.sh` reports "Bourne-Again shell script" (valid bash script)
- [ ] `file scripts/install_worker_deps.ps1` reports "CRLF" line endings (per `.gitattributes`)
- [ ] `head -1 scripts/install_worker_deps.ps1` does not contain BOM or garbage (clean UTF-8)
