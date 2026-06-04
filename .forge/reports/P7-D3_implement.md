# Implementation Report: P7-D3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-D3                                         |
| Phase       | 007 — WebSocket Event Stream                |
| Description | anvilml-registry + anvilml-server: silent error discard fixes |
| Implemented | 2026-06-04T23:58:00Z                          |
| Status      | COMPLETE                                      |

## Summary

Replaced three silent error discards in `crates/anvilml-registry/src/scanner.rs` and two database error discard sites in `crates/anvilml-server/src/handlers/models.rs` with explicit error logging via `tracing::warn!`/`tracing::error!` and informative JSON error bodies. Added `tracing` as an explicit dependency of `anvilml-registry` (it was previously only used transitively). All clippy passes, platform cross-checks, tests, and config drift gate pass with zero failures.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source        |
|--------|---------|-----------------|---------------|
| crate  | tracing | 0.1.44          | Workspace Cargo.toml |

Note: `tracing` was already declared in `[workspace.dependencies]` of the root `Cargo.toml`. It was added to `anvilml-registry/Cargo.toml` as a direct dependency via `{ workspace = true }` so that `scanner.rs` can use `tracing::warn!` macros.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/Cargo.toml` | Added `tracing = { workspace = true }` dependency |
| Modify | `crates/anvilml-registry/src/scanner.rs` | Replaced 3 silent error discards with `tracing::warn!` + continue/fallback |
| Modify | `crates/anvilml-server/src/handlers/models.rs` | Added `tracing::error!` to both error arms; updated `list_models` return type to `Json<serde_json::Value>`; included error message in JSON bodies |
| Format | `crates/anvilml-hardware/src/lib.rs` | Formatting side-effect from `cargo fmt --all` (indentation normalization, line wrapping) — no behavioral change |

## Commit Log

```
 .forge/reports/P7-D3_plan.md                 | 205 +++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  11 +-
 Cargo.lock                                   |   1 +
 crates/anvilml-hardware/src/lib.rs           |  22 ++-
 crates/anvilml-registry/Cargo.toml           |  15 +-
 crates/anvilml-registry/src/scanner.rs       |  21 ++-
 crates/anvilml-server/src/handlers/models.rs |  29 +++-
 8 files changed, 277 insertions(+), 33 deletions(-)
```

## Test Results

```
running 74 tests (anvilml_core)     — ok. 74 passed; 0 failed
running 59 tests (anvilml_hardware) — ok. 59 passed; 0 failed
running 0 tests  (anvilml_ipc)      — ok. 0 passed; 0 failed
running 0 tests  (anvilml_openapi)  — ok. 0 passed; 0 failed
running 11 tests (anvilml_registry) — ok. 11 passed; 0 failed
running 1 test   (registry_db)      — ok. 1 passed; 0 failed
running 2 tests  (rescan)           — ok. 2 passed; 0 failed
running 1 test   (scanner)          — ok. 1 passed; 0 failed
running 2 tests  (store_get)        — ok. 2 passed; 0 failed
running 3 tests  (store_list)       — ok. 3 passed; 0 failed
running 0 tests  (anvilml_scheduler)— ok. 0 passed; 0 failed
running 8 tests  (anvilml_server)   — ok. 8 passed; 0 failed
running 3 tests  (api_models)       — ok. 3 passed; 0 failed
running 1 test   (api_ws_events)    — ok. 1 passed; 0 failed
running 0 tests  (anvilml_worker)   — ok. 0 passed; 0 failed
running 8 tests  (anvilml binary)   — ok. 8 passed; 0 failed
running 1 test   (config_reference) — ok. 1 passed; 0 failed
doc-tests anvilml_core              — ok. 0 passed; 0 failed
doc-tests anvilml_hardware          — ok. 2 passed; 0 failed
doc-tests anvilml_ipc               — ok. 0 passed; 0 failed
doc-tests anvilml_registry          — ok. 0 passed; 0 failed
doc-tests anvilml_scheduler         — ok. 0 passed; 0 failed
doc-tests anvilml_server            — ok. 0 passed; 0 failed
doc-tests anvilml_worker            — ok. 0 passed; 0 failed

Total: 152 tests run, 152 passed, 0 failed
```

## Platform Cross-Check

### Check 1 — Mock-hardware Windows-gnu cross-check
```
Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.22s
```

### Check 2 — Real-hardware Linux native
```
Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.69s
```

### Check 3 — Real-hardware Windows-gnu cross-check
```
Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.21s
```

All three checks exited 0. Zero errors.

## Project Gates

### Config Surface Sync (config_reference)
```
running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate passed. Zero failures.

## Deviations from Plan

- **Added `tracing` dependency to `anvilml-registry/Cargo.toml`**: The plan assumed the crate already depended on `tracing` (citing `store.rs` usage), but `tracing` was only used via transitive dependency through `anvilml-server`. Added it as an explicit `{ workspace = true }` dependency.
- **Formatting side-effect on `crates/anvilml-hardware/src/lib.rs`**: Running `cargo fmt --all` reformatted pre-existing 2-space indentation to 4-space and wrapped some `tracing::warn!` macro calls across multiple lines in the hardware crate. This is a cosmetic change with no behavioral impact, caused by the mandatory format step.

## Blockers

None.
