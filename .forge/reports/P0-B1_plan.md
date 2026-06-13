# Plan Report: P0-B1

| Field       | Value                           |
|-------------|---------------------------------|
| Task ID     | P0-B1                           |
| Phase       | 000 — Repository Preamble       |
| Description | Create workspace Cargo.toml + all 9 crate skeletons |
| Depends on  | P0-A1                           |
| Project     | anvilml                         |
| Planned at  | 2026-06-14T00:50:00Z            |
| Attempt     | 1                               |

## Objective

Create the root `Cargo.toml` declaring the AnvilML Cargo workspace with `resolver = "2"`, `[workspace.package] version = "0.1.0"`, and a `[workspace.dependencies]` block pre-populated with all major dependencies. Create stub `Cargo.toml` and source files for all 9 crates: `backend` (binary), `anvilml-core`, `anvilml-hardware`, `anvilml-registry`, `anvilml-ipc`, `anvilml-worker`, `anvilml-scheduler`, `anvilml-server`, and `anvilml-openapi` (binary). The `anvilml-hardware` crate declares the `mock-hardware` feature; all crates that transitively depend on it forward it. The observable state when complete is that `cargo build --workspace --features mock-hardware` exits 0 with zero warnings, confirming the workspace compiles cleanly before any phase 001+ code is written.

## Scope

### In Scope
- **Root `Cargo.toml`** — `[workspace]` with `members = ["backend", "crates/anvilml-core", "crates/anvilml-hardware", "crates/anvilml-registry", "crates/anvilml-ipc", "crates/anvilml-worker", "crates/anvilml-scheduler", "crates/anvilml-server", "crates/anvilml-openapi"]`, `resolver = "2"`, `[workspace.package] version = "0.1.0"`, and `[workspace.dependencies]` block with `serde`, `serde_json`, `tokio`, `axum`, `tracing`, `zeromq`, `rmp-serde`, `sqlx`, `uuid`, `thiserror`, `tower-http`.
- **`backend/Cargo.toml`** — package name `anvilml`, binary target, depends on workspace crates (server, registry, etc. as path deps) and workspace dependencies.
- **`backend/src/main.rs`** — stub `fn main() {}`.
- **`crates/anvilml-core/Cargo.toml`** + `src/lib.rs` — stub `pub fn stub() {}`. Depends on workspace `serde`, `serde_json`, `uuid`.
- **`crates/anvilml-hardware/Cargo.toml`** + `src/lib.rs` — stub `pub fn stub() {}`. Declares `[features] mock-hardware = []`. Depends on workspace `anvilml-core` and `tracing`.
- **`crates/anvilml-registry/Cargo.toml`** + `src/lib.rs` — stub `pub fn stub() {}`. Depends on workspace `anvilml-core`, `sqlx`, `uuid`, `tracing`.
- **`crates/anvilml-ipc/Cargo.toml`** + `src/lib.rs` — stub `pub fn stub() {}`. Depends on workspace `anvilml-core`, `zeromq`, `rmp-serde`, `tokio`, `tracing`.
- **`crates/anvilml-worker/Cargo.toml`** + `src/lib.rs` — stub `pub fn stub() {}`. Declares `[features] mock-hardware = ["anvilml-hardware/mock-hardware"]`. Depends on workspace `anvilml-core`, `anvilml-hardware`, `anvilml-ipc`, `tokio`, `tracing`.
- **`crates/anvilml-scheduler/Cargo.toml`** + `src/lib.rs` — stub `pub fn stub() {}`. Declares `[features] mock-hardware = ["anvilml-hardware/mock-hardware"]`. Depends on workspace `anvilml-core`, `anvilml-registry`, `anvilml-worker`, `tokio`, `tracing`.
- **`crates/anvilml-server/Cargo.toml`** + `src/lib.rs` — stub `pub fn stub() {}`. Declares `[features] mock-hardware = ["anvilml-hardware/mock-hardware"]`. Depends on workspace `anvilml-core`, `anvilml-hardware`, `anvilml-ipc`, `anvilml-scheduler`, `anvilml-worker`, `axum`, `tower-http`, `tracing`.
- **`crates/anvilml-openapi/Cargo.toml`** + `src/main.rs` — stub `fn main() {}`. Depends on workspace `anvilml-core`, `anvilml-server`, `axum`, `tracing`.

