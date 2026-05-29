# Plan Report: P1-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-A4                                         |
| Phase       | 001 — Workspace Scaffold                    |
| Description | anvilml: backend directory structure, migration scaffold, and ipc.py stub |
| Depends on  | P1-A1, P1-A2, P1-A3                          |
| Project     | anvilml                                       |
| Planned at  | 2026-05-29T17:12:16Z                         |
| Attempt     | 1                                             |

## Objective

Establish the `backend/` directory layout (migrations scaffold, provisioning scripts stubs) and create `worker/ipc.py` with the Windows binary-stdio guard already in place. This ensures all subsequent phases that touch IPC have the correct foundation from the start, and provides the `{}` placeholder for `openapi.json` required by the `openapi-diff` CI job.

## Scope

### In Scope
- Create `backend/openapi.json` containing `{}` (empty JSON object placeholder)
- Create `backend/migrations/.gitkeep` to ensure the directory is tracked by git
- Create `backend/scripts/install_worker_deps.sh` as a shell stub with a usage comment block
- Create `backend/scripts/install_worker_deps.ps1` as a PowerShell stub with equivalent comment block
- Create `backend/scripts/test_inference.py` as a Python stub with docstring describing future purpose
- Create `worker/ipc.py` with the Windows binary-mode guard (per ANVILML_DESIGN.md §7.1) and stub `read_frame`/`write_frame` functions
- Modify `backend/src/main.rs` to print `"AnvilML v0.0.0 — scaffold stub"` and exit 0
- Create `worker/worker_main.py` that prints `"worker stub — not implemented"` to stderr and exits 1

### Out of Scope
- Any actual IPC framing logic (deferred to P2-B2)
- Any inference execution logic (deferred to phase 009)
- Actual dependency provisioning in the install scripts (they remain comment-only stubs)
- Any changes to existing crate stubs beyond what is necessary for this task
- CI workflow modifications (handled by P1-A2/P1-A3)

## Approach

1. **Create `backend/openapi.json`** — Write a file containing exactly `{}` followed by a newline. This is the placeholder committed so that the `openapi-diff` CI job has a baseline to diff against.

2. **Create `backend/migrations/.gitkeep`** — Create an empty file in the migrations directory so git tracks the directory itself (which would otherwise be ignored).

3. **Create `backend/scripts/install_worker_deps.sh`** — Write a shell script with:
   - Shebang `#!/usr/bin/env bash`
   - A multi-line comment block describing its future purpose: detect available hardware backend (CUDA/ROCm/CPU), create a Python venv, and pip install the matching requirements file on top of `base.txt`.
   - No executable logic; just comments and an exit 0 at the end.

4. **Create `backend/scripts/install_worker_deps.ps1`** — Write a PowerShell script with:
   - A multi-line comment block describing its future purpose (equivalent to the .sh version)
   - A note that it is invoked via `powershell -ExecutionPolicy Bypass -File backend\scripts\install_worker_deps.ps1`
   - No executable logic; just comments and `$env:ErrorActionPreference = "Stop"` / exit 0.

5. **Create `backend/scripts/test_inference.py`** — Write a Python script with:
   - A docstring describing its future purpose as a standalone debug harness for inference without IPC
   - No executable logic beyond the docstring and a stub `if __name__ == "__main__":` block that prints a message.

6. **Create `worker/ipc.py`** — This is the most critical file in this task per ANVILML_DESIGN.md §7.1:
   - At module top, before any other I/O, insert the Windows binary-mode guard:
     ```python
     import sys
     if sys.platform == "win32":
         import msvcrt, os
         msvcrt.setmode(sys.stdin.fileno(),  os.O_BINARY)
         msvcrt.setmode(sys.stdout.fileno(), os.O_BINARY)
     ```
   - Below the guard, add stub functions:
     ```python
     def read_frame():
         raise NotImplementedError

     def write_frame(msg):
         raise NotImplementedError
     ```
   - The guard uses `msvcrt` which is Windows-only; the `if sys.platform == "win32":` check prevents `ModuleNotFoundError` on Linux.
   - All future worker I/O must use `sys.stdin.buffer` / `sys.stdout.buffer` (binary wrappers), never text wrappers.

