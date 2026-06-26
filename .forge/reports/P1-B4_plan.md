# Plan Report: P1-B4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-B4                                       |
| Phase       | 001 — Repository Scaffold                   |
| Description | anvilml-ipc, anvilml-worker: empty crate stubs |
| Depends on  | P1-B1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-26T12:55:00Z                        |
| Attempt     | 1                                           |

## Objective

Create two empty crate stubs (`anvilml-ipc` and `anvilml-worker`) in correct dependency order, add them to the workspace members, and establish the `mock-hardware` feature-forwarding pattern at the first crate where it applies (`anvilml-worker`). After this task, `cargo build -p anvilml-ipc -p anvilml-worker --features mock-hardware` exits 0.

## Scope

### In Scope
- Create `crates/anvilml-ipc/Cargo.toml` — depends on `anvilml-core` (path), workspace-inherited version/edition/rust-version, no external crates.
- Create `crates/anvilml-ipc/src/lib.rs` — crate-level `//!` doc comment only ("ZeroMQ ROUTER transport + message types. No process management."), no submodules, no implementation code.
- Create `crates/anvilml-worker/Cargo.toml` — depends on `anvilml-ipc`, `anvilml-hardware`, `anvilml-core` (all path), workspace-inherited version/edition/rust-version, `[features] mock-hardware = ["anvilml-hardware/mock-hardware"]`.
- Create `crates/anvilml-worker/src/lib.rs` — crate-level `//!` doc comment only ("Spawns/supervises Python worker subprocesses."), no submodules, no implementation code.
- Modify root `Cargo.toml` — add `"crates/anvilml-ipc"` and `"crates/anvilml-worker"` to `members`.

### Out of Scope
None. `defers_to (from JSON): []` — this task implements its full scope with no deferrals.

## Existing Codebase Assessment

No prior source exists for `anvilml-ipc` or `anvilml-worker` — neither directory exists under `crates/`. The workspace already contains five stub crates (`anvilml-core`, `anvilml-hardware`, `anvilml-registry`, `anvilml-artifacts`) following a consistent pattern: each has a minimal `Cargo.toml` with workspace-inherited `version`, `edition`, and `rust-version`, path dependencies on earlier crates in the graph, and a `src/lib.rs` containing only a `//!` crate-level doc comment (1–2 lines). The `mock-hardware` feature is declared on `anvilml-hardware` as `mock-hardware = []` — an empty feature that exists purely for later crates to forward. This task follows the exact same pattern, adding the dependency edges `anvilml-ipc → anvilml-core` and `anvilml-worker → {anvilml-ipc, anvilml-hardware, anvilml-core}`.

## Resolved Dependencies

None. Both crates use only path dependencies on existing workspace crates (`anvilml-core`, `anvilml-hardware`, `anvilml-ipc`). No external crates or version pins are introduced.

## Approach

1. Create `crates/anvilml-ipc/src/` directory.
2. Create `crates/anvilml-ipc/Cargo.toml` with:
   - `[package]` section: `name = "anvilml-ipc"`, `version.workspace = true`, `edition.workspace = true`, `rust-version.workspace = true`
   - `[dependencies]` section: `anvilml-core = { path = "../anvilml-core" }`
   - No features section (ipc does not forward `mock-hardware`; only crates that transitively depend on `anvilml-hardware` do).
3. Create `crates/anvilml-ipc/src/lib.rs` with exactly one crate-level doc comment: `//! ZeroMQ ROUTER transport + message types. No process management.` — no `pub mod`, no `pub use`, no implementation code.
4. Create `crates/anvilml-worker/src/` directory.
5. Create `crates/anvilml-worker/Cargo.toml` with:
   - `[package]` section: `name = "anvilml-worker"`, `version.workspace = true`, `edition.workspace = true`, `rust-version.workspace = true`
   - `[dependencies]` section: `anvilml-ipc = { path = "../anvilml-ipc" }`, `anvilml-hardware = { path = "../anvilml-hardware" }`, `anvilml-core = { path = "../anvilml-core" }`
   - `[features]` section: `mock-hardware = ["anvilml-hardware/mock-hardware"]` — this is the first crate that forwards the feature, establishing the pattern repeated by all downstream crates (`anvilml-scheduler`, `anvilml-server`, `backend`) per `ARCHITECTURE.md §5`.
6. Create `crates/anvilml-worker/src/lib.rs` with exactly one crate-level doc comment: `//! Spawns/supervises Python worker subprocesses.` — no submodules, no implementation code.
7. Modify root `Cargo.toml` — append `"crates/anvilml-ipc"` and `"crates/anvilml-worker"` to the `members` array.

## Public API Surface

None. Both `lib.rs` files contain only a crate-level doc comment. No `pub` items are declared.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | crates/anvilml-ipc/Cargo.toml | IPC crate manifest; depends on anvilml-core |
| CREATE | crates/anvilml-ipc/src/lib.rs | IPC crate stub; doc comment only |
| CREATE | crates/anvilml-worker/Cargo.toml | Worker crate manifest; depends on ipc, hardware, core; forwards mock-hardware |
| CREATE | crates/anvilml-worker/src/lib.rs | Worker crate stub; doc comment only |
| MODIFY | Cargo.toml | Add both new crate paths to workspace members |

## Tests

None. This task creates only empty crate stubs with no functions, types, or logic. The acceptance criterion is a successful `cargo build`, which implicitly verifies that the dependency graph and feature forwarding compile correctly.

## CI Impact

No CI changes required. The new crates are added to the workspace members; when the full workspace test suite runs (via `cargo test --workspace --features mock-hardware` in later tasks), both crates will be compiled as part of the workspace build. No new CI job or gate is introduced.

## Platform Considerations

None identified. Both crate stubs are platform-neutral — no `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The `mock-hardware` feature forwarding is a pure Cargo feature flag with no platform-specific code. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Root `Cargo.toml` `members` array formatting — adding two entries could introduce trailing commas or duplicate entries if the workspace already has pending changes from concurrent tasks. | Low | Medium | Read the current `members` array before editing; append entries with proper comma separation. The workspace currently lists 5 members; this task adds 2 more. |
| `anvilml-hardware/mock-hardware` feature does not exist yet, causing the forward reference in `anvilml-worker/Cargo.toml` to fail at parse time. | Low | High | The feature was declared in P1-B2 (`anvilml-hardware/Cargo.toml` line 10: `mock-hardware = []`), which is a prerequisite of P1-B1, which is a prerequisite of P1-B4. Verified present in the existing file. |
| Feature forwarding syntax error — writing `mock-hardware = ["anvilml-hardware/mock-hardware"]` with incorrect quoting or missing `anvilml-hardware` dependency would cause a Cargo parse error. | Low | Medium | Follow the exact pattern from `anvilml-hardware/Cargo.toml`'s own feature declaration. The syntax is `feature_name = ["dep-crate/feature-name"]` where the dep-crate is a path dependency. |

## Acceptance Criteria

- [ ] `cargo build -p anvilml-ipc -p anvilml-worker --features mock-hardware` exits 0
