# Plan Report: P1-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-A3                                       |
| Phase       | 001 — Workspace Scaffold                    |
| Description | anvilml: Windows CI job (rust full suite on windows-latest) |
| Depends on  | P1-A1, P1-A2                                |
| Project     | anvilml                                     |
| Planned at  | 2026-05-29T14:55:55Z                        |
| Attempt     | 1                                           |

## Objective

Add a fourth CI job `rust-windows` to `.github/workflows/ci.yml` that runs the full Rust test suite on `windows-latest`. This proves cross-platform correctness from the first commit, ensuring the stub workspace compiles and passes all tests on Windows before any real logic is implemented. The job runs in parallel with the existing Linux jobs (no `needs:` dependency) to avoid serialising CI wall-clock time.

## Scope

### In Scope
- Add a single new job `rust-windows` to `.github/workflows/ci.yml`
- Job runs on `windows-latest` with steps: checkout, toolchain install, cache, clippy, test
- Separate Cargo cache key with `-windows` suffix to avoid cross-contamination with the Linux cache
- No `cargo fmt --all --check` step (platform-neutral, already run on Linux)
- No `python-worker` or `openapi-diff` steps (not applicable on Windows for this phase)
- No changes to any other files

### Out of Scope
- Any source code changes in Rust crates
- Changes to `.github/workflows/` jobs other than adding the new job
- Modifications to existing `rust`, `python-worker`, or `openapi-diff` jobs
- Adding Windows-specific test code or conditional compilation
- Any CI steps beyond clippy and test

## Approach

1. **Read current CI workflow** — confirm the existing three jobs (`rust`, `python-worker`, `openapi-diff`) are intact and note their structure for consistency.

2. **Append new job block** to `.github/workflows/ci.yml` after the `openapi-diff` job:
   - Job name: `rust-windows`
   - `runs-on: windows-latest`
   - No `needs:` declaration (runs in parallel)
   - Steps in order:
     1. `actions/checkout@v4` — check out the repository
     2. Install stable toolchain with `dtolnay/rust-toolchain@stable` action, declaring components `["rustfmt", "clippy"]` (required because `windows-latest` runners do not pre-install these)
     3. `actions/cache@v4` — cache `~/.cargo/registry` and `target/` with key `cargo-Windows-${{ hashFiles('Cargo.lock') }}-windows` (the `-windows` suffix prevents cross-contamination with the Linux cache key `cargo-Linux-...`)
     4. `cargo clippy --workspace --features mock-hardware -- -D warnings`
     5. `cargo test --workspace --features mock-hardware`

3. **Verify YAML structure** — ensure proper indentation, that the new job is at the same nesting level as existing jobs under `jobs:`, and that no trailing whitespace or formatting issues are introduced.

4. **No other file modifications** — this task touches only `.github/workflows/ci.yml`.

## Files Affected

| Action   | Path                              | Description                                    |
|----------|-----------------------------------|------------------------------------------------|
| MODIFY   | .github/workflows/ci.yml          | Append `rust-windows` job block after `openapi-diff` |

## Tests

No test files are written or modified. The CI job itself serves as the verification mechanism:

| Test ID / Name            | File                     | Validates               |
|---------------------------|--------------------------|-------------------------|
| rust-windows clippy step  | .github/workflows/ci.yml | Clippy lint passes on Windows with stub crates |
| rust-windows test step    | .github/workflows/ci.yml | `cargo test --workspace` passes on Windows with stub crates |

## CI Impact

One new job added to `.github/workflows/ci.yml`:

```yaml
  rust-windows:
    runs-on: windows-latest
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Install stable toolchain with rustfmt and clippy
        uses: dtolnay/rust-toolchain@stable
        with:
          components: [rustfmt, clippy]

      - name: Cache Cargo registry and build
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            target/
          key: cargo-Windows-${{ hashFiles('Cargo.lock') }}-windows

      - name: Clippy lint
        run: cargo clippy --workspace --features mock-hardware -- -D warnings

      - name: Run tests
        run: cargo test --workspace --features mock-hardware
```

No changes to existing jobs. The `rust-windows` job runs in parallel with all three Linux jobs.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| Cache key collision with Linux cache | Low | Medium | Using `-windows` suffix ensures separate keys; `Cargo.lock` hash still provides invalidation on dep changes |
| `dtolnay/rust-toolchain` action unavailable or misconfigured | Low | High | Fallback: use `actions-rs/toolchain@v1` with `toolchain: stable --component rustfmt --component clippy` |
| Windows runner lacks `cargo` binary | Very Low | High | `windows-latest` includes Rust toolchain by default; `dtolnay/rust-toolchain` ensures fmt/clippy components are present |
| `mock-hardware` feature not forwarded to all crates on Windows | Low | Medium | Forwarding is already declared in crate `Cargo.toml` files from P1-A1/P2 phases; if missing, clippy will fail with a clear error that must be fixed before this job passes |

## Acceptance Criteria

- [ ] `.github/workflows/ci.yml` contains a new job named `rust-windows` at the same indentation level as existing jobs
- [ ] Job declares `runs-on: windows-latest`
- [ ] Job does **not** declare any `needs:` dependency
- [ ] Job includes `actions/checkout@v4` step
- [ ] Job installs stable toolchain with `rustfmt` and `clippy` components via `dtolnay/rust-toolchain@stable`
- [ ] Job caches `~/.cargo/registry` and `target/` with a cache key containing `-windows` suffix
- [ ] Job runs `cargo clippy --workspace --features mock-hardware -- -D warnings`
- [ ] Job runs `cargo test --workspace --features mock-hardware`
- [ ] No `cargo fmt --all --check` step in the Windows job
- [ ] No `python-worker` or `openapi-diff` steps in the Windows job
- [ ] Existing `rust`, `python-worker`, and `openapi-diff` jobs are unchanged