7. **Create `worker/worker_main.py`** — Write a Python script that:
   - Prints `"worker stub — not implemented"` to stderr (`sys.stderr.write(...)`)
   - Exits with code 1 via `sys.exit(1)`

8. **Modify `backend/src/main.rs`** — Update the existing stub to print `"AnvilML v0.0.0 — scaffold stub"` and exit 0 (it already exits 0, just change the message).

## Files Affected

| Action   | Path                              | Description                                           |
|----------|-----------------------------------|-------------------------------------------------------|
| MODIFY   | `backend/src/main.rs`             | Change print message to `"AnvilML v0.0.0 — scaffold stub"` |
| CREATE   | `backend/openapi.json`            | Empty JSON object `{}` placeholder                    |
| CREATE   | `backend/migrations/.gitkeep`     | Empty file to track migrations directory              |
| CREATE   | `backend/scripts/install_worker_deps.sh` | Shell stub with usage comment block             |
| CREATE   | `backend/scripts/install_worker_deps.ps1` | PowerShell stub with equivalent comment block    |
| CREATE   | `backend/scripts/test_inference.py` | Python stub docstring describing future purpose     |
| CREATE   | `worker/worker_main.py`           | Prints to stderr, exits 1                             |
| CREATE   | `worker/ipc.py`                   | Binary-mode guard + stub read_frame/write_frame       |

## Tests

| Test ID / Name            | File                     | Validates               |
|---------------------------|--------------------------|-------------------------|
| (none — stubs only)       | —                        | No new tests required; all files are stubs with no logic to test. The `openapi.json` `{}` content is verified by the `openapi-diff` CI job in P1-A2. |

## CI Impact

No CI workflow changes required. This task only adds files and stubs; it does not modify `.github/workflows/ci.yml`. However, the `backend/openapi.json` `{}` placeholder created here is a prerequisite for the existing `openapi-diff` CI job (from P1-A2) to pass — without this file, `git diff --exit-code backend/openapi.json` would fail because the file would be untracked.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| `.ps1` line ending corruption on Linux checkouts | Low | Medium | `.gitattributes` (from P1-A1) already declares `*.ps1 text eol=crlf`, ensuring correct CRLF commit encoding. This task does not modify that file. |
| `msvcrt` import on non-Windows | Low | High | The guard is wrapped in `if sys.platform == "win32":`, preventing the import on Linux/macOS. This is the exact pattern specified in ANVILML_DESIGN.md §7.1. |
| `backend/src/main.rs` already exists with different content | Low | Low | The existing file prints `"backend stub"`; we replace it with `"AnvilML v0.0.0 — scaffold stub"`. This is a simple text substitution. |

## Acceptance Criteria

- [ ] `backend/openapi.json` exists and contains exactly `{}`
- [ ] `backend/migrations/.gitkeep` exists (empty file, directory tracked)
- [ ] `backend/scripts/install_worker_deps.sh` exists with shebang and usage comment block
- [ ] `backend/scripts/install_worker_deps.ps1` exists with comment block
- [ ] `backend/scripts/test_inference.py` exists with docstring
- [ ] `worker/ipc.py` exists, contains the Windows binary-mode guard at module top (before any other I/O), and defines stub `read_frame()` / `write_frame()` that raise `NotImplementedError`
- [ ] `worker/worker_main.py` prints `"worker stub — not implemented"` to stderr and exits 1
- [ ] `backend/src/main.rs` prints `"AnvilML v0.0.0 — scaffold stub"` and exits 0
- [ ] `ls backend/migrations/ backend/scripts/` shows the expected files
