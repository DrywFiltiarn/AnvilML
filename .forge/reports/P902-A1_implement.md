# Implementation Report: P902-A1

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P902-A1                            |
| Phase         | 902 — ArtifactStore Relocation Retrofit |
| Description   | Create anvilml-artifacts crate; move store.rs verbatim; correct module doc |
| Implemented   | 2026-06-20T18:15:00Z               |
| Status        | COMPLETE                           |

## Summary

Created the `crates/anvilml-artifacts` crate as a new workspace member hosting the `ArtifactStore` content-addressed PNG artifact storage backend. The crate consists of `Cargo.toml`, `src/lib.rs` (crate doc + `pub mod store` + `pub use`), `src/store.rs` (moved verbatim from `anvilml-ipc/src/artifact_store.rs` with corrected module doc), and `tests/store_tests.rs` (5 integration tests). The module doc was rewritten to replace the false "Why in `anvilml-ipc`?" cycle rationale with the correct shared-crate rationale. Workspace members list was updated. All 224 workspace tests pass, clippy is clean, all 4 platform cross-checks pass, and the config_reference gate passes.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source          |
|--------|-----------|------------------|-----------------|
| crate  | sha2      | 0.10             | Cargo.lock      |
| crate  | sqlx      | 0.9.0            | Workspace dep   |
| crate  | tokio     | 1.52.3           | Workspace dep   |
| crate  | chrono    | 0.4.45           | Workspace dep   |
| crate  | tracing   | 0.1.44           | Workspace dep   |
| crate  | uuid      | 1.23.3           | Workspace dep   |
| crate  | serial_test | 3.5            | Cargo.lock      |
| crate  | tempfile  | 3.27.0           | Workspace dep   |

All versions match the workspace `Cargo.toml` or the project's `Cargo.lock`. `sha2` is declared directly as `"0.10"` (not a workspace dep), matching the `anvilml-registry` pattern. `sqlx` adds `features = ["chrono"]` to support `chrono::DateTime<Utc>` in `row.get::<DateTime<Utc>, _>()`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-artifacts/Cargo.toml` | New crate manifest with deps from anvilml-ipc + sqlx chrono feature |
| CREATE | `crates/anvilml-artifacts/src/lib.rs` | Crate root: crate doc, `pub mod store`, `pub use store::ArtifactStore` |
| CREATE | `crates/anvilml-artifacts/src/store.rs` | Moved verbatim from `anvilml-ipc/src/artifact_store.rs` with corrected module doc (lines 1-29 replaced) |
| CREATE | `crates/anvilml-artifacts/tests/store_tests.rs` | 5 integration tests: save_and_get, save_idempotency, list_all, list_filtered, get_missing_hash |
| MODIFY | `Cargo.toml` | Added `"crates/anvilml-artifacts"` to workspace members |
| MODIFY | `docs/TESTS.md` | Added 5 test entries for new integration tests |

## Commit Log

```
 .forge/reports/P902-A1_plan.md                | 157 ++++++++++++++
 .forge/state/CURRENT_TASK.md                  |   6 +-
 .forge/state/state.json                       |  13 +-
 Cargo.lock                                    |  15 ++
 Cargo.toml                                    |   1 +
 crates/anvilml-artifacts/Cargo.toml           |  18 ++
 crates/anvilml-artifacts/src/lib.rs           |   9 +
 crates/anvilml-artifacts/src/store.rs         | 295 ++++++++++++++++++++++++++
 crates/anvilml-artifacts/tests/store_tests.rs | 258 ++++++++++++++++++++++
 docs/TESTS.md                                 |  45 ++++
 10 files changed, 808 insertions(+), 9 deletions(-)
```

## Test Results

```
     Running tests/store_tests.rs (target/debug/deps/store_tests-9fbadc38bb6e8439)

running 5 tests
test test_get_missing_hash ... ok
test test_save_and_get ... ok
test test_list_filtered ... ok
test test_save_idempotency ... ok
test test_list_all ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: 224 tests passed, 0 failed, 0 ignored.

## Format Gate

```
cargo fmt --all -- --check
# exited 0 — no formatting drift
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.96s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.57s

# 3. Real-hardware Linux
cargo check --bin anvilml
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s
```

All 4 checks exited 0.

## Project Gates

```
cargo test -p anvilml --features mock-hardware -- config_reference
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 1 (config_reference) passed. No handler signatures or node types were modified, so gates 2 and 3 are not applicable.

## Public API Delta

```
crates/anvilml-artifacts/src/lib.rs:7:pub mod store;
crates/anvilml-artifacts/src/lib.rs:9:pub use store::ArtifactStore;
crates/anvilml-artifacts/src/store.rs:52:pub struct ArtifactStore {
```

New public items introduced:
- `pub mod store` — module declaration in `anvilml_artifacts` (lib.rs)
- `pub use store::ArtifactStore` — re-export at crate root (lib.rs)
- `pub struct ArtifactStore` — content-addressed artifact storage backend (store.rs)

All three items match the plan's Public API Surface table exactly. The existing `pub async fn new`, `pub async fn save`, `pub async fn get`, and `pub async fn list` methods on `ArtifactStore` were moved verbatim from `anvilml-ipc` and were already present in the original code.

## Deviations from Plan

- Added `features = ["chrono"]` to the `sqlx` dependency in `Cargo.toml`. The plan specified `sqlx = { workspace = true }` but the workspace base features (`runtime-tokio`, `sqlite`, `json`) do not include `chrono`. The `store.rs` code uses `chrono::DateTime<Utc>` which requires the `chrono` feature for `row.get::<DateTime<Utc>, _>()`. This matches the `anvilml-registry` pattern (`sqlx = { workspace = true, features = ["chrono"] }`).
- Added `features = ["fs", "io-util", "rt"]` to `tokio` in `anvilml-registry`'s Cargo.toml as reference — the `anvilml-artifacts` crate uses `tokio::fs::write` which is covered by the workspace's `tokio = { features = ["full"] }`, so no additional features are needed in `anvilml-artifacts`' Cargo.toml.
- Removed unused imports (`AnvilError`, `PathBuf`) from `tests/store_tests.rs` to satisfy clippy `-D warnings`. These were present in the initial draft but not needed by any test function.

## Blockers

None.
