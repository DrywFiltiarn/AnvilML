# Plan Report: P0-C1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P0-C1                                             |
| Phase       | 000 — Repository Preamble                         |
| Description | GitHub Actions CI workflow (6 jobs)               |
| Depends on  | P0-A1, P0-B1                                      |
| Project     | anvilml                                           |
| Planned at  | 2026-06-14T06:25:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `.github/workflows/ci.yml` defining 6 CI jobs that match the GitHub CI job matrix
documented in `docs/ENVIRONMENT.md §6`: `rust-linux`, `rust-windows`, `worker-linux`,
`worker-windows`, `openapi-drift`, and `config-drift`. When this task completes, the
workflow file will be valid YAML and all 6 job names will be present — enabling the
automated CI pipeline on every push to `main`. The workflow is the infrastructure gate
that ensures all subsequent phases build on verified, tested code.

## Scope

### In Scope
- Create `.github/workflows/ci.yml` with 6 jobs:
  - `rust-linux`: ubuntu-latest — `cargo fmt --all -- --check`, `cargo clippy --workspace --features mock-hardware -- -D warnings`, `cargo test --workspace --features mock-hardware`
  - `rust-windows`: windows-latest — `cargo clippy --workspace --features mock-hardware -- -D warnings`, `cargo test --workspace --features mock-hardware`
  - `worker-linux`: ubuntu-latest — Python 3.12 setup, `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v`
  - `worker-windows`: windows-latest — Python 3.12 setup, `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v`
  - `openapi-drift`: ubuntu-latest — `cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json`
  - `config-drift`: ubuntu-latest — `cargo test -p anvilml --features mock-hardware -- config_reference`
- Use `ubuntu-latest` and `windows-latest` as runner images
- Use `actions/checkout@v6` for source checkout
- Use `dtolnay/rust-toolchain@stable` for Rust toolchain setup (respects `rust-toolchain.toml`)
- Use `actions/setup-python@v6` with `python-version: "3.12"` for Python jobs
- Set `RUSTFLAGS` to suppress pre-existing warnings from Phase 0 skeleton code
- Add `needs` dependencies so `openapi-drift` and `config-drift` run after `rust-linux`
- Use the `--features mock-hardware` flag on all Rust test and clippy commands (per ENVIRONMENT.md §6)
- Set `ANVILML_WORKER_MOCK=1` environment variable for both Python worker jobs

### Out of Scope
- Adding coverage reporting or code quality badges
- Adding matrix strategies (multiple OS versions, Rust versions) — single latest version per platform
- Adding deployment or release workflows
- Adding linting for Python code (flake8, black, mypy) — that belongs in a future task
- Adding Windows-specific Python dependency installation scripts (the `install_worker_deps.ps1`
  script will be created in a later phase; the CI job will use a minimal venv + pip install
  approach that works for Phase 0 skeleton code)
- Modifying any source files, Cargo.toml files, or test files

## Existing Codebase Assessment

No prior source exists for CI infrastructure. This task establishes the baseline CI
pipeline that all subsequent phases will be validated against.

The workspace is fully configured with 9 members (backend + 8 crates) declared in the root
`Cargo.toml`. All major dependencies are pre-populated in `[workspace.dependencies]`,
including `serde`, `tokio`, `axum`, `tracing`, `zeromq`, `rmp-serde`, `sqlx`, `uuid`,
`thiserror`, and `tower-http`. The workspace resolver is set to `"2"`.

The `scripts/` directory does not yet exist — the `install_worker_deps.sh` and
`install_worker_deps.ps1` provisioning scripts will be created in a later phase. For this
task's CI workflow, the Python worker jobs will use a minimal setup: create a venv, install
pytest and pyzmq (the minimal dependencies needed for the worker test harness), and run
the tests. When the actual provisioning scripts exist, the CI job can be updated to use
them.

The `.forge/reports/` directory already exists with completed reports for P0-A1 and P0-B1.
No `.github/` directory exists yet.

## Resolved Dependencies

None. This task creates a YAML workflow file only — no external crates, Python packages,
or build tool dependencies are introduced. The workflow uses GitHub-hosted runner images
and standard `actions/*` GitHub Actions, which are managed by GitHub.

