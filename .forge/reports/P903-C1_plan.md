# Plan Report: P903-C1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P903-C1                                     |
| Phase       | 903 — IPC Transport Rework                  |
| Description | Full workspace clean gate after IPC transport rework |
| Depends on  | P903-A1, P903-A2, P903-A3, P903-A3x, P903-A4, P903-A5 |
| Project     | anvilml                                     |
| Planned at  | 2026-06-09T06:33:00Z                        |
| Attempt     | 1                                           |

## Objective

Verify that the entire workspace compiles, passes all tests, and meets linting standards after the IPC transport rework (Phase 903 groups A) has been implemented. No source changes are made in this task — it is purely a gate that runs four verification commands and records their verbatim output. All four commands must exit 0 for the task to be marked COMPLETE.

## Scope

### In Scope
- Run `cargo clippy --workspace --features mock-hardware -- -D warnings` and record output
- Run `env -i HOME=$HOME PATH=$PATH ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./worker/.venv cargo test --workspace --features mock-hardware` and record output
- Run `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` and record output
- Run `python -m pytest worker/tests/ -v` and record output
- Report results; task is COMPLETE only when all four exit codes are 0

### Out of Scope
- Any source code changes
- Any test modifications
- Any configuration file edits
- Any dependency version changes
- Any documentation updates
- Any git operations (commit, push, branch)

## Approach

1. **Gate 1 — Lint (clippy):** Run `cargo clippy --workspace --features mock-hardware -- -D warnings`. This checks all workspace crates for clippy warnings treated as errors. Expected: exit 0, no warnings.

2. **Gate 2 — Full test suite (Rust):** Run `env -i HOME=$HOME PATH=$PATH ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./worker/.venv cargo test --workspace --features mock-hardware`. The `env -i` clears the environment, then sets only `HOME`, `PATH`, `ANVILML_WORKER_MOCK=1` (enables Python worker stub mode), and `ANVILML_VENV_PATH` (points to the venv for worker subprocess tests). Expected: exit 0, all tests pass.

3. **Gate 3 — Windows cross-check:** Run `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu`. This cross-compiles the entire workspace for Windows (GNU target) exercising `#[cfg(windows)]` code paths that the native Linux mock-hardware build never touches. Expected: exit 0, no errors.

4. **Gate 4 — Python worker tests:** Run `python -m pytest worker/tests/ -v`. This runs the Python-side pytest suite covering the IPC transport, node execution, and worker tests. Expected: exit 0, all tests pass.

5. **Record verbatim output** of all four commands in the implementation report body. If any command exits non-zero, the task is BLOCKED — record the failure and stop.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| (none) | — | No source, test, config, or CI files are modified. This task is a pure verification gate. |

## Tests

None. This task does not write or modify any test files. It runs the existing test suite as a verification gate.

## CI Impact

No CI changes required. The four commands executed in this gate are already documented in `docs/ARCHITECTURE.md §9 (CI Gates)` and `docs/ENVIRONMENT.md §6` as the canonical CI checks. This task simply re-runs them to confirm the workspace is green after the IPC transport rework.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| A prior group-A task left the workspace in a failing state (e.g. clippy warning, test failure) | Medium | High — blocks this gate and all downstream phases | If any gate fails, record the verbatim output under `## Blockers` in the implementation report and STOP. The orchestrator will route the failure back to the responsible group-A task. |
| Missing cross-compilation toolchain (`x86_64-pc-windows-gnu` target) | Low | High — `cargo check --target x86_64-pc-windows-gnu` fails | Verify `rustup target list --installed` includes `x86_64-pc-windows-gnu` and `gcc-mingw-w64` linker is present before running. If missing, install via `rustup target add x86_64-pc-windows-gnu` and `sudo apt install gcc-mingw-w64`. |
| Python worker venv (`./worker/.venv`) not provisioned | Low | Medium — `cargo test` fails when it tries to spawn a Python subprocess | Ensure `./worker/.venv` exists with `msgpack` and `pillow` installed. If missing, run `bash backend/scripts/install_worker_deps.sh` before the gate. |
| `env -i` clears required variables causing test failures | Low | Medium — overly aggressive environment clearing | The `env -i` invocation explicitly restores `HOME`, `PATH`, and sets the two required AnvilML env vars. If a test needs additional env vars (e.g. `RUST_LOG`), they should be added to the `env -i` command. |

## Acceptance Criteria

- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `env -i HOME=$HOME PATH=$PATH ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./worker/.venv cargo test --workspace --features mock-hardware` exits 0
- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0
- [ ] `python -m pytest worker/tests/ -v` exits 0
- [ ] Verbatim output of all four commands recorded in the implementation report body
