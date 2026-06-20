# Plan Report: P902-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P902-A2                                           |
| Phase       | 902 — ArtifactStore Relocation Retrofit           |
| Description | anvilml-ipc: remove ArtifactStore and dead deps   |
| Depends on  | P902-A1                                           |
| Project     | anvilml                                           |
| Planned at  | 2026-06-20T18:10:00Z                              |
| Attempt     | 1                                                 |

## Objective

Remove the relocated `ArtifactStore` module and its four now-dead dependencies (`chrono`, `sha2`, `sqlx`, `base64`) from the `anvilml-ipc` crate, restoring the crate's documented scope (ZeroMQ transport + message types + event broadcasting, zero business logic). After this task, `cargo check -p anvilml-ipc` and `cargo tree -p anvilml-ipc` must confirm none of the four removed crates appear in the dependency graph.

## Scope

### In Scope
- Delete `crates/anvilml-ipc/src/artifact_store.rs` entirely.
- Remove `pub mod artifact_store;` and `pub use artifact_store::ArtifactStore;` from `crates/anvilml-ipc/src/lib.rs`.
- Remove `base64 = "0.22"` from `crates/anvilml-ipc/Cargo.toml`.
- Remove `chrono = { workspace = true }` from `crates/anvilml-ipc/Cargo.toml`.
- Remove `sha2 = "0.10"` from `crates/anvilml-ipc/Cargo.toml`.
- Remove `sqlx = { workspace = true }` from `crates/anvilml-ipc/Cargo.toml`.
- Bump `anvilml-ipc` patch version from `0.1.8` to `0.1.9` in `crates/anvilml-ipc/Cargo.toml`.
- Verify the lib.rs doc comment "does not contain process management or business logic" is still accurate.

### Out of Scope
- `crates/anvilml-ipc/src/ws/` (EventBroadcaster) — out of scope per task spec; its cycle rationale is real.
- Any other crate's imports or dependencies — those are handled by P902-A3 and P902-A4.
- `anvilml-artifacts` crate creation — handled by P902-A1.
- Documentation files (`ARCHITECTURE.md`, `ANVILML_DESIGN.md`, `README.md`) — already corrected by manual commit `5cb6a8b`.

## Existing Codebase Assessment

The `anvilml-ipc` crate currently contains five modules: `artifact_store`, `error`, `messages`, `transport`, and `ws`. The `artifact_store.rs` module (296 lines) was relocated here during P15-A2 under a false dependency-cycle rationale. It is the only module in this crate that performs I/O (filesystem writes, SQLite queries) or business logic — all other modules are purely about ZeroMQ routing, message serialization, and event broadcasting.

The four dependencies targeted for removal (`chrono`, `sha2`, `sqlx`, `base64`) are used exclusively within `artifact_store.rs`:
- `chrono::Utc` — only in `artifact_store.rs` (line 33) for `created_at` timestamps.
- `sha2::{Digest, Sha256}` — only in `artifact_store.rs` (line 34) for content-addressed hashing.
- `sqlx::SqlitePool` / `sqlx::query` — only in `artifact_store.rs` (lines 64, 78, 151, 169, 227, 257, 263).
- `base64` — not imported anywhere in the crate; only referenced in a comment in `messages.rs` line 153 ("Optional base64-encoded thumbnail preview").

No tests in `crates/anvilml-ipc/tests/` reference `ArtifactStore`. The existing test files (`roundtrip_tests.rs`, `stress_test.rs`, `transport_tests.rs`) exercise only the transport and message serialization layers.

The lib.rs doc comment (line 5: "does not contain process management or business logic") is accurate for the remaining modules and will remain accurate after artifact_store removal.

## Resolved Dependencies

No new dependencies are introduced. This task removes four. The workspace-level declarations for `chrono` and `sqlx` remain in the root `Cargo.toml` because other crates (`anvilml-registry`, `anvilml-artifacts`, `anvilml-server`) still need them. The non-workspace declarations for `sha2` and `base64` are removed entirely from `anvilml-ipc/Cargo.toml`.

| Type   | Name    | Action    | Notes                                                        |
|--------|---------|-----------|--------------------------------------------------------------|
| crate  | chrono  | Remove    | Workspace dep; still used by anvilml-registry, anvilml-server |
| crate  | sha2    | Remove    | Non-workspace dep; unused elsewhere in this crate            |
| crate  | sqlx    | Remove    | Workspace dep; still used by anvilml-registry, anvilml-artifacts |
| crate  | base64  | Remove    | Non-workspace dep; unused anywhere in this crate             |

## Approach

1. **Delete `crates/anvilml-ipc/src/artifact_store.rs`.** This file (296 lines) is the sole consumer of `chrono`, `sha2`, and `sqlx` within this crate. The `anvilml-artifacts` crate created in P902-A1 now holds the relocated `ArtifactStore` implementation.

