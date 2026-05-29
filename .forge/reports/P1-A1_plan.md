# Plan Report: P1-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-A1                                         |
| Phase       | 001 — Workspace Scaffold                    |
| Description | anvilml: Cargo workspace root, crate skeletons, and .gitattributes |
| Depends on  | none                                          |
| Project     | anvilml                                       |
| Planned at  | 2026-05-29T14:01:59Z                          |
| Attempt     | 1                                             |

## Objective

Establish a compilable Cargo workspace for the AnvilML Rust backend that contains all 8 library crates plus the launcher binary, together with infrastructure files (`rust-toolchain.toml`, `anvilml.toml`, `.gitattributes`) that ensure correct tooling and line endings. This is the foundational scaffold from which every subsequent phase builds; at this stage no business logic is implemented — only stubs that compile cleanly under `--features mock-hardware`.

## Scope

### In Scope
- Workspace root `Cargo.toml` with `[workspace]` listing all 8 crates + backend as members
- `rust-toolchain.toml` declaring stable channel with `rustfmt` and `clippy` components
- `anvilml.toml` placeholder config file (empty body, comment header)
- `.gitattributes` with line-ending rules for `.sh`, `.ps1`, `.py`, `.rs`, `.toml`, `.json`, `.md`
- 8 crate directories under `crates/`, each with:
  - `Cargo.toml` (minimal, no dependencies beyond what is needed for the stub to compile)
  - `src/lib.rs` stub containing only a module-level doc comment and an empty `#[cfg(test)] mod tests { #[test] fn it_works() { assert!(true); } }`
- `crates/anvilml-openapi/Cargo.toml` with `[[bin]]` section (not a library) and `src/main.rs` stub that prints `"openapi stub"` and exits 0
- `mock-hardware` feature declared in `anvilml-hardware/Cargo.toml`
- All files must compile under `cargo build --workspace --features mock-hardware` with exit code 0

### Out of Scope
- Any business logic, domain types, or I/O code
- Dependencies beyond what is strictly needed for stub compilation (no external crates yet)
- CI workflow files (handled by P1-A2 and P1-A3)
- Backend directory structure, migration scaffold, or `ipc.py` stub (handled by P1-A4)
- Python worker package layout (handled by P1-B1)
- Git commits or pushes (The Forge handles this)

## Approach

1. **Create workspace root `Cargo.toml`**
   - Declare `[workspace]` with `members = ["backend", "crates/anvilml-core", "crates/anvilml-hardware", "crates/anvilml-registry", "crates/anvilml-ipc", "crates/anvilml-worker", "crates/anvilml-scheduler", "crates/anvilml-server", "crates/anvilml-openapi"]`
   - Set `resolver = "2"`

2. **Create `rust-toolchain.toml`**
   - `[toolchain] channel = "stable" components = ["rustfmt", "clippy"]`

3. **Create `anvilml.toml`**
   - Empty TOML body with a comment block explaining it is the default configuration file path.

4. **Create `.gitattributes`**
   - Write exactly these rules in this order:
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

5. **Create crate directories and stubs** (8 crates)
   - For each of `anvilml-core`, `anvilml-hardware`, `anvilml-registry`, `anvilml-ipc`, `anvilml-worker`, `anvilml-scheduler`, `anvilml-server`:
     - Create `crates/<name>/Cargo.toml` with `[package] name = "anvilml-<name>" version = "0.0.0" edition = "2021"`
     - For `anvilml-hardware`: add `[features] mock-hardware = []`
     - Create `crates/<name>/src/lib.rs` with a module-level doc comment and an empty test module
   - For `anvilml-openapi`:
     - Create `crates/anvilml-openapi/Cargo.toml` with `[package] name = "anvilml-openapi" version = "0.0.0" edition = "2021"` and `[[bin]] name = "anvilml-openapi" path = "src/main.rs"`
     - Create `crates/anvilml-openapi/src/main.rs` with a `fn main()` that prints `"openapi stub"` and calls `std::process::exit(0)`

6. **Verify compilation**
   - Run `cargo build --workspace --features mock-hardware` to confirm exit code 0

## Files Affected

