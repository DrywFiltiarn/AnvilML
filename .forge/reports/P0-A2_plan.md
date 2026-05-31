# Plan Report: P0-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P0-A2                                       |
| Phase       | 000 — Repository Preamble                   |
| Description | anvilml: .gitattributes enforcing cross-platform line endings |
| Depends on  | P0-A1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-05-31T23:05:00Z                        |
| Attempt     | 1                                           |

## Objective

Create a `.gitattributes` file at the repository root to enforce consistent line endings across Linux and Windows checkouts. This prevents CRLF/LF drift that previously caused rustfmt failures on Linux when files were committed from Windows (or vice versa), and prevented shell scripts from executing correctly due to embedded carriage-return characters. The file uses `* text=auto` as a catch-all normalisation rule, then explicitly overrides line-ending behaviour per-extension: LF for source code and data files (.rs, .py, .sh, .toml, .md, .json, .yml), CRLF for Windows-native PowerShell scripts (.ps1), and `binary` mode for known binary asset types (.png, .safetensors, .ckpt). This is a prerequisite for Phase 001's workspace scaffold so that every subsequent `git add -A` stages files with the correct line endings from the first commit.

## Scope

### In Scope
- Create `.gitattributes` at the repository root (`AnvilML/.gitattributes`).
- Define `* text=auto` as the default normalisation rule (git stores LF on commit, converts on checkout per-platform).
- Define `eol=lf` patterns for: `*.rs`, `*.py`, `*.sh`, `*.toml`, `*.md`, `*.json`, `*.yml`.
- Define `eol=crlf` pattern for: `*.ps1` (Windows-native PowerShell scripts).
- Define `binary` patterns for known binary assets: `*.png`, `*.safetensors`, `*.ckpt`.
- Structure the file with section comments for readability, no trailing blank lines at EOF.

### Out of Scope
- Creating or modifying `.gitignore` (task P0-A1, already completed).
- Creating or modifying `rust-toolchain.toml` (task P0-A3).
- Any source code, tests, or build configuration changes.
- Modifying any existing files in the repository.
- Running builds, tests, or git operations as verification (those are acceptance criteria for the ACT session).
- Adding patterns for file types not mentioned in the task specification.

## Approach

1. Create `.gitattributes` at the repository root with `* text=auto` as the first line — this is the Git default but stated explicitly for clarity and to serve as a comment anchor.
2. Add explicit `eol=lf` rules for each text extension that must always be LF: `*.rs`, `*.py`, `*.sh`, `*.toml`, `*.md`, `*.json`, `*.yml`. These are grouped by category in the file (source code, config/data, documentation) with section comments.
3. Add explicit `eol=crlf` rule for `*.ps1` — PowerShell scripts on Windows require CRLF line endings to execute correctly via `powershell -File`, and git must not normalise them to LF.
4. Add `binary` rules for known binary asset types: `*.png`, `*.safetensors`, `*.ckpt`. The `binary` attribute tells git to never attempt text processing (no line-ending conversion, no diff, no merge). These file types are expected in Phase 021–022 when real ML models and generated images enter the repo.
5. Order rules from most general (`* text=auto`) to most specific (extension overrides), following Git's last-match-wins semantics.

## Files Affected

| Action   | Path                      | Description                                           |
|----------|---------------------------|-------------------------------------------------------|
| CREATE   | `.gitattributes`          | Cross-platform line-ending rules for the entire repo  |

## Tests

| Test ID / Name | File | Validates |
|----------------|------|-----------|
| None.          | —    | This task produces a configuration file only; no test code is written or modified. |

## CI Impact

No CI changes required.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| A `text=auto` file on a Windows checkout introduces CRLF into files that should be LF (e.g., a `.rs` file edited in a Windows editor without respecting git attributes) | Low | High | Explicit `*.rs text eol=lf` override forces LF regardless of platform; verified during ACT session with `git check-attr` |
| PowerShell scripts (.ps1) may not exist yet in the repo (they are added in Phase 021+), so early verification is impossible | Low | Low | The rule is added now as forward-looking scaffolding; verification is deferred to the ACT session once `.ps1` files exist, or skipped with a note |
| Binary patterns (`*.safetensors`, `*.ckpt`) could match unintended files if a future file type shares an extension | Low | Low | These are well-established extensions for ML model weights; no ambiguity in practice. If needed, scope can be tightened later without breaking existing checkouts |
| Existing tracked files with wrong line endings will not be auto-corrected by adding `.gitattributes` alone (requires `git add --renormalize`) | Medium | Medium | The ACT session for P0-A2 should include `git add --renormalize -A` as part of the verification step to re-normalise all tracked files against the new rules |

## Acceptance Criteria

- [ ] `.gitattributes` exists at the repository root and contains `* text=auto`
- [ ] `.gitattributes` contains `*.rs text eol=lf`, `*.py text eol=lf`, `*.sh text eol=lf`, `*.toml text eol=lf`, `*.md text eol=lf`, `*.json text eol=lf`, `*.yml text eol=lf`
- [ ] `.gitattributes` contains `*.ps1 text eol=crlf`
- [ ] `.gitattributes` contains `*.png binary`, `*.safetensors binary`, `*.ckpt binary`
- [ ] After creating the file, `git check-attr eol -- README.md` resolves to `eol: lf`
- [ ] After creating the file, if any `.rs` files exist, `git check-attr eol -- <path>.rs` resolves to `eol: lf`
- [ ] If any `.ps1` files exist (e.g., `backend/scripts/install_worker_deps.ps1`), `git check-attr eol -- backend/scripts/install_worker_deps.ps1` resolves to `eol: crlf`
- [ ] After `git add -A`, no text files are staged with CRLF line endings (verified by `git diff --cached --check` showing no errors)