2. **Edit `crates/anvilml-ipc/src/lib.rs`.** Remove two lines:
   - `pub mod artifact_store;` (line 10) — removes the module declaration.
   - `pub use artifact_store::ArtifactStore;` (line 16) — removes the re-export.
   The remaining five pub items (`error`, `messages`, `transport`, `ws`, and their re-exports) are unchanged. The lib.rs doc comment on line 5 ("does not contain process management or business logic") is verified accurate: `error.rs` defines error types, `messages.rs` defines enums and serialization functions, `transport.rs` wraps a ZeroMQ socket, and `ws/broadcaster.rs` wraps a broadcast channel — none perform I/O or business logic.

3. **Edit `crates/anvilml-ipc/Cargo.toml`.** Remove four dependency lines:
   - `base64 = "0.22"` (line 8)
   - `chrono = { workspace = true }` (line 10)
   - `sha2 = "0.10"` (line 14)
   - `sqlx = { workspace = true }` (line 15)
   Then bump the version: change `version = "0.1.8"` to `version = "0.1.9"` (per FORGE_AGENT_RULES §14 / ENVIRONMENT.md §12).

4. **Verify the crate compiles.** Run `cargo check -p anvilml-ipc --features mock-hardware` to confirm no residual references to removed items.

5. **Verify the dependency tree is clean.** Run `cargo tree -p anvilml-ipc --features mock-hardware` and confirm `chrono`, `sha2`, `sqlx`, and `base64` do not appear as direct or transitive dependencies of `anvilml-ipc`.

## Public API Surface

No new public items are introduced. The following public items are **removed**:

| Item | Module Path | Description |
|------|-------------|-------------|
| `pub mod artifact_store` | `anvilml_ipc::artifact_store` | Module containing `ArtifactStore` |
| `ArtifactStore` (re-export) | `anvilml_ipc::ArtifactStore` | Struct: content-addressed artifact storage |

All other public items remain unchanged.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| DELETE | `crates/anvilml-ipc/src/artifact_store.rs` | Remove relocated ArtifactStore module (296 lines) |
| MODIFY | `crates/anvilml-ipc/src/lib.rs` | Remove `pub mod artifact_store;` and `pub use artifact_store::ArtifactStore;` |
| MODIFY | `crates/anvilml-ipc/Cargo.toml` | Remove 4 deps; bump version 0.1.8 → 0.1.9 |

## Tests

The task deletes code and removes dead dependencies — it does not add new functionality. The existing test suite for `anvilml-ipc` (`crates/anvilml-ipc/tests/`) does not reference `ArtifactStore` (confirmed via grep). No new test file is needed. The acceptance criterion is that the existing tests continue to pass after the deletion.

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| (existing) | `crates/anvilml-ipc/tests/roundtrip_tests.rs` | msgpack roundtrip for all message variants | `cargo test -p anvilml-ipc --features mock-hardware --test roundtrip_tests` exits 0 |
| (existing) | `crates/anvilml-ipc/tests/transport_tests.rs` | RouterTransport bind/send/recv | `cargo test -p anvilml-ipc --features mock-hardware --test transport_tests` exits 0 |
| (existing) | `crates/anvilml-ipc/tests/stress_test.rs` | transport under load | `cargo test -p anvilml-ipc --features mock-hardware --test stress_test` exits 0 |

## CI Impact

No CI changes required. The task removes dead code and dependencies from `anvilml-ipc`. The existing CI jobs (`rust-linux`, `rust-windows`) run `cargo test --workspace --features mock-hardware`, which includes `anvilml-ipc` tests — these will continue to pass since no test references the deleted module. No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The task is a pure deletion of code and dependencies — no platform-specific code paths, no `#[cfg(unix)]` / `#[cfg(windows)]` guards, no path-separator handling. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| A downstream crate still imports `anvilml_ipc::ArtifactStore` (P902-A1 did not complete or regressed) | Low | High | `cargo check -p anvilml-ipc` will succeed but `cargo check --workspace` will fail with a missing import. The failing crate's error message will identify the exact import path. Fix: add the import update to the same task or block until P902-A3/A4. |
| Removing `sqlx` from workspace deps causes a build failure in another crate | Low | Medium | `sqlx` is declared as a workspace dependency in the root `Cargo.toml` but is used by `anvilml-registry` and `anvilml-artifacts` directly (not via workspace). Removing it only from `anvilml-ipc/Cargo.toml` is safe. Verify: `cargo tree -p anvilml-registry` and `cargo tree -p anvilml-artifacts` still show `sqlx`. |
| `base64` is actually used by code not found by grep (e.g. re-exported from another module) | Very Low | Low | Confirmed via exhaustive grep: no `use base64` or `base64::` import exists anywhere in `crates/anvilml-ipc/src/`. Only a comment mentions base64. |

## Acceptance Criteria

- `cargo check -p anvilml-ipc --features mock-hardware` exits 0
- `cargo tree -p anvilml-ipc --features mock-hardware | grep -E 'chrono|sha2|sqlx|base64'` returns empty (no match)
- `cargo test -p anvilml-ipc --features mock-hardware` exits 0
- `grep -c 'artifact_store' crates/anvilml-ipc/src/lib.rs` returns 0
- `test ! -f crates/anvilml-ipc/src/artifact_store.rs` (file is deleted)
- `grep '^version' crates/anvilml-ipc/Cargo.toml` returns `version = "0.1.9"`
