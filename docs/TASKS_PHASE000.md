# Tasks: Phase 000 — Repository Preamble

| Field | Value |
|-------|-------|
| Phase | 000 |
| Name | Repository Preamble |
| Milestone group | Pre-flight (before the runnable skeleton) |
| Depends on phases | none |
| Task file | `forge/tasks/tasks_phase000.json` |
| Tasks | 3 |

## Overview

Phase 000 establishes repository hygiene **before any code exists**. These tasks are not about application behaviour — they exist so that the very first `git add -A` (in Phase 001) stages the right files and nothing else, so line endings are consistent across Linux and Windows checkouts from the first commit, and so the Rust toolchain is pinned before the first build runs.

Without this phase, Phase 001 would stage `target/` (potentially gigabytes), the runtime `anvilml.db`, the `venv/`, and OS cruft into the repository, and would build against whatever floating toolchain the machine happens to have — the exact source of the local-versus-CI `rustfmt` drift seen previously. Doing this work first, as its own phase, keeps Phase 001 focused purely on the workspace and crate skeletons.

**Pre-existing in the starting repository (NOT created by these tasks):** `README.md`, `LICENSE`, `CODE_OF_CONDUCT.md` and any similar front-matter files; the `docs/` directory (`ARCHITECTURE.md`, `ENVIRONMENT.md`, `PHASES.md`, and the `TASKS_PHASE*.md` set); the `.clinerules` file; and the `.forge/` reports/state structure. The `docs/`, `.clinerules`, and `.forge/` contents are mandatory for `forge.py` to start at all, so they are guaranteed present before the first session runs. Cline must not attempt to create or overwrite any of these.

Two files that earlier drafts folded into Phase 001 — `.gitattributes` and `rust-toolchain.toml` — are lifted here into their own atomic tasks, consistent with the one-file-per-task rule.

Each task implements exactly one root-level file. There is no compiled output and no test suite in this phase; the Runnable Proof is verification by `git` plumbing commands and `rustup`, which is the appropriate "does it work" check for repository configuration.

## Tasks

| Task | File | Summary |
|------|------|---------|
| P0-A1 | `.gitignore` | anvilml: .gitignore covering Rust, Python, runtime, and OS/editor artifacts |
| P0-A2 | `.gitattributes` | anvilml: .gitattributes enforcing cross-platform line endings |
| P0-A3 | `rust-toolchain.toml` | anvilml: rust-toolchain.toml pinned to 1.95.0 with windows-gnu target |

## Task details

#### P0-A1: anvilml: .gitignore covering Rust, Python, runtime, and OS/editor artifacts

- **Prereqs:** none
- **Tags:** scaffold

Create .gitignore at repo root. Rust: /target, **/*.rs.bk, Cargo.lock is NOT ignored (binary app commits it). Python: __pycache__/, *.py[cod], .pytest_cache/, /venv, .venv, *.egg-info/. Runtime data: /anvilml.db, /anvilml.db-wal, /anvilml.db-shm, /artifacts/, /logs/, /models/. Env: .env. OS/editor: .DS_Store, Thumbs.db, .idea/, .vscode/, *.swp. Do NOT ignore .forge/ (committed by The Forge) or backend/openapi.json (committed). Verify: git status --porcelain shows no target/ or *.db entries after a build.

#### P0-A2: anvilml: .gitattributes enforcing cross-platform line endings

- **Prereqs:** P0-A1
- **Tags:** scaffold

Create .gitattributes at repo root to prevent CRLF/LF drift between Linux and Windows checkouts (the cause of prior rustfmt + shell-script breakage). Lines: '* text=auto' (normalise on commit); '*.rs text eol=lf'; '*.py text eol=lf'; '*.sh text eol=lf'; '*.toml text eol=lf'; '*.md text eol=lf'; '*.json text eol=lf'; '*.yml text eol=lf'; '*.ps1 text eol=crlf'; binary safety: '*.png binary', '*.safetensors binary', '*.ckpt binary'. Verify: git check-attr eol -- src/x.rs reports eol=lf; install_worker_deps.ps1 reports eol=crlf.

#### P0-A3: anvilml: rust-toolchain.toml pinned to 1.95.0 with windows-gnu target

- **Prereqs:** P0-A2
- **Tags:** scaffold

Create rust-toolchain.toml at repo root pinned EXACTLY: [toolchain] channel = "1.95.0", components = ["rustfmt", "clippy"], targets = ["x86_64-pc-windows-gnu"]. The explicit channel pin prevents rustfmt/clippy version drift between local and CI (a previously observed failure). The windows-gnu target enables the local cross-check (cargo check --target x86_64-pc-windows-gnu) that catches cfg-gated API breakage before the native Windows CI job. Verify: rustup show active-toolchain (run in repo root) reports 1.95.0; rustc --version prints 1.95.0.


## Runnable Proof

These are configuration files, so the proof uses `git` plumbing and `rustup` rather than running the binary.

```bash
# 1. .gitignore works — build, then confirm target/ and the DB are NOT staged:
cargo build 2>/dev/null || true        # creates target/ (may fail pre-skeleton; that's fine)
touch anvilml.db
git add -A
git status --porcelain | grep -E 'target/|anvilml\.db' && echo "FAIL: ignored paths staged" || echo "OK: target/ and *.db ignored"

# 2. .gitattributes enforces line endings:
git check-attr eol -- backend/src/main.rs        # → backend/src/main.rs: eol: lf
git check-attr eol -- backend/scripts/install_worker_deps.ps1   # → eol: crlf

# 3. rust-toolchain.toml pins 1.95.0:
rustc --version            # → rustc 1.95.0 (...)
rustup show active-toolchain   # → 1.95.0-... (overridden by '.../rust-toolchain.toml')
```

Expected: ignored paths never appear in `git status`; `.rs`/`.py`/`.sh` resolve to `eol=lf` and `.ps1` to `eol=crlf`; and `rustc` reports exactly `1.95.0`. Phase done when all three checks pass — at which point Phase 001 can scaffold the workspace on a clean, correctly-configured repository.