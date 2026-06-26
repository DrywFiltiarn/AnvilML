# Plan Report: P1-E1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P1-E1                                             |
| Phase       | 1 — Repository Scaffold                           |
| Description | CI: ci.yml rust-test matrix job (real commands)   |
| Depends on  | P1-D1                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-06-26T14:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create the GitHub Actions CI workflow file (`.github/workflows/ci.yml`) containing a single `rust-test` job that exercises the now-buildable AnvilML workspace across both target operating systems (Linux and Windows) via a matrix-driven job body. The job runs the same three real commands as the local pre-push gate: `cargo fmt --all -- --check` (Linux-only), `cargo clippy --workspace --features mock-hardware -- -D warnings`, and `cargo test --workspace --features mock-hardware`. This establishes the CI baseline so that every subsequent push is validated against the workspace's formatting, linting, and test gates.

## Scope

### In Scope
- Create `.github/workflows/ci.yml` as a new file.
- Define one job named `rust-test` with `strategy.matrix.os: [ubuntu-latest, windows-latest]`.
- Steps in order:
  1. `actions/checkout@v4` — check out the repository.
  2. `rust-actions/rustup@v1` (or equivalent) — install Rust; the toolchain is automatically selected from `rust-toolchain.toml`.
  3. `cargo fmt --all -- --check` — gated with `if: matrix.os == 'ubuntu-latest'`, placed before clippy (Linux-only per ENVIRONMENT.md §6 Step 11 and CI §18.3).
  4. `cargo clippy --workspace --features mock-hardware -- -D warnings` — runs on both matrix entries.
  5. `cargo test --workspace --features mock-hardware` — runs on both matrix entries.
- Use the GitHub Actions `rust-actions/rustup` action (or `dtolnay/rust-toolchain`) for Rust installation — it reads `rust-toolchain.toml` automatically.

### Out of Scope
None. This task's `defers_to` field is `[]` (from JSON): `absent`. No scope is deferred.

## Existing Codebase Assessment

No prior source exists for this task — it creates a CI workflow file from scratch. The `.github/` directory does not yet exist in the repository. However, the workspace is fully scaffolded by prior tasks (P1-A1 through P1-D2): the Cargo workspace root lists all 10 members, `rust-toolchain.toml` pins Rust 1.96.0 with rustfmt and clippy components, and the `mock-hardware` feature flag is declared in `anvilml-hardware` and forwarded through the dependency chain. All crates compile (they are doc-comment stubs), so the CI commands (`fmt --check`, clippy, test) are expected to pass once the workflow file is in place.

## Resolved Dependencies

None. This task introduces no external Rust crates, Python packages, or npm modules. It creates a YAML workflow file that references GitHub-hosted actions (`actions/checkout`) and uses the project's own `cargo` toolchain. No MCP lookup is required.

## Approach

1. **Create the directory structure.** Ensure `.github/workflows/` exists. This is a new directory — no parent files need modification.

2. **Write `.github/workflows/ci.yml`.** The file contains a single job `rust-test` with the following structure:
   - `name: rust-test`
   - `on: [push, pull_request]` — triggers on every push and PR to `main` and other branches.
   - `jobs:` → `rust-test:` with:
     - `runs-on: ${{ matrix.os }}`
     - `strategy:` → `matrix:` → `os: [ubuntu-latest, windows-latest]`
     - `steps:` (in exact order):
       - **Step 1:** `actions/checkout@v4` — standard checkout.
       - **Step 2:** `rust-actions/rustup@v1` with `default-toolchain: none` (or `dtolnay/rust-toolchain@master`) — this reads `rust-toolchain.toml` and installs the pinned toolchain (1.96.0) with rustfmt and clippy components. No explicit version pin is needed; the action respects the workspace's `rust-toolchain.toml`.
       - **Step 3 (Linux-only):** A step with `if: matrix.os == 'ubuntu-latest'` running `run: cargo fmt --all -- --check`. This is placed before clippy as required by the project convention that `fmt --check` is Linux-only (ENVIRONMENT.md §6 Step 11, CI §18.3).
       - **Step 4:** `run: cargo clippy --workspace --features mock-hardware -- -D warnings`. Runs on both OS entries. The `--features mock-hardware` flag enables `MockDetector` for all crates that depend on `anvilml-hardware`, ensuring CI builds without requiring real GPU hardware.
       - **Step 5:** `run: cargo test --workspace --features mock-hardware`. Runs on both OS entries. Uses the same feature flag as clippy.

3. **Rationale for action choice.** `actions/checkout@v4` is the current stable version. For Rust installation, `dtolnay/rust-toolchain@master` is preferred because it directly reads `rust-toolchain.toml` and is the most widely used action for this purpose. An alternative `rust-actions/rustup@v1` also works but requires slightly more configuration. Either is acceptable; the plan specifies `dtolnay/rust-toolchain@master` as it is the simplest approach for this use case.

