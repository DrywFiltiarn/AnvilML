# Tasks: Phase 001 — Workspace Scaffold

| Field            | Value                                                                    |
|------------------|--------------------------------------------------------------------------|
| Phase            | 001                                                                      |
| Name             | Workspace Scaffold                                                       |
| ANVIL Milestone  | M0                                                                       |
| Status           | Draft                                                                    |
| Depends on phases| none                                                                     |
| Task file        | `forge/tasks/tasks_phase001.json`                                        |
| Design reference | `ANVILML_DESIGN.md` §2 (Crate Decomposition), §7.1 (IPC / binary stdio), §20 (Testing Strategy), §22.4 (Cross-Platform Notes) |

---

## Overview

Phase 001 establishes the repository skeleton from which every subsequent phase builds. It produces no business logic, no types, and no running server. Its deliverables are: a Cargo workspace that compiles cleanly on both Linux and Windows; a `.gitattributes` file that locks line endings so scripts are never corrupted by `core.autocrlf`; a CI workflow covering Linux (fmt, clippy, test, python-worker, openapi-diff) and Windows (clippy, test) from the very first commit; a stub launcher binary; and the Python worker directory with a minimal pytest harness and the Windows binary-stdio guard already present in `ipc.py`.

Linux and Windows are co-equal first-class targets per `ANVILML_DESIGN.md §1.5`. Installing the Windows CI job in phase 001 — not later — means every subsequent phase is proven cross-platform as it lands, rather than accumulating a debt of platform bugs discovered only at release. The `mock-hardware` feature flag makes the Windows runner fully hermetic: no GPU, no model downloads, no OS-specific inference stack required.

The binary-stdio guard in `worker/ipc.py` is stubbed here rather than in phase 009 (when the full worker is implemented) because the file must exist from the first commit that CI validates on Windows. If the guard is added later, any phase that happens to run the worker stub on a Windows runner would produce corrupted frames silently. Placing the guard in the scaffold ensures it is always present.

At the end of this phase: `cargo build/test --workspace --features mock-hardware` passes on both Ubuntu and Windows runners; `cargo fmt --all --check` passes on Linux; the Python placeholder test passes; `.gitattributes` is committed.

---

## Group Reference

| Group | Subsystem               | Tasks           | Summary                                                              |
|-------|-------------------------|-----------------|----------------------------------------------------------------------|
| A     | Rust workspace & CI     | P1-A1 … P1-A4  | Workspace, gitattributes, Linux CI, Windows CI, backend dirs + ipc stub |
| B     | Python worker skeleton  | P1-B1           | worker/ package layout, pytest harness, requirements stubs           |

---

## Prerequisites

None. This is the initial phase. The only prerequisite is that the `anvilml` project is registered in `forge/repos.json` pointing to the AnvilML repository root on disk.

---

## Contract Documents Applicable to This Phase

| Document section         | Relevant tasks | What must match                                                     |
|--------------------------|----------------|---------------------------------------------------------------------|
| `ANVILML_DESIGN.md` §7.1 | P1-A4          | Binary-mode stdio guard in `worker/ipc.py` — exact `msvcrt.setmode` pattern |
| `ANVILML_DESIGN.md` §22.4| P1-A1          | `.gitattributes` line-ending rules for `.sh`, `.ps1`, `.py`, `.rs`  |
| `ANVILML_DESIGN.md` §20  | P1-A2, P1-A3   | CI job structure: three Linux jobs + one Windows job                |

---

## Task Descriptions

### Group A — Rust Workspace & CI

#### P1-A1: anvilml — Cargo workspace root, crate skeletons, and .gitattributes

**Goal:** Produce a compilable Cargo workspace containing all 8 library crates and the launcher binary crate, together with the `.gitattributes` file that enforces correct line endings on both OSes.

**Files to create or modify:**
- `Cargo.toml` — workspace root; `[workspace]` with `members = ["backend", "crates/*"]`
- `rust-toolchain.toml` — `[toolchain] channel = "stable" components = ["rustfmt", "clippy"]`
- `anvilml.toml` — empty config placeholder with a comment block
- `.gitattributes` — line-ending rules (see implementation notes)
- `crates/anvilml-core/Cargo.toml` + `src/lib.rs` — stub
- `crates/anvilml-hardware/Cargo.toml` + `src/lib.rs` — stub; declare `[features] mock-hardware = []`
- `crates/anvilml-registry/Cargo.toml` + `src/lib.rs` — stub
- `crates/anvilml-ipc/Cargo.toml` + `src/lib.rs` — stub
- `crates/anvilml-worker/Cargo.toml` + `src/lib.rs` — stub
- `crates/anvilml-scheduler/Cargo.toml` + `src/lib.rs` — stub
- `crates/anvilml-server/Cargo.toml` + `src/lib.rs` — stub
- `crates/anvilml-openapi/Cargo.toml` + `src/main.rs` — stub `[[bin]]` that prints `"openapi stub"` and exits 0