### Out of Scope
- Any implementation code beyond stub functions.
- Migration files, seed files, or SQL.
- Python worker code.
- `.github/workflows/ci.yml` (covered by P0-C1).
- `.forge/` directory layout (covered by P0-D1).
- `anvilml.toml` config file (not required by this task; it is a reference config that will be created when config structs are implemented).

## Existing Codebase Assessment

No prior source exists. This task establishes the baseline Cargo workspace structure for subsequent phases. The repository currently contains only `.forge/`, `.git/`, `.gitattributes`, `.gitignore`, `LICENSE`, `README.md`, `docs/`, and `rust-toolchain.toml` — all created by P0-A1. There are no existing `Cargo.toml`, `backend/`, or `crates/` directories. This task creates the entire Rust crate skeleton from scratch, establishing the file layout, naming conventions, dependency graph, and feature-forwarding patterns that every subsequent task will follow.

## Resolved Dependencies

All versions resolved via crates.io API (webfetch) at 2026-06-14. The MCP `rust-docs` server was unavailable (rate-limited by crates.io), so direct API queries were used as the verification source.

| Type   | Name        | Version verified | MCP source       | Feature flags confirmed          |
|--------|-------------|-----------------|------------------|----------------------------------|
| crate  | serde       | 1.0.228         | crates.io API    | default, derive                  |
| crate  | serde_json  | 1.0.150         | crates.io API    | default                          |
| crate  | tokio       | 1.52.3          | crates.io API    | full (default)                   |
| crate  | axum        | 0.8.9           | crates.io API    | default (json, http1, tokio, ws) |
| crate  | tracing     | 0.1.44          | crates.io API    | default (std, attributes)        |
| crate  | zeromq      | 0.6.0           | crates.io API    | tokio-runtime, tcp-transport     |
| crate  | rmp-serde   | 1.3.1           | crates.io API    | (none — no optional features)    |
| crate  | sqlx        | 0.9.0           | crates.io API    | runtime-tokio, sqlite, json      |
| crate  | uuid        | 1.23.3          | crates.io API    | serde, v4                        |
| crate  | thiserror   | 2.0.18          | crates.io API    | default                          |
| crate  | tower-http  | 0.6.11          | crates.io API    | cors, trace, timeout             |

Note: `zeromq` 0.6.0 is the current stable version (confirmed by design doc §8.1 which specifies `zeromq = "0.6"`). The API types `RouterSocket` and `DealerSocket` exist in zeromq 0.6.x — confirmed by the crate's documentation and the `tokio-runtime` feature which enables `tokio`-compatible async sockets.

## Approach

1. **Create `crates/` directory.** This is the parent for all library crates.

2. **Create root `Cargo.toml`.** Write a workspace manifest with:
   - `[workspace]` with `members` listing all 9 crates using the paths from ARCHITECTURE.md §2.
   - `resolver = "2"` for edition-2021 crate resolution.
   - `[workspace.package] version = "0.1.0"` — this is the workspace release version, read-only per FORGE_AGENT_RULES §14.
   - `[workspace.dependencies]` block with all 11 dependencies at their resolved versions.
   - Rationale: Pre-declaring dependencies here means subsequent tasks can use `{ workspace = true }` without modifying the workspace manifest, preventing version divergence.

