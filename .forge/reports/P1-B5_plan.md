# Plan Report: P1-B5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-B5                                       |
| Phase       | 1 — Repository Scaffold                     |
| Description | anvilml-scheduler, anvilml-server: empty crate stubs |
| Depends on  | P1-B3, P1-B4                                |
| Project     | anvilml                                     |
| Planned at  | 2026-06-26T14:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Create two empty library crate stubs (`anvilml-scheduler` and `anvilml-server`) at the top of the dependency graph, wire them into the workspace and backend dependencies, and establish the feature-forwarding chain for `mock-hardware` all the way from `anvilml-hardware` to `backend`. This completes the crate skeleton so that `cargo build --workspace --features mock-hardware` succeeds end-to-end.

## Scope

### In Scope
- Create `crates/anvilml-scheduler/Cargo.toml` with path dependencies on `anvilml-worker`, `anvilml-registry`, `anvilml-artifacts`, `anvilml-core`, and `[features] mock-hardware = ["anvilml-hardware/mock-hardware"]`.
- Create `crates/anvilml-scheduler/src/lib.rs` with a single `//!` crate-level doc comment (≤80 lines, no submodules).
- Create `crates/anvilml-server/Cargo.toml` with path dependencies on `anvilml-worker`, `anvilml-registry`, `anvilml-artifacts`, `anvilml-core`, `anvilml-ipc`, and `axum` (version to be resolved live via MCP at ACT time), plus `[features] mock-hardware = ["anvilml-hardware/mock-hardware"]`.
- Create `crates/anvilml-server/src/lib.rs` with a single `//!` crate-level doc comment (≤80 lines, no submodules).
- Add `"crates/anvilml-scheduler"` and `"crates/anvilml-server"` to root `Cargo.toml` `members`.
- Add path dependencies on `anvilml-scheduler` and `anvilml-server` to `backend/Cargo.toml`.
- Acceptance: `cargo build --workspace --features mock-hardware` exits 0.

### Out of Scope

defers_to (from JSON): []

None. This task has an empty `defers_to` field and must implement its full scope without deferring any functionality. The stub crates are intentionally minimal — no implementation code, no submodules, no tests (the crates are empty).

## Existing Codebase Assessment

No prior source exists for `anvilml-scheduler` or `anvilml-server` — these directories are not yet present under `crates/`. The existing crate stubs (`anvilml-core`, `anvilml-hardware`, `anvilml-registry`, `anvilml-artifacts`, `anvilml-ipc`, `anvilml-worker`) follow a consistent pattern:

- **Cargo.toml**: `[package]` with `version.workspace = true`, `edition.workspace = true`, `rust-version.workspace = true`; dependencies listed as `{ path = "../<crate>" }` entries; `[features]` section with `mock-hardware = ["anvilml-hardware/mock-hardware"]` on crates that forward the flag.
- **lib.rs**: Exactly one line — a `//!` crate-level doc comment describing the crate's responsibility, ≤80 lines.
- **No submodules, no test directories, no implementation code** at this phase.

The root `Cargo.toml` currently lists 7 members (backend + 6 crates). `backend/Cargo.toml` currently depends on `clap` and `tokio` only, with a no-op `mock-hardware = []` feature (since it does not depend on `anvilml-hardware`).

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | axum    | Not resolved (MCP unavailable) | N/A (fallback) | n/a |

**Note:** The `rust-docs` MCP tool is configured in `opencode.json` but is not available as a callable tool in this session. Per FORGE_AGENT_RULES.md §6.4, the fallback is to use the most recent version from the project's lockfile — but `axum` is not yet in `Cargo.lock` (it is a new dependency introduced by this task). The ACT agent MUST resolve the current stable version of `axum` via the `rust-docs` MCP tool (or crates.io registry) before writing `Cargo.toml`. A reasonable starting version for Rust 2024 edition + tokio 1.x is `0.8` (or the latest stable at ACT time). The exact version must be confirmed at ACT time.

## Approach

1. **Create `crates/anvilml-scheduler/` directory structure.**
   - Create `crates/anvilml-scheduler/src/lib.rs` containing only:
     ```rust
     //! Job queue, VRAM ledger, DAG validation, and dispatch loop.
     ```
     This is a single-line crate doc comment (well under 80 lines). No submodules, no implementation code.

2. **Create `crates/anvilml-scheduler/Cargo.toml`.**
   - Package name: `anvilml-scheduler`, version/edition/rust-version inherited from workspace.
   - Dependencies (all path-based, matching the dependency graph in `ARCHITECTURE.md §3`):
     - `anvilml-worker = { path = "../anvilml-worker" }`
     - `anvilml-registry = { path = "../anvilml-registry" }`
     - `anvilml-artifacts = { path = "../anvilml-artifacts" }`
     - `anvilml-core = { path = "../anvilml-core" }`
   - Feature forwarding: `[features] mock-hardware = ["anvilml-hardware/mock-hardware"]`
     This forwards the flag through to `anvilml-hardware`, matching the established pattern from `anvilml-worker/Cargo.toml`.