**Key implementation notes:**
- `.gitattributes` must contain exactly these rules, in this order:
  ```
  * text=auto
  *.sh text eol=lf
  *.ps1 text eol=crlf
  *.py text eol=lf
  *.rs text eol=lf
  *.toml text eol=lf
  *.json text eol=lf
  *.md text eol=lf
  ```
  This prevents `core.autocrlf` on Windows developer checkouts from corrupting shell scripts (which must stay LF) and ensures PowerShell scripts are committed with CRLF as Windows tools expect.
- The `mock-hardware` feature must be declared in `anvilml-hardware/Cargo.toml`. Every crate that will later depend on `anvilml-hardware` must forward it: `[features] mock-hardware = ["anvilml-hardware/mock-hardware"]`. At this stub stage, only `anvilml-hardware` needs the declaration; forwarding is added when the dependency is added in later phases.
- Do not add any `[dependencies]` beyond what is strictly needed for the stub to compile. All dependencies are added in the phases that implement the relevant logic.
- `crates/anvilml-openapi` is a `[[bin]]` crate, not a library. Its `Cargo.toml` must have `[[bin]] name = "anvilml-openapi"`.

**Acceptance criterion:** `cargo build --workspace --features mock-hardware` exits 0.

---

#### P1-A2: anvilml — Linux CI jobs (rust, python-worker, openapi-diff)

**Goal:** Install the three Linux CI jobs that gate every push: the Rust quality suite, the Python worker test harness, and the OpenAPI diff check.

**Files to create or modify:**
- `.github/workflows/ci.yml` — three jobs, all `runs-on: ubuntu-latest`

**Key implementation notes:**
- Job `rust` steps in order: (1) `cargo fmt --all --check`, (2) `cargo clippy --workspace --features mock-hardware -- -D warnings`, (3) `cargo test --workspace --features mock-hardware`. Each is a separate `run:` step so the failure is attributed to the correct step in the GitHub Actions log.
- Job `python-worker` steps: (1) `pip install -r worker/requirements/base.txt`, (2) `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v`. Zero collected tests is acceptable at this phase; the job must exit 0.
- Job `openapi-diff` steps: (1) `cargo run -p anvilml-openapi`, (2) `git diff --exit-code backend/openapi.json`. The stub `openapi.json` is `{}` committed in P1-A4; the generator also outputs `{}` at stub stage. The diff check confirms nothing has drifted.
- Cache the Cargo registry (`~/.cargo/registry`) and `target/` directory using `actions/cache`. Use a cache key that includes the `Cargo.lock` hash so the cache is invalidated when dependencies change.
- `python-worker` and `openapi-diff` jobs should declare `needs: [rust]` so they only run after the Rust suite passes, avoiding wasted runner minutes.

**Acceptance criterion:** Pushing to the repository triggers CI and all three Linux jobs pass.

---

#### P1-A3: anvilml — Windows CI job (rust full suite on windows-latest)

**Goal:** Add a fourth CI job that runs the full Rust test suite on Windows, proving cross-platform correctness from the first commit.

**Files to create or modify:**
- `.github/workflows/ci.yml` — add job `rust-windows`

**Key implementation notes:**
- Job `rust-windows` runs on `windows-latest`. Steps: (1) `actions/checkout`, (2) install stable toolchain with `rustfmt` and `clippy` components via `dtolnay/rust-toolchain`, (3) `actions/cache` with a separate cache key suffix `-windows` to avoid cross-contaminating the Linux cache, (4) `cargo clippy --workspace --features mock-hardware -- -D warnings`, (5) `cargo test --workspace --features mock-hardware`.
- `cargo fmt --all --check` is intentionally **omitted** from the Windows job. `rustfmt` output is platform-neutral; running it on Linux once is sufficient. Running it on both adds no signal and wastes minutes.
- `python-worker` and `openapi-diff` steps are omitted from the Windows job. Python behaviour is identical across OSes for the mock worker; the binary-mode guard will be tested by the IPC framing tests (phase 002) which run on both OSes via `cargo test`. OpenAPI generation is a Linux-only step.
- The Windows job must **not** use `needs:` on the Linux jobs — it should run in parallel with them so the total CI wall-clock time is not serialised.
- With only stub crates, the Windows job should pass in under 5 minutes on a fresh cache.

**Acceptance criterion:** The `rust-windows` job appears in the GitHub Actions run and passes with only stub crates present.

---

#### P1-A4: anvilml — backend directory structure, migration scaffold, and ipc.py stub

**Goal:** Establish the `backend/` layout and create `worker/ipc.py` with the Windows binary-stdio guard already in place, so that all subsequent phases that touch IPC have the correct foundation from the start.