3. **Create `backend/Cargo.toml`.** Package `anvilml` (binary). Depends on `anvilml-server`, `anvilml-registry`, `anvilml-hardware`, `anvilml-core`, `anvilml-scheduler`, `anvilml-worker` as workspace path deps. Also depends on `clap` (CLI parsing, not in workspace deps — declared inline since it's only used by backend). Include `[features] mock-hardware = ["anvilml-hardware/mock-hardware"]`. Rationale: `backend` is the final binary and must forward `mock-hardware` to all dependents that transitively need it.

4. **Create `backend/src/main.rs`.** Stub with `fn main() {}`. No imports, no logic.

5. **Create each library crate directory** (`crates/anvilml-{core,hardware,registry,ipc,worker,scheduler,server}/`) with `Cargo.toml` and `src/lib.rs`. For each:
   - `Cargo.toml`: package name matches crate name, version inherits from workspace via `workspace = true`, depends on workspace path deps and workspace dependencies.
   - `src/lib.rs`: crate-level `//!` doc comment describing the crate's ownership and hard constraints, followed by `pub fn stub() {}`.
   - Rationale: The `//!` doc comment establishes the crate-level documentation pattern that all subsequent tasks must follow (FORGE_AGENT_RULES §12.3).

6. **Create `crates/anvilml-openapi/Cargo.toml` and `src/main.rs`.** Package `anvilml-openapi` with `[[bin]]` section. Depends on `anvilml-server`, `anvilml-core`, `axum`, `tracing`. Rationale: This is a binary crate, not a library, so it uses `src/main.rs` and declares `[[bin]]`.

7. **Feature forwarding setup.** The following crates declare `[features] mock-hardware`:
   - `anvilml-hardware`: `mock-hardware = []` (declares the feature)
   - `anvilml-worker`: `mock-hardware = ["anvilml-hardware/mock-hardware"]` (forwards)
   - `anvilml-scheduler`: `mock-hardware = ["anvilml-hardware/mock-hardware"]` (forwards)
   - `anvilml-server`: `mock-hardware = ["anvilml-hardware/mock-hardware"]` (forwards)
   - `backend`: `mock-hardware = ["anvilml-hardware/mock-hardware"]` (forwards)
   - Rationale: Per ARCHITECTURE.md §5, every crate that depends on `anvilml-hardware` must forward the feature. `anvilml-registry` does not depend on `anvilml-hardware`, so it does not forward it.

8. **Verify with `cargo build --workspace --features mock-hardware`.** The build must exit 0 with zero warnings. Since all source files are stubs containing only `fn main() {}` or `pub fn stub() {}`, and `stub()` is `pub`, it will generate a `dead_code` warning. To prevent this, each `stub()` function will be annotated with `#[allow(dead_code)]`. Rationale: This is a skeleton task — the stubs are not yet called. Suppressing the warning is minimal and correct; the ACT agent will replace stubs with real code in subsequent phases.

## Public API Surface

None. All public items are stubs (`pub fn stub() {}`) that will be replaced by real implementations in subsequent phases. The `stub()` functions are annotated with `#[allow(dead_code)]` to prevent compiler warnings during this skeleton phase.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | Cargo.toml | Workspace root: members, resolver, workspace.package, workspace.dependencies |
| CREATE | backend/Cargo.toml | Backend binary crate manifest |
| CREATE | backend/src/main.rs | Stub `fn main() {}` |
| CREATE | crates/anvilml-core/Cargo.toml | Core domain types crate manifest |
| CREATE | crates/anvilml-core/src/lib.rs | Stub with `//!` doc comment and `pub fn stub()` |
| CREATE | crates/anvilml-hardware/Cargo.toml | Hardware detection crate manifest with mock-hardware feature |
| CREATE | crates/anvilml-hardware/src/lib.rs | Stub with `//!` doc comment and `pub fn stub()` |
| CREATE | crates/anvilml-registry/Cargo.toml | Model registry crate manifest |
| CREATE | crates/anvilml-registry/src/lib.rs | Stub with `//!` doc comment and `pub fn stub()` |
| CREATE | crates/anvilml-ipc/Cargo.toml | IPC transport crate manifest |
| CREATE | crates/anvilml-ipc/src/lib.rs | Stub with `//!` doc comment and `pub fn stub()` |
| CREATE | crates/anvilml-worker/Cargo.toml | Worker pool crate manifest with mock-hardware forwarding |
| CREATE | crates/anvilml-worker/src/lib.rs | Stub with `//!` doc comment and `pub fn stub()` |
| CREATE | crates/anvilml-scheduler/Cargo.toml | Job scheduler crate manifest with mock-hardware forwarding |
| CREATE | crates/anvilml-scheduler/src/lib.rs | Stub with `//!` doc comment and `pub fn stub()` |
| CREATE | crates/anvilml-server/Cargo.toml | HTTP server crate manifest with mock-hardware forwarding |
| CREATE | crates/anvilml-server/src/lib.rs | Stub with `//!` doc comment and `pub fn stub()` |
| CREATE | crates/anvilml-openapi/Cargo.toml | OpenAPI generator binary crate manifest |
| CREATE | crates/anvilml-openapi/src/main.rs | Stub `fn main() {}` |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| (none — build-only) | workspace-build | Workspace compiles with mock-hardware feature, zero warnings | `cargo build --workspace --features mock-hardware` exits 0 |
| (none — build-only) | clippy-clean | No clippy warnings in any crate | `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 |

Note: No unit test files are created in this phase because the stub functions contain no logic to test. The acceptance criterion is a clean build and clippy check, which serves as the test for this skeleton task.

## CI Impact

No CI changes required. The GitHub Actions workflow stubs are created by P0-C1, not this task. Once P0-C1 completes, the existing `rust-linux` and `rust-windows` CI jobs will automatically pick up the new workspace members and run `cargo build`, `cargo clippy`, and `cargo test` on all crates. The `config-drift` job references `backend/Cargo.toml` which this task creates (stub), so it will work once the config structs are implemented in a later phase.

## Platform Considerations

None identified. The workspace root `Cargo.toml` and crate skeletons are platform-neutral. The `#[cfg(unix)]` / `#[cfg(windows)]` guards will be introduced by subsequent phases that implement platform-specific hardware detection. The Windows cross-check in ENVIRONMENT.md §7 (`cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu`) will pass because all stub files contain no platform-specific code.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `#[allow(dead_code)]` on stub functions may mask legitimate dead-code issues in later phases if not removed when stubs are replaced. | Low | Low | The ACT agent for the next task that modifies a crate will replace `stub()` with real code, making the `#[allow(dead_code)]` obsolete. Document this in the plan's Approach so the ACT agent knows to remove it. |
| `sqlx` 0.9.0 requires `rust_version = "1.94.0"` per crates.io data. If the pinned stable toolchain is older, the build will fail at the dependency resolution stage. | Low | High | The `rust-toolchain.toml` (created by P0-A1) sets `channel = "stable"`. The ACT agent must verify that `rustc --version` ≥ 1.94.0 before building. If not, the toolchain must be updated. |
| `zeromq` 0.6.0 requires `tokio-runtime` feature for `RouterSocket`. If the feature flag is wrong, the type won't resolve in later phases. | Low | Medium | The workspace dependencies declare `zeromq = { version = "0.6.0", features = ["tokio-runtime", "tcp-transport"] }`. This is confirmed against the crates.io feature list. The ACT agent will verify `RouterSocket` exists at build time. |
| `thiserror` 2.0.x has a different derive macro name than 1.x. If the plan references `thiserror` 1.x API shapes, later phases will fail to compile. | Low | High | The resolved version is `thiserror = "2.0.18"`. All subsequent tasks must use `#[derive(thiserror::Error)]` (same in both versions). The `thiserror` 2.x API is compatible at the derive level — no breaking changes for the `Error` derive. |

## Acceptance Criteria

- [ ] `cargo build --workspace --features mock-hardware` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0 (Windows cross-check)
- [ ] All 9 crate directories exist: `backend/`, `crates/anvilml-core/`, `crates/anvilml-hardware/`, `crates/anvilml-registry/`, `crates/anvilml-ipc/`, `crates/anvilml-worker/`, `crates/anvilml-scheduler/`, `crates/anvilml-server/`, `crates/anvilml-openapi/`
- [ ] Root `Cargo.toml` contains `[workspace]` with all 9 members listed
- [ ] `anvilml-hardware/Cargo.toml` declares `mock-hardware = []` feature
- [ ] `anvilml-worker/Cargo.toml`, `anvilml-scheduler/Cargo.toml`, `anvilml-server/Cargo.toml`, and `backend/Cargo.toml` each declare `mock-hardware = ["anvilml-hardware/mock-hardware"]`
- [ ] `anvilml-openapi/Cargo.toml` declares `[[bin]]` section for the binary target