4. **No additional CI jobs.** This task creates only the `rust-test` job. The remaining jobs (`worker-test`, `openapi-drift`, `config-drift`) are placeholders for P1-E2, which completes the CI workflow file.

## Public API Surface

None. This task creates a YAML configuration file, not source code.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `.github/workflows/ci.yml` | New CI workflow file with one `rust-test` job and matrix strategy |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `.github/workflows/ci.yml` | ci_yml_structure | The YAML file is syntactically valid, contains exactly one job named `rust-test`, has the correct matrix (`ubuntu-latest`, `windows-latest`), and all five steps are present in the correct order with the fmt step gated on `ubuntu-latest`. | `python3 -c "import yaml; d=yaml.safe_load(open('.github/workflows/ci.yml')); assert 'rust-test' in d['jobs']; assert d['jobs']['rust-test']['strategy']['matrix']['os'] == ['ubuntu-latest','windows-latest']; steps=[s.get('run','') if isinstance(s,dict) else '' for s in d['jobs']['rust-test']['steps']]; assert any('fmt --all' in s for s in steps); assert any('clippy' in s for s in steps); assert any('cargo test' in s for s in steps)"` exits 0 |
| Local environment | ci_steps_local_match | All three cargo commands in the CI job exit 0 locally, confirming the workflow steps are correct. | `cargo fmt --all -- --check && cargo clippy --workspace --features mock-hardware -- -D warnings && cargo test --workspace --features mock-hardware` exits 0 |

## CI Impact

This task introduces the first CI job for the project. The new `.github/workflows/ci.yml` file will trigger the `rust-test` job on every push to `main` and on every pull request. The job runs on both `ubuntu-latest` and `windows-latest` runners. No existing CI jobs are modified (there were none). Subsequent tasks (P1-E2) will add additional jobs to this same file.

## Platform Considerations

The `cargo fmt --all -- --check` step is gated to `ubuntu-latest` only, following the project convention that `fmt --check` is Linux-only (ENVIRONMENT.md §6 Step 11, CI §18.3). The clippy and test steps run on both platforms identically. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed — these are CI runner configuration choices, not source code platform checks. The workspace's `rust-toolchain.toml` already includes `x86_64-pc-windows-gnu` as a target for cross-platform checks, but this is not exercised by the CI job (it runs natively on each runner).

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `dtolnay/rust-toolchain@master` may not be available or may have changed API. The action's configuration schema could differ from what is assumed. | Low | Medium | Verify the action's current documentation at GitHub Actions marketplace before writing. Use `rust-actions/rustup@v1` as fallback if `dtolnay` is unavailable. Both actions read `rust-toolchain.toml`. |
| The workspace does not yet compile on `windows-latest` due to platform-specific code in stub crates (e.g. `anvilml-worker`'s `job_object.rs`). The CI test step will fail on Windows. | Medium | High | Run `cargo build --workspace --features mock-hardware` locally on the current platform first. If it passes, the stub crates are platform-agnostic at this phase (they contain only doc comments). If a Windows-specific file exists, the CI job will fail on `windows-latest` — the ACT agent must either add a `--target` flag or ensure all stub code compiles cross-platform. |
| GitHub Actions runner environment lacks `cargo` pre-installed and the Rust installation step fails silently. | Low | High | Use well-established actions (`actions/checkout@v4`, `dtolnay/rust-toolchain@master`) that are known to work on both Linux and Windows runners. The `dtolnay` action explicitly handles the toolchain installation from `rust-toolchain.toml`. |

## Acceptance Criteria

- [ ] `python3 -c "import yaml; d=yaml.safe_load(open('.github/workflows/ci.yml')); assert d['name'] == 'rust-test'"` exits 0
- [ ] `python3 -c "import yaml; d=yaml.safe_load(open('.github/workflows/ci.yml')); assert 'rust-test' in d['jobs']; m=d['jobs']['rust-test']['strategy']['matrix']['os']; assert m == ['ubuntu-latest','windows-latest']"` exits 0
- [ ] `grep -c 'runs-on' .github/workflows/ci.yml` outputs `1` (single job body, not duplicated blocks)
- [ ] `grep -A1 'if:' .github/workflows/ci.yml | grep -q 'matrix.os.*ubuntu-latest'` exits 0 (fmt step is gated on Linux)
- [ ] `grep -n 'fmt\|clippy\|test' .github/workflows/ci.yml` shows fmt before clippy, clippy before test
- [ ] `cargo fmt --all -- --check && cargo clippy --workspace --features mock-hardware -- -D warnings && cargo test --workspace --features mock-hardware` exits 0