| Action   | Path                                          | Description                                                      |
|----------|-----------------------------------------------|------------------------------------------------------------------|
| CREATE   | Cargo.toml                                    | Workspace root with all 8 crates + backend as members            |
| CREATE   | rust-toolchain.toml                           | Stable toolchain with rustfmt + clippy components                |
| CREATE   | anvilml.toml                                  | Empty config placeholder with comment header                     |
| CREATE   | .gitattributes                                | Line-ending rules for scripts, code, and docs                    |
| CREATE   | crates/anvilml-core/Cargo.toml                | Package manifest (no deps)                                       |
| CREATE   | crates/anvilml-core/src/lib.rs                | Stub lib with doc comment + empty test                           |
| CREATE   | crates/anvilml-hardware/Cargo.toml            | Package manifest + `mock-hardware` feature declaration           |
| CREATE   | crates/anvilml-hardware/src/lib.rs            | Stub lib                                                         |
| CREATE   | crates/anvilml-registry/Cargo.toml            | Package manifest (no deps)                                       |
| CREATE   | crates/anvilml-registry/src/lib.rs            | Stub lib                                                         |
| CREATE   | crates/anvilml-ipc/Cargo.toml                 | Package manifest (no deps)                                       |
| CREATE   | crates/anvilml-ipc/src/lib.rs                 | Stub lib                                                         |
| CREATE   | crates/anvilml-worker/Cargo.toml              | Package manifest (no deps)                                       |
| CREATE   | crates/anvilml-worker/src/lib.rs              | Stub lib                                                         |
| CREATE   | crates/anvilml-scheduler/Cargo.toml           | Package manifest (no deps)                                       |
| CREATE   | crates/anvilml-scheduler/src/lib.rs           | Stub lib                                                         |
| CREATE   | crates/anvilml-server/Cargo.toml              | Package manifest (no deps)                                       |
| CREATE   | crates/anvilml-server/src/lib.rs              | Stub lib                                                         |
| CREATE   | crates/anvilml-openapi/Cargo.toml             | Package manifest with `[[bin]]` section                          |
| CREATE   | crates/anvilml-openapi/src/main.rs            | Binary stub: prints "openapi stub", exits 0                      |

## Tests

No new test files are written in this task. Each crate's `src/lib.rs` includes an inline empty test module (`#[cfg(test)] mod tests { #[test] fn it_works() { assert!(true); } }`) which serves as a compile-time smoke check for the stub. The workspace-level `cargo test --workspace --features mock-hardware` will exercise all 8 stubs.

| Test ID / Name            | File                              | Validates               |
|---------------------------|-----------------------------------|-------------------------|
| `it_works` (x8)           | crates/*/src/lib.rs               | Each crate compiles and its stub test passes |

## CI Impact

No CI changes required. This task only creates source files; CI workflow changes are handled by P1-A2 and P1-A3.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| `cargo build` fails due to missing workspace members or misconfigured paths | Low | High | Verify all member paths match actual directory names; use relative paths from workspace root |
| `mock-hardware` feature not forwarded causes compile error in later phases when dependencies are added | Medium | Low | Document forwarding rule in plan; add forwarding in the phase where the dependency relationship is established (per TASKS_PHASE001.md §Known Constraints) |
| `.gitattributes` rules conflict with existing tracked files that have wrong line endings | Low | Medium | Rules apply to future commits; existing files are not present yet (fresh repo) — no conflict possible |
| `anvilml-openapi` binary conflicts with a library target | Low | Low | Explicitly use `[[bin]]` section, not `[lib]`; crate name differs from package name only by convention |

## Acceptance Criteria

- [ ] `Cargo.toml` exists at workspace root with `[workspace]` listing all 8 crates + backend as members and `resolver = "2"`
- [ ] `rust-toolchain.toml` exists with `channel = "stable"` and `components = ["rustfmt", "clippy"]`
- [ ] `anvilml.toml` exists as an empty config placeholder
- [ ] `.gitattributes` contains the 8 line-ending rules in the specified order
- [ ] All 8 crate directories exist under `crates/` with `Cargo.toml` and source files
- [ ] `anvilml-hardware/Cargo.toml` declares `[features] mock-hardware = []`
- [ ] `anvilml-openapi/Cargo.toml` contains a `[[bin]]` section (not `[lib]`)
- [ ] `cargo build --workspace --features mock-hardware` exits 0
