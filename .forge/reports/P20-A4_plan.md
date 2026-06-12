# Plan Report: P20-A4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P20-A4                                            |
| Phase       | 020 — OpenAPI & Launcher Polish                   |
| Description | CI openapi-diff gate + python-worker pytest job   |
| Depends on  | P20-A1, P20-A2, P20-A3                            |
| Project     | anvilml                                           |
| Planned at  | 2026-06-12T09:05:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add two CI gates to `.github/workflows/ci.yml`:
(1) an openapi-diff gate in the `rust-linux` path that regenerates `backend/openapi.json`
via `anvilml-openapi` and fails the job if the committed file is stale, and
(2) a `python-worker` job (ubuntu-latest) that installs the full Python worker dependency
set from `worker/requirements/base.txt` and runs the pytest suite under `ANVILML_WORKER_MOCK=1`.

## Scope

### In Scope
- Modify `.github/workflows/ci.yml`:
  - Add `openapi-diff` step block to the `rust` job matrix entry for ubuntu-latest (after checkout, before other steps).
  - Update the existing `python-worker` job to install dependencies from `worker/requirements/base.txt` and run `pytest worker/tests/` with `ANVILML_WORKER_MOCK=1`.
- Verify locally that `cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json` exits 0.
- Verify locally that `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` exits 0.

### Out of Scope
- No changes to any Rust source, test, or config files.
- No changes to Python source, worker, or requirements files.
- No changes to the Windows CI path (the openapi-diff gate is added to the rust-linux path only, per the task spec).
- No changes to existing gate ordering beyond the insertion points described above.

## Approach

1. **Read current CI** — confirm the existing structure of `.github/workflows/ci.yml` (already done).

2. **Add openapi-diff gate to rust-linux job** — Insert a new step block after the `Checkout` step (or after toolchain setup) in the `rust` job, guarded for ubuntu-latest only:
   ```yaml
   - name: Generate OpenAPI spec
     if: matrix.os == 'ubuntu-latest'
     run: cargo run -p anvilml-openapi
   - name: Check OpenAPI spec is up to date
     if: matrix.os == 'ubuntu-latest'
     run: git diff --exit-code backend/openapi.json
   ```
   These steps are placed before the lint and test steps so that a stale spec fails early.

3. **Update python-worker job** — Replace the current `pip install msgpack pillow pytest` line with:
   ```yaml
   - name: Install dependencies
     run: pip install -r worker/requirements/base.txt
   ```
   Keep the existing `Run worker tests (mock mode)` step unchanged (it already runs `pytest worker/tests/ -v` with `ANVILML_WORKER_MOCK=1`).

4. **Local verification — openapi-diff** — Run:
   ```bash
   cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json
   ```
   Confirm exit code 0 (spec is already committed and matches).

5. **Local verification — pytest** — Run:
   ```bash
   ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v
   ```
   Confirm exit code 0 (all worker tests pass in mock mode).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `.github/workflows/ci.yml` | Add openapi-diff steps to rust-linux path; update python-worker deps to use `worker/requirements/base.txt` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| (local) | `cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json` | OpenAPI spec is committed and matches current handler annotations |
| (local) | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` | Python worker tests pass under mock mode |

## CI Impact

This task adds two new CI gates that were previously absent or partial:

- **OpenAPI diff gate** — Previously listed in `ARCHITECTURE.md §9` as a gate but not actually present in the CI workflow. The new steps will cause the `rust` job to fail on ubuntu-latest if `backend/openapi.json` is out of sync with handler annotations. This is a new enforcement gate; no existing gates are removed or disabled.

- **Python worker deps** — The existing `python-worker` job installs dependencies inline (`pip install msgpack pillow pytest`). The updated job uses `worker/requirements/base.txt` which adds `numpy`, `safetensors`, and `diffusers`/`transformers` on top. These additional packages are needed for full worker test coverage but may increase install time. The `python-worker` job already runs on both ubuntu-latest and windows-latest; the updated dependency installation applies to both platforms.

No existing jobs are removed, renamed, or disabled. The `fail-fast: false` strategy is preserved.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `pip install -r worker/requirements/base.txt` pulls in heavy packages (diffusers, transformers, safetensors, numpy) causing CI timeout | Medium | High | The base.txt file is already documented as the canonical dependency list. If install time becomes an issue, a CI cache for pip can be added in a follow-up task. For now, the existing CI timeout should be sufficient. |
| `cargo run -p anvilml-openapi` fails because P20-A1 or P20-A2 are not yet merged | Low (prereqs verified) | High | Task depends on P20-A1 and P20-A2; the orchestrator ensures they land first. If a prereq is missing, the gate fails early and the orchestrator handles it. |
| `git diff --exit-code` fails if openapi.json has trailing-whitespace or formatting differences | Low | Medium | `anvilml-openapi` uses `serde_json::to_string_pretty` which produces deterministic output. If a mismatch occurs, the fix is to regenerate the spec (a one-line change to the workflow step). |
| pytest dependency conflict from base.txt (e.g. pytest version clash) | Low | Medium | base.txt already lists `pytest` as a dependency; the existing CI tests pass with it locally. If a conflict appears, pin the version in base.txt. |

## Acceptance Criteria

- [ ] `.github/workflows/ci.yml` contains `cargo run -p anvilml-openapi` and `git diff --exit-code backend/openapi.json` steps in the rust-linux CI path
- [ ] `.github/workflows/ci.yml` python-worker job uses `pip install -r worker/requirements/base.txt` for dependency installation
- [ ] `cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json` exits 0 locally
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` exits 0 locally
- [ ] All existing CI gates are preserved (no jobs or steps removed or disabled)
