# Implementation Report: P3-A2

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P3-A2                              |
| Phase         | 003 — Core Domain Types            |
| Description   | anvilml-core: model and artifact types |
| Implemented   | 2026-06-14T18:00:00Z               |
| Status        | COMPLETE                           |

## Summary

Created `ModelMeta` struct, `ModelKind`/`ModelDtype`/`ModelFormat` enums in `crates/anvilml-core/src/types/model.rs`, and `ArtifactMeta` struct in `crates/anvilml-core/src/types/artifact.rs`. Wired these into the crate's module tree (`types/mod.rs`, `lib.rs`), bumped the crate patch version to 0.1.4, and added 6 integration tests (3 per test file). All 29 workspace tests pass, all 4 platform cross-checks pass, format gate passes, and lint passes with zero warnings.

## Resolved Dependencies

No new dependencies added. All types use existing workspace dependencies: `serde`, `chrono`, `uuid`, `utoipa`.

| Type   | Name     | Version resolved | Source         |
|--------|----------|------------------|----------------|
| crate  | serde    | 1.0.228          | Workspace manifest |
| crate  | chrono   | 0.4.45           | Workspace manifest |
| crate  | uuid     | 1.23.3           | Workspace manifest |
| crate  | utoipa   | 5.5.0            | Workspace manifest |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/types/model.rs` | ModelMeta struct + ModelKind, ModelDtype, ModelFormat enums |
| CREATE | `crates/anvilml-core/src/types/artifact.rs` | ArtifactMeta struct with Default derive |
| MODIFY | `crates/anvilml-core/src/types/mod.rs` | Add model and artifact modules + pub use re-exports |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Re-export new types at crate root |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Bump patch version 0.1.3 → 0.1.4 |
| CREATE | `crates/anvilml-core/tests/model_tests.rs` | 3 tests for model types |
| CREATE | `crates/anvilml-core/tests/artifact_tests.rs` | 3 tests for artifact type |
| MODIFY | `docs/TESTS.md` | Added 6 test catalogue entries |

## Commit Log

```
 .forge/reports/P3-A2_plan.md                | 163 ++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                |   6 +-
 .forge/state/state.json                     |  13 ++-
 Cargo.lock                                  |   2 +-
 crates/anvilml-core/Cargo.toml              |   2 +-
 crates/anvilml-core/src/lib.rs              |   5 +-
 crates/anvilml-core/src/types/artifact.rs   |  30 +++++
 crates/anvilml-core/src/types/mod.rs        |  10 +-
 crates/anvilml-core/src/types/model.rs      | 100 +++++++++++++++++
 crates/anvilml-core/tests/artifact_tests.rs | 107 ++++++++++++++++++
 crates/anvilml-core/tests/model_tests.rs    | 129 ++++++++++++++++++++++
 docs/TESTS.md                               |  48 ++++++++
 12 files changed, 601 insertions(+), 14 deletions(-)
```

## Test Results

```
     Running tests/artifact_tests.rs (target/debug/deps/artifact_tests-dfbe6447d572ebc5)

running 3 tests
test test_artifact_meta_default ... ok
test test_artifact_hash_format ... ok
test test_artifact_meta_json_roundtrip ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/model_tests.rs (target/debug/deps/model_tests-d6d4ba58f3c80499)

running 3 tests
test test_model_dtype_format_variants ... ok
test test_model_kind_variants ... ok
test test_model_meta_json_roundtrip ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace: 29 tests passed, 0 failed, 0 ignored.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.22s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.73s

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.69s
```

All four cross-checks exit 0.

## Project Gates

Gate 1 (Config Surface Sync): Not triggered — task does not modify `ServerConfig` or nested config fields.

Gate 2 (OpenAPI Drift): Not triggered — the `api/openapi.json` file does not yet exist in this repository (per ENVIRONMENT.md §8, the gate is skipped when the file does not exist).

Gate 3 (Node Parity): Not triggered — task does not add, remove, or rename node types.

## Public API Delta

```
crates/anvilml-core/src/types/model.rs:pub struct ModelMeta {
crates/anvilml-core/src/types/model.rs:pub enum ModelKind {
crates/anvilml-core/src/types/model.rs:pub enum ModelDtype {
crates/anvilml-core/src/types/model.rs:pub enum ModelFormat {
crates/anvilml-core/src/types/artifact.rs:pub struct ArtifactMeta {
```

From `types/mod.rs`:
- `pub mod artifact;` — new module
- `pub mod model;` — new module
- `pub use artifact::ArtifactMeta;` — re-export
- `pub use model::{ModelDtype, ModelFormat, ModelKind, ModelMeta};` — re-exports

From `lib.rs`:
- `pub use types::{ArtifactMeta, Job, JobSettings, JobStatus, ModelDtype, ModelFormat, ModelKind, ModelMeta, SubmitJobRequest, SubmitJobResponse};` — extended re-export

All 5 new pub types match the plan's Public API Surface table exactly.

## Deviations from Plan

- **Path field type changed from `PathBuf` to `String`** in both `ModelMeta` and `ArtifactMeta`. The plan specified `path: PathBuf`, but `PathBuf` does not implement `utoipa::ToSchema`, causing a compile error on the `#[derive(ToSchema)]` macro. The plan's risk section (§Risks and Mitigations) explicitly suggested this fallback: "If utoipa fails on `PathBuf`, the ACT agent should change `path: PathBuf` to `path: String`." This is functionally equivalent for JSON transport and matches how paths are represented in API responses.
- **Added `Default` derive to `ArtifactMeta`** — the plan specified deriving `Default` on the struct (via `#[derive(Default)]`). This was required for the `test_artifact_meta_default` test.
- **No `#[serde(with = "path_as_string")]` on path fields** — because the type was changed to `String`, no custom serde attribute is needed. The `String` type serialises and deserialises as a JSON string by default.

## Blockers

None.
