# Plan Report: P907-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P907-A1                                     |
| Phase       | 907 — ZeroMQ IPC Transport                  |
| Description | Add zeromq workspace dependency             |
| Depends on  | none                                        |
| Project     | anvilml                                     |
| Planned at  | 2026-06-13T08:15:00Z                        |
| Attempt     | 1                                           |

## Objective

Add the `zeromq` crate as a workspace dependency and register it in `anvilml-worker`,
removing the legacy `interprocess` dependency in preparation for the ZeroMQ IPC transport
replacement (Phase 907, Group A).

## Scope

### In Scope
- Add `zeromq = { version = "0.4", features = ["tokio"] }` to `[workspace.dependencies]` in root `Cargo.toml`
- Add `zeromq = { workspace = true }` to `[dependencies]` in `crates/anvilml-worker/Cargo.toml`
- Remove `interprocess = { version = "2.4", features = ["tokio"] }` from `crates/anvilml-worker/Cargo.toml`
- Bump `anvilml-worker` patch version from `0.1.22` to `0.1.23` in `crates/anvilml-worker/Cargo.toml`
- Verify `cargo check --workspace --features mock-hardware` exits 0
- Verify `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0

### Out of Scope
- Any source code changes in `anvilml-worker/src/` (handled by subsequent tasks P907-A2–A8)
- Changes to `anvilml-ipc` crate
- Changes to any other crate
- Python-side `pyzmq` dependency (handled by P907-A5)
- Removal of `framing.rs` or IPC message type changes (handled by P907-A8)

## Approach

1. Open root `Cargo.toml` and append `zeromq = { version = "0.4", features = ["tokio"] }` to the `[workspace.dependencies]` section, placed alphabetically among the existing entries (between `uuid` and `walkdir`).
2. Open `crates/anvilml-worker/Cargo.toml`:
   a. Add `zeromq = { workspace = true }` to `[dependencies]`, placed alphabetically between `anvilml-ipc` and `rmp-serde`.
   b. Remove the line `interprocess = { version = "2.4", features = ["tokio"] }` from `[dependencies]`.
   c. Bump `[package] version` from `"0.1.22"` to `"0.1.23"`.
3. Run `cargo check --workspace --features mock-hardware` to verify compilation on the native Linux target.
4. Run `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` to verify cross-compilation to Windows.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `Cargo.toml` | Add `zeromq` to `[workspace.dependencies]` |
| Modify | `crates/anvilml-worker/Cargo.toml` | Add `zeromq = { workspace = true }`, remove `interprocess`, bump patch version `0.1.22 → 0.1.23` |

## Tests

None. This task modifies only Cargo.toml manifests (dependency declarations). No source code
or test files are written or changed. The acceptance criteria are compile-check commands,
not test runs.

## CI Impact

No CI workflow files are modified. The existing CI gates (format, clippy, tests, cross-checks)
already cover `--features mock-hardware` builds, so the new dependency will be exercised
through the standard CI pipeline once this task lands. No new CI jobs or steps are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `zeromq` 0.4 with `tokio` feature has incompatible transitive dependencies with existing workspace crates | Low | High | Verified by `cargo check --workspace`; if failure, adjust version or feature set |
| `interprocess` is still transitively referenced by another crate in the workspace | Low | Medium | If `cargo check` fails with unresolved `interprocess` imports, identify the other consumer and coordinate with its task |
| `zeromq` 0.4 API changes between minor releases | Low | Low | Task pins `version = "0.4"` (semver-compatible range); if API is incompatible with downstream code, subsequent tasks will adapt |
| Windows cross-compile fails due to zeromq FFI dependencies | Low | Medium | The `zeromq` crate is pure Rust; cross-check will confirm. If it fails, add a `target.'cfg(windows)'.dependencies` block or file a blocker |

## Acceptance Criteria

- [ ] `zeromq = { version = "0.4", features = ["tokio"] }` present in `[workspace.dependencies]` of root `Cargo.toml`
- [ ] `zeromq = { workspace = true }` present in `[dependencies]` of `crates/anvilml-worker/Cargo.toml`
- [ ] `interprocess` line removed from `crates/anvilml-worker/Cargo.toml`
- [ ] `anvilml-worker` version bumped to `0.1.23`
- [ ] `cargo check --workspace --features mock-hardware` exits 0
- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0
