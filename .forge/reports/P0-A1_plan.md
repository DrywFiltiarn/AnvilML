# Plan Report: P0-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P0-A1                                       |
| Phase       | 000 — Repository Preamble                   |
| Description | anvilml: .gitignore covering Rust, Python, runtime, and OS/editor artifacts |
| Depends on  | none                                        |
| Project     | anvilml                                     |
| Planned at  | 2026-05-31T22:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Create a `.gitignore` file at the repository root to prevent build artifacts, virtual environments, runtime data files, and OS/editor cruft from being accidentally committed to the repository. This is a prerequisite for Phase 001's workspace scaffold so that the first `git add -A` stages only source code and configuration, not gigabytes of compiled output or local runtime state.

## Scope

### In Scope
- Create `.gitignore` at the repository root (`AnvilML/.gitignore`).
- Cover Rust build artifacts: `/target`, `**/*.rs.bk`.
- Explicitly allow `Cargo.lock` (binary app commits it).
- Cover Python artifacts: `__pycache__/`, `*.py[cod]`, `.pytest_cache/`, `/venv`, `.venv`, `*.egg-info/`.
- Cover runtime data files: `/anvilml.db`, `/anvilml.db-wal`, `/anvilml.db-shm`, `/artifacts/`, `/logs/`, `/models/`.
- Cover environment secrets: `.env`.
- Cover OS/editor artifacts: `.DS_Store`, `Thumbs.db`, `.idea/`, `.vscode/`, `*.swp`.
- Explicitly NOT ignore `.forge/` (committed by The Forge) and `backend/openapi.json` (committed).

### Out of Scope
- Creating or modifying `.gitattributes` (task P0-A2).
- Creating or modifying `rust-toolchain.toml` (task P0-A3).
- Any source code, tests, or build configuration changes.
- Modifying any existing files in the repository.
- Running builds, tests, or git operations as verification (those are acceptance criteria for the ACT session).

## Approach

1. Create `.gitignore` at the repository root with all required ignore patterns organized into logical groupings (Rust, Python, Runtime data, Environment, OS/editor).
2. Ensure `Cargo.lock` is explicitly allowed by omitting it from any glob that would match it — no `Cargo.lock` entry appears in the file.
3. Add explicit allow rules using `!` prefix for `.forge/` and `backend/openapi.json` so that even if a parent pattern could match them, they are tracked.
4. Structure the file with section comments (e.g., `# Rust`, `# Python`) for readability but no trailing blank lines at EOF.

## Files Affected

| Action   | Path                          | Description                                      |
|----------|-------------------------------|--------------------------------------------------|
| CREATE   | `.gitignore`                  | Repository gitignore covering Rust, Python, runtime, env, OS/editor artifacts |

## Tests

| Test ID / Name | File | Validates |
|----------------|------|-----------|
| None.          | —    | This task produces a configuration file only; no test code is written or modified. |

## CI Impact

No CI changes required.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| A pattern accidentally ignores a tracked file (e.g., `*.db` matching an intended commit) | Low | High | Explicit allow rules for `.forge/` and `backend/openapi.json`; manual verification in ACT session with `git status --porcelain` after build |
| Runtime DB files (`anvilml.db-wal`, `anvilml.db-shm`) are added later by SQLite and could be missed | Low | Medium | All three WAL/SHM variants are listed in the plan; verified during acceptance criteria check |
| The `.forge/` directory is ignored if a parent-level pattern catches it | Low | High | Explicit `!.forge/` allow rule placed after any broad patterns |

## Acceptance Criteria

- [ ] `.gitignore` exists at the repository root and contains patterns for `/target`, `**/*.rs.bk`, `__pycache__/`, `*.py[cod]`, `.pytest_cache/`, `/venv`, `.venv`, `*.egg-info/`
- [ ] `.gitignore` does NOT contain any entry matching `Cargo.lock`
- [ ] `.gitignore` contains patterns for `/anvilml.db`, `/anvilml.db-wal`, `/anvilml.db-shm`, `/artifacts/`, `/logs/`, `/models/`, `.env`
- [ ] `.gitignore` contains patterns for `.DS_Store`, `Thumbs.db`, `.idea/`, `.vscode/`, `*.swp`
- [ ] `.gitignore` contains explicit allow rules: `!.forge/` and `!backend/openapi.json`
- [ ] After a build (`cargo build 2>/dev/null || true`) and `touch anvilml.db`, running `git add -A` then `git status --porcelain` shows no lines matching `target/` or `anvilml.db`
