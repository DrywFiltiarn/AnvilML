# Plan Report: P1-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-A1                                       |
| Phase       | 001 — Walking Skeleton                      |
| Description | anvilml: Cargo workspace root, crate skeletons, .gitattributes |
| Depends on  | P0-A3                                        |
| Project     | anvilml                                     |
| Planned at  | 2026-05-31T21:37:27Z                        |
| Attempt     | 1                                           |

## Objective

Establish the Cargo workspace structure for AnvilML by creating a workspace-level `Cargo.toml` that declares all eight crates plus the backend binary, scaffolding each crate directory with minimal `Cargo.toml` and source stubs, and configuring the `mock-hardware` feature flag on `anvilml-hardware`. This produces a compilable (empty) workspace that serves as the foundation for all subsequent Phase 1 tasks.

## Scope

### In Scope
- Create workspace-level `Cargo.toml` at repo root with `members = ["backend", "crates/anvilml-core", "crates/anvilml-hardware", "crates/anvilml-registry", "crates/anvilml-ipc", "crates/anvilml-worker", "crates/anvilml-scheduler", "crates/anvilml-server", "crates/anvilml-openapi"]`
- Create 8 crate directories under `crates/`, each with:
  - `Cargo.toml` declaring the package name, version (0.1.0), and dependency on crates below it in the dependency graph
  - `src/lib.rs` stub (empty `pub fn stub() {}` function)
- Create `crates/anvilml-openapi/src/main.rs` stub instead of `lib.rs` (it is a binary crate)
- Declare `[features] mock-hardware = []` in `anvilml-hardware/Cargo.toml`
- Forward the `mock-hardware` feature in all crates that depend on `anvilml-hardware` (worker, scheduler, server) per ARCHITECTURE.md §5
- Create `Cargo.lock` by running `cargo build --workspace --features mock-hardware`
- Do NOT recreate `rust-toolchain.toml` or `.gitattributes` (already exist from Phase 000)

### Out of Scope
- Any business logic or real implementations in crate stubs
- Backend binary implementation (P1-A2)
- Server/router implementation (P1-A3, P1-A4)
- CI workflow creation (P1-A5)
- Modifying `rust-toolchain.toml` or `.gitattributes`
- Python worker code
- Database, config, or IPC implementations

## Approach

1. **Create workspace Cargo.toml** at `/home/dryw/AnvilML/Cargo.toml` with:
   - `workspace.package.version = "0.1.0"`
   - `workspace.members` listing `backend` and all 8 crate paths under `crates/`
   - `resolver = "2"`

2. **Create crate directories** — `mkdir -p` for each of the 8 crates plus their `src/` subdirectories.

3. **Create anvilml-core/Cargo.toml** — package name `anvilml-core`, no dependencies, no features.
   - Create `src/lib.rs` with `pub fn stub() {}`

4. **Create anvilml-hardware/Cargo.toml** — depends on `anvilml-core = { path = "../anvilml-core" }`, declares `[features] mock-hardware = []`.
   - Create `src/lib.rs` with `pub fn stub() {}`

5. **Create anvilml-registry/Cargo.toml** — depends on `anvilml-core = { path = "../anvilml-core" }`.
   - Create `src/lib.rs` with `pub fn stub() {}`

6. **Create anvilml-ipc/Cargo.toml** — depends on `anvilml-core = { path = "../anvilml-core" }`.
   - Create `src/lib.rs` with `pub fn stub() {}`

7. **Create anvilml-worker/Cargo.toml** — depends on `anvilml-ipc`, `anvilml-hardware`, `anvilml-core` (all path deps); forwards `mock-hardware = ["anvilml-hardware/mock-hardware"]`.
   - Create `src/lib.rs` with `pub fn stub() {}`

8. **Create anvilml-scheduler/Cargo.toml** — depends on `anvilml-worker`, `anvilml-registry`, `anvilml-core`; forwards `mock-hardware = ["anvilml-hardware/mock-hardware"]`.
   - Create `src/lib.rs` with `pub fn stub() {}`

9. **Create anvilml-server/Cargo.toml** — depends on `anvilml-worker`, `anvilml-scheduler`, `anvilml-registry`, `anvilml-ipc`, `anvilml-hardware`, `anvilml-core`; forwards `mock-hardware = ["anvilml-hardware/mock-hardware"]`.
   - Create `src/lib.rs` with `pub fn stub() {}`

