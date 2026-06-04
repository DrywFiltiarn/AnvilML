# Implementation Report: P6-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P6-A3                                       |
| Phase       | 006 — Model Registry                        |
| Description | anvilml-registry: ModelRegistry list (with kind filter) |
| Implemented | 2026-06-04T07:30:00Z                        |
| Status      | COMPLETE                                    |

## Summary

Added a `list` method to `ModelRegistry` in `crates/anvilml-registry/src/store.rs` that returns all scanned model metadata from the SQLite `models` table, with an optional `kind` filter and deterministic ordering by name ascending. Created three integration tests in `crates/anvilml-registry/tests/store_list.rs` covering empty database, ordered results after multiple upserts, and kind-based filtering. No new dependencies were required — only existing `sqlx`, `serde_json`, and `chrono` crates are used.

## Resolved Dependencies

No new dependencies added. The task uses only `sqlx`, `serde_json`, and `chrono` which are already declared in `crates/anvilml-registry/Cargo.toml`.

| Type   | Name        | Version resolved | Source        |
|--------|-------------|-----------------|---------------|
| crate  | sqlx        | 0.9             | Cargo.toml    |
| crate  | serde_json  | 1               | Cargo.toml    |
| crate  | chrono      | 0.4             | Cargo.toml    |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/src/store.rs` | Added `async fn list(&self, kind: Option<ModelKind>) -> Result<Vec<ModelMeta>, AnvilError>` method to `impl ModelRegistry` |
| Create | `crates/anvilml-registry/tests/store_list.rs` | Integration tests: empty DB, ordered results, kind filter |

## Commit Log

```
 .forge/reports/P6-A3_plan.md                |  85 ++++++++++++++
 .forge/state/CURRENT_TASK.md                |   6 +-
 .forge/state/state.json                     |  13 ++-
 crates/anvilml-registry/src/store.rs        |  55 +++++++++
 crates/anvilml-registry/tests/store_list.rs | 166 ++++++++++++++++++++++++++++
 5 files changed, 316 insertions(+), 9 deletions(-)
```

## Test Results

```
Running tests/store_list.rs (target/debug/deps/store_list-1eadd146c1590f23)

running 3 tests
test test_list_empty_returns_empty_vec ... ok
test test_list_kind_filter ... ok
test test_list_after_upserts_returns_ordered ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out, finished in 0.04s
```

Full workspace test suite: 154 tests passed, 0 failed across all crates (anvilml-core: 74, anvilml-hardware: 59, anvilml-registry unit: 10, anvilml-registry db integration: 1, scanner integration: 1, store_get integration: 2, store_list integration: 3, anvilml-server: 3, anvilml binary: 8, backend config_reference: 1, doc-tests: 2).

## Platform Cross-Check

```
Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.86s
```

Zero errors on x86_64-pc-windows-gnu target.

## Project Gates

### Config Surface Sync (Gate 1)
```
Running tests/config_reference.rs (target/debug/deps/config_reference-1c602ac5823733ee)

running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out, finished in 0.02s
```

### Clippy Lint
```
Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.57s
```

Zero warnings.

## Deviations from Plan

None. Implementation follows the approved plan exactly:
- `list` method added to `store.rs` with optional `kind` filter and `ORDER BY name ASC`
- Kind bound as JSON string (matching `upsert` serialization)
- Row deserialization mirrors the existing `get` method logic
- Three integration tests created in `tests/store_list.rs` matching the specified test names and assertions

## Blockers

None. All gates passed, all tests pass, clippy clean, Windows cross-check clean.
