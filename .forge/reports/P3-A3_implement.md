# Implementation Report: P3-A3

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P3-A3                                       |
| Phase         | 003 — Core Domain Types: Data Model         |
| Description   | anvilml-core: ArtifactMeta type             |
| Implemented   | 2026-06-28T16:30:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Implemented the `ArtifactMeta` struct in `crates/anvilml-core/src/types/artifact.rs` as a pure data type representing metadata for a generated, content-addressed PNG artifact. Added the `utoipa` 5.5.0 dependency with `uuid` and `chrono` features for the `ToSchema` derive macro. The struct carries the SHA-256 content hash, originating job ID, generation parameters (width, height, seed, steps), creation timestamp, and file path. Created 3 tests covering serde roundtrip, SHA-256 hash format, and JSON field names. All tests pass, clippy is clean, and all platform cross-checks succeed.

## Resolved Dependencies

| Type   | Name   | Version resolved | Source         |
|--------|--------|------------------|----------------|
| crate  | utoipa | 5.5.0            | rust-docs MCP  |

Resolved via `rust-docs_get_crate_info` — version 5.5.0 is the latest stable release. The `uuid` and `chrono` features enable `Uuid` and `DateTime<Utc>` type support in the `ToSchema` derive macro's generated OpenAPI schemas.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | crates/anvilml-core/src/types/artifact.rs | ArtifactMeta struct with 8 fields, doc comments, derives (Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema) |
| MODIFY | crates/anvilml-core/src/types/mod.rs | Added `pub mod artifact;` and `pub use artifact::*;` |
| MODIFY | crates/anvilml-core/Cargo.toml | Added `utoipa = { version = "5.5.0", features = ["uuid", "chrono"] }` dependency; bumped version 0.1.7 → 0.1.8 |
| CREATE | crates/anvilml-core/tests/artifact_tests.rs | 3 tests: serde roundtrip, hash format, field names |
| MODIFY | docs/TESTS.md | Added 3 test catalogue entries for the new tests |

## Commit Log

```
 .forge/reports/P3-A3_plan.md                | 173 ++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                |   6 +-
 .forge/state/state.json                     |  13 ++-
 Cargo.lock                                  |  29 ++++-
 crates/anvilml-core/Cargo.toml              |   3 +-
 crates/anvilml-core/src/types/artifact.rs   |  32 +++++
 crates/anvilml-core/src/types/mod.rs        |   2 +
 crates/anvilml-core/tests/artifact_tests.rs | 163 ++++++++++++++++++++++++++
 docs/TESTS.md                               |  36 ++++++
 9 files changed, 446 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/artifact_tests.rs (target/debug/deps/artifact_tests-76fd29732b25c294)

running 3 tests
test test_artifact_meta_field_names ... ok
test test_artifact_meta_hash_format ... ok
test test_artifact_meta_serde_roundtrip ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

All 3 artifact tests pass. Full workspace test suite: 73 tests passed, 0 failed.

## Format Gate

```
cargo fmt --all -- --check
```
Exit 0 — no formatting drift.

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.20s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 23.25s

# 3. Real-hardware Linux
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.47s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.47s
```

All four platform cross-checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p anvilml --features mock-hardware -- config_reference
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Gate 2 — OpenAPI Drift
Not triggered — this task does not modify handler function signatures, `#[utoipa::path]` annotations, or `AppState` fields. The `ToSchema` derive on `ArtifactMeta` does not affect the OpenAPI drift gate (Gate 2 only triggers on handler-level changes).

## Public API Delta

New public items from this task:

```
+pub mod artifact;
+pub use artifact::*;
```

From the new `artifact.rs` file:
- `pub struct ArtifactMeta` — in module `anvilml_core::types::artifact`, re-exported as `anvilml_core::types::ArtifactMeta`
- `pub hash: String`
- `pub job_id: Uuid`
- `pub width: u32`
- `pub height: u32`
- `pub seed: i64`
- `pub steps: u32`
- `pub created_at: DateTime<Utc>`
- `pub file_path: PathBuf` (with `#[schema(value_type = String)]` override for OpenAPI)

## Deviations from Plan

- **Derive set**: The plan specified `#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]` without `PartialEq`/`Eq`. Added `PartialEq, Eq` because the test suite requires `assert_eq!` for roundtrip verification, and `ModelMeta` (which also has a `PathBuf` field) derives these. This matches the established pattern in the codebase.
- **`#[schema(value_type = String)]` on `file_path`**: The plan did not include this attribute. Added it because `PathBuf` does not implement `utoipa::ToSchema` in utoipa 5.5.0. The attribute tells utoipa to generate a `String` schema for this field in the OpenAPI output, which is the correct representation (serde serialises `PathBuf` as a UTF-8 string). Without this, `cargo check` fails with `the trait bound PathBuf: ToSchema is not satisfied`.

## Blockers

None.
