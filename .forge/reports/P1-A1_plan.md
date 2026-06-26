# Plan Report: P1-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-A1                                       |
| Phase       | 001 — Repository Scaffold                   |
| Description | Workspace: Cargo.toml, toolchain pin, gitattributes |
| Depends on  | none                                        |
| Project     | anvilml                                     |
| Planned at  | 2026-06-26T09:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Establish the Cargo workspace root (`Cargo.toml`), the pinned Rust toolchain (`rust-toolchain.toml`), and repository line-ending conventions (`.gitattributes`) so that every subsequent crate has a place to register itself and a fixed Rust version/edition to compile against. After this task completes, `cargo build --workspace --features mock-hardware` will succeed once the crate stubs are added by later tasks, and the workspace is a valid, compilable Rust project.

## Scope

### In Scope
- Create `Cargo.toml` at the repository root as a Cargo workspace with `members = ["backend"]` and `[workspace.package]` section containing `version = "0.1.0"`, `edition = "2024"`, `rust-version = "1.96.0"`.
- Create `rust-toolchain.toml` pinning `channel = "1.96.0"`, `components = ["rustfmt", "clippy"]`, `targets = ["x86_64-pc-windows-gnu"]`.
- Create `.gitattributes` with line-ending rules: `*.sh text eol=lf`, `*.py text eol=lf`, `*.rs text eol=lf`, `*.ps1 text eol=crlf`.
- Verify the toolchain is installed and `rustc --version` prints `1.96.0`.

### Out of Scope
None. This task may not defer any scope (`defers_to (from JSON): []`).

## Existing Codebase Assessment

No prior source exists. This task establishes the baseline patterns for subsequent phases. The repository contains only `LICENSE` and `docs/` — there is no `Cargo.toml`, no `backend/` directory, no `crates/` directory, and no `rust-toolchain.toml`. This is a near-empty repository at the start of Phase 1, and P1-A1 is the first task to create any build infrastructure.

## Resolved Dependencies

None. This task creates only configuration files — no external crates, packages, or dependencies are introduced or referenced.

## Approach

1. **Create `Cargo.toml`** at the repository root with the following exact content:
   - `[workspace]` section with `members = ["backend"]`.
   - `[workspace.package]` section with `version = "0.1.0"`, `edition = "2024"`, `rust-version = "1.96.0"`.
   - No other members listed — crate paths under `crates/*` are added incrementally by later tasks (P1-B1 through P1-B6), because Cargo errors on workspace member paths that do not exist on disk.
   - Rationale: `backend` is the only crate that exists (or will exist) at this point. The workspace root serves as the single source of truth for version and edition, which downstream crates inherit via `workspace = true`.

2. **Create `rust-toolchain.toml`** at the repository root with the following exact content:
   - `channel = "1.96.0"` — pins the Rust toolchain channel.
   - `components = ["rustfmt", "clippy"]` — ensures formatter and linter are available automatically.
   - `targets = ["x86_64-pc-windows-gnu"]` — enables Windows cross-compilation from Linux/WSL2 as required by the ENVIRONMENT.md §1 platform cross-check.
   - Rationale: Both the channel and edition are exact pins per ENVIRONMENT.md §1. The Windows target is required for the WSL2 local gate (ENVIRONMENT.md §7) that checks `--target x86_64-pc-windows-gnu`.

3. **Create `.gitattributes`** at the repository root with the following exact content:
   - `*.sh text eol=lf`
   - `*.py text eol=lf`
   - `*.rs text eol=lf`
   - `*.ps1 text eol=crlf`
   - Rationale: Shell scripts, Python files, and Rust source files use LF line endings (standard on Linux/WSL2). PowerShell scripts use CRLF (standard on Windows). This ensures consistent line endings across platforms and prevents git diff noise from line-ending mismatches.

4. **Verify toolchain installation**: Run `rustc --version` and confirm it prints `1.96.0`. If rustup is not installed, install it first via the standard rustup installer. If 1.96.0 is not installed, run `rustup component add rustc --toolchain 1.96.0`.

## Public API Surface

None. This task creates configuration files only — no source code, no functions, no types, no re-exports.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | Cargo.toml | Workspace root: members = ["backend"], [workspace.package] with version/edition/rust-version |
| CREATE | rust-toolchain.toml | Pinned toolchain: channel 1.96.0, components rustfmt/clippy, target x86_64-pc-windows-gnu |
| CREATE | .gitattributes | Line-ending rules for .sh, .py, .rs (LF) and .ps1 (CRLF) |

## Tests

None. This task creates only configuration files with no executable logic. The acceptance criterion is structural (files exist and are non-empty, rustc version matches).

## CI Impact

No CI changes required. These files are configuration scaffolding that will be consumed by CI once the workspace has buildable crates, but they do not alter any CI job definitions or behavior.

## Platform Considerations

None identified. The `.gitattributes` file is platform-neutral — it declares line-ending conventions that Git enforces on checkout regardless of the platform. The `rust-toolchain.toml` is read by rustup on all platforms identically. The Windows cross-check target (`x86_64-pc-windows-gnu`) is specified in the toolchain file for the benefit of the WSL2 local gate, but the file itself is not platform-specific.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `rustc --version` does not print 1.96.0 because the toolchain is not yet installed | Medium | High | Install via `rustup install 1.96.0 && rustup component add rustfmt clippy --toolchain 1.96.0`. The acceptance criterion requires this check to pass. |
| Cargo workspace `members` lists a path that doesn't exist yet, causing build errors for subsequent tasks | Low | Medium | Only `members = ["backend"]` is listed. Later tasks (P1-B1 through P1-B6) add their own crate paths to `members` in the same task that creates the crate directory, as documented in TASKS_PHASE001.md. |

## Acceptance Criteria

- [ ] `test -s /home/dryw/AnvilML/Cargo.toml` exits 0
- [ ] `test -s /home/dryw/AnvilML/rust-toolchain.toml` exits 0
- [ ] `test -s /home/dryw/AnvilML/.gitattributes` exits 0
- [ ] `rustc --version` prints a line containing `1.96.0`