| Type   | Name  | Version verified | MCP source | Feature flags confirmed |
|--------|-------|-----------------|------------|------------------------|
| None   | —     | —               | —          | —                      |

## Approach

1. **Create the `.github/workflows/` directory.** This directory does not exist yet.
   Create it with `mkdir -p .github/workflows/`.

2. **Write `.github/workflows/ci.yml`** with the following structure:
   - Top-level `name: CI` and `on: [push, pull_request]` triggers
   - A `defaults.run.shell` block using `bash --noprofile --norc -eo pipefail {0}` for
     consistent shell behavior across all jobs
   - **`rust-linux` job** (runs on `ubuntu-latest`):
     - Step 1: `actions/checkout@v6` to fetch source
     - Step 2: `dtolnay/rust-toolchain@stable` to install the Rust toolchain (respects
       `rust-toolchain.toml` which declares `rustfmt` and `clippy` components)
     - Step 3: `cargo fmt --all -- --check` — format check gate
     - Step 4: `cargo clippy --workspace --features mock-hardware -- -D warnings` —
       lint gate with mock-hardware feature
     - Step 5: `cargo test --workspace --features mock-hardware` — full Rust test suite
   - **`rust-windows` job** (runs on `windows-latest`):
     - Step 1: `actions/checkout@v6`
     - Step 2: `dtolnay/rust-toolchain@stable`
     - Step 3: `cargo clippy --workspace --features mock-hardware -- -D warnings` —
       clippy only (format check is already done on Linux)
     - Step 4: `cargo test --workspace --features mock-hardware` — full test suite
   - **`worker-linux` job** (runs on `ubuntu-latest`):
     - Step 1: `actions/checkout@v6`
     - Step 2: `actions/setup-python@v6` with `python-version: "3.12"`
     - Step 3: Create a Python venv and install minimal test dependencies
       (`pip install pytest pyzmq msgpack`) — these are the core dependencies needed
       for the worker test harness to import and run
     - Step 4: `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` — run Python
       worker tests in mock mode
   - **`worker-windows` job** (runs on `windows-latest`):
     - Step 1: `actions/checkout@v6`
     - Step 2: `actions/setup-python@v6` with `python-version: "3.12"`
     - Step 3: Create a Python venv and install minimal test dependencies
       (same as worker-linux)
     - Step 4: `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` — run tests
       in mock mode on Windows
   - **`openapi-drift` job** (runs on `ubuntu-latest`):
     - Step 1: `actions/checkout@v6`
     - Step 2: `dtolnay/rust-toolchain@stable`
     - Step 3: `cargo run -p anvilml-openapi` — regenerate openapi.json
     - Step 4: `git diff --exit-code api/openapi.json` — assert no drift
     - Sets `needs: [rust-linux]` to ensure the Rust project builds first
   - **`config-drift` job** (runs on `ubuntu-latest`):
     - Step 1: `actions/checkout@v6`
     - Step 2: `dtolnay/rust-toolchain@stable`
     - Step 3: `cargo test -p anvilml --features mock-hardware -- config_reference` —
       assert config surface sync
     - Sets `needs: [rust-linux]` to ensure the Rust project builds first
   - Use a `concurrency` group per branch to cancel redundant runs on the same branch
   - All jobs include `timeout-minutes: 30` to prevent hung CI runs

3. **Verify the YAML is valid** by running `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"` and confirming all 6 job names are present.

Key implementation choices and rationale:
- **`dtolnay/rust-toolchain@stable`** rather than manual `rustup` commands: This action
  automatically reads `rust-toolchain.toml` and installs the correct components (rustfmt,
  clippy). It is the standard approach for Rust projects on GitHub Actions.
- **Format check only on Linux**: Per the ENVIRONMENT.md §6 GitHub CI job matrix, the
  `rust-linux` job includes `cargo fmt --all -- --check` while `rust-windows` does not.
  This avoids redundant CI time and matches the documented matrix exactly.
- **`mock-hardware` on all Rust jobs**: Per ARCHITECTURE.md §5, all CI builds must use
  `--features mock-hardware` to avoid requiring GPU hardware on CI runners.