**Files to create or modify:**
- `backend/src/main.rs` — stub `fn main()` printing `"AnvilML v0.0.0 — scaffold stub"`, exits 0
- `backend/openapi.json` — empty JSON object `{}`, committed as a placeholder
- `backend/migrations/.gitkeep` — ensures directory is tracked by git
- `backend/scripts/install_worker_deps.sh` — shell stub with a usage comment block describing its future purpose (detect CUDA/ROCm/CPU, create venv, pip install)
- `backend/scripts/install_worker_deps.ps1` — PowerShell stub with equivalent comment block; note `powershell -ExecutionPolicy Bypass -File …` invocation
- `backend/scripts/test_inference.py` — Python stub with docstring describing its future purpose
- `worker/worker_main.py` — prints `"worker stub — not implemented"` to stderr and exits 1
- `worker/ipc.py` — binary-mode guard + stub functions (see implementation notes)

**Key implementation notes:**
- `worker/ipc.py` must contain the following at module import time, **before any other I/O**, as specified in `ANVILML_DESIGN.md §7.1`:
  ```python
  import sys
  if sys.platform == "win32":
      import msvcrt, os
      msvcrt.setmode(sys.stdin.fileno(),  os.O_BINARY)
      msvcrt.setmode(sys.stdout.fileno(), os.O_BINARY)
  ```
  Below the guard, add stub functions `def read_frame(): raise NotImplementedError` and `def write_frame(msg): raise NotImplementedError`. These stubs are replaced in phase 002 (P2-B2). The guard itself must never be removed or moved.
- All worker I/O in future phases must use `sys.stdin.buffer` / `sys.stdout.buffer` (the binary wrappers), never the text wrappers `sys.stdin` / `sys.stdout`. This constraint applies to all phases that modify `ipc.py`.
- Do not add any logic to the provisioning scripts; they are placeholders only.

**Acceptance criterion:** `cargo build -p anvilml-openapi` exits 0 and `ls backend/migrations/ backend/scripts/` shows the expected files.

---

### Group B — Python Worker Skeleton

#### P1-B1: anvilml — worker/ Python package structure and pytest skeleton

**Goal:** Make `worker/` a properly structured Python package so that future phases can add modules without restructuring, and ensure the CI pytest job has at least one passing test immediately.

**Files to create or modify:**
- `worker/__init__.py` — empty
- `worker/nodes/__init__.py` — empty
- `worker/tests/__init__.py` — empty
- `worker/tests/test_placeholder.py` — `def test_placeholder(): assert True`
- `worker/requirements/base.txt` — `msgpack>=1.0`, `pillow>=10.0`, `pytest>=8.0`
- `worker/requirements/cuda.txt` — `# torch + CUDA — populated in phase 009`
- `worker/requirements/rocm.txt` — `# torch + ROCm — populated in phase 009`
- `worker/requirements/cpu.txt` — `# torch CPU-only — populated in phase 009`

**Key implementation notes:**
- The requirements files must be valid pip requirements format. A blank file or a comment-only file is valid; an invalid syntax line causes `pip install` to fail in CI.
- Do not import from `worker_main.py`, `ipc.py`, or any other worker module in the test file. The placeholder test must pass with zero imports beyond the standard library.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` exits 0 with 1 test passing.

---

## Phase Acceptance Criteria

The following must all pass before phase 001 is considered complete:

```
# Linux
cargo build --workspace --features mock-hardware
cargo test --workspace --features mock-hardware
cargo fmt --all --check
cargo clippy --workspace --features mock-hardware -- -D warnings
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v

# Windows (verified via GitHub Actions rust-windows job)
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware
```

---

## Known Constraints and Gotchas

- The `mock-hardware` feature flag must be declared in `anvilml-hardware/Cargo.toml` with `[features] mock-hardware = []`. In every crate that will eventually depend on `anvilml-hardware`, it must be forwarded as `mock-hardware = ["anvilml-hardware/mock-hardware"]`. At phase 001 only the declaration in `anvilml-hardware` is needed, but be aware this forwarding must be added whenever a dependency relationship is established in later phases.
- `rust-toolchain.toml` must declare `components = ["rustfmt", "clippy"]` or `cargo fmt` and `cargo clippy` will fail on a fresh runner that has not pre-installed them.
- The `.gitattributes` file must be committed in P1-A1, **before** any `.sh` or `.ps1` files are committed in P1-A4. If the attribute rules are absent when the scripts land, git may store them with incorrect line endings that cannot be fixed by a later commit without explicitly re-normalising with `git add --renormalize`.
- The `openapi-diff` CI job depends on `backend/openapi.json` being committed. If the file is absent, `git diff --exit-code` exits non-zero. Commit the `{}` placeholder in P1-A4, not in a later phase.
- The Windows CI cache key must use a `-windows` suffix (or equivalent OS tag). Sharing a cache key with the Linux job causes `target/` cache misses because Windows produces `.pdb` and `.exe` artefacts that do not exist in the Linux cache, causing `actions/cache` to never find a hit.
- Python `pytest` must be installed from `worker/requirements/base.txt` in the CI step. Do not assume it is pre-installed on `ubuntu-latest`.
- The binary-mode guard in `worker/ipc.py` uses `msvcrt`, which is a Windows-only stdlib module. The `if sys.platform == "win32":` guard is mandatory — importing `msvcrt` unconditionally will raise `ModuleNotFoundError` on Linux.
