# Implementation Report: P14-A2

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P14-A2                                                      |
| Phase       | 014 — Artifact Storage                                      |
| Description | anvilml-server: ArtifactStore.save (decode, hash, write, db insert) |
| Implemented | 2026-06-09T18:20:00Z                                        |
| Status      | COMPLETE                                                    |

## Summary

Implemented the `ArtifactStore` module in `anvilml-server` that provides content-addressed PNG artifact persistence. The `save` method decodes a base64-encoded PNG image, computes its SHA-256 hash, writes the file to a two-char-prefix-sharded directory under `artifact_dir`, inserts artifact metadata into the SQLite `artifacts` table, and increments the job's `artifact_count`. A comprehensive integration test verifies the full pipeline.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|-----------------|----------------|
| crate  | sha2      | 0.11            | rust-docs MCP  |
| crate  | hex       | 0.4.3           | rust-docs MCP  |
| crate  | base64    | 0.22            | rust-docs MCP  |
| crate  | tokio     | 1.52.3 (fs feat) | rust-docs MCP |

Note: `sha2` and `hex` were already in the workspace `[workspace.dependencies]`. `base64 = "0.22"` was added to the workspace and crate manifests. The `fs` feature was added to the `tokio` dependency in `anvilml-server`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `Cargo.toml` (workspace root) | Add `base64 = "0.22"` to `[workspace.dependencies]` |
| Modify | `crates/anvilml-server/Cargo.toml` | Add `base64`, `hex`, `sha2`, `thiserror` deps; add `fs` feature to `tokio`; add dev-deps; bump version 0.1.4→0.1.5; add `test-helpers` feature to `anvilml-worker` |
| Create | `crates/anvilml-server/src/artifact/mod.rs` | Module entry point; re-exports `ArtifactStore` and `ArtifactStoreInput` |
| Create | `crates/anvilml-server/src/artifact/store.rs` | `ArtifactMeta`, `ArtifactStoreInput`, `ArtifactError`, `ArtifactStore::new`, `ArtifactStore::save` |
| Modify | `crates/anvilml-server/src/lib.rs` | Add `pub mod artifact;` to expose the artifact module |
| Create | `crates/anvilml-server/tests/api_artifact_save.rs` | Integration test verifying decode → hash → write → DB insert → count increment |

## Commit Log

```
 .forge/reports/P14-A2_plan.md                    | 128 ++++++++++++++++++
 .forge/state/CURRENT_TASK.md                     |   6 +-
 .forge/state/state.json                          |  13 +-
 Cargo.lock                                       |   6 +-
 Cargo.toml                                       |   1 +
 crates/anvilml-server/Cargo.toml                 |  31 +++--
 crates/anvilml-server/src/artifact/mod.rs        |   3 +
 crates/anvilml-server/src/artifact/store.rs      | 160 +++++++++++++++++++++++
 crates/anvilml-server/src/lib.rs                 |   1 +
 crates/anvilml-server/tests/api_artifact_save.rs | 117 +++++++++++++++++
 10 files changed, 444 insertions(+), 22 deletions(-)
```

## Test Results

```
     Running tests/api_artifact_save.rs (target/debug/deps/api_artifact_save-78686d3bd6471389)

running 1 test
test artifact_save ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Full workspace test suite: 264 passed, 0 failed, 0 ignored.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

All four cross-checks passed:

1. `cargo check --workspace --features mock-hardware` — Finished (Linux mock-hardware)
2. `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` — Finished (Windows cross)
3. `cargo check --bin anvilml` — Finished (Linux real-hardware)
4. `cargo check --bin anvilml --target x86_64-pc-windows-gnu` — Finished (Windows cross)

## Project Gates

```
cargo test -p backend --features mock-hardware -- config_reference
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out
```

Gate passed — config surface sync check is clean.

## Deviations from Plan

- **Added `hex`, `sha2`, and `thiserror` as direct dependencies** in `anvilml-server/Cargo.toml` (plan only mentioned `base64` and `tokio/fs`). These were needed for the `ArtifactError` type and the `store.rs` implementation.
- **Added `test-helpers` feature to `anvilml-worker` dependency** — a pre-existing test in `handlers/jobs.rs` uses `WorkerPool::new_test_pool()` which is gated behind `#[cfg(any(test, feature = "test-helpers"))]`. Without this feature, the test suite would not compile.
- **Exported `ArtifactStoreInput`** from `artifact/mod.rs` — the plan only mentioned re-exporting `ArtifactStore`, but the test needs `ArtifactStoreInput` to construct the `meta_input` parameter.
- **Added `db()` accessor method** to `ArtifactStore` — the test needs access to the database pool to verify DB state.
- **Pre-existing test compilation fix**: The `WorkerPool::new_test_pool()` call in `handlers/jobs.rs` required the `test-helpers` feature to be enabled on `anvilml-worker`. This was a pre-existing gap, not introduced by this task.

## Blockers

None.