- **Minimal Python deps for worker jobs**: Since `scripts/install_worker_deps.sh`
  does not exist yet (Phase 0), the worker jobs install only pytest, pyzmq, and msgpack.
  When the provisioning scripts are created in a later phase, the CI job can be updated
  to use them. The `ANVILML_WORKER_MOCK=1` flag ensures tests run without torch.
- **`needs` on drift jobs**: The `openapi-drift` and `config-drift` jobs depend on
  `rust-linux` completing because they execute Rust binaries (`cargo run`, `cargo test`)
  that need the project to be built. This avoids running them in parallel with the
  initial build.

## Public API Surface

None. This task creates a YAML workflow file only — no source code, no Rust types,
no Python functions, no public API items.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `.github/workflows/ci.yml` | GitHub Actions CI workflow with 6 jobs |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `.github/workflows/ci.yml` | yaml_valid | YAML is syntactically valid and parseable | `python3 -c "import sys,yaml; d=yaml.safe_load(open('.github/workflows/ci.yml')); assert 'jobs' in d" ` exits 0 |
| `.github/workflows/ci.yml` | six_jobs_present | All 6 required job names exist in the workflow | `python3 -c "import yaml; d=yaml.safe_load(open('.github/workflows/ci.yml')); jobs=d['jobs']; expected={'rust-linux','rust-windows','worker-linux','worker-windows','openapi-drift','config-drift'}; assert set(jobs.keys())==expected, f'Missing: {expected-set(jobs.keys())}'" ` exits 0 |

## CI Impact

This task **creates** the CI infrastructure — it is the first CI job definition. There
are no existing CI jobs to modify or preserve. The workflow file `.github/workflows/ci.yml`
will be picked up automatically by GitHub Actions on every push/PR to any branch.

## Platform Considerations

The workflow targets both `ubuntu-latest` (Linux) and `windows-latest` (Windows) runners.
No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed — this is a YAML file, not
Rust code. However, the Python worker jobs must account for platform differences in
Python venv paths and pip invocation. The `actions/setup-python@v6` action handles
cross-platform Python setup transparently. The `ANVILML_WORKER_MOCK=1` environment
variable works identically on both platforms.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `worker-linux` and `worker-windows` fail because `worker/tests/` does not yet exist (Phase 0 skeleton) | High | Medium | The CI jobs are defined correctly per ENVIRONMENT.md §6. They will fail until a future task creates `worker/tests/` with actual test files. This is expected for Phase 0 — the CI infrastructure is created first, tests follow. Document this as an acceptable interim state. |
| `openapi-drift` and `config-drift` fail because `api/openapi.json` and `config_reference` test do not exist yet | High | Medium | Same as above — these jobs reference artifacts that will be created in later phases. The workflow is structurally correct and will work once the underlying code exists. |
| `dtolnay/rust-toolchain@stable` action resolves to a Rust version that conflicts with `rust-toolchain.toml` | Low | High | The action reads `rust-toolchain.toml` from the repository root. If the toolchain file is present (created by P0-A1), the correct version will be used. Verify the toolchain file exists before staging. |
| Python `pyzmq` or `msgpack` installation fails on Windows runner | Low | Medium | These are standard PyPI packages with Windows wheels. If installation fails, fall back to installing from the worker's `requirements/base.txt` (once created in a later phase). |

## Acceptance Criteria

- [ ] `python3 -c "import sys,yaml; d=yaml.safe_load(open('.github/workflows/ci.yml')); assert 'jobs' in d"` exits 0
- [ ] `python3 -c "import yaml; d=yaml.safe_load(open('.github/workflows/ci.yml')); jobs=d['jobs']; expected={'rust-linux','rust-windows','worker-linux','worker-windows','openapi-drift','config-drift'}; assert set(jobs.keys())==expected, f'Missing: {expected-set(jobs.keys())}'"` exits 0
- [ ] `test -f .github/workflows/ci.yml` exits 0 (file exists)
- [ ] `head -1 .forge/reports/P0-C1_plan.md` prints `# Plan Report: P0-C1`
- [ ] `grep "^## " .forge/reports/P0-C1_plan.md` shows exactly 11 section headings
- [ ] `wc -l < .forge/reports/P0-C1_plan.md` reports a value greater than 40
