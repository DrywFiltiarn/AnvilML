# Plan Report: P1-A5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-A5                                         |
| Phase       | 001 — Walking Skeleton                      |
| Description | anvilml: CI workflow (Linux fmt+clippy+test, Windows clippy+test) |
| Depends on  | P1-A4                                         |
| Project     | anvilml                                       |
| Planned at  | 2026-06-01T05:21:32Z                          |
| Attempt     | 1                                             |

## Objective

Create a GitHub Actions CI workflow (`.github/workflows/ci.yml`) that validates the AnvilML Rust codebase on both Linux and Windows. The workflow ensures code formatting correctness via `cargo fmt`, linting via `cargo clippy`, and functional correctness via `cargo test`, all using the `mock-hardware` feature flag for deterministic CI runs. A cross-compilation check against `x86_64-pc-windows-gnu` on Linux catches platform-specific drift before native Windows CI runs.

## Scope

### In Scope
- Create `.github/workflows/ci.yml` with two jobs:
  - **rust-linux** (`ubuntu-latest`): `cargo fmt --all --check`, `cargo clippy --workspace --features mock-hardware -- -D warnings`, `cargo test --workspace --features mock-hardware`, cross-check `cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware`
  - **rust-windows** (`windows-latest`): `cargo clippy --workspace --features mock-hardware -- -D warnings`, `cargo test --workspace --features mock-hardware` (no `fmt` on Windows)
- Cargo cache keyed by OS to speed up subsequent runs
- All jobs use the pinned `rust-toolchain.toml` (1.95.0) automatically via `actions/checkout` + `actions-rust-lang/setup-rust-toolchain`
- No other CI jobs (python-worker, openapi-diff) — those belong to later phases

### Out of Scope
- python-worker test job (P20-A4)
- openapi-diff gate job (P20-A4)
- Any source code changes in the anvilml crates
- Dependency additions or version changes
- Cache key tuning beyond standard pattern
- Matrix expansion (macOS, other targets)

## Approach

1. **Create `.github/workflows/ci.yml`** with `name: CI`, `on: [push, pull_request]`, targeting `branches: [main]`.
2. **Define global env**: set `CARGO_TERM_COLOR: always` for readable output.
3. **Define job `rust-linux`**:
   - `runs-on: ubuntu-latest`
   - Step 1: `actions/checkout@v4`
   - Step 2: `actions-rust-lang/setup-rust-toolchain@v1` with cache enabled, rustfmt and clippy components
   - Step 3: `cargo fmt --all --check`
   - Step 4: `cargo clippy --workspace --features mock-hardware -- -D warnings`
   - Step 5: `cargo test --workspace --features mock-hardware`
   - Step 6: Install mingw-w64 (`apt-get install -y mingw-w64`), then `rustup target add x86_64-pc-windows-gnu`, then `cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware`
4. **Define job `rust-windows`**:
   - `runs-on: windows-latest`
   - Step 1: `actions/checkout@v4`
   - Step 2: `actions-rust-lang/setup-rust-toolchain@v1` with cache enabled, clippy component
   - Step 3: `cargo clippy --workspace --features mock-hardware -- -D warnings`
   - Step 4: `cargo test --workspace --features mock-hardware`
5. **Verify the YAML** is syntactically valid by reading it back.

## Files Affected

| Action | Path                              | Description                                          |
|--------|-----------------------------------|------------------------------------------------------|
| CREATE | `.github/workflows/ci.yml`        | GitHub Actions CI workflow with rust-linux and rust-windows jobs |

## Tests

| Test ID / Name            | File                     | Validates               |
|---------------------------|--------------------------|-------------------------|
| CI YAML syntax             | `.github/workflows/ci.yml` | Valid YAML structure, correct job names, correct runners |
| rust-linux fmt check       | CI job step              | `cargo fmt --all --check` passes (no pending formatting) |
| rust-linux clippy          | CI job step              | `cargo clippy` exits 0 with `-D warnings`                |
| rust-linux test            | CI job step              | `cargo test --workspace` exits 0                         |
| rust-linux cross-check     | CI job step              | `cargo check --target x86_64-pc-windows-gnu` compiles    |
| rust-windows clippy        | CI job step              | `cargo clippy` on native Windows exits 0                 |
| rust-windows test          | CI job step              | `cargo test` on native Windows exits 0                   |

## CI Impact

This task **creates** the primary CI workflow file `.github/workflows/ci.yml`. It establishes the base gate (Linux fmt + clippy + test + cross-check, Windows clippy + test) that will be extended in later phases with `python-worker` and `openapi-diff` jobs. The workflow uses the standard `actions/checkout@v4` and `actions-rust-lang/setup-rust-toolchain@v1` actions. No existing CI files are modified because none exist yet.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| `actions-rust-lang/setup-rust-toolchain@v1` unavailable or outdated | Low | High | Use `rust-actions/setup-rust@v1` or `dtolnay/rust-toolchain` as fallback; verify version before writing |
| Cross-check step fails due to missing `gcc-mingw-w64` on Ubuntu runner | Medium | Medium | Explicitly run `apt-get install -y mingw-w64` before `rustup target add`; also try `g++-mingw-w64-x86-64` for the linker |
| `cargo test` hangs or times out due to async test fixtures | Low | High | Add timeout-minutes: 30 to both jobs; use `mock-hardware` feature which avoids real hardware I/O |
| Cache key collision between Linux and Windows jobs | Low | Low | Use OS-specific cache key prefixes (`cargo-cache-ubuntu` / `cargo-cache-windows`) |

## Acceptance Criteria

- [ ] `.github/workflows/ci.yml` exists and is valid YAML
- [ ] Workflow defines exactly two jobs: `rust-linux` and `rust-windows`
- [ ] `rust-linux` runs on `ubuntu-latest` and executes all four steps (fmt check, clippy, test, cross-check)
- [ ] `rust-windows` runs on `windows-latest` and executes both steps (clippy, test)
- [ ] Both jobs use the pinned Rust 1.95.0 toolchain via `actions-rust-lang/setup-rust-toolchain`
- [ ] Both jobs have cargo dependency cache enabled
- [ ] All steps use `--features mock-hardware` for deterministic CI
- [ ] `rust-windows` does NOT include a `cargo fmt` step (per spec)
- [ ] Cross-check on Linux uses `--target x86_64-pc-windows-gnu` with mingw-w64 installed
