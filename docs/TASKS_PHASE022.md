# Tasks: Phase 022 — Release Packaging

| Field | Value |
|-------|-------|
| Phase | 022 |
| Name | Release Packaging |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 21 |

## Overview

Phase 022 prepares the release distribution: a GitHub Actions workflow that builds and publishes signed release artifacts on tag push.

**The provisioning-script hardware-detection work originally planned for this phase has moved to Phase 021 (`P21-A2`), found during a later authoring review.** `P21-A3` (Phase 021's auto-provisioning task) calls `scripts/install_worker_deps.sh`/`.ps1` directly; running hardware detection one phase later than the auto-provisioning that depends on it was a genuine sequencing defect, not a `defers_to`-shaped one — nothing was left unimplemented with an expected later fix, the phase order itself put the consumer before the capability. See `TASKS_PHASE021.md`'s Overview for the full explanation. Phase 022 now contains only the release-packaging task.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | release | P22-A1 | GitHub Release workflow |

## Prerequisites

Phase 021 complete (including `P21-A2`, hardware detection in the provisioning scripts — relocated here from this phase). `cargo build --release` exits 0.

## Task Descriptions

### Group A — release

#### P22-A1: backend: cargo build --release and GitHub Release workflow

**Goal:** Create `.github/workflows/release.yml` triggered on `v*.*.*` tag push. The workflow builds release binaries for Linux and Windows, packages them into distributable archives, generates `SHA256SUMS`, and publishes a GitHub Release with all artifacts attached.

**Files to create or modify:**
- `.github/workflows/release.yml` — new file; two build jobs and one release job

**Key implementation notes:**
- `build-linux` job: `ubuntu-latest`; `cargo build --release`; create `anvilml-linux-x64.tar.gz` containing the binary + `worker/` + `anvilml.toml` + `database/seeds/`
- `build-windows` job: `windows-latest`; `cargo build --release --target x86_64-pc-windows-msvc`; create `anvilml-windows-x64.zip` with same contents
- Release job: `sha256sum` over both archives → `SHA256SUMS` file; `gh release create ${{ github.ref_name }}` uploading all three files and a `CHANGELOG.md` excerpt as the release body
- `cargo build --release` must exit 0 locally before this task is considered complete

**Acceptance criterion:** `cargo build --release` exits 0; release zip (manually constructed from the workflow's artifact packaging logic) contains the `anvilml` binary, `worker/` directory, and `anvilml.toml`; `sha256sum --check SHA256SUMS` exits 0 against the archive.

---

## Phase Acceptance Criteria

```bash
cargo build --release
```

## Known Constraints and Gotchas

- The release zip must contain `worker/` source so auto-provisioning works on first run. Python source is not embedded in the binary. The bundled `worker/` source includes the hardware-detection logic added to `scripts/install_worker_deps.sh`/`.ps1` in Phase 021 (`P21-A2`) — confirm the release archive picks up the post-`P21-A2` version of these scripts, not a stale copy.
- PSScriptAnalyzer must be installed on the Windows runner for the `.ps1` lint gate to work. Add an installation step to the release workflow if it is not available by default.