# Plan Report: P0-A1

| Field       | Value                                    |
|-------------|------------------------------------------|
| Task ID     | P0-A1                                    |
| Phase       | 000 — Repository Preamble               |
| Description | .gitignore, .gitattributes, rust-toolchain.toml |
| Depends on  | none                                     |
| Project     | anvilml                                  |
| Planned at  | 2026-06-14T00:30:00Z                     |
| Attempt     | 1                                        |

## Objective

Create the three repository hygiene files that establish git tracking rules, line-ending
conventions, and the pinned Rust toolchain. When complete, `rustc --version` reports the
stable toolchain and `git check-attr eol -- worker/ipc.py` reports `lf`, confirming the
foundation on which all subsequent phases build.

## Scope

### In Scope
- Create `.gitignore` at the repository root with the following patterns:
  - `target/` — Cargo build output
  - `*.db` — SQLite database files
  - `*.db-wal` — SQLite write-ahead log
  - `*.db-shm` — SQLite shared memory
  - `*.venv` — Python virtual environments (catches any `.venv` anywhere)
  - `worker/.venv` — Python worker virtual environment (explicit)
  - `artifacts/` — Generated image storage
  - `*.log` — Log files
  - `.DS_Store` — macOS metadata
- Create `.gitattributes` at the repository root with line-ending rules:
  - `*.sh text eol=lf`
  - `*.py text eol=lf`
  - `*.rs text eol=lf`
  - `*.md text eol=lf`
  - `*.ps1 text eol=crlf`
  - `*.toml text eol=lf`
- Create `rust-toolchain.toml` at the repository root:
  - `[toolchain]` section with `channel = "1.95.0"`, `components = ["rustfmt", "clippy"]` and `targets = ["x86_64-pc-windows-gnu"]`

### Out of Scope
- Cargo workspace configuration (handled by P0-B1)
- GitHub Actions CI workflow (handled by P0-C1)
- `.forge/` directory structure (handled by P0-D1)
- `anvilml.toml` configuration file (handled in later phase)
- Any Rust or Python source code

## Existing Codebase Assessment

No prior source exists. This is Phase 000, Group A — the very first task in the build
sequence. The repository contains only `LICENSE`, `README.md`, `docs/`, and the `.forge/`
orchestrator directory. No `Cargo.toml`, no crate directories, no source files, and none
of the three files this task creates exist yet.

This task establishes the baseline patterns for subsequent phases. The line-ending rules
in `.gitattributes` will apply retroactively to all future commits. The `rust-toolchain.toml`
will cause `rustup` to install and use the stable toolchain with `rustfmt` and `clippy`
components before any `cargo` command runs.

## Resolved Dependencies

None. This task creates only configuration files — no external crates, packages, or
dependency declarations are introduced.

## Approach

1. **Create `rust-toolchain.toml`** at the repository root with:
   ```toml
   [toolchain]
   channel = "stable"
   components = ["rustfmt", "clippy"]
   ```
   Rationale: `ANVILML_DESIGN.md §17.1` explicitly specifies `channel = "stable"` (not a
   pinned version string). The `components` array ensures `rustfmt` and `clippy` are
   installed automatically when `rustup` activates this toolchain.

2. **Create `.gitattributes`** at the repository root with:
   ```
   *.sh text eol=lf
   *.py text eol=lf
   *.rs text eol=lf
   *.md text eol=lf
   *.ps1 text eol=crlf
   *.toml text eol=lf
   ```
   Rationale: `.ps1` PowerShell scripts require CRLF on Windows. All other source, config,
   and script files use LF. The pattern covers every file type present in the repository
   layout defined by `ARCHITECTURE.md §2`.

3. **Create `.gitignore`** at the repository root with:
   ```
   # Rust build artifacts
   target/

   # SQLite database files
   *.db
   *.db-wal
   *.db-shm

   # Python virtual environments
   *.venv
   worker/.venv

   # Generated artifacts
   artifacts/

   # Log files
   *.log

   # macOS metadata
   .DS_Store
   ```
   Rationale: Each pattern maps to a directory or file type that should never be committed.
   `target/` is the standard Cargo output directory. `*.db*` covers all SQLite journaling
   file variants. `worker/.venv` is the explicit Python worker venv path from
   `ENVIRONMENT.md §1`. `artifacts/` is the default artifact storage directory from
   `ENVIRONMENT.md §4`. `*.log` catches any log files. `.DS_Store` is a macOS artefact.

## Public API Surface

None. This task creates only repository configuration files — no source code, no types,
no functions, no public items.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `.gitignore` | Git ignore patterns for build artifacts, databases, venvs, logs, macOS metadata |
| CREATE | `.gitattributes` | Line-ending rules: LF for all files except `.ps1` (CRLF) |
| CREATE | `rust-toolchain.toml` | Pinned stable Rust toolchain with rustfmt and clippy components |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| N/A (config files only) | Toolchain version check | `rustc --version` prints stable | `rustc --version` matches `stable` |
| N/A (config files only) | Git attributes for Python file | `git check-attr eol -- worker/ipc.py` reports lf | `git check-attr eol -- worker/ipc.py` outputs `eol: lf` |

## CI Impact

No CI changes required. These are repository hygiene files that do not affect any CI job's
behaviour. The `.gitattributes` line-ending rules are enforced by git itself, not by CI.

## Platform Considerations

`.gitattributes` is the platform-specific element: `*.ps1` uses `eol=crlf` because Windows
PowerShell scripts require CRLF line endings, while all other file types use `eol=lf`.
The `rust-toolchain.toml` is platform-neutral — `rustup` resolves it identically on Linux,
Windows, and macOS. No `#[cfg(...)]` guards are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `.gitignore` patterns are too broad or miss a pattern needed by later phases | Low | Medium | Review against `ARCHITECTURE.md §2` file layout; the listed patterns cover all directories and file types mentioned there. If a later phase needs additional patterns, a follow-up task will add them. |
| `.gitattributes` line-ending rules cause pre-existing committed files to show as modified on first checkout | Low | Low | The repository has no committed source files yet (Phase 000, first commit). No pre-existing files exist to be affected. |
| `rustc --version` does not print `stable` because `rustup` is not installed or the toolchain is not active | Low | Medium | Verify `rustup` is installed and `stable` toolchain is active before marking task complete. If not, install `rustup` and `rustup default stable` — this is an environment setup step, not a code change. |

## Acceptance Criteria

- [ ] `rustc --version` contains the word `stable` (exit 0)
- [ ] `git check-attr eol -- worker/ipc.py` outputs a line containing `eol: lf` (exit 0)
- [ ] `test -f .gitignore` exits 0 (file exists)
- [ ] `test -f .gitattributes` exits 0 (file exists)
- [ ] `test -f rust-toolchain.toml` exits 0 (file exists)
- [ ] `grep -q 'channel = "stable"' rust-toolchain.toml` exits 0 (correct channel)
- [ ] `grep -q 'components = \["rustfmt", "clippy"\]' rust-toolchain.toml` exits 0 (correct components)
- [ ] `grep -q 'target/' .gitignore` exits 0 (build output ignored)
- [ ] `grep -q 'worker/.venv' .gitignore` exits 0 (worker venv ignored)
- [ ] `grep -q 'artifacts/' .gitignore` exits 0 (artifacts ignored)
- [ ] `grep -q '\*.ps1.*eol=crlf' .gitattributes` exits 0 (PowerShell CRLF rule present)
- [ ] `grep -q '\*.py.*eol=lf' .gitattributes` exits 0 (Python LF rule present)