3. **Create `crates/anvilml-server/` directory structure.**
   - Create `crates/anvilml-server/src/lib.rs` containing only:
     ```rust
     //! axum HTTP/WS server, all handlers.
     ```
     Single-line crate doc comment (well under 80 lines). No submodules, no implementation code.

4. **Create `crates/anvilml-server/Cargo.toml`.**
   - Package name: `anvilml-server`, version/edition/rust-version inherited from workspace.
   - Dependencies:
     - `anvilml-worker = { path = "../anvilml-worker" }`
     - `anvilml-registry = { path = "../anvilml-registry" }`
     - `anvilml-artifacts = { path = "../anvilml-artifacts" }`
     - `anvilml-core = { path = "../anvilml-core" }`
     - `anvilml-ipc = { path = "../anvilml-ipc" }`
     - `axum = "<resolved_version>"` — version to be confirmed via MCP at ACT time.
   - Feature forwarding: `[features] mock-hardware = ["anvilml-hardware/mock-hardware"]`

5. **Update root `Cargo.toml`** — append `"crates/anvilml-scheduler"` and `"crates/anvilml-server"` to the `members` array.

6. **Update `backend/Cargo.toml`** — add path dependencies:
   - `anvilml-scheduler = { path = "../crates/anvilml-scheduler" }`
   - `anvilml-server = { path = "../crates/anvilml-server" }`
   - Update the `mock-hardware` feature from a no-op `[]` to forward through the actual crate dependency: `mock-hardware = ["anvilml-scheduler/mock-hardware"]` (which chains through to `anvilml-hardware/mock-hardware`).

   **Rationale:** Previously, `backend`'s `mock-hardware` feature was a no-op because `backend` did not depend on any crate that depended on `anvilml-hardware`. Now that `backend` depends on `anvilml-scheduler` (which depends transitively on `anvilml-hardware`), the feature can be meaningfully forwarded. This is a minimal, non-breaking change — it upgrades the feature from a no-op to a real forwarding path without changing any behavior at this phase (all stubs compile to nothing).

## Public API Surface

None. Both crates contain only a crate-level doc comment in `lib.rs` — no `pub mod`, `pub use`, `pub fn`, `pub struct`, or any other public item.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/Cargo.toml` | New crate manifest with path deps on worker/registry/artifacts/core, mock-hardware forwarding |
| CREATE | `crates/anvilml-scheduler/src/lib.rs` | Crate doc comment only |
| CREATE | `crates/anvilml-server/Cargo.toml` | New crate manifest with path deps on worker/registry/artifacts/core/ipc/axum, mock-hardware forwarding |
| CREATE | `crates/anvilml-server/src/lib.rs` | Crate doc comment only |
| MODIFY | `Cargo.toml` (root) | Add both new crate paths to `members` |
| MODIFY | `backend/Cargo.toml` | Add path deps on scheduler and server; upgrade mock-hardware feature from no-op to real forwarding |

## Tests

None. Both crates are empty stubs containing only a doc comment. No source code is written, so no tests are needed. The acceptance criterion is the workspace-level build command.

## CI Impact

No CI changes required. The new crates are part of the workspace, so `cargo test --workspace --features mock-hardware` (already defined in CI) will automatically include them. Since they have no test modules, this adds zero test overhead.

## Platform Considerations

None identified. The crates are platform-neutral — no `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The `axum` dependency is cross-platform by design. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `axum` version not resolvable via MCP — the `rust-docs` tool is configured but not exposed as a callable tool in this session. The ACT agent may need to determine the version independently. | Medium | High | The plan notes the fallback: use the latest stable version available on crates.io at ACT time. For Rust 2024 + tokio 1.x, `axum = "0.8"` or `axum = "0.7"` are reasonable candidates. The ACT agent should verify compatibility before pinning. |
| `backend/Cargo.toml` `mock-hardware` feature upgrade changes behavior — the no-op `[]` becomes a real forwarding path. If any crate in the chain does not properly declare the feature, the build will fail with an unknown-feature error. | Low | Medium | The forwarding path is `backend → anvilml-scheduler → anvilml-worker → anvilml-hardware`. All intermediate crates (`anvilml-worker`, `anvilml-ipc`) already declare `mock-hardware` as a no-op or forwarding feature in this phase. The ACT agent should verify the full chain at build time. |
| Root `Cargo.toml` `members` array grows and becomes hard to read. | Low | Low | The members array is short (9 entries after this task). No action needed. |

## Acceptance Criteria

- [ ] `cargo build --workspace --features mock-hardware` exits 0
