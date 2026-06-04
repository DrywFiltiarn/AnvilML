# Implementation Report: P6-A6

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P6-A6                                               |
| Phase       | 006 — Model Registry                                |
| Description | anvilml-server: GET /v1/models handler (list with kind filter) |
| Implemented | 2026-06-04T10:05:00Z                                |
| Status      | COMPLETE                                            |

## Summary

Implemented the `GET /v1/models` endpoint in `anvilml-server`. Created a new `handlers::models` module with a `list_models` async handler that accepts an optional `kind` query parameter (filtered by `ModelKind`) and returns `Json<Vec<ModelMeta>>` from `registry.list(kind)`. Wired the route in `build_router()`. Added three integration tests verifying: (1) unfiltered listing returns scanned models with correct metadata, (2) kind filter for matching type returns only matching models, (3) kind filter for non-matching type returns empty array. Also added `#[serde(rename_all = "snake_case")]` to `ModelKind` and `DType` enums to enable lowercase query parameter parsing (`?kind=diffusion`, `dtype_hint: "f16"`) consistent with the existing `anvilml.toml` config format.

## Resolved Dependencies

| Type | Name | Version resolved | Source |
|------|------|-----------------|--------|
| crate | axum | 0.7 (already in Cargo.toml) | existing |
| crate | serde | 1 (already in Cargo.toml) | existing |
| crate | anvilml-core | local path | existing |
| crate | anvilml-registry | local path | existing |

No new dependencies introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-server/src/handlers/models.rs` | New handler module with `list_models` function and `ModelsListQuery` struct |
| Modify | `crates/anvilml-server/src/handlers/mod.rs` | Added `pub mod models;` |
| Modify | `crates/anvilml-server/src/lib.rs` | Wired `.route("/v1/models", get(handlers::models::list_models))` in `build_router()` |
| Create | `crates/anvilml-server/tests/api_models.rs` | Integration tests: `list_models_returns_scanned_models`, `list_models_kind_filter_diffusion`, `list_models_kind_filter_no_match` |
| Modify | `crates/anvilml-core/src/config.rs` | Added `#[serde(rename_all = "snake_case")]` to `ModelKind` enum for lowercase query param support |
| Modify | `crates/anvilml-core/src/types/model.rs` | Added `#[serde(rename_all = "snake_case")]` to `DType` enum; updated test assertions for snake_case JSON values |

## Commit Log

```
.forge/reports/P6-A6_plan.md                 | 122 +++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  13 +--
 crates/anvilml-core/src/config.rs            |   1 +
 crates/anvilml-core/src/types/model.rs       |   9 +-
 crates/anvilml-server/src/handlers/mod.rs    |   1 +
 crates/anvilml-server/src/handlers/models.rs |  28 +++++
 crates/anvilml-server/src/lib.rs             |   1 +
 crates/anvilml-server/tests/api_models.rs    | 152 +++++++++++++++++++++++++++
 9 files changed, 320 insertions(+), 13 deletions(-)
```

## Test Results

```
     Running tests/api_models.rs (target/debug/deps/api_models-1426ec52d3886a3e)

running 3 tests
test list_models_returns_scanned_models ... ok
test list_models_kind_filter_no_match ... ok
test list_models_kind_filter_diffusion ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: **74 + 59 + 10 + 1 + 2 + 1 + 2 + 3 + 3 + 8 + 1 = 165 tests passed, 0 failed**.

## Platform Cross-Check

```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.02s
```

`cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware` — **passed with zero errors**.

## Project Gates

Gate 1 — Config Surface Sync:
```
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

- Added `#[serde(rename_all = "snake_case")]` to `ModelKind` (in `config.rs`) — not listed in the plan's Files Affected table, but required for query parameter values like `?kind=diffusion` to deserialize correctly. The existing `anvilml.toml` already uses lowercase kind values (`kind = "diffusion"`), so this aligns with the existing config format.
- Added `#[serde(rename_all = "snake_case")]` to `DType` (in `types/model.rs`) — required for consistency, since the handler returns `ModelMeta` objects where `dtype_hint` is serialized as `"f16"` (snake_case) rather than `"F16"` (PascalCase).
- Updated existing test assertions in `crates/anvilml-core/src/types/model.rs` to use snake_case JSON values (`"upscale"`, `"clip"`, `"lora"`, `"q8"`).
- Used `std::env::temp_dir()` with pre-created database file instead of `tempfile` crate, since `anvilml_registry::open` requires the database file to exist before connecting (SQLite/sqlx behavior).

## Blockers

None. All gates passed, all tests pass, clippy clean, Windows cross-check clean.