10. **Create anvilml-openapi/Cargo.toml** — depends on `anvilml-core`, `anvilml-server`; `[package]` declares `[[bin]]` name = `anvilml-openapi`.
    - Create `src/main.rs` with `fn main() { println!("anvilml-openapi stub"); }`

11. **Verify build** — run `cargo build --workspace --features mock-hardware` and confirm exit code 0.

12. **Verify toolchain** — run `rustc --version` and confirm it reports 1.95.0.

## Files Affected

| Action   | Path                                         | Description                                              |
|----------|----------------------------------------------|----------------------------------------------------------|
| CREATE   | Cargo.toml                                   | Workspace root with members declaration                  |
| CREATE   | crates/anvilml-core/Cargo.toml               | Package stub, no dependencies                            |
| CREATE   | crates/anvilml-core/src/lib.rs               | Empty stub function                                      |
| CREATE   | crates/anvilml-hardware/Cargo.toml           | Depends on core, declares mock-hardware feature          |
| CREATE   | crates/anvilml-hardware/src/lib.rs           | Empty stub function                                      |
| CREATE   | crates/anvilml-registry/Cargo.toml           | Depends on core                                          |
| CREATE   | crates/anvilml-registry/src/lib.rs           | Empty stub function                                      |
| CREATE   | crates/anvilml-ipc/Cargo.toml                | Depends on core                                          |
| CREATE   | crates/anvilml-ipc/src/lib.rs                | Empty stub function                                      |
| CREATE   | crates/anvilml-worker/Cargo.toml             | Depends on ipc, hardware, core; forwards mock-hardware   |
| CREATE   | crates/anvilml-worker/src/lib.rs             | Empty stub function                                      |
| CREATE   | crates/anvilml-scheduler/Cargo.toml          | Depends on worker, registry, core; forwards mock-hardware|
| CREATE   | crates/anvilml-scheduler/src/lib.rs          | Empty stub function                                      |
| CREATE   | crates/anvilml-server/Cargo.toml             | Depends on all above; forwards mock-hardware             |
| CREATE   | crates/anvilml-server/src/lib.rs             | Empty stub function                                      |
| CREATE   | crates/anvilml-openapi/Cargo.toml            | Depends on core, server; binary crate                    |
| CREATE   | crates/anvilml-openapi/src/main.rs           | Stub main function                                       |
| CREATE   | Cargo.lock                                   | Generated by cargo build (binary app, committed)         |

## Tests

None. This task creates only empty stub crates with no logic to test. Unit tests will be added in later tasks as functionality is implemented.

## CI Impact

No CI changes required for this task. The CI workflow (`P1-A5`) is a separate task that will be addressed after the workspace compiles. The existing `.github/` directory from Phase 000 does not yet contain `ci.yml`; that file will be created in P1-A5.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation                                              |
|---------------------------|-----------|--------|---------------------------------------------------------|
| Cargo.toml path resolution fails due to typo in crate paths | Low | High | Use absolute path construction; verify with `cargo build` immediately after creation |
| Feature forwarding mismatch causes mock-hardware not to propagate | Low | Medium | Follow the exact forwarding pattern from ARCHITECTURE.md §5 for all crates that depend on anvilml-hardware |
| Cargo.lock conflicts with existing git state | Low | Low | Cargo.lock is newly created; ensure .gitignore does not exclude it (it currently doesn't — line 4 of .gitignore explicitly comments it is tracked) |
| Build fails due to missing workspace resolver | Low | Medium | Explicitly set `resolver = "2"` in workspace Cargo.toml |

## Acceptance Criteria

- [ ] Workspace `Cargo.toml` exists at repo root with all 9 members listed (backend + 8 crates)
- [ ] All 8 crate directories exist under `crates/` with both `Cargo.toml` and source stub
- [ ] `anvilml-openapi` has `src/main.rs` (not `src/lib.rs`)
- [ ] `anvilml-hardware/Cargo.toml` declares `[features] mock-hardware = []`
- [ ] Crates depending on `anvilml-hardware` forward the feature: worker, scheduler, server
- [ ] `cargo build --workspace --features mock-hardware` exits with code 0
- [ ] `rustc --version` reports 1.95.0
- [ ] `Cargo.lock` is generated and present at repo root
- [ ] `rust-toolchain.toml` and `.gitattributes` were NOT modified
