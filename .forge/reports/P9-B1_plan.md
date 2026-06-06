# Plan Report: P9-B1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P9-B1                                              |
| Phase       | 009 — Worker Spawn & Handshake                     |
| Description | ci: add Python venv setup to rust-linux and rust-windows jobs for worker subprocess tests |
| Depends on  | P9-A6                                              |
| Project     | anvilml                                            |
| Planned at  | 2026-06-06T15:30:00Z                               |
| Attempt     | 1                                                  |

## Objective

Enable the Rust worker subprocess tests (`managed` and `pool` modules in `anvilml-worker`) to run successfully in CI by provisioning a minimal Python virtual environment with `msgpack` and `pillow` on both Linux and Windows runners, and passing `ANVILML_VENV_PATH` and `ANVILML_WORKER_MOCK=1` as environment variables so that the Rust code can spawn the Python worker subprocess in mock mode.

## Scope

### In Scope
- Modify `.github/workflows/ci.yml` only (single file)
- Add a "Setup Python for worker tests" step before "Run tests" in the `rust-linux` job: `python3 -m venv .ci-venv && .ci-venv/bin/pip install msgpack pillow`
- Add a "Setup Python for worker tests" step before "Run tests" in the `rust-windows` job: `python -m venv .ci-venv && .ci-venv\Scripts\pip install msgpack pillow`
- Set `ANVILML_VENV_PATH: .ci-venv` as a step-level environment variable on the "Run tests" step in both jobs
- Ensure `ANVILML_WORKER_MOCK=1` is set on the "Run tests" step (or in step env) in both jobs so the Python worker subprocess runs in stub mode without requiring torch
- Preserve all existing CI jobs, steps, and gate commands

### Out of Scope
- Any source code changes in Rust or Python crates
- Adding a separate `python-worker` job (covered by P9-B2)
- Installing additional Python packages beyond `msgpack` and `pillow`
- Modifying any other CI workflow file
- Version pinning of `msgpack` or `pillow` (latest compatible versions suffice for mock mode)

## Approach

1. **Read current `.github/workflows/ci.yml`** to confirm the existing step structure, job names, and gate ordering. The file has two jobs (`rust-linux` on `ubuntu-latest`, `rust-windows` on `windows-latest`), each with: checkout → setup Rust toolchain → format check (linux only) → lint (mock + real) → run tests → compile checks.

2. **Insert "Setup Python for worker tests" step** in the `rust-linux` job, placed immediately before the existing "Run tests" step. The step runs:
   ```yaml
   - name: Setup Python for worker tests
     run: python3 -m venv .ci-venv && .ci-venv/bin/pip install msgpack pillow
   ```

3. **Insert "Setup Python for worker tests" step** in the `rust-windows` job, placed immediately before the existing "Run tests" step. The step runs:
   ```yaml
   - name: Setup Python for worker tests
     run: python -m venv .ci-venv && .ci-venv\Scripts\pip install msgpack pillow
   ```

4. **Update the "Run tests" step** in both jobs to include the required environment variables as a step-level `env:` block:
   ```yaml
   - name: Run tests
     run: cargo test --workspace --features mock-hardware
     env:
       ANVILML_VENV_PATH: .ci-venv
       ANVILML_WORKER_MOCK: "1"
   ```

5. **Verify the resulting YAML** is syntactically valid by confirming the file parses correctly (the CI runner will validate on push/PR).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `.github/workflows/ci.yml` | Add Python venv setup steps and env vars to both `rust-linux` and `rust-windows` jobs |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| CI (linux) | `rust-linux` job, "Run tests" step | `cargo test --workspace --features mock-hardware` exits 0 with `ANVILML_VENV_PATH=.ci-venv` and `ANVILML_WORKER_MOCK=1`; includes managed/pool worker subprocess tests that spawn a real Python child process |
| CI (windows) | `rust-windows` job, "Run tests" step | Same as Linux but on `windows-latest` runner with Windows venv paths |

## CI Impact

This task modifies the existing `.github/workflows/ci.yml` file. No new jobs are added (that is P9-B2). Both existing jobs (`rust-linux` and `rust-windows`) gain one additional step each and two environment variables on their test steps. All pre-existing gates (format check, clippy lint, compile checks) remain unchanged and in the same order. The change is additive only — no existing step is removed or skipped.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `python3` not available on `ubuntu-latest` runner | Very low | Build fails before tests | `ubuntu-latest` includes Python 3; if needed, add a `actions/setup-python@v5` step — but the task spec uses bare `python3` which is standard on Ubuntu runners |
| `pip install msgpack pillow` fails or hangs | Low | Test step skipped, CI passes without worker subprocess coverage | Both packages are pure-Python / small wheels on PyPI; they resolve instantly. No torch dependency means no CUDA/ROCm download |
| `python` not found on `windows-latest` runner | Very low | Windows tests fail before setup | `windows-latest` includes Python 3 via the `python` command; if needed, add `actions/setup-python@v5` |
| `.ci-venv` directory persists across steps and causes conflicts | Low | Pip install errors from existing venv | Using `python -m venv .ci-venv` on an already-existing venv is idempotent — it recreates the environment cleanly |
| `ANVILML_WORKER_MOCK=1` not propagated to worker child process | Medium (if env injection logic is wrong) | Worker subprocess tests fail with "python missing" or torch import error | The mock variable must be set in the step env so it is inherited by the Rust test binary and forwarded into `build_worker_env()` which copies it to the child environment. P9-A3 already implements this forwarding |

## Acceptance Criteria

- [ ] `.github/workflows/ci.yml` contains a "Setup Python for worker tests" step in both `rust-linux` and `rust-windows` jobs, placed before the "Run tests" step
- [ ] Linux setup step uses `python3 -m venv .ci-venv && .ci-venv/bin/pip install msgpack pillow`
- [ ] Windows setup step uses `python -m venv .ci-venv && .ci-venv\Scripts\pip install msgpack pillow`
- [ ] Both "Run tests" steps declare `ANVILML_VENV_PATH: .ci-venv` and `ANVILML_WORKER_MOCK: "1"` in a step-level `env:` block
- [ ] All pre-existing CI steps (format check, clippy, compile checks) remain untouched and in the same order
- [ ] No new jobs are added (reserved for P9-B2)
