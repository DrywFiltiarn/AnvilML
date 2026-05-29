# Plan Report: P1-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-A2                                         |
| Phase       | 001 — Workspace Scaffold                    |
| Description | anvilml — Linux CI jobs (rust, python-worker, openapi-diff) |
| Depends on  | P1-A1                                         |
| Project     | anvilml                                       |
| Planned at  | 2026-05-29T14:23:06Z                         |
| Attempt     | 1                                             |

## Objective

Install the three Linux CI jobs that gate every push to the AnvilML repository: a Rust quality suite (fmt, clippy, test), a Python worker test harness, and an OpenAPI diff check. These jobs ensure that all code landing on the default branch compiles cleanly, passes formatting and linting checks, runs the full workspace test suite with the `mock-hardware` feature flag, and does not drift from the committed OpenAPI spec. The jobs run on `ubuntu-latest` with Cargo registry and build cache to keep CI fast.

## Scope

### In Scope
- Create `.github/workflows/ci.yml` with three jobs:
  - **`rust`**: fmt check, clippy lint, cargo test — all with `--features mock-hardware`
  - **`python-worker`**: pip install from `worker/requirements/base.txt`, then pytest with `ANVILML_WORKER_MOCK=1`
  - **`openapi-diff`**: regenerate `openapi.json` via `cargo run -p anvilml-openapi`, then diff against committed version
- Cache Cargo registry (`~/.cargo/registry`) and `target/` directory keyed on `Cargo.lock` hash
- `python-worker` and `openapi-diff` declare `needs: [rust]` so they only run after Rust quality gates pass
- Each CI step is a separate `run:` block for clear failure attribution

### Out of Scope
- Windows CI job (`rust-windows`) — handled by P1-A3
- Any source code changes
- Any test file creation (tests are covered by P1-B1)
- Backend directory structure or migration scaffolding (P1-A4)
- Python worker package layout (P1-B1)
- `.gitattributes` (P1-A1)

## Approach

1. Create the `.github/workflows/` directory if it does not exist.
2. Write `.github/workflows/ci.yml` with the following structure:
   - `name: CI`
   - `on: [push, pull_request]` targeting the default branch
   - **Job `rust`** (runs on `ubuntu-latest`):
     - Step 1: `actions/checkout@v4`
     - Step 2: Cache Cargo registry and target dir using `actions/cache` with key `cargo-${{ runner.os }}-${{ hashFiles('Cargo.lock') }}`
     - Step 3: `cargo fmt --all --check` (separate `run:` step)
     - Step 4: `cargo clippy --workspace --features mock-hardware -- -D warnings` (separate `run:` step)
     - Step 5: `cargo test --workspace --features mock-hardware` (separate `run:` step)
   - **Job `python-worker`** (runs on `ubuntu-latest`, `needs: [rust]`):
     - Step 1: `actions/checkout@v4`
     - Step 2: `pip install -r worker/requirements/base.txt`
     - Step 3: `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v`
   - **Job `openapi-diff`** (runs on `ubuntu-latest`, `needs: [rust]`):
     - Step 1: `actions/checkout@v4`
     - Step 2: Install Rust toolchain with stable + rustfmt + clippy components
     - Step 3: Cache Cargo registry and target dir
     - Step 4: `cargo run -p anvilml-openapi`
     - Step 5: `git diff --exit-code backend/openapi.json`
3. Validate the YAML syntax is correct (no trailing commas, proper indentation).

## Files Affected

| Action   | Path                              | Description                                            |
|----------|-----------------------------------|--------------------------------------------------------|
| CREATE   | `.github/workflows/ci.yml`        | GitHub Actions CI workflow with 3 Linux jobs           |

## Tests

No test files are written or modified by this task. The acceptance criterion is that the CI workflow file itself is syntactically valid YAML and defines three jobs that execute the specified commands. Test validation occurs when the Forge pushes to the repository and GitHub Actions runs.

| Test ID / Name            | File                     | Validates               |
|---------------------------|--------------------------|-------------------------|
| ci-yaml-syntax            | `.github/workflows/ci.yml` | Valid YAML structure    |
| rust-job-steps            | `.github/workflows/ci.yml` | 3 separate run steps (fmt, clippy, test) |
| python-worker-job         | `.github/workflows/ci.yml` | pip install + pytest with mock env var |
| openapi-diff-job          | `.github/workflows/ci.yml` | cargo run + git diff --exit-code |
| needs-dependency          | `.github/workflows/ci.yml` | python-worker and openapi-diff depend on rust |
| cache-configuration       | `.github/workflows/ci.yml` | Cargo registry + target cached with Cargo.lock hash |

## CI Impact

This task **creates** `.github/workflows/ci.yml` — the first CI workflow file for the repository. It defines three jobs:

1. **`rust`** — The quality gate. Runs `cargo fmt --all --check`, `cargo clippy --workspace --features mock-hardware -- -D warnings`, and `cargo test --workspace --features mock-hardware`. Each step is independent so failures are attributed correctly in the GitHub Actions UI.

2. **`python-worker`** — Depends on `rust` passing. Installs Python dependencies from `worker/requirements/base.txt` and runs pytest with `ANVILML_WORKER_MOCK=1`. At Phase 001 stub stage, zero collected tests is acceptable (the job must exit 0).

3. **`openapi-diff`** — Depends on `rust` passing. Regenerates `backend/openapi.json` via the `anvilml-openapi` binary and verifies it matches the committed version using `git diff --exit-code`. This catches accidental manual edits to the OpenAPI spec.

All three jobs use `ubuntu-latest`. The `rust` job caches Cargo registry and target directory. The `openapi-diff` job also caches these for faster builds. Cache keys are based on `Cargo.lock` hash so they invalidate when dependencies change.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| `worker/requirements/base.txt` does not exist yet (P1-B1) | Low | Medium | The CI job will fail until P1-B1 lands; this is expected as tasks are sequenced. The plan assumes P1-B1 runs before or in parallel with P1-A2 execution. |
| `backend/openapi.json` does not exist yet (P1-A4) | Low | Medium | Same sequencing concern — the file must be committed before `openapi-diff` can diff against it. P1-A4 should run before or alongside P1-A2 in the ACT phase. |
| Cache miss on first CI run | High | Low | First run will always be slower; subsequent runs benefit from cache hit. Not a blocker. |
| `mock-hardware` feature not declared in all crates that need it | Medium | High | P1-A1 declares it in `anvilml-hardware`. Forwarding must be added in later phases as dependencies are established. The stub workspace compiles without forwarding at this stage. |

## Acceptance Criteria

- [ ] `.github/workflows/ci.yml` exists and is valid YAML
- [ ] Job `rust` runs on `ubuntu-latest` with 3 separate steps: fmt check, clippy lint, cargo test
- [ ] All Rust jobs use `--features mock-hardware`
- [ ] Job `python-worker` runs on `ubuntu-latest`, declares `needs: [rust]`, installs deps from `worker/requirements/base.txt`, and runs pytest with `ANVILML_WORKER_MOCK=1`
- [ ] Job `openapi-diff` runs on `ubuntu-latest`, declares `needs: [rust]`, runs `cargo run -p anvilml-openapi`, then `git diff --exit-code backend/openapi.json`
- [ ] Cargo registry (`~/.cargo/registry`) and `target/` directory are cached with a key based on `Cargo.lock` hash
- [ ] No other CI jobs (e.g. `rust-windows`) are included — those belong to P1-A3
